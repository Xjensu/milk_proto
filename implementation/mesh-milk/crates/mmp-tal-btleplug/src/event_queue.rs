//! Bounded, control-priority event ingress for the central BLE backend.
//!
//! The backend lifecycle gate must never be held while waiting for the host
//! event loop.  This module therefore keeps a synchronous two-lane ingress
//! (data plus lifecycle control) and lets a detached dispatcher be the only
//! task that awaits the public Tokio receiver.  Data carries the central link
//! generation so a replacement or retirement can discard stale notifications
//! before they reach the node.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mmp_tal::{LinkId, TalEvent};
use tokio::sync::Notify;

pub(crate) const TOTAL_CAPACITY: usize = 2048;
pub(crate) const CONTROL_CAPACITY: usize = 64;
pub(crate) const DATA_CAPACITY: usize = TOTAL_CAPACITY - CONTROL_CAPACITY;
/// Hard cap for lifecycle tombstones. Once exhausted, new link identities are
/// rejected fail-closed rather than evicting history and admitting replays.
const MAX_GENERATION_TOMBSTONES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataStamp {
    /// Advertisement data is not associated with a live link generation.
    Unstamped,
    /// Link data must still belong to this generation when dispatched.
    Link { link: LinkId, generation: u64 },
}

#[derive(Debug)]
struct DataRecord {
    stamp: DataStamp,
    event: TalEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlKind {
    Up,
    Down,
}

#[derive(Debug)]
struct ControlRecord {
    link: LinkId,
    generation: u64,
    kind: ControlKind,
    event: TalEvent,
}

#[derive(Debug, Default)]
struct State {
    data: VecDeque<DataRecord>,
    controls: VecDeque<ControlRecord>,
    /// Emergency lifecycle tombstones retained when the normal control lane
    /// is full. One entry per link keeps the fallback bounded while ensuring
    /// retirement is eventually visible to the public event stream.
    emergency_down: HashMap<LinkId, u64>,
    /// Current generation known to the ingress.  `None` means the link is
    /// retired; queued records from an older generation are stale.
    current: HashMap<LinkId, u64>,
    /// Highest lifecycle generation admitted per stable link identity.
    last_generation: HashMap<LinkId, u64>,
    /// Sticky fail-closed state after the bounded tombstone ledger fills.
    generation_ledger_exhausted: bool,
}

struct Inner {
    state: Mutex<State>,
    notify: Notify,
    sender_count: AtomicUsize,
    control_events_dropped: std::sync::atomic::AtomicU64,
}

/// Cloneable producer for the bounded central event ingress.
pub(crate) struct EventSender(Arc<Inner>);

/// Result of a nonblocking data enqueue.  The caller must retire the affected
/// link when `Full` is returned; silently dropping one encrypted chunk would
/// leave the session nonce/order stream ambiguous.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DataEnqueueError {
    Full,
    Stale,
}

/// Result of a nonblocking lifecycle-control enqueue.  Control saturation is
/// a transport-wide overload and must never be silently ignored by callers.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ControlEnqueueError {
    Full,
    InvalidEvent,
}

/// Create the bounded internal ingress and the unchanged public Tokio FIFO.
/// The dispatcher owns the only await on the public queue, so producer tasks
/// never hold a backend lifecycle lock across queue backpressure.
pub(crate) fn event_channel() -> (EventSender, tokio::sync::mpsc::Receiver<TalEvent>) {
    let (public_tx, public_rx) = tokio::sync::mpsc::channel(TOTAL_CAPACITY);
    let inner = Arc::new(Inner {
        state: Mutex::new(State::default()),
        notify: Notify::new(),
        sender_count: AtomicUsize::new(1),
        control_events_dropped: std::sync::atomic::AtomicU64::new(0),
    });
    tokio::spawn(dispatch(inner.clone(), public_tx));
    (EventSender(inner), public_rx)
}

impl Clone for EventSender {
    fn clone(&self) -> Self {
        self.0.sender_count.fetch_add(1, Ordering::Relaxed);
        EventSender(self.0.clone())
    }
}

impl Drop for EventSender {
    fn drop(&mut self) {
        if self.0.sender_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.notify.notify_waiters();
        }
    }
}

impl EventSender {
    /// Number of lifecycle controls rejected because both bounded control
    /// lanes were full. This sticky signal lets a host fail closed even when
    /// the rejected `LinkDown` cannot be reconstructed from the public FIFO.
    pub(crate) fn control_events_dropped(&self) -> u64 {
        self.0.control_events_dropped.load(Ordering::Acquire)
    }

