//! Allocation-free journal framing and recovery scanning for embedded providers.
//!
//! This crate deliberately stops at a byte-level recovery primitive. It does
//! not implement `BundleStore`, choose a board capacity, or claim that a
//! particular flash controller makes a write atomic. A provider must still
//! supply its own aligned read/write/erase and power-loss policy.

#![no_std]
#![warn(missing_docs)]

pub mod backend;

pub use backend::{BackendError, Geometry, GeometryError, JournalBackend};

use core::convert::TryFrom;

/// Four-byte format marker for this implementation-local journal record.
pub const MAGIC: [u8; 4] = *b"FJ01";
/// Four-byte marker written last by a provider after the record body is valid.
pub const COMMIT_MARKER: [u8; 4] = *b"CMT!";
/// Maximum payload accepted by the scanner and encoder.
///
/// This is a parser-safety bound, not a board storage quota or a wire limit.
pub const MAX_PAYLOAD_LEN: usize = 65_536;
const HEADER_LEN: usize = 4 + 8 + 4;
const CHECKSUM_LEN: usize = 32;
const TRAILER_LEN: usize = CHECKSUM_LEN + COMMIT_MARKER.len();
const MIN_RECORD_LEN: usize = HEADER_LEN + TRAILER_LEN;

/// A valid committed record borrowed from the caller's journal image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record<'a> {
    /// Monotonically increasing provider generation.
    pub generation: u64,
    /// Payload bytes, borrowed from the scanned image.
    pub payload: &'a [u8],
}

/// Result of scanning a journal image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanResult<'a> {
    /// Highest-generation committed record, if one is valid.
    pub selected: Option<Record<'a>>,
    /// Number of candidate magic markers encountered.
    pub candidates: u32,
    /// Number of records with valid checksum and commit marker.
    pub valid: u32,
    /// Number of candidates rejected before the scanner reached the end.
    pub rejected: u32,
}

/// Conservative result of classifying an attempted journal generation after
/// recovery scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryClassification {
    /// The scan selected the generation that the caller attempted to append.
    Committed {
        /// The generation proven visible by the scan.
        generation: u64,
    },
    /// The scan selected the caller's immediately previous generation, or the
    /// journal was empty before this was the first attempted append.
    NotCommitted,
    /// The scan cannot distinguish whether the attempted append became
    /// durable.
    Indeterminate,
}

/// In-memory guard for strictly increasing journal generations.
///
/// A floor of zero represents an empty journal; the first accepted generation
/// is therefore one. The guard is provider-neutral and allocation-free. It
/// does not persist the floor, provide hardware anti-rollback, or establish
/// that a snapshot is durable. A caller must validate the recovered snapshot
/// first, persist the floor using its provider's crash-safe policy, and recover
/// again after any indeterminate write before advancing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationFloor {
    known_floor: u64,
}

/// Errors returned when advancing a [`GenerationFloor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationFloorError {
    /// Generation zero is reserved for the empty/uninitialized state.
    Zero,
    /// The attempted generation is not strictly greater than the known floor.
    NotMonotonic {
        /// Highest generation accepted by this in-memory guard.
        known_floor: u64,
        /// Generation supplied by the caller.
        attempted: u64,
    },
    /// The known floor is already [`u64::MAX`], so no next generation exists.
    Overflow,
}

impl GenerationFloor {
    /// Create a guard from a validated recovered generation.
    ///
    /// Pass zero for an empty journal. This constructor does not validate or
    /// persist the supplied value; the provider must establish those facts.
    pub const fn new(known_floor: u64) -> Self {
        Self { known_floor }
    }

    /// Return the highest generation accepted by this guard.
    pub const fn known_floor(self) -> u64 {
        self.known_floor
    }

    /// Accept a caller-supplied generation if it is strictly increasing.
    ///
    /// The guard is updated only after all validation succeeds. A rejected
    /// attempt leaves the known floor unchanged.
    pub fn advance(&mut self, attempted: u64) -> Result<(), GenerationFloorError> {
        if attempted == 0 {
            return Err(GenerationFloorError::Zero);
        }
        if attempted <= self.known_floor {
            return Err(GenerationFloorError::NotMonotonic {
                known_floor: self.known_floor,
                attempted,
            });
        }
        self.known_floor = attempted;
        Ok(())
    }

    /// Reserve and return the next generation.
    ///
    /// On overflow the guard remains at [`u64::MAX`].
    pub fn reserve_next(&mut self) -> Result<u64, GenerationFloorError> {
        let next = self
            .known_floor
            .checked_add(1)
            .ok_or(GenerationFloorError::Overflow)?;
        self.advance(next)?;
        Ok(next)
    }
}

/// In-memory lifecycle for one provider journal transaction.
///
/// The state machine reserves a generation without publishing it, quarantines
/// the transaction after an unknown write result, and leaves quarantine only
/// after a conservative recovery classification. It does not persist the
/// floor, validate snapshot contents, or establish flash atomicity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalTransaction {
    floor: GenerationFloor,
    state: JournalTransactionState,
}

