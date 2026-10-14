#[cfg(target_arch = "arm")]
use core::cell::RefCell;
use mmp_flash_journal::Geometry;
#[cfg(target_arch = "arm")]
use mmp_flash_journal::JournalBackend;

/// NVMC's word-programming alignment on nRF52840.
pub const NVMC_WRITE_ALIGNMENT_BYTES: usize = 4;
/// NVMC's page-erase alignment on nRF52840.
pub const NVMC_ERASE_ALIGNMENT_BYTES: usize = 4096;

/// Errors returned while constructing or translating an NVMC region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NvmcRegionError {
    /// A zero base would overlap the vector/application start and is rejected.
    BaseIsZero,
    /// The region base is not page aligned.
    BaseUnaligned,
    /// The region length is zero.
    ZeroLength,
    /// The region length is not page aligned.
    LengthUnaligned,
    /// The region exceeds the documented nRF52840 flash capacity.
    OutOfBounds,
    /// Checked arithmetic overflowed while validating the region or range.
    ArithmeticOverflow,
    /// A translated operation exceeds the selected region.
    RangeOutOfBounds,
}

/// A caller-selected, page-aligned subregion of nRF52840 internal flash.
///
/// The constructor intentionally accepts the region rather than selecting a
/// linker map, bootloader reservation, or journal offset. The caller must
/// supply values derived from its linked image and board policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NvmcRegion {
    base: usize,
    length: usize,
}

impl NvmcRegion {
    /// Validate a region against the board's documented internal flash size.
    pub const fn new(base: usize, length: usize) -> Result<Self, NvmcRegionError> {
        if base == 0 {
            return Err(NvmcRegionError::BaseIsZero);
        }
        if !base.is_multiple_of(NVMC_ERASE_ALIGNMENT_BYTES) {
            return Err(NvmcRegionError::BaseUnaligned);
        }
        if length == 0 {
            return Err(NvmcRegionError::ZeroLength);
        }
        if !length.is_multiple_of(NVMC_ERASE_ALIGNMENT_BYTES) {
            return Err(NvmcRegionError::LengthUnaligned);
        }
        let end = match base.checked_add(length) {
            Some(end) => end,
            None => return Err(NvmcRegionError::ArithmeticOverflow),
        };
        if end > super::FLASH_CAPACITY_BYTES {
            return Err(NvmcRegionError::OutOfBounds);
        }
        Ok(Self { base, length })
    }

    /// Return the region's capacity in bytes.
    pub const fn capacity_bytes(self) -> usize {
        self.length
    }

    /// Return the provider-neutral geometry exposed to the journal layer.
    pub const fn geometry(self) -> Geometry {
        Geometry {
            capacity: self.length,
            write_alignment: NVMC_WRITE_ALIGNMENT_BYTES,
            erase_alignment: NVMC_ERASE_ALIGNMENT_BYTES,
        }
    }

    /// Translate a region-relative operation to an absolute flash address.
    pub const fn translate(self, offset: usize, length: usize) -> Result<usize, NvmcRegionError> {
        let end = match offset.checked_add(length) {
            Some(end) => end,
            None => return Err(NvmcRegionError::ArithmeticOverflow),
        };
        if end > self.length {
            return Err(NvmcRegionError::RangeOutOfBounds);
        }
        match self.base.checked_add(offset) {
            Some(address) => Ok(address),
            None => Err(NvmcRegionError::ArithmeticOverflow),
        }
    }
}

/// Errors returned by the ARM-only NVMC journal backend.
#[cfg(target_arch = "arm")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NvmcJournalError {
    /// The requested operation is outside or misaligned for the selected region.
    Region(NvmcRegionError),
    /// The operation is not aligned to the nRF52840 NVMC write/erase unit.
    Unaligned,
    /// A re-entrant operation attempted to borrow the NVMC driver.
    Borrowed,
    /// The HAL reported an NVMC operation failure.
    Flash(embassy_nrf::nvmc::Error),
}

/// ARM-only adapter from Embassy's internal NVMC driver to `JournalBackend`.
///
/// The adapter does not select a flash region or claim power-loss atomicity;
/// callers still need the journal transaction/recovery policy and linked-image
/// reservation described by the E7 provider contract.
#[cfg(target_arch = "arm")]
pub struct NvmcJournalBackend<'d> {
    flash: RefCell<embassy_nrf::nvmc::Nvmc<'d>>,
    region: NvmcRegion,
}