    fn control_full(&self) -> Result<(), ControlEnqueueError> {
        self.0.control_events_dropped.fetch_add(1, Ordering::Relaxed);
        Err(ControlEnqueueError::Full)
    }

    /// Enqueue a beacon or other non-link event without waiting.
    pub(crate) fn try_data(&self, event: TalEvent) -> Result<(), DataEnqueueError> {
        self.try_data_stamped(DataStamp::Unstamped, event)
    }

    /// Enqueue one generation-fenced link-data event without waiting.
    pub(crate) fn try_link_data(
        &self,
        link: LinkId,
        generation: u64,
        event: TalEvent,
    ) -> Result<(), DataEnqueueError> {
        self.try_data_stamped(DataStamp::Link { link, generation }, event)
    }

    fn try_data_stamped(&self, stamp: DataStamp, event: TalEvent) -> Result<(), DataEnqueueError> {
        let mut state = self.0.state.lock().expect("event ingress mutex poisoned");
        if let DataStamp::Link { link, generation } = stamp {
            if state.current.get(&link).copied() != Some(generation) {
                return Err(DataEnqueueError::Stale);
            }
        }
        if state.data.len() >= DATA_CAPACITY {
            return Err(DataEnqueueError::Full);
        }
        state.data.push_back(DataRecord { stamp, event });
        drop(state);
        self.0.notify.notify_one();
        Ok(())
    }

    /// Enqueue a LinkUp/LinkDown control event without waiting.
    ///
    /// LinkUp replaces an already-pending LinkUp for the same link (the most
    /// recent generation is the only useful one). Duplicate LinkDown events
    /// collapse.  A Down followed by an Up remains ordered in that sequence.
    pub(crate) fn try_control(
        &self,
        event: TalEvent,
        generation: u64,
    ) -> Result<(), ControlEnqueueError> {
        let (link, kind) = match event {
            TalEvent::LinkUp { link, .. } => (link, ControlKind::Up),
            TalEvent::LinkDown { link } => (link, ControlKind::Down),
            _ => return Err(ControlEnqueueError::InvalidEvent),
        };
        let mut state = self.0.state.lock().expect("event ingress mutex poisoned");

        if !state.can_track_generation(link) {
            return self.control_full();
        }

        if state.lifecycle_is_stale(link, generation) {
            // Delayed callbacks from an older transport generation are
            // idempotent no-ops and must not mutate a replacement link.
            return Ok(());
        }

        match kind {
            ControlKind::Down => {
                // Retire the ingress generation before checking capacity.
                // A physical teardown must never leave stale encrypted bytes
                // dispatchable merely because the normal control lane is full.
                state.record_generation(link, generation);
                state.current.remove(&link);
                purge_link(&mut state, link);
                if let Some(index) = state.controls.iter().position(|pending| {
                    pending.link == link && pending.kind == ControlKind::Down
                }) {
                    state.controls[index].generation = state.controls[index].generation.max(generation);
                    let mut seen_down = false;
                    state.controls.retain(|pending| {
                        if pending.link != link {
                            true
                        } else if !seen_down {
                            seen_down = pending.kind == ControlKind::Down;
                            true
                        } else {
                            false
                        }
                    });
                    return Ok(());
                }
                if let Some(existing) = state.emergency_down.get_mut(&link) {
                    *existing = (*existing).max(generation);
                    drop(state);
                    self.0.notify.notify_one();
                    return Ok(());
                }
                if state.controls.len() >= CONTROL_CAPACITY {
                    // If a LinkUp for this link is already pending, remove it
                    // before escalating Down. Otherwise emergency dispatch
                    // could expose Down before that Up and create a session
                    // after the transport has already been retired.
                    let had_pending_up = state.controls.iter().any(|pending| {
                        pending.link == link && pending.kind == ControlKind::Up
                    });
                    if had_pending_up {
                        state.controls.retain(|pending| {
                            pending.link != link || pending.kind != ControlKind::Up
                        });
                        state.controls.push_back(ControlRecord { link, generation, kind, event });
                        drop(state);
                        self.0.notify.notify_one();
                        return Ok(());
                    }
                    if state.emergency_down.len() >= CONTROL_CAPACITY {
                        return self.control_full();
                    }
                    state.emergency_down.insert(link, generation);
                    drop(state);
                    self.0.notify.notify_one();
                    return Ok(());
                }
                state.controls.push_back(ControlRecord { link, generation, kind, event });
            }
            ControlKind::Up => {
                // An emergency Down is already pending for this link. Do not
                // enqueue an Up behind it: callers must tear down the newly
                // established transport and retry after overload clears.
                if state.emergency_down.contains_key(&link) {
                    return self.control_full();
                }
                if let Some(index) = state.controls.iter().rposition(|pending| pending.link == link) {
                    if state.controls[index].kind == ControlKind::Up {
                        state.controls[index].generation = generation;
                        state.controls[index].event = event;
                        state.record_generation(link, generation);
                        state.current.insert(link, generation);
                        purge_except_generation(&mut state, link, generation);
                        drop(state);
                        self.0.notify.notify_one();
                        return Ok(());
                    }
                }
                if state.controls.len() >= CONTROL_CAPACITY {
                    return self.control_full();
                }
                state.record_generation(link, generation);
                state.current.insert(link, generation);
                purge_except_generation(&mut state, link, generation);
                state.controls.push_back(ControlRecord { link, generation, kind, event });
            }
        }
        drop(state);
        self.0.notify.notify_one();
        Ok(())
    }

}

