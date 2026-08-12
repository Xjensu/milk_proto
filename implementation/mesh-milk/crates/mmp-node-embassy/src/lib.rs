//! mmp-node-embassy — embedded orchestration on the Embassy executor.
//!
//! The core stack is sans-IO with an injected monotonic clock, so this crate
//! owns only bounded ingress/egress buffering. It does not add an executor,
//! hardware backend, or a second protocol implementation.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use mmp_tal::{LinkId, LinkQuality, TalEvent};
pub use mmp_tal_embedded::{EmbeddedEventQueue, EnqueueError};

/// Maximum outbound burst accepted by the embedded pump.
///
/// This is the same bound used by the embedded TAL for one raw link-data
/// record. The pump owns an already-created `Vec<u8>` and never grows it.
pub const MAX_OUTBOUND_BYTES: usize = mmp_tal_embedded::MAX_LINK_DATA_BYTES;

/// One already-created outbound burst waiting for the native TAL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundData {
    /// Host-assigned identifier of the link that should receive the burst.
    pub link: LinkId,
    /// One bounded, length-prefixed wire burst.
    pub bytes: Vec<u8>,
}

/// Failure returned when an outbound burst cannot be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboundError {
    /// The fixed-capacity egress queue has no free slot.
    Full,
    /// The burst is larger than the existing TAL/frame boundary.
    Oversize,
}

/// Fixed-capacity bridge between radio events and outbound wire bursts.
///
/// Ingress reuses [`EmbeddedEventQueue`] so its event-size validation and FIFO
/// semantics remain identical to the embedded TAL. Egress owns only the
/// `Vec<u8>` values supplied by the already-running sans-IO node; it allocates
/// no queue storage after construction.
pub struct EmbeddedEventPump<const INGRESS_CAPACITY: usize, const EGRESS_CAPACITY: usize> {
    ingress: EmbeddedEventQueue<INGRESS_CAPACITY>,
    egress: [Option<OutboundData>; EGRESS_CAPACITY],
    egress_head: usize,
    egress_len: usize,
}

impl<const INGRESS_CAPACITY: usize, const EGRESS_CAPACITY: usize>
    EmbeddedEventPump<INGRESS_CAPACITY, EGRESS_CAPACITY>
{
    /// Construct an empty pump. Either capacity may be zero.
    pub fn new() -> Self {
        Self {
            ingress: EmbeddedEventQueue::new(),
            egress: core::array::from_fn(|_| None),
            egress_head: 0,
            egress_len: 0,
        }
    }

    /// Configured ingress slot capacity.
    pub const fn ingress_capacity(&self) -> usize {
        INGRESS_CAPACITY
    }

    /// Configured egress slot capacity.
    pub const fn egress_capacity(&self) -> usize {
        EGRESS_CAPACITY
    }

    /// Number of queued ingress events.
    pub const fn ingress_len(&self) -> usize {
        self.ingress.len()
    }

    /// Number of queued outbound bursts.
    pub const fn egress_len(&self) -> usize {
        self.egress_len
    }

    /// Admit one radio event using the embedded TAL's existing bounds.
    pub fn try_push_ingress(&mut self, event: TalEvent) -> Result<(), EnqueueError> {
        self.ingress.try_push(event)
    }

    /// Remove the oldest radio event.
    pub fn pop_ingress(&mut self) -> Option<TalEvent> {
        self.ingress.pop()
    }

    /// Admit one already-created outbound burst without growing the queue.
    pub fn try_push_egress(&mut self, outbound: OutboundData) -> Result<(), OutboundError> {
        if outbound.bytes.len() > MAX_OUTBOUND_BYTES {
            return Err(OutboundError::Oversize);
        }
        if EGRESS_CAPACITY == 0 || self.egress_len == EGRESS_CAPACITY {
            return Err(OutboundError::Full);
        }
        let index = (self.egress_head + self.egress_len) % EGRESS_CAPACITY;
        self.egress[index] = Some(outbound);
        self.egress_len += 1;
        Ok(())
    }

    /// Remove the oldest outbound burst.
    pub fn pop_egress(&mut self) -> Option<OutboundData> {
        if self.egress_len == 0 {
            return None;
        }
        let outbound = self.egress[self.egress_head].take();
        self.egress_head = (self.egress_head + 1) % EGRESS_CAPACITY;
        self.egress_len -= 1;
        outbound
    }
}

