#![no_std]

//! Static configuration for the first embedded MMP implementation target.
//!
//! This crate is intentionally a profile and compile probe, not a board
//! support package.  It does not select an executor, configure clocks or
//! radio peripherals, provide a linker script, or assign a flash journal
//! region.  Those decisions require board and power-loss evidence.

mod nvmc_backend;

#[cfg(target_arch = "arm")]
pub use nvmc_backend::{NvmcJournalBackend, NvmcJournalError};
pub use nvmc_backend::{NvmcRegion, NvmcRegionError};

/// Stable implementation identifier for the Nordic PCA10056 development kit.
pub const BOARD_ID: &str = "nrf52840-dk-pca10056";

/// Rust target used by the repository's embedded checks for this profile.
pub const TARGET: &str = "thumbv7em-none-eabihf";

/// Documented nRF52840 on-chip flash capacity in bytes (1 MiB).
pub const FLASH_CAPACITY_BYTES: usize = 1_048_576;

/// Documented nRF52840 on-chip RAM capacity in bytes (256 KiB).
pub const RAM_CAPACITY_BYTES: usize = 262_144;

/// The small, evidence-backed portion of the board profile that is safe to
/// expose before linker and storage geometry are selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardConfig {
    /// Board identifier, suitable for diagnostic output.
    pub board_id: &'static str,
    /// Rust target triple used for the compile probe.
    pub target: &'static str,
    /// Total flash capacity reported for the SoC, not application space.
    pub flash_capacity_bytes: usize,
    /// Total RAM capacity reported for the SoC, not free runtime RAM.
    pub ram_capacity_bytes: usize,
}

/// Return the profile constants without allocating or touching hardware.
pub const fn board_config() -> BoardConfig {
    BoardConfig {
        board_id: BOARD_ID,
        target: TARGET,
        flash_capacity_bytes: FLASH_CAPACITY_BYTES,
        ram_capacity_bytes: RAM_CAPACITY_BYTES,
    }
}

/// Parameters supplied by a board-specific storage profile.
///
/// The profile intentionally carries accounting inputs instead of selecting
/// a flash map. A board adapter must provide values backed by its linker map,
/// HAL, and power-loss evidence before it can use this configuration in a
/// target image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageConfig {
    /// Absolute start of the journal region in flash.
    pub store_region_offset_bytes: usize,
    /// Total bytes reserved for the journal region.
    pub store_region_bytes: usize,
    /// Smallest aligned flash-program operation supported by the provider.
    pub program_unit_bytes: usize,
    /// Erase-page size supported by the provider.
    pub erase_page_bytes: usize,
    /// Maximum number of live bundle slots, including reserved slots.
    pub max_live_bundles: usize,
    /// Maximum encoded bundle bytes admitted to one slot.
    pub max_bundle_bytes: usize,
    /// Fixed bytes assigned to one bundle slot in the snapshot.
    pub slot_bytes: usize,
    /// Maximum number of index entries in the snapshot.
    pub max_index_entries: usize,
    /// Maximum outstanding reservations held in runtime memory.
    pub max_reservations: usize,
    /// Maximum entries in one atomic batch held in runtime memory.
    pub max_batch_entries: usize,
    /// Maximum payload accepted by the journal encoder.
    pub max_snapshot_payload_bytes: usize,
    /// Fixed snapshot payload bytes, including generation metadata but not
    /// the journal record framing or alignment padding.
    pub snapshot_fixed_overhead_bytes: usize,
    /// Per-slot metadata bytes in the snapshot.
    pub metadata_bytes_per_slot: usize,
    /// Per-index-entry bytes in the snapshot.
    pub index_bytes_per_entry: usize,
    /// Auxiliary state bytes assigned to this provider profile.
    pub auxiliary_bytes: usize,
}

/// Deterministic validation failures for [`StorageConfig`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageConfigError {
    /// A required geometry or capacity value is zero.
    ZeroValue,
    /// The region offset is not aligned to the program or erase unit.
    RegionOffsetUnaligned,
    /// The region size is not aligned to the program or erase unit.
    RegionSizeUnaligned,
    /// The region end exceeds the selected flash capacity or overflows.
    RegionOutOfBounds,
    /// The index capacity does not match the live-slot capacity.
    IndexCapacityMismatch,
    /// A bundle does not fit in its configured slot.
    BundleExceedsSlot,
    /// Snapshot accounting overflowed checked arithmetic.
    SnapshotArithmeticOverflow,
    /// The complete snapshot exceeds the journal encoder payload bound.
    SnapshotExceedsPayload,
    /// The complete snapshot exceeds the configured journal region.
    SnapshotExceedsRegion,
}