/// Observable lifecycle state of [`JournalTransaction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTransactionState {
    /// No generation is currently being written.
    Ready,
    /// A generation has been reserved and is awaiting write/recovery.
    InFlight {
        /// Reserved generation.
        generation: u64,
    },
    /// A write outcome is unknown and no new reservation is permitted.
    Quarantined {
        /// Generation whose outcome requires recovery.
        generation: u64,
    },
}

/// Errors returned by [`JournalTransaction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTransactionError {
    /// A generation is already reserved or quarantined.
    AlreadyInFlight {
        /// Generation currently awaiting resolution.
        generation: u64,
    },
    /// The transaction is quarantined until recovery resolves its outcome.
    Quarantined {
        /// Generation requiring recovery.
        generation: u64,
    },
    /// No generation is awaiting recovery.
    NoAttempt,
    /// The caller supplied a generation other than the active attempt.
    MismatchedAttempt {
        /// Active generation.
        expected: u64,
        /// Caller-supplied generation.
        supplied: u64,
    },
    /// The local monotonic floor cannot produce or accept the transition.
    Generation(GenerationFloorError),
}

impl JournalTransaction {
    /// Create a ready transaction from a caller-validated in-memory floor.
    pub const fn new(floor: GenerationFloor) -> Self {
        Self {
            floor,
            state: JournalTransactionState::Ready,
        }
    }

    /// Return the currently accepted local floor.
    pub const fn known_floor(self) -> u64 {
        self.floor.known_floor()
    }

    /// Return the lifecycle state without exposing mutable internals.
    pub const fn state(self) -> JournalTransactionState {
        self.state
    }

    /// Reserve the next generation while leaving the floor unchanged until a
    /// committed recovery result is proven.
    pub fn reserve_generation(&mut self) -> Result<u64, JournalTransactionError> {
        match self.state {
            JournalTransactionState::Ready => {
                let mut candidate = self.floor;
                let generation = candidate
                    .reserve_next()
                    .map_err(JournalTransactionError::Generation)?;
                self.state = JournalTransactionState::InFlight { generation };
                Ok(generation)
            }
            JournalTransactionState::InFlight { generation } => {
                Err(JournalTransactionError::AlreadyInFlight { generation })
            }
            JournalTransactionState::Quarantined { generation } => {
                Err(JournalTransactionError::Quarantined { generation })
            }
        }
    }

    /// Mark the active write outcome as unknown and quarantine the provider.
    pub fn mark_indeterminate(&mut self, supplied: u64) -> Result<(), JournalTransactionError> {
        let expected = match self.state {
            JournalTransactionState::InFlight { generation }
            | JournalTransactionState::Quarantined { generation } => generation,
            JournalTransactionState::Ready => return Err(JournalTransactionError::NoAttempt),
        };
        if expected != supplied {
            return Err(JournalTransactionError::MismatchedAttempt { expected, supplied });
        }
        self.state = JournalTransactionState::Quarantined {
            generation: expected,
        };
        Ok(())
    }

    /// Resolve the active write from a post-write scan.
    ///
    /// A `NotCommitted` result releases the attempt without advancing the
    /// floor. A `Committed` result advances it after the frame is proven
    /// visible. An `Indeterminate` result keeps the transaction quarantined.
    /// The caller must separately validate the recovered snapshot before
    /// treating a committed frame as usable state.
    pub fn resolve_scan(
        &mut self,
        supplied: u64,
        scan_result: ScanResult<'_>,
    ) -> Result<RecoveryClassification, JournalTransactionError> {
        let expected = match self.state {
            JournalTransactionState::InFlight { generation }
            | JournalTransactionState::Quarantined { generation } => generation,
            JournalTransactionState::Ready => return Err(JournalTransactionError::NoAttempt),
        };
        if expected != supplied {
            return Err(JournalTransactionError::MismatchedAttempt { expected, supplied });
        }
        let previous = (self.floor.known_floor() != 0).then_some(self.floor.known_floor());
        let classification = classify_recovery(scan_result, previous, supplied);
        match classification {
            RecoveryClassification::Committed { generation } => {
                self.floor
                    .advance(generation)
                    .map_err(JournalTransactionError::Generation)?;
                self.state = JournalTransactionState::Ready;
            }
            RecoveryClassification::NotCommitted => {
                self.state = JournalTransactionState::Ready;
            }
            RecoveryClassification::Indeterminate => {
                self.state = JournalTransactionState::Quarantined {
                    generation: expected,
                };
            }
        }
        Ok(classification)
    }
}

/// Provider-facing name for [`JournalTransaction`].
///
/// This alias names the same in-memory state machine; it does not add durable
/// persistence, board policy, or flash-I/O semantics.
pub type ProviderTransaction = JournalTransaction;

