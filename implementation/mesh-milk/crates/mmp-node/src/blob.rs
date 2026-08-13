//! Blob plane / file transfer orchestration (doc 16, handbook §4.4, E5-S1).
//!
//! All state hangs off `NodeCore` exactly like `friends`/`convs` (handbook
//! §4.5 precedent, see `mmp-node/src/lib.rs`). `mmp-node` is std-only and
//! `mmp-node-embassy` never depends on this module — there is no `Tier::Core`
//! branch anywhere here (handbook: "forbid the blob plane on the Core tier").
//!
//! Symbols are never buffered in RAM beyond one stripe: they live in the
//! bundle store, indexed by `NodeCore::blob_index`; this module's
//! [`BlobAssembly`] only tracks which symbols have been VERIFIED (the `have`
//! bitmap — byte-identical to the BLOB_HAVE wire form, decision D4) and
//! writes decoded stripes straight to a `.part` file on disk.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use mmp_types::consts::{RS_TOTAL_N, STRIPE_DATA_BYTES};
use mmp_types::ids::{Id16, Key32};

/// Doc 11 §6 per-root FSM (decision D3). `REJECTED` is deliberately absent —
/// a manifest that fails `verify_strict` never creates an assembly at all
/// (doc 16 §3.1 fail-safe), so that spec edge is unreachable here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobState {
    /// Registered via `Node::fetch_file` but no verified symbol has arrived
    /// for it yet.
    Wanted,
    /// At least one symbol has been Merkle-verified and set in `have`; still
    /// short of a full assembly.
    Fetching,
    /// Every stripe decoded, the whole-file BLAKE3 check passed, and the
    /// `.part` file was renamed to its final destination.
    Assembled,
    /// Terminal failure state: whole-file hash mismatch, TTL lapse,
    /// `Node::cancel_fetch`, or a filesystem error prevented finalizing.
    /// Already-verified partial symbols remain servable via `blob_index`.
    Abandoned,
}

/// Requester-side assembly of one `file_root`/`thumb_root`. `have`/`stripe_done`
/// are plain bit-vectors — `have` shares its byte layout with the BLOB_HAVE
/// wire bitmap (decision D4: stripe-major, `RS_TOTAL_N` = 40 bits = 5 B per
/// stripe, LSB-first within a byte), so advertising it is a straight copy.
pub(crate) struct BlobAssembly {
    pub(crate) root: Id16,
    /// True plaintext length (manifest `FileEntry.size`).
    pub(crate) size: u32,
    pub(crate) full_hash: [u8; 32],
    pub(crate) stripes: u16,
    pub(crate) state: BlobState,
    /// `stripes * 5` bytes: bit `sym_no` set iff that symbol has been
    /// Merkle-verified for this root (D1 checks a+b).
    pub(crate) have: Vec<u8>,
    /// `ceil(stripes / 8)` bytes: bit `stripe` set once ≥`RS_DATA_K` verified
    /// symbols for that stripe have been RS-decoded and written to `part`.
    pub(crate) stripe_done: Vec<u8>,
    pub(crate) done_count: u16,
    pub(crate) dest_path: PathBuf,
    /// Opened lazily on the first stripe write; `set_len`'d to the full
    /// padded size at that point.
    pub(crate) part: Option<File>,
    /// `BLOB_INTEREST_TTL_MS` clock (monotonic `now_ms`).
    pub(crate) started_at: u64,
    /// Monotonic timestamp at which the assembly entered a terminal state.
    /// Terminal progress remains queryable for one further interest window,
    /// then `NodeCore::sweep_blob_ttl` drops only this in-memory record. The
    /// holder-side symbol index/store is deliberately independent and keeps
    /// serving verified partial symbols.
    pub(crate) terminal_at: Option<u64>,
    /// Merkle-verification-failure counter (doc 11 §6 "blacklist sender
    /// count++") — diagnostic only in this MVP, see decision D3 residual 2.
    pub(crate) bad_symbols: u32,
    /// Sharer identity from the verified manifest offer — carried so a
    /// `HashMismatch` failure (decision D2 finalize) is attributable
    /// (doc 16 §5.2 accountability framing).
    #[allow(dead_code)] // surfaced via FileProgress/diagnostics in a later story
    pub(crate) sharer: Key32,
}

/// `stripes = max(1, ceil(size / STRIPE_DATA_BYTES))` — size 0 ⇒ one zero
/// stripe (decision D2/D8 padding-edge rule).
pub(crate) fn stripes_for_size(size: u32) -> u16 {
    let n = (size as usize).div_ceil(STRIPE_DATA_BYTES).max(1);
    n as u16
}