impl StorageConfig {
    /// Validate this configuration against the documented SoC capacity.
    pub const fn validate(self) -> Result<(), StorageConfigError> {
        self.validate_against_flash(FLASH_CAPACITY_BYTES)
    }

    /// Validate this configuration against a caller-supplied flash capacity.
    ///
    /// This is `const` so a board crate can place the result in a compile-time
    /// assertion once its linker and provider values are authoritative.
    pub const fn validate_against_flash(
        self,
        flash_capacity_bytes: usize,
    ) -> Result<(), StorageConfigError> {
        if flash_capacity_bytes == 0
            || self.store_region_offset_bytes == 0
            || self.store_region_bytes == 0
            || self.program_unit_bytes == 0
            || self.erase_page_bytes == 0
            || self.max_live_bundles == 0
            || self.max_bundle_bytes == 0
            || self.slot_bytes == 0
            || self.max_index_entries == 0
            || self.max_reservations == 0
            || self.max_batch_entries == 0
            || self.max_snapshot_payload_bytes == 0
            || self.snapshot_fixed_overhead_bytes == 0
            || self.metadata_bytes_per_slot == 0
            || self.index_bytes_per_entry == 0
        {
            return Err(StorageConfigError::ZeroValue);
        }
        if !self
            .store_region_offset_bytes
            .is_multiple_of(self.program_unit_bytes)
            || !self
                .store_region_offset_bytes
                .is_multiple_of(self.erase_page_bytes)
        {
            return Err(StorageConfigError::RegionOffsetUnaligned);
        }
        if !self
            .store_region_bytes
            .is_multiple_of(self.program_unit_bytes)
            || !self
                .store_region_bytes
                .is_multiple_of(self.erase_page_bytes)
        {
            return Err(StorageConfigError::RegionSizeUnaligned);
        }
        let Some(region_end) = self
            .store_region_offset_bytes
            .checked_add(self.store_region_bytes)
        else {
            return Err(StorageConfigError::RegionOutOfBounds);
        };
        if region_end > flash_capacity_bytes {
            return Err(StorageConfigError::RegionOutOfBounds);
        }
        if self.max_index_entries != self.max_live_bundles {
            return Err(StorageConfigError::IndexCapacityMismatch);
        }
        if self.max_bundle_bytes > self.slot_bytes {
            return Err(StorageConfigError::BundleExceedsSlot);
        }
        if self.max_snapshot_payload_bytes > mmp_flash_journal::MAX_PAYLOAD_LEN {
            return Err(StorageConfigError::SnapshotExceedsPayload);
        }
        let Some(slot_with_metadata) = self.slot_bytes.checked_add(self.metadata_bytes_per_slot)
        else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        let Some(per_slot) = slot_with_metadata.checked_add(self.index_bytes_per_entry) else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        let Some(live_bytes) = per_slot.checked_mul(self.max_live_bundles) else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        let Some(snapshot_with_live) = self.snapshot_fixed_overhead_bytes.checked_add(live_bytes)
        else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        let Some(snapshot_bytes) = snapshot_with_live.checked_add(self.auxiliary_bytes) else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        if snapshot_bytes > self.max_snapshot_payload_bytes {
            return Err(StorageConfigError::SnapshotExceedsPayload);
        }
        let Some(encoded_snapshot_bytes) = mmp_flash_journal::encoded_len(snapshot_bytes) else {
            return Err(StorageConfigError::SnapshotArithmeticOverflow);
        };
        let remainder = encoded_snapshot_bytes % self.program_unit_bytes;
        let aligned_snapshot_bytes = if remainder == 0 {
            encoded_snapshot_bytes
        } else {
            let Some(aligned) =
                encoded_snapshot_bytes.checked_add(self.program_unit_bytes - remainder)
            else {
                return Err(StorageConfigError::SnapshotArithmeticOverflow);
            };
            aligned
        };
        if aligned_snapshot_bytes > self.store_region_bytes {
            return Err(StorageConfigError::SnapshotExceedsRegion);
        }
        Ok(())
    }
}

