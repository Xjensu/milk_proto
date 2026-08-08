//! Bounded, control-priority ingress for the Linux peripheral backend.
//!
//! BlueZ reader callbacks must not await the host event stream while holding
//! lifecycle locks.  This module keeps a synchronous two-lane ingress and
//! leaves the only public-queue await to a detached dispatcher.  Stream data
//! is fenced by direction-specific generations so bytes from a retired
//! packet socket cannot reach the node after reconnect.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mmp_tal::{LinkId, TalEvent};
use tokio::sync::Notify;

pub(crate) const TOTAL_CAPACITY: usize = 2048;
pub(crate) const CONTROL_CAPACITY: usize = 64;
pub(crate) const DATA_CAPACITY: usize = TOTAL_CAPACITY - CONTROL_CAPACITY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StreamStamp {
    pub(crate) generation: u64,
    /// 0 = reader (inbound writes), 1 = writer (outbound notifications).
    pub(crate) side: u8,
}

#[derive(Debug)]
struct DataRecord {
    link: LinkId,
    stamp: StreamStamp,
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
    /// Bounded fallback for LinkDown when the normal lifecycle lane is full.
    emergency_down: HashMap<LinkId, u64>,
    current: HashMap<LinkId, [Option<StreamStamp>; 2]>,
}

struct Inner {
    state: Mutex<State>,
    notify: Notify,
    sender_count: AtomicUsize,
    control_events_dropped: std::sync::atomic::AtomicU64,
}

pub(crate) struct EventSender(Arc<Inner>);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DataEnqueueError {
    Full,
    Stale,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ControlEnqueueError {
    Full,
    InvalidEvent,
}

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
        Self(self.0.clone())
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

    /// Mark the stream endpoints currently admitted for a link and discard
    /// queued bytes from any previous direction/generation.
    pub(crate) fn set_streams(&self, link: LinkId, streams: [Option<StreamStamp>; 2]) {
        let mut state = self.0.state.lock().expect("event ingress mutex poisoned");
        state.current.insert(link, streams);
        state.data.retain(|record| {
            record.link != link || streams.iter().flatten().any(|stamp| *stamp == record.stamp)
        });
        drop(state);
        self.0.notify.notify_one();
    }

    /// Retire every stream for a link and purge pending bytes.  This is used
    /// on overflow and teardown; control delivery remains a separate path.
    pub(crate) fn retire_link(&self, link: LinkId) {
        let mut state = self.0.state.lock().expect("event ingress mutex poisoned");
        state.current.remove(&link);
        state.data.retain(|record| record.link != link);
        drop(state);
        self.0.notify.notify_one();
    }

    pub(crate) fn try_stream_data(
        &self,
        link: LinkId,
        stamp: StreamStamp,
        event: TalEvent,
    ) -> Result<(), DataEnqueueError> {
        let mut state = self.0.state.lock().expect("event ingress mutex poisoned");
        let side = usize::from(stamp.side);
        if side >= 2
            || state.current.get(&link).and_then(|streams| streams[side]) != Some(stamp)
        {
            return Err(DataEnqueueError::Stale);
        }
        if state.data.len() >= DATA_CAPACITY {
            return Err(DataEnqueueError::Full);
        }
        state.data.push_back(DataRecord { link, stamp, event });
        drop(state);
        self.0.notify.notify_one();
        Ok(())
    }

    /// Enqueue a lifecycle control event.  Duplicate Down records collapse;
    /// a Down followed by a new Up remains ordered.
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
        if let ControlKind::Down = kind {
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
                drop(state);
                self.0.notify.notify_one();
                return Ok(());
            }
            if let Some(existing) = state.emergency_down.get_mut(&link) {
                *existing = (*existing).max(generation);
                drop(state);
                self.0.notify.notify_one();
                return Ok(());
            }
            if state.controls.len() >= CONTROL_CAPACITY {
                let had_same_link = state.controls.iter().any(|pending| pending.link == link);
                if had_same_link {
                    state.controls.retain(|pending| pending.link != link);
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
        } else if state.emergency_down.contains_key(&link) {
            return self.control_full();
        }
        if let Some(index) = state.controls.iter().rposition(|pending| pending.link == link) {
            if state.controls[index].kind == kind {
                state.controls[index].generation = state.controls[index].generation.max(generation);
                if kind == ControlKind::Up {
                    state.controls[index].event = event;
                }
                drop(state);
                self.0.notify.notify_one();
                return Ok(());
            }
        }
        if state.controls.len() >= CONTROL_CAPACITY {
            return self.control_full();
        }
        state.controls.push_back(ControlRecord { link, generation, kind, event });
        drop(state);
        self.0.notify.notify_one();
        Ok(())
    }
}

fn purge_link(state: &mut State, link: LinkId) {
    state.data.retain(|record| record.link != link);
}