impl State {
    fn can_track_generation(&mut self, link: LinkId) -> bool {
        if self.last_generation.contains_key(&link) {
            return true;
        }
        if self.generation_ledger_exhausted {
            return false;
        }
        if self.last_generation.len() >= MAX_GENERATION_TOMBSTONES {
            self.generation_ledger_exhausted = true;
            return false;
        }
        true
    }

    fn record_generation(&mut self, link: LinkId, generation: u64) -> bool {
        if let Some(last) = self.last_generation.get_mut(&link) {
            *last = (*last).max(generation);
            return true;
        }
        if self.generation_ledger_exhausted || self.last_generation.len() >= MAX_GENERATION_TOMBSTONES {
            self.generation_ledger_exhausted = true;
            return false;
        }
        self.last_generation.insert(link, generation);
        true
    }

    fn lifecycle_is_stale(&self, link: LinkId, generation: u64) -> bool {
        if let Some(current) = self.current.get(&link).copied() {
            return generation < current;
        }
        self.last_generation
            .get(&link)
            .is_some_and(|last| generation <= *last)
    }
}

fn purge_link(state: &mut State, link: LinkId) {
    state.data.retain(|record| {
        !matches!(record.stamp, DataStamp::Link { link: r_link, .. } if r_link == link)
    });
}

fn purge_except_generation(state: &mut State, link: LinkId, generation: u64) {
    state.data.retain(|record| {
        !matches!(record.stamp, DataStamp::Link { link: r_link, generation: r_gen } if r_link == link && r_gen != generation)
    });
}