/// Classify an attempted append from a post-write journal scan.
///
/// The caller must provide the generation selected immediately before the
/// append (if any) and the generation it attempted to write. The scanner only
/// reports the highest valid record, so this helper deliberately does not infer
/// anything from rejected candidates, lower valid generations, or the number
/// of records. It applies these conservative rules in order:
///
/// 1. an attempted generation must be strictly greater than the supplied
///    `previous_generation` (when present), otherwise the result is
///    [`Indeterminate`];
/// 2. selected generation equal to `attempted_generation` is [`Committed`];
/// 3. selected generation equal to `previous_generation`, or no selected
///    generation when there was no previous generation, is [`NotCommitted`];
/// 4. every other state is [`Indeterminate`].
///
/// In particular, an empty scan with a known previous generation is
/// indeterminate: the previous record may have been lost or hidden by a
/// provider-level failure, and this byte-level crate cannot resolve that
/// possibility. The helper applies only the caller-supplied in-memory
/// monotonic precondition; it does not persist or authenticate a generation
/// floor and does not claim flash atomicity.
pub fn classify_recovery(
    scan_result: ScanResult<'_>,
    previous_generation: Option<u64>,
    attempted_generation: u64,
) -> RecoveryClassification {
    if previous_generation.is_some_and(|previous| attempted_generation <= previous) {
        return RecoveryClassification::Indeterminate;
    }
    match scan_result.selected.map(|record| record.generation) {
        Some(generation) if generation == attempted_generation => {
            RecoveryClassification::Committed { generation }
        }
        Some(generation) if previous_generation == Some(generation) => {
            RecoveryClassification::NotCommitted
        }
        None if previous_generation.is_none() => RecoveryClassification::NotCommitted,
        _ => RecoveryClassification::Indeterminate,
    }
}

/// Errors returned when encoding a record into caller-provided storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The payload exceeds [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge,
    /// The output slice cannot hold the complete framed record.
    OutputTooSmall,
}

/// Geometry validation failures for [`write_record`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteGeometryError {
    /// The backend returned a zero write alignment.
    ZeroWriteAlignment,
    /// The absolute record offset is not aligned to the backend write unit.
    UnalignedOffset,
    /// The record length could not be rounded up to a write unit.
    LengthOverflow,
    /// The rounded record does not fit in the backend geometry.
    OutOfBounds,
}

/// Errors returned by [`write_record`].
///
/// A [`JournalWriteError::CommitWrite`] means that the final write unit may
/// have reached the flash device before the backend reported an error. Its
/// outcome is therefore indeterminate: the caller must recover by scanning
/// the journal before retrying, and must not treat it as an ordinary capacity
/// rejection or abort. The [`JournalWriteError::BodyWrite`] variant can also
/// leave a partial body on a backend with indeterminate write semantics, but
/// it cannot have requested the commit-marker unit yet.
#[derive(Debug, PartialEq, Eq)]
pub enum JournalWriteError<E> {
    /// Encoding failed, including an output buffer shorter than the encoded
    /// (unpadded) record.
    Encode(EncodeError),
    /// The scratch buffer held the encoded record but not its required erased
    /// alignment padding.
    ScratchTooSmall {
        /// Number of bytes required for the complete aligned write image.
        required: usize,
    },
    /// The backend geometry or absolute record offset is not usable.
    Geometry(WriteGeometryError),
    /// A complete aligned prefix write failed before the final unit.
    BodyWrite(E),
    /// The final aligned unit containing the commit marker failed.
    ///
    /// This is an indeterminate outcome and requires recovery scanning.
    CommitWrite(E),
}

/// Return the encoded length for a payload, if it is representable.
pub const fn encoded_len(payload_len: usize) -> Option<usize> {
    if payload_len > MAX_PAYLOAD_LEN {
        return None;
    }
    let Some(with_header) = HEADER_LEN.checked_add(payload_len) else {
        return None;
    };
    with_header.checked_add(TRAILER_LEN)
}

/// Encode one complete record into caller-owned memory.
///
/// The function writes the commit marker into the returned image, but this is
/// only a complete byte representation. A flash provider must arrange its
/// physical write order so the marker is committed last and must report an
/// unknown outcome conservatively to its caller.
pub fn encode<'a>(
    generation: u64,
    payload: &[u8],
    out: &'a mut [u8],
) -> Result<&'a [u8], EncodeError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(EncodeError::PayloadTooLarge);
    }
    let length = encoded_len(payload.len()).ok_or(EncodeError::PayloadTooLarge)?;
    if out.len() < length {
        return Err(EncodeError::OutputTooSmall);
    }

    out[..4].copy_from_slice(&MAGIC);
    out[4..12].copy_from_slice(&generation.to_le_bytes());
    let payload_len = u32::try_from(payload.len()).map_err(|_| EncodeError::PayloadTooLarge)?;
    out[12..16].copy_from_slice(&payload_len.to_le_bytes());
    out[16..16 + payload.len()].copy_from_slice(payload);

    let checksum_start = 16 + payload.len();
    let checksum = checksum(&out[..checksum_start]);
    out[checksum_start..checksum_start + CHECKSUM_LEN].copy_from_slice(&checksum);
    out[checksum_start + CHECKSUM_LEN..length].copy_from_slice(&COMMIT_MARKER);
    Ok(&out[..length])
}