async fn dispatch(inner: Arc<Inner>, public_tx: tokio::sync::mpsc::Sender<TalEvent>) {
    loop {
        let notified = inner.notify.notified();
        let event = {
            let mut state = inner.state.lock().expect("event ingress mutex poisoned");
            if let Some(control) = state.controls.pop_front() {
                Some(control.event)
            } else if let Some((link, _generation)) = state.emergency_down.iter().next().map(|(link, generation)| (*link, *generation)) {
                state.emergency_down.remove(&link);
                Some(TalEvent::LinkDown { link })
            } else {
                loop {
                    let Some(data) = state.data.pop_front() else { break None };
                    let current = state
                        .current
                        .get(&data.link)
                        .is_some_and(|streams| streams.iter().flatten().any(|stamp| *stamp == data.stamp));
                    if current {
                        break Some(data.event);
                    }
                }
            }
        };
        let Some(event) = event else {
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
    use super::{event_channel, DataEnqueueError, StreamStamp, CONTROL_CAPACITY, DATA_CAPACITY};
    use mmp_tal::{LinkQuality, TalEvent};
    use tokio::time::{timeout, Duration};

    fn data(byte: u8) -> TalEvent {
        TalEvent::LinkData { link: 7, bytes: vec![byte] }
    }

    fn reader(generation: u64) -> StreamStamp {
        StreamStamp { generation, side: 0 }
    }

    #[tokio::test]
    async fn control_survives_full_data_lane() {
        let (tx, mut rx) = event_channel();
        tx.set_streams(7, [Some(reader(1)), None]);
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }, 1).unwrap();
        for byte in 0..DATA_CAPACITY { tx.try_stream_data(7, reader(1), data(byte as u8)).unwrap(); }
        assert_eq!(tx.try_stream_data(7, reader(1), data(0xff)), Err(DataEnqueueError::Full));
        tx.retire_link(7);
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        drop(tx);
        assert!(timeout(Duration::from_millis(100), rx.recv()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn stale_stream_data_is_rejected_after_replacement() {
        let (tx, mut rx) = event_channel();
        tx.set_streams(7, [Some(reader(1)), None]);
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }, 1).unwrap();
        tx.try_stream_data(7, reader(1), data(1)).unwrap();
        tx.set_streams(7, [Some(reader(2)), None]);
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.set_streams(7, [Some(reader(2)), None]);
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }, 2).unwrap();
        assert_eq!(tx.try_stream_data(7, reader(1), data(2)), Err(DataEnqueueError::Stale));
        tx.try_stream_data(7, reader(2), data(3)).unwrap();
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality::default() }));
        assert_eq!(rx.recv().await, Some(data(3)));
    }

    #[tokio::test]
    async fn enqueue_is_nonblocking_when_public_receiver_is_not_drained() {
        let (tx, _rx) = event_channel();
        tx.set_streams(7, [Some(reader(1)), None]);
        for byte in 0..DATA_CAPACITY { tx.try_stream_data(7, reader(1), data(byte as u8)).unwrap(); }
        let task = tokio::spawn(async move { tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap(); });
        timeout(Duration::from_millis(100), task).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn control_ledger_preserves_replacement_order_and_coalesces_duplicates() {
        let (tx, mut rx) = event_channel();
        let quality = LinkQuality::default();
        tx.set_streams(7, [Some(reader(1)), None]);
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: false, quality }, 1).unwrap();
        tx.retire_link(7);
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        tx.set_streams(7, [Some(reader(2)), None]);
        tx.try_control(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality { mtu: 244, ..quality } }, 2).unwrap();

        assert_eq!(rx.recv().await, Some(TalEvent::LinkUp { link: 7, initiator: false, quality }));
        assert_eq!(rx.recv().await, Some(TalEvent::LinkDown { link: 7 }));
        assert_eq!(
            rx.recv().await,
            Some(TalEvent::LinkUp { link: 7, initiator: false, quality: LinkQuality { mtu: 244, ..quality } })
        );
    }

    #[tokio::test]
    async fn emergency_down_is_ordered_after_normal_controls() {
        let (tx, mut rx) = event_channel();
        for link in 8..(8 + CONTROL_CAPACITY as u64) {
            tx.try_control(
                TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() },
                1,
            )
            .unwrap();
        }
        tx.try_control(TalEvent::LinkDown { link: 7 }, 1).unwrap();
        let mut seen = Vec::new();
        for _ in 0..=CONTROL_CAPACITY {
            seen.push(rx.recv().await.expect("control records must drain"));
        }
        assert!(matches!(seen.last(), Some(TalEvent::LinkDown { link: 7 })));
    }

    #[tokio::test]
    async fn control_saturation_is_sticky_and_visible() {
        let (tx, _rx) = event_channel();
        for link in 8..(8 + CONTROL_CAPACITY as u64) {
            tx.try_control(TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() }, 1)
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