#[cfg(target_arch = "arm")]
impl<'d> NvmcJournalBackend<'d> {
    /// Bind an already-created Embassy NVMC driver to a validated region.
    pub const fn new(flash: embassy_nrf::nvmc::Nvmc<'d>, region: NvmcRegion) -> Self {
        Self {
            flash: RefCell::new(flash),
            region,
        }
    }

    /// Return the selected region.
    pub const fn region(&self) -> NvmcRegion {
        self.region
    }
}

#[cfg(target_arch = "arm")]
impl<'d> JournalBackend for NvmcJournalBackend<'d> {
    type Error = NvmcJournalError;

    fn geometry(&self) -> Geometry {
        self.region.geometry()
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), Self::Error> {
        let address = self
            .region
            .translate(offset, out.len())
            .map_err(NvmcJournalError::Region)?;
        let mut flash = self
            .flash
            .try_borrow_mut()
            .map_err(|_| NvmcJournalError::Borrowed)?;
        embedded_storage::nor_flash::ReadNorFlash::read(&mut *flash, address as u32, out)
            .map_err(NvmcJournalError::Flash)
    }

    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Self::Error> {
        if !offset.is_multiple_of(NVMC_WRITE_ALIGNMENT_BYTES)
            || !bytes.len().is_multiple_of(NVMC_WRITE_ALIGNMENT_BYTES)
        {
            return Err(NvmcJournalError::Unaligned);
        }
        let address = self
            .region
            .translate(offset, bytes.len())
            .map_err(NvmcJournalError::Region)?;
        let mut flash = self
            .flash
            .try_borrow_mut()
            .map_err(|_| NvmcJournalError::Borrowed)?;
        embedded_storage::nor_flash::NorFlash::write(&mut *flash, address as u32, bytes)
            .map_err(NvmcJournalError::Flash)
    }

    fn erase(&mut self, offset: usize, length: usize) -> Result<(), Self::Error> {
        if !offset.is_multiple_of(NVMC_ERASE_ALIGNMENT_BYTES)
            || !length.is_multiple_of(NVMC_ERASE_ALIGNMENT_BYTES)
        {
            return Err(NvmcJournalError::Unaligned);
        }
        let address = self
            .region
            .translate(offset, length)
            .map_err(NvmcJournalError::Region)?;
        let end = address.checked_add(length).ok_or(NvmcJournalError::Region(
            NvmcRegionError::ArithmeticOverflow,
        ))?;
        let mut flash = self
            .flash
            .try_borrow_mut()
            .map_err(|_| NvmcJournalError::Borrowed)?;
        embedded_storage::nor_flash::NorFlash::erase(&mut *flash, address as u32, end as u32)
            .map_err(NvmcJournalError::Flash)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NvmcRegion, NvmcRegionError, NVMC_ERASE_ALIGNMENT_BYTES, NVMC_WRITE_ALIGNMENT_BYTES,
    };

    #[test]
    fn region_accepts_aligned_subregion_and_translates_bounded_ranges() {
        let region = NvmcRegion::new(0x20_000, 0x10_000).unwrap();
        assert_eq!(region.capacity_bytes(), 0x10_000);
        assert_eq!(
            region.geometry().write_alignment,
            NVMC_WRITE_ALIGNMENT_BYTES
        );
        assert_eq!(
            region.geometry().erase_alignment,
            NVMC_ERASE_ALIGNMENT_BYTES
        );
        assert_eq!(region.translate(0x100, 0x200).unwrap(), 0x20_100);
    }

    #[test]
    fn region_rejects_unaligned_or_out_of_bounds_ranges_without_panicking() {
        assert_eq!(NvmcRegion::new(0, 0x1000), Err(NvmcRegionError::BaseIsZero));
        assert_eq!(
            NvmcRegion::new(0x20_004, 0x10_000),
            Err(NvmcRegionError::BaseUnaligned)
        );
        assert_eq!(
            NvmcRegion::new(0x20_000, 0x1001),
            Err(NvmcRegionError::LengthUnaligned)
        );
        assert_eq!(
            NvmcRegion::new(crate::FLASH_CAPACITY_BYTES, 0x1000),
            Err(NvmcRegionError::OutOfBounds)
        );
        assert_eq!(
            NvmcRegion::new(usize::MAX - 0x0fff, 0x1000),
            Err(NvmcRegionError::ArithmeticOverflow)
        );

        let region = NvmcRegion::new(0x20_000, 0x10_000).unwrap();
        assert_eq!(
            region.translate(0x0ffff, 2),
            Err(NvmcRegionError::RangeOutOfBounds)
        );
    }
}
