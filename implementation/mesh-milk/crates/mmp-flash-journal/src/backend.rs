//! Provider-neutral flash I/O boundary for the journal scanner.
//!
//! This module describes the minimum aligned operations a board adapter must
//! provide. It intentionally does not promise power-loss atomicity: the
//! adapter and its board-specific acceptance tests must establish write order,
//! erase behavior, and recovery policy.

#![allow(missing_docs)]

/// Flash geometry required by a journal backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub capacity: usize,
    pub write_alignment: usize,
    pub erase_alignment: usize,
}

impl Geometry {
    /// Construct geometry, rejecting zero alignment values.
    pub const fn new(
        capacity: usize,
        write_alignment: usize,
        erase_alignment: usize,
    ) -> Result<Self, GeometryError> {
        if capacity == 0 || write_alignment == 0 || erase_alignment == 0 {
            return Err(GeometryError::ZeroValue);
        }
        Ok(Self {
            capacity,
            write_alignment,
            erase_alignment,
        })
    }
}

/// Invalid geometry configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    ZeroValue,
}

/// Errors a backend may return before or during an I/O operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendError {
    OutOfBounds,
    Unaligned,
    ProgramViolation,
    InjectedFailure,
}

/// Aligned, provider-neutral flash operations.
pub trait JournalBackend {
    type Error;

    fn geometry(&self) -> Geometry;
    fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), Self::Error>;
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Self::Error>;
    fn erase(&mut self, offset: usize, len: usize) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use super::*;
    use crate::{encode, encoded_len, scan, MAGIC};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FaultOp {
        Read,
        Write,
        Erase,
    }

    struct FaultNor<const CAPACITY: usize> {
        bytes: [u8; CAPACITY],
        geometry: Geometry,
        fail_next: Cell<Option<FaultOp>>,
    }

    impl<const CAPACITY: usize> FaultNor<CAPACITY> {
        fn new(write_alignment: usize, erase_alignment: usize) -> Self {
            Self {
                bytes: [0xff; CAPACITY],
                geometry: Geometry::new(CAPACITY, write_alignment, erase_alignment).unwrap(),
                fail_next: Cell::new(None),
            }
        }

        fn fail_next(&mut self, op: FaultOp) {
            self.fail_next.set(Some(op));
        }

        fn check_range(&self, offset: usize, len: usize) -> Result<(), BackendError> {
            let end = offset.checked_add(len).ok_or(BackendError::OutOfBounds)?;
            if end > self.geometry.capacity {
                Err(BackendError::OutOfBounds)
            } else {
                Ok(())
            }
        }

        fn take_fault(&self, op: FaultOp) -> Result<(), BackendError> {
            if self.fail_next.take() == Some(op) {
                Err(BackendError::InjectedFailure)
            } else {
                Ok(())
            }
        }
    }

    impl<const CAPACITY: usize> JournalBackend for FaultNor<CAPACITY> {
        type Error = BackendError;

        fn geometry(&self) -> Geometry {
            self.geometry
        }

        fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), Self::Error> {
            self.check_range(offset, out.len())?;
            self.take_fault(FaultOp::Read)?;
            out.copy_from_slice(&self.bytes[offset..offset + out.len()]);
            Ok(())
        }

        fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Self::Error> {
            self.check_range(offset, bytes.len())?;
            if !offset.is_multiple_of(self.geometry.write_alignment)
                || !bytes.len().is_multiple_of(self.geometry.write_alignment)
            {
                return Err(BackendError::Unaligned);
            }
            self.take_fault(FaultOp::Write)?;
            if bytes.iter().enumerate().any(|(index, value)| {
                value | self.bytes[offset + index] != self.bytes[offset + index]
            }) {
                return Err(BackendError::ProgramViolation);
            }
            for (index, value) in bytes.iter().enumerate() {
                self.bytes[offset + index] &= *value;
            }
            Ok(())
        }

        fn erase(&mut self, offset: usize, len: usize) -> Result<(), Self::Error> {
            self.check_range(offset, len)?;
            if !offset.is_multiple_of(self.geometry.erase_alignment)
                || !len.is_multiple_of(self.geometry.erase_alignment)
            {
                return Err(BackendError::Unaligned);
            }
            self.take_fault(FaultOp::Erase)?;
            self.bytes[offset..offset + len].fill(0xff);
            Ok(())
        }
    }

    #[test]
    fn geometry_and_alignment_are_enforced() {
        assert_eq!(Geometry::new(0, 4, 16), Err(GeometryError::ZeroValue));
        let mut nor = FaultNor::<64>::new(4, 16);
        assert_eq!(nor.write(1, &[0; 4]), Err(BackendError::Unaligned));
        assert_eq!(nor.write(0, &[0; 2]), Err(BackendError::Unaligned));
        assert_eq!(nor.erase(0, 8), Err(BackendError::Unaligned));
        assert_eq!(nor.read(60, &mut [0; 8]), Err(BackendError::OutOfBounds));
    }

    #[test]
    fn program_is_one_way_and_erase_restores_ff() {
        let mut nor = FaultNor::<64>::new(4, 16);
        nor.write(0, &[0x0f; 4]).unwrap();
        assert_eq!(
            nor.write(0, &[0xf0; 4]),
            Err(BackendError::ProgramViolation)
        );
        nor.erase(0, 16).unwrap();
        let mut out = [0; 4];
        nor.read(0, &mut out).unwrap();
        assert_eq!(out, [0xff; 4]);
    }

    #[test]
    fn injected_operations_fail_before_mutation() {
        let mut nor = FaultNor::<64>::new(4, 16);
        nor.fail_next(FaultOp::Write);
        assert_eq!(nor.write(0, &[0; 4]), Err(BackendError::InjectedFailure));
        let mut out = [0; 4];
        nor.read(0, &mut out).unwrap();
        assert_eq!(out, [0xff; 4]);
        nor.fail_next(FaultOp::Erase);
        assert_eq!(nor.erase(0, 16), Err(BackendError::InjectedFailure));
        nor.fail_next(FaultOp::Read);
        assert_eq!(nor.read(0, &mut out), Err(BackendError::InjectedFailure));
        nor.read(0, &mut out).unwrap();
        assert_eq!(out, [0xff; 4]);
    }

    #[test]
    fn scanner_keeps_previous_generation_after_incomplete_append() {
        let mut nor = FaultNor::<128>::new(4, 16);
        let mut first = [0; 128];
        let first_len = encoded_len(4).unwrap();
        encode(1, b"keep", &mut first).unwrap();
        nor.write(0, &first[..first_len]).unwrap();

        let second_len = encoded_len(6).unwrap();
        let mut second = [0; 128];
        encode(2, b"newer!", &mut second).unwrap();
        nor.write(first_len, &second[..8]).unwrap();

        let mut image = [0xff; 128];
        nor.read(0, &mut image).unwrap();
        let result = scan(&image);
        assert_eq!(result.selected.unwrap().generation, 1);
        assert_eq!(result.selected.unwrap().payload, b"keep");
        assert!(second_len > 8);
        assert_eq!(image[first_len..first_len + 4], MAGIC);
    }
}