async fn dispatch(inner: Arc<Inner>, public_tx: tokio::sync::mpsc::Sender<TalEvent>) {
    loop {
        let notified = inner.notify.notified();
        let record = {
            let mut state = inner.state.lock().expect("event ingress mutex poisoned");
            if let Some(control) = state.controls.pop_front() {
                Some(control.event)
            } else if let Some((link, _generation)) = state.emergency_down.iter().next().map(|(link, generation)| (*link, *generation)) {
                state.emergency_down.remove(&link);
                Some(TalEvent::LinkDown { link })
            } else {
                loop {
                    let Some(data) = state.data.pop_front() else { break None };
                    let current = match data.stamp {
                        DataStamp::Unstamped => true,
                        DataStamp::Link { link, generation } => {
                            state.current.get(&link).copied() == Some(generation)
                        }
                    };
                    if current {
                        break Some(data.event);
                    }
                }
            }
        };

        let Some(event) = record else {
            if inner.sender_count.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
            continue;
        };
        if public_tx.send(event).await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{event_channel, DataEnqueueError, CONTROL_CAPACITY, DATA_CAPACITY};
    use mmp_tal::{LinkQuality, TalEvent};
    use tokio::time::{timeout, Duration};

    fn data(sequence: usize) -> TalEvent {
        TalEvent::LinkData { link: 7, bytes: vec![sequence as u8] }
    }

    #[tokio::test]
    async fn control_survives_full_data_lane() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1)
            .expect("link must become current before data");
        for sequence in 0..DATA_CAPACITY {
            tx.try_link_data(7, 1, data(sequence)).expect("data budget must fit");
        }
        assert_eq!(tx.try_link_data(7, 1, data(0xff)), Err(DataEnqueueError::Full));
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).expect("reserved control lane must fit");
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        drop(tx);
        assert!(tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .expect("dispatcher must close after stale data is purged")
            .is_none());
    }

    #[tokio::test]
    async fn stale_data_is_dropped_after_replacement() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        tx.try_link_data(7, 1, data(1)).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 2).unwrap();
        tx.try_link_data(7, 1, data(2)).expect_err("old generation must be rejected");
        tx.try_link_data(7, 2, data(3)).unwrap();
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(data(3)));
    }

    #[tokio::test]
    async fn stale_lifecycle_controls_cannot_retire_new_generation() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 2).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality { mtu: 244, ..Default::default() } }, 1).unwrap();
        tx.try_link_data(7, 2, data(9)).expect("current generation must remain live");

        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(data(9)));
    }

    #[tokio::test]
    async fn stale_up_cannot_reopen_retired_generation() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        assert_eq!(tx.try_link_data(7, 1, data(9)), Err(DataEnqueueError::Stale));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
    }

    #[tokio::test]
    async fn control_enqueue_is_nonblocking_when_data_full() {
        let (tx, _rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        for sequence in 0..DATA_CAPACITY {
            tx.try_link_data(7, 1, data(sequence)).unwrap();
        }
        let task = tokio::spawn(async move {
            tx.try_control(TalEvent::LinkDown { link: 7 }, 1)
                .expect("control enqueue is synchronous");
        });
        timeout(Duration::from_millis(100), task)
            .await
            .expect("control enqueue must not wait for public receiver")
            .unwrap();
    }

    #[tokio::test]
    async fn control_ledger_coalesces_duplicate_down_and_latest_up() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality { mtu: 244, ..Default::default() } }, 2).unwrap();
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality { mtu: 244, ..Default::default() } }));
    }

    #[tokio::test]
    async fn full_control_lane_tombstones_data_before_emergency_down() {
        let (tx, mut rx) = event_channel();
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        tx.try_link_data(7, 1, data(1)).unwrap();
        for link in 8..(8 + CONTROL_CAPACITY as u64 - 1) {
            tx.try_control(TalEvent::LinkUp { link, initiator: true, quality: LinkQuality::default() }, 1).unwrap();
        }
        assert_eq!(tx.try_control(TalEvent::LinkDown { link: 7 }, 1), Ok(()));
        assert_eq!(tx.try_link_data(7, 1, data(2)), Err(DataEnqueueError::Stale));
        let mut seen = Vec::new();
        for _ in 0..CONTROL_CAPACITY {
            seen.push(rx.recv().await.expect("pending controls must drain"));
        }
        assert!(seen.iter().all(|event| {
            !matches!(event, TalEvent::LinkUp { link: 7, .. })
        }));
        assert!(seen.iter().any(|event| {
            matches!(event, TalEvent::LinkDown { link: 7 })
        }));
        // A second distinct teardown can use the bounded emergency lane while
        // the normal control ledger remains full.
        let second = 8 + CONTROL_CAPACITY as u64 - 2;
        tx.try_link_data(second, 1, TalEvent::LinkData { link: second, bytes: vec![3] }).unwrap();
        assert_eq!(tx.try_control(TalEvent::LinkDown { link: second }, 1), Ok(()));
        assert_eq!(tx.try_link_data(second, 1, TalEvent::LinkData { link: second, bytes: vec![4] }), Err(DataEnqueueError::Stale));
    }

    #[tokio::test]
    async fn control_saturation_is_sticky_and_visible() {
        let (tx, _rx) = event_channel();
        for link in 8..(8 + CONTROL_CAPACITY as u64) {
            tx.try_control(TalEvent::LinkUp { link, initiator: true, quality: LinkQuality::default() }, 1)
                .expect("normal control lane must fill");
        }
        for link in 100..(100 + CONTROL_CAPACITY as u64) {
            tx.try_control(TalEvent::LinkDown { link }, 1)
                .expect("emergency down lane must fill");
        }
        assert_eq!(tx.try_control(TalEvent::LinkDown { link: 999 }, 1), Err(super::ControlEnqueueError::Full));
        assert_eq!(tx.control_events_dropped(), 1);
    }
}
