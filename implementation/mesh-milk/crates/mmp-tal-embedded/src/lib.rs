//! mmp-tal-embedded — nRF52 (nrf-softdevice) / ESP32 (NimBLE) TAL.
//!
//! Status: out of MVP scope by decision (HANDOFF §0 — "keep the `no_std`
//! discipline, ship no embedded TAL"). The lower protocol layers are
//! `no_std`-clean so this crate can be filled in without touching them; the
//! Core-profile budget gate (≤256 KiB flash / 48 KiB RAM, doc 12 §7.6) and the
//! OQ-7 RS-decode bench are the acceptance criteria when this work starts.

#![no_std]

extern crate alloc;

pub use mmp_tal::{LinkId, LinkQuality, TalEvent};

/// Maximum legacy advertisement payload accepted by the bounded ingress seam.
pub const MAX_BEACON_PAYLOAD: usize = 31;
/// Maximum raw session envelope accepted by one ingress record.
pub const MAX_LINK_DATA_BYTES: usize = mmp_types::frame::MAX_ENVELOPE_PAYLOAD + 3;

/// Failure returned when an embedded event cannot be admitted without growing
/// memory or accepting a wire-sized input outside the existing frame bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueError {
    /// The fixed-capacity queue has no free slot.
    Full,
    /// The event carries more raw bytes than the existing TAL/frame boundary.
    Oversize,
}

/// A fixed-capacity, allocation-free-after-construction event queue.
///
/// The queue owns the `Vec<u8>` already present in a [`TalEvent`], but never
/// allocates storage for queue slots during `try_push`. An eventual board
/// adapter can choose a capacity from its RAM budget without changing the
/// shared TAL vocabulary or protocol wire format.
pub struct EmbeddedEventQueue<const CAPACITY: usize> {
    slots: [Option<TalEvent>; CAPACITY],
    head: usize,
    len: usize,
}

impl<const CAPACITY: usize> EmbeddedEventQueue<CAPACITY> {
    /// Construct an empty queue. `CAPACITY == 0` is valid but cannot admit an
    /// event; this keeps the const-generic API total for compile-time probes.
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
            head: 0,
            len: 0,
        }
    }

    /// Number of occupied slots.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Configured slot capacity.
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    /// Whether no event is queued.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether every slot is occupied.
    pub const fn is_full(&self) -> bool {
        self.len == CAPACITY
    }

    /// Admit one bounded event without allocating a queue slot.
    pub fn try_push(&mut self, event: TalEvent) -> Result<(), EnqueueError> {
        validate_event(&event)?;
        if CAPACITY == 0 || self.len == CAPACITY {
            return Err(EnqueueError::Full);
        }
        let index = (self.head + self.len) % CAPACITY;
        self.slots[index] = Some(event);
        self.len += 1;
        Ok(())
    }

    /// Remove the oldest event, if any.
    pub fn pop(&mut self) -> Option<TalEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.slots[self.head].take();
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        event
    }
}

impl<const CAPACITY: usize> Default for EmbeddedEventQueue<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_event(event: &TalEvent) -> Result<(), EnqueueError> {
    match event {
        TalEvent::Beacon { payload, .. } if payload.len() > MAX_BEACON_PAYLOAD => {
            Err(EnqueueError::Oversize)
        }
        TalEvent::LinkData { bytes, .. } if bytes.len() > MAX_LINK_DATA_BYTES => {
            Err(EnqueueError::Oversize)
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{EmbeddedEventQueue, EnqueueError, LinkQuality, TalEvent, MAX_LINK_DATA_BYTES};

    fn data(byte: u8) -> TalEvent {
        TalEvent::LinkData {
            link: 7,
            bytes: vec![byte],
        }
    }

    #[test]
    fn queue_is_fifo_and_bounded() {
        let mut queue = EmbeddedEventQueue::<2>::new();
        assert_eq!(queue.capacity(), 2);
        queue.try_push(data(1)).unwrap();
        queue.try_push(data(2)).unwrap();
        assert_eq!(queue.try_push(data(3)), Err(EnqueueError::Full));
        assert_eq!(queue.pop(), Some(data(1)));
        queue.try_push(data(3)).unwrap();
        assert_eq!(queue.pop(), Some(data(2)));
        assert_eq!(queue.pop(), Some(data(3)));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn queue_rejects_oversize_wire_inputs_before_admission() {
        let mut queue = EmbeddedEventQueue::<2>::new();
        let oversized = TalEvent::LinkData {
            link: 1,
            bytes: vec![0; MAX_LINK_DATA_BYTES + 1],
        };
        assert_eq!(queue.try_push(oversized), Err(EnqueueError::Oversize));
        let oversized_beacon = TalEvent::Beacon {
            payload: vec![0; super::MAX_BEACON_PAYLOAD + 1],
            rssi: -40,
        };
        assert_eq!(
            queue.try_push(oversized_beacon),
            Err(EnqueueError::Oversize)
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn control_events_and_zero_capacity_fail_without_allocation() {
        let mut queue = EmbeddedEventQueue::<0>::new();
        assert_eq!(
            queue.try_push(TalEvent::LinkUp {
                link: 1,
                initiator: true,
                quality: LinkQuality::default(),
            }),
            Err(EnqueueError::Full)
        );
        assert_eq!(queue.pop(), None);
    }
}