impl<const INGRESS_CAPACITY: usize, const EGRESS_CAPACITY: usize> Default
    for EmbeddedEventPump<INGRESS_CAPACITY, EGRESS_CAPACITY>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{EmbeddedEventPump, OutboundData, OutboundError, TalEvent, MAX_OUTBOUND_BYTES};

    fn event(byte: u8) -> TalEvent {
        TalEvent::LinkData {
            link: 7,
            bytes: vec![byte],
        }
    }

    fn outbound(byte: u8) -> OutboundData {
        OutboundData {
            link: 11,
            bytes: vec![byte],
        }
    }

    #[test]
    fn ingress_and_egress_are_fifo_and_bounded() {
        let mut pump = EmbeddedEventPump::<2, 2>::new();
        assert_eq!(pump.ingress_capacity(), 2);
        assert_eq!(pump.egress_capacity(), 2);

        pump.try_push_ingress(event(1)).unwrap();
        pump.try_push_ingress(event(2)).unwrap();
        assert_eq!(pump.pop_ingress(), Some(event(1)));
        pump.try_push_ingress(event(3)).unwrap();
        assert_eq!(pump.pop_ingress(), Some(event(2)));
        assert_eq!(pump.pop_ingress(), Some(event(3)));
        assert_eq!(pump.pop_ingress(), None);

        pump.try_push_egress(outbound(1)).unwrap();
        pump.try_push_egress(outbound(2)).unwrap();
        assert_eq!(pump.pop_egress(), Some(outbound(1)));
        pump.try_push_egress(outbound(3)).unwrap();
        assert_eq!(pump.pop_egress(), Some(outbound(2)));
        assert_eq!(pump.pop_egress(), Some(outbound(3)));
        assert_eq!(pump.pop_egress(), None);
    }

    #[test]
    fn egress_capacity_rejection_preserves_existing_entries() {
        let mut pump = EmbeddedEventPump::<1, 1>::new();
        pump.try_push_egress(outbound(1)).unwrap();
        assert_eq!(pump.try_push_egress(outbound(2)), Err(OutboundError::Full));
        assert_eq!(pump.egress_len(), 1);
        assert_eq!(pump.pop_egress(), Some(outbound(1)));
    }

    #[test]
    fn egress_oversize_is_rejected_before_capacity_admission() {
        let mut pump = EmbeddedEventPump::<1, 1>::new();
        let oversized = OutboundData {
            link: 11,
            bytes: vec![0; MAX_OUTBOUND_BYTES + 1],
        };
        assert_eq!(
            pump.try_push_egress(oversized),
            Err(OutboundError::Oversize)
        );
        assert_eq!(pump.egress_len(), 0);

        pump.try_push_egress(outbound(1)).unwrap();
        let oversized_when_full = OutboundData {
            link: 11,
            bytes: vec![0; MAX_OUTBOUND_BYTES + 1],
        };
        assert_eq!(
            pump.try_push_egress(oversized_when_full),
            Err(OutboundError::Oversize)
        );
        assert_eq!(pump.egress_len(), 1);
    }

    #[test]
    fn zero_capacity_rejects_without_touching_other_queue() {
        let mut pump = EmbeddedEventPump::<0, 0>::default();
        assert_eq!(pump.ingress_capacity(), 0);
        assert_eq!(pump.egress_capacity(), 0);
        assert_eq!(
            pump.try_push_ingress(event(1)),
            Err(super::EnqueueError::Full)
        );
        assert_eq!(pump.try_push_egress(outbound(1)), Err(OutboundError::Full));
        assert_eq!(pump.pop_ingress(), None);
        assert_eq!(pump.pop_egress(), None);
    }
}
