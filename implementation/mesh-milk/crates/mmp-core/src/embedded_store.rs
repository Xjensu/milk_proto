//! Allocation-free-facing storage contract for fixed-capacity providers.
//!
//! This trait is deliberately separate from [`crate::BundleStore`]. The host
//! compatibility trait keeps allocating methods for existing applications;
//! an embedded provider must instead implement these caller-owned operations
//! directly and must not rely on allocation-backed default methods.

use mmp_types::ids::Id16;

use crate::store::{BundleMeta, StoreReadError};

/// Caller-owned storage operations required by an embedded bundle provider.
///
/// The trait contains no collection-returning methods and no input-derived
/// allocation hooks. Implementations choose their own fixed-capacity slot and
/// journal policy. Mutation/reservation methods remain provider-specific until
/// the board profile fixes its crash-recovery and indeterminate-write policy.
pub trait EmbeddedBundleStore {
    /// Copy metadata for `id` into `out` without changing `out` on failure.
    fn read_meta(&self, id: &Id16, out: &mut BundleMeta) -> Result<(), StoreReadError>;

    /// Copy the encoded bundle for `id` into `out`.
    ///
    /// A short output buffer returns [`StoreReadError::OutputTooSmall`] before
    /// modifying the buffer or provider state.
    fn read_bundle(&self, id: &Id16, out: &mut [u8]) -> Result<usize, StoreReadError>;

    /// Visit IDs in inclusive lexical range order.
    ///
    /// The callback receives copied IDs and must not retain references to
    /// provider state. A provider must bound the number of records it can
    /// visit through its static capacity configuration.
    fn visit_ids(&self, lo: &Id16, hi: &Id16, visitor: &mut dyn FnMut(Id16));

    /// Visit all live metadata records in the provider's documented order.
    fn visit_meta(&self, visitor: &mut dyn FnMut(BundleMeta));

    /// Remove expired records into caller-owned storage.
    ///
    /// The provider must preflight the output capacity. On
    /// [`StoreReadError::OutputTooSmall`], neither the output nor store state
    /// may change.
    fn expire_into(&mut self, now_ms: u64, out: &mut [Id16]) -> Result<usize, StoreReadError>;
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::EmbeddedBundleStore;
    use crate::store::{BundleMeta, SlotState, StoreReadError};
    use mmp_types::ids::Id16;

    #[derive(Clone, Copy)]
    struct Entry<const B: usize> {
        meta: BundleMeta,
        bytes: [u8; B],
        len: usize,
    }

    struct FixedStore<const N: usize, const B: usize> {
        slots: [Option<Entry<B>>; N],
    }

    impl<const N: usize, const B: usize> FixedStore<N, B> {
        fn new() -> Self {
            Self { slots: [None; N] }
        }

        fn insert(&mut self, meta: BundleMeta, bytes: &[u8]) {
            assert!(bytes.len() <= B);
            let slot = self
                .slots
                .iter_mut()
                .find(|slot| slot.is_none())
                .expect("test capacity");
            let mut stored = [0; B];
            stored[..bytes.len()].copy_from_slice(bytes);
            *slot = Some(Entry {
                meta,
                bytes: stored,
                len: bytes.len(),
            });
        }
    }

    impl<const N: usize, const B: usize> EmbeddedBundleStore for FixedStore<N, B> {
        fn read_meta(&self, id: &Id16, out: &mut BundleMeta) -> Result<(), StoreReadError> {
            let Some(entry) = self
                .slots
                .iter()
                .flatten()
                .find(|entry| entry.meta.id == *id)
            else {
                return Err(StoreReadError::NotFound);
            };
            *out = entry.meta;
            Ok(())
        }

        fn read_bundle(&self, id: &Id16, out: &mut [u8]) -> Result<usize, StoreReadError> {
            let Some(entry) = self
                .slots
                .iter()
                .flatten()
                .find(|entry| entry.meta.id == *id)
            else {
                return Err(StoreReadError::NotFound);
            };
            if entry.len > out.len() {
                return Err(StoreReadError::OutputTooSmall {
                    required: entry.len,
                });
            }
            out[..entry.len].copy_from_slice(&entry.bytes[..entry.len]);
            Ok(entry.len)
        }