impl BlobAssembly {
    pub(crate) fn new(
        root: Id16,
        size: u32,
        full_hash: [u8; 32],
        sharer: Key32,
        dest_path: PathBuf,
        started_at: u64,
    ) -> BlobAssembly {
        let stripes = stripes_for_size(size);
        BlobAssembly {
            root,
            size,
            full_hash,
            stripes,
            state: BlobState::Wanted,
            have: vec![0u8; stripes as usize * (RS_TOTAL_N / 8)],
            stripe_done: vec![0u8; (stripes as usize).div_ceil(8)],
            done_count: 0,
            dest_path,
            part: None,
            started_at,
            terminal_at: None,
            bad_symbols: 0,
            sharer,
        }
    }

    pub(crate) fn have_bit(&self, sym_no: u16) -> bool {
        let i = sym_no as usize;
        let byte = i / 8;
        byte < self.have.len() && (self.have[byte] & (1 << (i % 8))) != 0
    }

    pub(crate) fn set_have_bit(&mut self, sym_no: u16) {
        let i = sym_no as usize;
        let byte = i / 8;
        if byte < self.have.len() {
            self.have[byte] |= 1 << (i % 8);
        }
    }

    /// How many of a stripe's `RS_TOTAL_N` symbol slots are verified-have.
    pub(crate) fn stripe_have_count(&self, stripe: u16) -> usize {
        let base = stripe as usize * RS_TOTAL_N;
        (0..RS_TOTAL_N)
            .filter(|&j| self.have_bit((base + j) as u16))
            .count()
    }

    pub(crate) fn stripe_done_bit(&self, stripe: u16) -> bool {
        let i = stripe as usize;
        let byte = i / 8;
        byte < self.stripe_done.len() && (self.stripe_done[byte] & (1 << (i % 8))) != 0
    }

    pub(crate) fn set_stripe_done(&mut self, stripe: u16) {
        let i = stripe as usize;
        let byte = i / 8;
        if byte < self.stripe_done.len() && !self.stripe_done_bit(stripe) {
            self.stripe_done[byte] |= 1 << (i % 8);
            self.done_count += 1;
        }
    }

    pub(crate) fn all_stripes_done(&self) -> bool {
        self.done_count >= self.stripes
    }

    /// `dest_path` with a `.part` suffix appended to the file name — the
    /// incremental-write target (decision D2), renamed to `dest_path` only
    /// once the whole-file BLAKE3 check passes.
    pub(crate) fn part_path(&self) -> PathBuf {
        let mut name = self
            .dest_path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".part");
        self.dest_path.with_file_name(name)
    }

    fn ensure_part(&mut self) -> std::io::Result<&mut File> {
        if self.part.is_none() {
            // Never truncate: an existing `.part` from a prior fetch attempt
            // (or an already-written region from this same pass) must
            // survive re-opening — writes happen at explicit stripe offsets.
            let f = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(self.part_path())?;
            f.set_len(self.stripes as u64 * STRIPE_DATA_BYTES as u64)?;
            self.part = Some(f);
        }
        Ok(self.part.as_mut().expect("just set"))
    }

    /// Write one decoded stripe's `RS_DATA_K` data symbols (already
    /// concatenated in order) at its stripe offset. Memory bound (decision
    /// D2): the caller holds one stripe (≤ 64 KiB) at a time, never the whole
    /// file.
    pub(crate) fn write_stripe(&mut self, stripe_no: u16, data: &[u8]) -> std::io::Result<()> {
        // Validate before opening/creating the partial file. Besides keeping
        // malformed internal calls from writing outside this assembly, this
        // ensures a bad stripe cannot create filesystem state as a side effect.
        if stripe_no >= self.stripes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "stripe index outside assembly",
            ));
        }
        if data.len() != STRIPE_DATA_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "decoded stripe has unexpected length",
            ));
        }
        let offset = stripe_no as u64 * STRIPE_DATA_BYTES as u64;
        let f = self.ensure_part()?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(data)?;
        Ok(())
    }

    /// Truncate `part` to the true `size`, stream-hash it, and compare
    /// against `full_hash`. Closes the file handle either way (needed before
    /// the caller can rename-or-remove it, notably on Windows). `Ok(true)` ⇒
    /// bit-exact; `Ok(false)` ⇒ hash mismatch (decision D2 finalize).
    pub(crate) fn finalize(&mut self) -> std::io::Result<bool> {
        let size = self.size as u64;
        let ok = {
            let f = self.ensure_part()?;
            f.set_len(size)?;
            f.seek(SeekFrom::Start(0))?;
            let mut hasher = blake3::Hasher::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            hasher.finalize().as_bytes() == &self.full_hash
        };
        self.part = None; // close before rename/remove
        Ok(ok)
    }
}

/// Cached, verified `FileManifest` fact for one entry — enough for
/// `Node::fetch_file` to register an assembly without re-parsing the
/// manifest (decision D2/D8).
pub(crate) struct FileOfferInfo {
    pub(crate) size: u32,
    pub(crate) full_hash: [u8; 32],
    pub(crate) sharer: Key32,
}