/// Encode and append one record using aligned backend writes.
///
/// `record_offset` is an absolute offset in the backend's address space. The
/// caller supplies `scratch`, which must hold the encoded record followed by
/// erased (`0xff`) padding through the next write-unit boundary. All aligned
/// body bytes before the marker-containing region are written in one prefix
/// call (when non-empty), followed by exactly one final aligned call
/// containing the commit marker. If the marker straddles a physical write-unit
/// boundary, that final call spans the minimum number of complete units needed
/// to contain it; it is still one backend call and the marker is never
/// requested in an earlier call.
///
/// This helper enforces ordering at the backend-call boundary only. It does
/// not claim that a backend write is power-loss atomic, nor that a backend
/// error is fail-before-mutation. In particular, [`JournalWriteError::CommitWrite`]
/// is explicitly indeterminate and the caller must scan the journal to resolve
/// it before retrying or reusing any generation.
pub fn write_record<B: JournalBackend>(
    backend: &mut B,
    record_offset: usize,
    generation: u64,
    payload: &[u8],
    scratch: &mut [u8],
) -> Result<usize, JournalWriteError<B::Error>> {
    let geometry = backend.geometry();
    let alignment = geometry.write_alignment;
    if alignment == 0 {
        return Err(JournalWriteError::Geometry(
            WriteGeometryError::ZeroWriteAlignment,
        ));
    }
    if !record_offset.is_multiple_of(alignment) {
        return Err(JournalWriteError::Geometry(
            WriteGeometryError::UnalignedOffset,
        ));
    }

    let encoded_length = encoded_len(payload.len())
        .ok_or(JournalWriteError::Encode(EncodeError::PayloadTooLarge))?;
    let padded_length = align_up(encoded_length, alignment).ok_or(JournalWriteError::Geometry(
        WriteGeometryError::LengthOverflow,
    ))?;
    let end = record_offset
        .checked_add(padded_length)
        .ok_or(JournalWriteError::Geometry(
            WriteGeometryError::LengthOverflow,
        ))?;
    if end > geometry.capacity {
        return Err(JournalWriteError::Geometry(WriteGeometryError::OutOfBounds));
    }

    let encoded = encode(generation, payload, scratch).map_err(JournalWriteError::Encode)?;
    debug_assert_eq!(encoded.len(), encoded_length);
    if scratch.len() < padded_length {
        return Err(JournalWriteError::ScratchTooSmall {
            required: padded_length,
        });
    }
    scratch[encoded_length..padded_length].fill(0xff);

    // Start the final aligned span at the unit containing the first marker
    // byte. This is deliberately not always `padded_length - alignment`: for
    // example, a four-byte marker can straddle an eight-byte boundary. The
    // span then covers the complete physical units through the padded end,
    // keeping every marker byte out of the prefix call.
    let marker_offset = encoded_length - COMMIT_MARKER.len();
    let prefix_length = marker_offset - marker_offset % alignment;
    if prefix_length != 0 {
        backend
            .write(record_offset, &scratch[..prefix_length])
            .map_err(JournalWriteError::BodyWrite)?;
    }
    backend
        .write(
            record_offset + prefix_length,
            &scratch[prefix_length..padded_length],
        )
        .map_err(JournalWriteError::CommitWrite)?;
    Ok(padded_length)
}

const fn align_up(value: usize, alignment: usize) -> Option<usize> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

/// Scan an implementation-local journal image without allocating.
///
/// Invalid checksums, invalid commit markers, and impossible candidate
/// lengths are skipped. A truncated candidate is treated as an incomplete
/// append and scanning continues byte-by-byte, so a malformed slot containing
/// a nested marker cannot mask a later complete record. If multiple valid
/// records share a generation, the first valid record wins deterministically.
pub fn scan(image: &[u8]) -> ScanResult<'_> {
    let mut cursor = 0usize;
    let mut result = ScanResult {
        selected: None,
        candidates: 0,
        valid: 0,
        rejected: 0,
    };

    while cursor.saturating_add(4) <= image.len() {
        if image[cursor..cursor + 4] != MAGIC {
            cursor += 1;
            continue;
        }
        result.candidates = result.candidates.saturating_add(1);
        if image.len() - cursor < MIN_RECORD_LEN {
            break;
        }

        let generation = u64::from_le_bytes(image[cursor + 4..cursor + 12].try_into().unwrap());
        let payload_len =
            u32::from_le_bytes(image[cursor + 12..cursor + 16].try_into().unwrap()) as usize;
        let Some(record_len) = encoded_len(payload_len) else {
            result.rejected = result.rejected.saturating_add(1);
            cursor += 1;
            continue;
        };
        if record_len > image.len() - cursor {
            // A final partial append is normally the end of the log, but a
            // malformed earlier record may contain a nested MAGIC marker.
            // Keep looking so that such a marker cannot mask a later complete
            // record placed by a provider that skips a damaged append slot.
            cursor += 1;
            continue;
        }

        let payload_start = cursor + HEADER_LEN;
        let payload_end = payload_start + payload_len;
        let checksum_end = payload_end + CHECKSUM_LEN;
        let expected = checksum(&image[cursor..payload_end]);
        if image[payload_end..checksum_end] != expected
            || image[checksum_end..checksum_end + COMMIT_MARKER.len()] != COMMIT_MARKER
        {
            result.rejected = result.rejected.saturating_add(1);
            cursor += 1;
            continue;
        }

        result.valid = result.valid.saturating_add(1);
        let record = Record {
            generation,
            payload: &image[payload_start..payload_end],
        };
        let replace = result
            .selected
            .map(|current| generation > current.generation)
            .unwrap_or(true);
        if replace {
            result.selected = Some(record);
        }
        cursor += record_len;
    }

    result
}

