#![no_std]

//! Static configuration for the first embedded MMP implementation target.
//!
//! This crate is intentionally a profile and compile probe, not a board
//! support package.  It does not select an executor, configure clocks or
//! radio peripherals, provide a linker script, or assign a flash journal
//! region.  Those decisions require board and power-loss evidence.

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

    #[test]
    fn profile_constants_are_explicit() {
        let config = board_config();
        assert_eq!(config.board_id, "nrf52840-dk-pca10056");
        assert_eq!(config.target, "thumbv7em-none-eabihf");
        assert_eq!(config.flash_capacity_bytes, 1_048_576);
        assert_eq!(config.ram_capacity_bytes, 262_144);
    }
}