        fn visit_ids(&self, lo: &Id16, hi: &Id16, visitor: &mut dyn FnMut(Id16)) {
            let mut visited = [false; N];
            for _ in 0..N {
                let next = self
                    .slots
                    .iter()
                    .enumerate()
                    .filter(|(index, slot)| {
                        !visited[*index]
                            && slot
                                .as_ref()
                                .is_some_and(|entry| entry.meta.id >= *lo && entry.meta.id <= *hi)
                    })
                    .min_by_key(|(_, slot)| slot.as_ref().expect("filtered entry").meta.id);
                let Some((index, slot)) = next else { break };
                visited[index] = true;
                visitor(slot.as_ref().expect("filtered entry").meta.id);
            }
        }

        fn visit_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
            for entry in self.slots.iter().flatten() {
                visitor(entry.meta);
            }
        }

        fn expire_into(&mut self, now_ms: u64, out: &mut [Id16]) -> Result<usize, StoreReadError> {
            let required = self
                .slots
                .iter()
                .flatten()
                .filter(|entry| entry.meta.expires_at <= now_ms)
                .count();
            if required > out.len() {
                return Err(StoreReadError::OutputTooSmall { required });
            }
            let mut count = 0;
            for slot in &mut self.slots {
                if slot
                    .as_ref()
                    .is_some_and(|entry| entry.meta.expires_at <= now_ms)
                {
                    out[count] = slot.as_ref().expect("expired entry").meta.id;
                    count += 1;
                    *slot = None;
                }
            }
            Ok(count)
        }
    }

    fn meta(id: u8, expires_at: u64) -> BundleMeta {
        BundleMeta {
            id: Id16([id; 16]),
            class: 1,
            origin: Id16([9; 16]),
            dest: Id16([8; 16]),
            priority: 1,
            effective_priority: 1,
            size: 3,
            stored_at: 0,
            expires_at,
            budget: 4,
            own_origin: false,
            slot: SlotState::Stored,
        }
    }

    #[test]
    fn read_operations_fail_closed_and_preserve_outputs() {
        let mut store = FixedStore::<2, 8>::new();
        store.insert(meta(2, 20), b"abc");
        let missing = Id16([99; 16]);
        let mut metadata = meta(7, 7);
        assert_eq!(
            store.read_meta(&missing, &mut metadata),
            Err(StoreReadError::NotFound)
        );
        assert_eq!(metadata.id, Id16([7; 16]));

        let mut output = [0xA5; 2];
        assert_eq!(
            store.read_bundle(&Id16([2; 16]), &mut output),
            Err(StoreReadError::OutputTooSmall { required: 3 })
        );
        assert_eq!(output, [0xA5; 2]);

        let mut bytes = [0; 8];
        assert_eq!(store.read_bundle(&Id16([2; 16]), &mut bytes), Ok(3));
        assert_eq!(&bytes[..3], b"abc");
    }

    #[test]
    fn visitation_is_bounded_by_fixed_capacity_and_sorted_for_ids() {
        let mut store = FixedStore::<3, 8>::new();
        store.insert(meta(9, 20), b"9");
        store.insert(meta(1, 20), b"1");
        store.insert(meta(5, 20), b"5");
        let mut ids = Vec::new();
        store.visit_ids(&Id16([1; 16]), &Id16([9; 16]), &mut |id| ids.push(id.0[0]));
        assert_eq!(ids, vec![1, 5, 9]);

        let mut count = 0;
        store.visit_meta(&mut |_| count += 1);
        assert_eq!(count, 3);
    }

    #[test]
    fn expiry_preflights_output_before_mutation() {
        let mut store = FixedStore::<2, 8>::new();
        store.insert(meta(1, 10), b"one");
        store.insert(meta(2, 100), b"two");
        let mut too_small = [Id16([0xEE; 16]); 0];
        assert_eq!(
            store.expire_into(10, &mut too_small),
            Err(StoreReadError::OutputTooSmall { required: 1 })
        );
        let mut metadata = meta(7, 7);
        assert_eq!(store.read_meta(&Id16([1; 16]), &mut metadata), Ok(()));

        let mut expired = [Id16([0; 16]); 1];
        assert_eq!(store.expire_into(10, &mut expired), Ok(1));
        assert_eq!(expired[0], Id16([1; 16]));
        assert_eq!(
            store.read_meta(&Id16([1; 16]), &mut metadata),
            Err(StoreReadError::NotFound)
        );
    }
}