fn checksum(body: &[u8]) -> [u8; CHECKSUM_LEN] {
    let mut hasher = blake3::Hasher::new_derive_key("mmp flash journal v1");
    hasher.update(body);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct WriteCall {
        offset: usize,
        len: usize,
    }

    struct TraceBackend<const CAPACITY: usize> {
        bytes: [u8; CAPACITY],
        geometry: Geometry,
        calls: [Option<WriteCall>; 8],
        call_count: usize,
        fail_at: Option<usize>,
    }

    impl<const CAPACITY: usize> TraceBackend<CAPACITY> {
        fn new(write_alignment: usize) -> Self {
            Self {
                bytes: [0xff; CAPACITY],
                geometry: Geometry::new(CAPACITY, write_alignment, 1).unwrap(),
                calls: [None; 8],
                call_count: 0,
                fail_at: None,
            }
        }

        fn check_range(&self, offset: usize, len: usize) -> Result<(), BackendError> {
            let end = offset.checked_add(len).ok_or(BackendError::OutOfBounds)?;
            if end > self.geometry.capacity {
                Err(BackendError::OutOfBounds)
            } else {
                Ok(())
            }
        }
    }

    impl<const CAPACITY: usize> JournalBackend for TraceBackend<CAPACITY> {
        type Error = BackendError;

        fn geometry(&self) -> Geometry {
            self.geometry
        }

        fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), Self::Error> {
            self.check_range(offset, out.len())?;
            out.copy_from_slice(&self.bytes[offset..offset + out.len()]);
            Ok(())
        }

        fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Self::Error> {
            self.check_range(offset, bytes.len())?;
            if self.geometry.write_alignment == 0
                || !offset.is_multiple_of(self.geometry.write_alignment)
                || !bytes.len().is_multiple_of(self.geometry.write_alignment)
            {
                return Err(BackendError::Unaligned);
            }
            let call = WriteCall {
                offset,
                len: bytes.len(),
            };
            if let Some(slot) = self.calls.get_mut(self.call_count) {
                *slot = Some(call);
            }
            let index = self.call_count;
            self.call_count = self.call_count.saturating_add(1);
            if self.fail_at == Some(index) {
                return Err(BackendError::InjectedFailure);
            }
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
            self.bytes[offset..offset + len].fill(0xff);
            Ok(())
        }
    }

    fn append(image: &mut [u8], used: &mut usize, generation: u64, payload: &[u8]) -> usize {
        let mut record = [0u8; 16 + 64 + 32 + 4];
        let encoded = encode(generation, payload, &mut record).unwrap();
        image[*used..*used + encoded.len()].copy_from_slice(encoded);
        *used += encoded.len();
        encoded.len()
    }

    #[test]
    fn selects_highest_generation_and_borrows_payload() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 2, b"new");
        append(&mut image, &mut used, 1, b"old");
        append(&mut image, &mut used, 3, b"latest");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().generation, 3);
        assert_eq!(result.selected.unwrap().payload, b"latest");
        assert_eq!(result.valid, 3);
    }

    #[test]
    fn truncated_tail_preserves_previous_generation() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 4, b"complete");
        image[used..used + 4].copy_from_slice(&MAGIC);
        image[used + 4..used + 12].fill(0xaa);
        let result = scan(&image[..used + 12]);
        assert_eq!(result.selected.unwrap().generation, 4);
        assert_eq!(result.valid, 1);
    }

    #[test]
    fn bad_checksum_and_marker_are_rejected() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 1, b"bad");
        image[20] ^= 0x80;
        let mut second = [0u8; 128];
        let second_len = encoded_len(b"also bad".len()).unwrap();
        encode(2, b"also bad", &mut second).unwrap();
        second[second_len - 1] ^= 1;
        image[used..used + second_len].copy_from_slice(&second[..second_len]);
        used += second_len;
        let result = scan(&image[..used]);
        assert_eq!(result.selected, None);
        assert_eq!(result.valid, 0);
        assert!(result.rejected >= 2);
    }

    #[test]
    fn malformed_higher_generation_does_not_replace_previous_record() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 4, b"durable");

        let malformed_start = used;
        let malformed_len = encoded_len(b"partial".len()).unwrap();
        encode(
            9,
            b"partial",
            &mut image[malformed_start..malformed_start + malformed_len],
        )
        .unwrap();
        image[malformed_start + malformed_len - COMMIT_MARKER.len()] ^= 1;
        used += malformed_len;

        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().generation, 4);
        assert_eq!(result.selected.unwrap().payload, b"durable");
        assert_eq!(result.valid, 1);
        assert_eq!(result.rejected, 1);
    }

    #[test]
    fn nested_truncated_marker_does_not_mask_later_generation() {
        let mut image = [0u8; 512];
        let mut first = [0u8; 128];
        let first_payload = [0u8; 64];
        let first_len = encoded_len(first_payload.len()).unwrap();
        encode(1, &first_payload, &mut first).unwrap();

        // Corrupt the first record and place a plausible but incomplete
        // nested candidate in its payload. The scanner must still recover
        // the complete record appended after this damaged slot.
        let nested = HEADER_LEN + 4;
        first[nested..nested + MAGIC.len()].copy_from_slice(&MAGIC);
        first[nested + 4..nested + 12].copy_from_slice(&99u64.to_le_bytes());
        first[nested + 12..nested + 16].copy_from_slice(&(MAX_PAYLOAD_LEN as u32).to_le_bytes());
        first[first_len - 1] ^= 1;
        image[..first_len].copy_from_slice(&first[..first_len]);

        let second_len = encoded_len(b"durable".len()).unwrap();
        encode(2, b"durable", &mut image[first_len..first_len + second_len]).unwrap();

        let result = scan(&image[..first_len + second_len]);
        assert_eq!(result.selected.unwrap().generation, 2);
        assert_eq!(result.selected.unwrap().payload, b"durable");
        assert_eq!(result.valid, 1);
    }

    #[test]
    fn impossible_length_is_skipped_and_later_record_is_recovered() {
        let mut image = [0u8; 512];
        let mut used = 16;
        image[..4].copy_from_slice(&MAGIC);
        image[4..12].copy_from_slice(&99u64.to_le_bytes());
        image[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        append(&mut image, &mut used, 7, b"usable");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().generation, 7);
        assert_eq!(result.valid, 1);
        assert!(result.rejected >= 1);
    }

    #[test]
    fn duplicate_generation_keeps_first_valid_record() {
        let mut image = [0u8; 256];
        let mut used = 0;
        append(&mut image, &mut used, 8, b"first");
        append(&mut image, &mut used, 8, b"second");
        let result = scan(&image[..used]);
        assert_eq!(result.selected.unwrap().payload, b"first");
    }

    #[test]
    fn recovery_classifies_attempted_generation_as_committed() {
        let result = ScanResult {
            selected: Some(Record {
                generation: 8,
                payload: b"new",
            }),
            candidates: 1,
            valid: 1,
            rejected: 0,
        };

        assert_eq!(
            classify_recovery(result, Some(7), 8),
            RecoveryClassification::Committed { generation: 8 }
        );
    }

    #[test]
    fn recovery_classifies_previous_generation_as_not_committed() {
        let result = ScanResult {
            selected: Some(Record {
                generation: 7,
                payload: b"old",
            }),
            candidates: 1,
            valid: 1,
            rejected: 0,
        };

        assert_eq!(
            classify_recovery(result, Some(7), 8),
            RecoveryClassification::NotCommitted
        );
    }

    #[test]
    fn recovery_classifies_empty_journal_without_previous_generation_as_not_committed() {
        let result = ScanResult {
            selected: None,
            candidates: 0,
            valid: 0,
            rejected: 0,
        };

        assert_eq!(
            classify_recovery(result, None, 1),
            RecoveryClassification::NotCommitted
        );
    }

    #[test]
    fn recovery_classifies_missing_or_unexpected_generations_as_indeterminate() {
        let missing = ScanResult {
            selected: None,
            candidates: 1,
            valid: 0,
            rejected: 1,
        };
        assert_eq!(
            classify_recovery(missing, Some(7), 8),
            RecoveryClassification::Indeterminate
        );

        let unexpected = ScanResult {
            selected: Some(Record {
                generation: 9,
                payload: b"future",
            }),
            candidates: 1,
            valid: 1,
            rejected: 0,
        };
        assert_eq!(
            classify_recovery(unexpected, Some(7), 8),
            RecoveryClassification::Indeterminate
        );
    }

    #[test]
    fn recovery_rejects_reused_or_rolled_back_attempt_generation() {
        let result = ScanResult {
            selected: Some(Record {
                generation: 7,
                payload: b"old",
            }),
            candidates: 2,
            valid: 2,
            rejected: 0,
        };

        assert_eq!(
            classify_recovery(result, Some(7), 7),
            RecoveryClassification::Indeterminate
        );
        assert_eq!(
            classify_recovery(result, Some(7), 6),
            RecoveryClassification::Indeterminate
        );
    }

    #[test]
    fn generation_floor_accepts_first_generation() {
        let mut floor = GenerationFloor::new(0);
        assert_eq!(floor.reserve_next().unwrap(), 1);
        assert_eq!(floor.known_floor(), 1);
    }

    #[test]
    fn generation_floor_accepts_strict_increment() {
        let mut floor = GenerationFloor::new(7);
        floor.advance(8).unwrap();
        assert_eq!(floor.known_floor(), 8);
        assert_eq!(floor.reserve_next().unwrap(), 9);
        assert_eq!(floor.known_floor(), 9);
    }

    #[test]
    fn generation_floor_rejects_zero_equal_and_rollback_without_mutating() {
        let mut floor = GenerationFloor::new(4);
        for attempted in [0, 4, 3] {
            let before = floor.known_floor();
            let error = floor.advance(attempted).unwrap_err();
            assert_eq!(floor.known_floor(), before);
            match attempted {
                0 => assert_eq!(error, GenerationFloorError::Zero),
                _ => assert_eq!(
                    error,
                    GenerationFloorError::NotMonotonic {
                        known_floor: 4,
                        attempted,
                    }
                ),
            }
        }
    }

    #[test]
    fn generation_floor_reports_next_overflow_without_mutating() {
        let mut floor = GenerationFloor::new(u64::MAX);
        assert_eq!(floor.reserve_next(), Err(GenerationFloorError::Overflow));
        assert_eq!(floor.known_floor(), u64::MAX);
    }

    #[test]
    fn journal_transaction_reserves_without_advancing_until_commit() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(7));

        assert_eq!(transaction.reserve_generation().unwrap(), 8);
        assert_eq!(transaction.known_floor(), 7);
        assert_eq!(
            transaction.state(),
            JournalTransactionState::InFlight { generation: 8 }
        );
        assert_eq!(
            transaction.reserve_generation(),
            Err(JournalTransactionError::AlreadyInFlight { generation: 8 })
        );
    }

    #[test]
    fn journal_transaction_committed_recovery_advances_floor_and_returns_ready() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(7));
        let attempted = transaction.reserve_generation().unwrap();
        let scan_result = ScanResult {
            selected: Some(Record {
                generation: attempted,
                payload: b"snapshot",
            }),
            candidates: 1,
            valid: 1,
            rejected: 0,
        };

        assert_eq!(
            transaction.resolve_scan(attempted, scan_result),
            Ok(RecoveryClassification::Committed { generation: 8 })
        );
        assert_eq!(transaction.known_floor(), 8);
        assert_eq!(transaction.state(), JournalTransactionState::Ready);
        assert_eq!(transaction.reserve_generation().unwrap(), 9);
    }

    #[test]
    fn journal_transaction_not_committed_clears_attempt_without_advancing_floor() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(7));
        let attempted = transaction.reserve_generation().unwrap();
        let scan_result = ScanResult {
            selected: Some(Record {
                generation: 7,
                payload: b"previous",
            }),
            candidates: 1,
            valid: 1,
            rejected: 0,
        };

        assert_eq!(
            transaction.resolve_scan(attempted, scan_result),
            Ok(RecoveryClassification::NotCommitted)
        );
        assert_eq!(transaction.known_floor(), 7);
        assert_eq!(transaction.state(), JournalTransactionState::Ready);
        assert_eq!(transaction.reserve_generation().unwrap(), 8);
    }

    #[test]
    fn journal_transaction_indeterminate_write_quarantines_and_blocks_reservation() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(7));
        let attempted = transaction.reserve_generation().unwrap();

        transaction.mark_indeterminate(attempted).unwrap();
        assert_eq!(
            transaction.state(),
            JournalTransactionState::Quarantined { generation: 8 }
        );
        assert_eq!(
            transaction.reserve_generation(),
            Err(JournalTransactionError::Quarantined { generation: 8 })
        );

        let unresolved = ScanResult {
            selected: None,
            candidates: 1,
            valid: 0,
            rejected: 1,
        };
        assert_eq!(
            transaction.resolve_scan(attempted, unresolved),
            Ok(RecoveryClassification::Indeterminate)
        );
        assert_eq!(
            transaction.state(),
            JournalTransactionState::Quarantined { generation: 8 }
        );
    }

    #[test]
    fn journal_transaction_recovery_can_release_quarantine_only_when_proven() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(0));
        let attempted = transaction.reserve_generation().unwrap();
        transaction.mark_indeterminate(attempted).unwrap();

        let not_committed = ScanResult {
            selected: None,
            candidates: 0,
            valid: 0,
            rejected: 0,
        };
        assert_eq!(
            transaction.resolve_scan(attempted, not_committed),
            Ok(RecoveryClassification::NotCommitted)
        );
        assert_eq!(transaction.state(), JournalTransactionState::Ready);
        assert_eq!(transaction.known_floor(), 0);
    }

    #[test]
    fn journal_transaction_rejects_mismatched_attempts_without_mutating_state() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(7));
        let attempted = transaction.reserve_generation().unwrap();
        assert_eq!(
            transaction.mark_indeterminate(attempted + 1),
            Err(JournalTransactionError::MismatchedAttempt {
                expected: attempted,
                supplied: attempted + 1,
            })
        );
        assert_eq!(
            transaction.resolve_scan(
                attempted + 1,
                ScanResult {
                    selected: None,
                    candidates: 0,
                    valid: 0,
                    rejected: 0,
                }
            ),
            Err(JournalTransactionError::MismatchedAttempt {
                expected: attempted,
                supplied: attempted + 1,
            })
        );
        assert_eq!(
            transaction.state(),
            JournalTransactionState::InFlight {
                generation: attempted
            }
        );
    }

    #[test]
    fn journal_transaction_rejects_resolution_without_attempt() {
        let mut transaction = JournalTransaction::new(GenerationFloor::new(0));
        assert_eq!(
            transaction.resolve_scan(
                1,
                ScanResult {
                    selected: None,
                    candidates: 0,
                    valid: 0,
                    rejected: 0,
                }
            ),
            Err(JournalTransactionError::NoAttempt)
        );
    }

    #[test]
    fn encode_rejects_small_output_and_large_payload() {
        let mut small = [0u8; 8];
        assert_eq!(
            encode(1, b"payload", &mut small),
            Err(EncodeError::OutputTooSmall)
        );
        let mut out = [0u8; 128];
        let too_large = [0u8; MAX_PAYLOAD_LEN + 1];
        assert_eq!(
            encode(1, &too_large, &mut out),
            Err(EncodeError::PayloadTooLarge)
        );
    }

    #[test]
    fn write_record_keeps_marker_in_one_final_aligned_call() {
        // 52-byte framing means payloads 3, 4, and 5 exercise immediately
        // below, exactly at, and immediately above an eight-byte boundary.
        for (generation, payload) in [(1, b"abc".as_slice()), (2, b"abcd"), (3, b"abcde")] {
            let mut backend = TraceBackend::<128>::new(8);
            let mut scratch = [0x55; 96];
            let encoded_length = encoded_len(payload.len()).unwrap();
            let padded_length = align_up(encoded_length, 8).unwrap();
            let prefix_length = (encoded_length - COMMIT_MARKER.len()) / 8 * 8;
            let written = write_record(&mut backend, 8, generation, payload, &mut scratch).unwrap();

            assert_eq!(written, padded_length);
            assert_eq!(backend.call_count, 2);
            assert_eq!(
                backend.calls[0],
                Some(WriteCall {
                    offset: 8,
                    len: prefix_length,
                })
            );
            assert_eq!(
                backend.calls[1],
                Some(WriteCall {
                    offset: 8 + prefix_length,
                    len: padded_length - prefix_length,
                })
            );
            assert!(8 + prefix_length <= 8 + encoded_length - COMMIT_MARKER.len());
            assert_eq!(
                &backend.bytes[8 + encoded_length - COMMIT_MARKER.len()..8 + encoded_length],
                &COMMIT_MARKER
            );
            assert!(backend.bytes[8 + encoded_length..8 + padded_length]
                .iter()
                .all(|byte| *byte == 0xff));

            let result = scan(&backend.bytes);
            assert_eq!(result.selected.unwrap().generation, generation);
            assert_eq!(result.selected.unwrap().payload, payload);
        }

        // A byte-programmable backend is valid too: the final backend call
        // simply spans the four marker bytes as four aligned one-byte units.
        let mut byte_backend = TraceBackend::<128>::new(1);
        let mut byte_scratch = [0; 96];
        let written =
            write_record(&mut byte_backend, 3, 4, b"byte-unit", &mut byte_scratch).unwrap();
        assert_eq!(byte_backend.call_count, 2);
        assert_eq!(byte_backend.calls[1].unwrap().len, COMMIT_MARKER.len());
        assert_eq!(scan(&byte_backend.bytes).selected.unwrap().generation, 4);
        assert_eq!(written, encoded_len(9).unwrap());
    }

    #[test]
    fn write_record_rejects_unaligned_offset_and_short_scratch() {
        let mut backend = TraceBackend::<128>::new(8);
        let mut scratch = [0; 96];
        assert_eq!(
            write_record(&mut backend, 1, 1, b"payload", &mut scratch),
            Err(JournalWriteError::Geometry(
                WriteGeometryError::UnalignedOffset
            ))
        );

        backend.geometry.write_alignment = 0;
        assert_eq!(
            write_record(&mut backend, 0, 1, b"payload", &mut scratch),
            Err(JournalWriteError::Geometry(
                WriteGeometryError::ZeroWriteAlignment
            ))
        );
        backend.geometry.write_alignment = 8;
        assert_eq!(
            write_record(&mut backend, 128, 1, b"payload", &mut scratch),
            Err(JournalWriteError::Geometry(WriteGeometryError::OutOfBounds))
        );

        let encoded_length = encoded_len(5).unwrap();
        let padded_length = align_up(encoded_length, 8).unwrap();
        let mut encoded_only = [0; 57];
        assert_eq!(
            write_record(&mut backend, 0, 1, b"abcde", &mut encoded_only),
            Err(JournalWriteError::ScratchTooSmall {
                required: padded_length
            })
        );

        let mut too_short = [0; 56];
        assert_eq!(
            write_record(&mut backend, 0, 1, b"abcde", &mut too_short),
            Err(JournalWriteError::Encode(EncodeError::OutputTooSmall))
        );
        assert_eq!(backend.call_count, 0);
    }

    #[test]
    fn write_record_classifies_prefix_and_final_failures() {
        let mut prefix_failure = TraceBackend::<128>::new(8);
        prefix_failure.fail_at = Some(0);
        let mut scratch = [0; 96];
        assert_eq!(
            write_record(&mut prefix_failure, 0, 1, b"payload", &mut scratch),
            Err(JournalWriteError::BodyWrite(BackendError::InjectedFailure))
        );
        assert_eq!(prefix_failure.call_count, 1);
        assert!(prefix_failure.bytes.iter().all(|byte| *byte == 0xff));

        let mut backend = TraceBackend::<256>::new(8);
        let first_payload = b"durable";
        let mut first_scratch = [0; 96];
        let first_padded =
            write_record(&mut backend, 0, 1, first_payload, &mut first_scratch).unwrap();
        backend.fail_at = Some(backend.call_count + 1);
        let mut second_scratch = [0; 96];
        assert_eq!(
            write_record(&mut backend, first_padded, 2, b"newer", &mut second_scratch,),
            Err(JournalWriteError::CommitWrite(
                BackendError::InjectedFailure
            ))
        );
        let result = scan(&backend.bytes);
        assert_eq!(result.selected.unwrap().generation, 1);
        assert_eq!(result.selected.unwrap().payload, first_payload);
    }
}