impl BoardConfig {
    /// Validate a storage profile against this board's documented capacity.
    pub const fn validate_storage(self, storage: StorageConfig) -> Result<(), StorageConfigError> {
        storage.validate_against_flash(self.flash_capacity_bytes)
    }
}

/// Return a type-name marker that proves the nRF52840 Embassy dependency is
/// selected in the ARM target build.
///
/// This is deliberately not a peripheral constructor: it cannot enable an
/// executor, claim a singleton, touch registers, or imply a radio backend.
#[cfg(target_arch = "arm")]
#[must_use]
pub fn embassy_peripherals_type_name() -> &'static str {
    core::any::type_name::<embassy_nrf::Peripherals>()
}

// The embedded target is checked as a library only.  Keep the host-side
// assertions out of the target test harness, which is unavailable until a
// board runner and linker image are selected.
#[cfg(all(test, not(target_arch = "arm")))]
mod tests {
    use super::*;

    const VALID_STORAGE: StorageConfig = StorageConfig {
        store_region_offset_bytes: 0x20_000,
        store_region_bytes: 0x10_000,
        program_unit_bytes: 4,
        erase_page_bytes: 4096,
        max_live_bundles: 4,
        max_bundle_bytes: 1024,
        slot_bytes: 1024,
        max_index_entries: 4,
        max_reservations: 2,
        max_batch_entries: 2,
        max_snapshot_payload_bytes: 65_536,
        snapshot_fixed_overhead_bytes: 256,
        metadata_bytes_per_slot: 64,
        index_bytes_per_entry: 32,
        auxiliary_bytes: 256,
    };

    #[test]
    fn profile_constants_are_explicit() {
        let config = board_config();
        assert_eq!(config.board_id, "nrf52840-dk-pca10056");
        assert_eq!(config.target, "thumbv7em-none-eabihf");
        assert_eq!(config.flash_capacity_bytes, 1_048_576);
        assert_eq!(config.ram_capacity_bytes, 262_144);
    }

    #[test]
    fn valid_storage_configuration_is_accepted() {
        assert_eq!(VALID_STORAGE.validate(), Ok(()));
        assert_eq!(board_config().validate_storage(VALID_STORAGE), Ok(()));
    }

    #[test]
    fn storage_configuration_rejects_zero_and_misaligned_values() {
        let mut zero = VALID_STORAGE;
        zero.program_unit_bytes = 0;
        assert_eq!(zero.validate(), Err(StorageConfigError::ZeroValue));

        let mut misaligned = VALID_STORAGE;
        misaligned.store_region_offset_bytes += 1;
        assert_eq!(
            misaligned.validate(),
            Err(StorageConfigError::RegionOffsetUnaligned)
        );
    }

    #[test]
    fn storage_configuration_rejects_overflow_and_accounting_mismatches() {
        let mut out_of_bounds = VALID_STORAGE;
        out_of_bounds.store_region_offset_bytes = usize::MAX - 4095;
        assert_eq!(
            out_of_bounds.validate(),
            Err(StorageConfigError::RegionOutOfBounds)
        );

        let mut wrong_index = VALID_STORAGE;
        wrong_index.max_index_entries = wrong_index.max_live_bundles + 1;
        assert_eq!(
            wrong_index.validate(),
            Err(StorageConfigError::IndexCapacityMismatch)
        );

        let mut too_large = VALID_STORAGE;
        too_large.max_bundle_bytes = too_large.slot_bytes + 1;
        assert_eq!(
            too_large.validate(),
            Err(StorageConfigError::BundleExceedsSlot)
        );

        let mut too_many = VALID_STORAGE;
        too_many.max_live_bundles = usize::MAX;
        too_many.max_index_entries = usize::MAX;
        assert_eq!(
            too_many.validate(),
            Err(StorageConfigError::SnapshotArithmeticOverflow)
        );
    }

    #[test]
    fn storage_configuration_rejects_snapshot_limits() {
        let mut payload = VALID_STORAGE;
        payload.max_snapshot_payload_bytes = 512;
        assert_eq!(
            payload.validate(),
            Err(StorageConfigError::SnapshotExceedsPayload)
        );

        let mut region = VALID_STORAGE;
        region.store_region_bytes = 4096;
        assert_eq!(
            region.validate(),
            Err(StorageConfigError::SnapshotExceedsRegion)
        );
    }
}
