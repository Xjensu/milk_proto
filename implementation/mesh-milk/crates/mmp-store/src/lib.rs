//! mmp-store — persistence over the `mmp_core::BundleStore` seam.
//!
//! MVP persistence is a checksummed snapshot (`SnapshotStore`): every record
//! carries a BLAKE3 checksum; a corrupt record is dropped, not repaired — the
//! network is the backup (doc 04 §7). Crash consistency: write-temp + rename,
//! so resume ≡ reboot. A redb-backed store slots in behind the same trait when
//! store sizes outgrow snapshot writes (doc 12 §4 alternative noted).
//!
//! Beyond the `BundleStore` contents, the same file also carries six more
//! record kinds under the same MAGIC/checksum/skip-corrupt discipline
//! (handbook §4.3 "persistence completion", steps 1–3): every currently-held
//! `Claim` (re-verified on load — a claim that fails signature verification
//! is dropped exactly like a corrupt record); an opaque blob per group,
//! produced/consumed by `mmp-node`'s `GroupState::encode`/`decode` (this
//! crate does not depend on `mmp-node`, so it treats those blobs as opaque
//! bytes); and — unlike groups — a typed `SeenIndex` and `ReceiptFilter`,
//! since both types already live in `mmp-core`, a crate this one already
//! depends on for `BundleMeta` etc., so there is no layering reason to treat
//! them as opaque. Authority admission and the explicit blocklist are also
//! validated, receiver-local records. One file, one framing, seven record
//! kinds are supported under the same framing.

#![cfg_attr(not(feature = "std"), no_std)]
// The no-default-features build is an intentional dependency fence while the
// fixed-capacity embedded backend is designed. Keep the filesystem API and
// its std imports entirely out of that profile rather than pretending that
// `SnapshotStore` is available on a target without an allocator/filesystem.
#![cfg(feature = "std")]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use mmp_core::store::{BundleMeta, BundleStore, MemStore, SlotState, StorePutError, StorePutOutcome};
use mmp_core::{ReceiptFilter, SeenIndex};
use mmp_crypto::verify;
use mmp_types::bundle::{class, prio};
use mmp_types::claim::Claim;
use mmp_types::domain::SigDomain;
use mmp_types::ids::Id16;

/// Host-selected bounds for one on-disk snapshot image.
///
/// `max_file_bytes` covers the complete `MMPSNAP2` image, including framing,
/// checksums, bundle records, and every auxiliary record kind. It therefore
/// must not be set to a bundle quota without accounting for claims, groups,
/// filters, authority state, and the blocklist. The default [`SnapshotStore::open`]
/// path remains unbounded for backwards compatibility; platform hosts that
/// know their storage policy should use [`SnapshotStore::open_with_limits`].
///
/// This is a local resource policy only. It does not alter the snapshot wire
/// format or the semantics of any record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotLimits {
    max_file_bytes: u64,
}

impl SnapshotLimits {
    /// Construct a limit for the complete snapshot file.
    pub const fn new(max_file_bytes: u64) -> SnapshotLimits {
        SnapshotLimits { max_file_bytes }
    }

    /// Maximum accepted/written snapshot image size in bytes.
    pub const fn max_file_bytes(self) -> u64 {
        self.max_file_bytes
    }
}

/// File-backed store: an in-memory working set with explicit `persist`.
pub struct SnapshotStore {
    mem: MemStore,
    path: PathBuf,
    limits: Option<SnapshotLimits>,
    /// Set when an explicitly bounded open rejected the primary image (or
    /// its crash-recovery candidate) for exceeding the host's file limit.
    /// Refusing a later `persist` avoids replacing an unreadable image with an
    /// empty in-memory store and silently losing state.
    load_rejected: bool,
    /// Claims to write on the next [`SnapshotStore::persist`] (staged by the
    /// caller via [`SnapshotStore::set_claims`]) — after [`SnapshotStore::open`]
    /// this instead holds whatever survived load, i.e. every signature-verified
    /// claim record found on disk.
    claims: Vec<Claim>,
    /// Opaque `GroupState::encode()` blobs, same staging discipline as
    /// `claims` (see [`SnapshotStore::set_group_blobs`]).
    group_blobs: Vec<Vec<u8>>,
    /// Singleton (at most one per node), same staging discipline as `claims`
    /// (see [`SnapshotStore::set_seen_index`]). `None` until staged or loaded.
    seen_index: Option<SeenIndex>,
    /// Singleton, mirrors `seen_index` (see [`SnapshotStore::set_receipt_filter`]).
    receipt_filter: Option<ReceiptFilter>,
    /// Receiver-local authority roots/certificates/subscriptions/revocations,
    /// encoded by `mmp_core::AuthorityStore`. This contains no private key or
    /// ratchet material.
    authority: Option<Vec<u8>>,
    /// Deterministic, non-secret local blocklist record.
    blocklist: Option<Vec<u8>>,
}

/// Bumped from `MMPSNAP1`: pre-existing snapshot files only ever held bundle
/// records with no leading kind byte: reusing the old magic on the new,
/// kind-tagged framing (see [`RECORD_BUNDLE`] etc.) could misparse an old
/// file's first payload byte as a record kind. This project has no shipped
/// snapshot files yet, so a clean break costs nothing.
const MAGIC: &[u8; 8] = b"MMPSNAP2";

/// Record-kind tag: a bundle, framed exactly as before (`encode_record`/`decode_record`).
const RECORD_BUNDLE: u8 = 0;
/// Record-kind tag: a `Claim::encode()` payload, re-verified on load.
const RECORD_CLAIM: u8 = 1;
/// Record-kind tag: an opaque `mmp_node::group::GroupState::encode()` payload.
const RECORD_GROUP: u8 = 2;
/// Record-kind tag: a `SeenIndex::encode()` payload (E4-S4, handbook §4.3 step 3).
const RECORD_SEEN_INDEX: u8 = 3;
/// Record-kind tag: a `ReceiptFilter::encode()` payload (E4-S4, handbook §4.3 step 3).
const RECORD_RECEIPT_FILTER: u8 = 4;
/// Record-kind tag: a validated `AuthorityStore::encode()` payload.
const RECORD_AUTHORITY: u8 = 5;
/// Record-kind tag: a bounded local blocklist snapshot.
const RECORD_BLOCKLIST: u8 = 6;
const BLOCKLIST_VERSION: u8 = 1;
const MAX_BLOCKLIST_ENTRIES: usize = 1024;

impl SnapshotStore {
    /// Open (or create) a snapshot at `path`. Corrupt records are skipped.
    pub fn open(path: impl Into<PathBuf>) -> SnapshotStore {
        Self::open_internal(path.into(), None)
    }

    /// Open (or create) a snapshot with an explicit complete-file bound.
    ///
    /// An image larger than `limits.max_file_bytes()` is rejected before its
    /// bytes are read into memory. The primary image still wins over a `.tmp`
    /// candidate, including when it is rejected for size. A later `persist`
    /// returns an error while that rejection is recorded, preventing silent
    /// replacement of state the host chose not to load.
    pub fn open_with_limits(path: impl Into<PathBuf>, limits: SnapshotLimits) -> SnapshotStore {
        Self::open_internal(path.into(), Some(limits))
    }

    fn open_internal(path: PathBuf, limits: Option<SnapshotLimits>) -> SnapshotStore {
        let mut store = SnapshotStore {
            mem: MemStore::new(),
            path,
            limits,
            load_rejected: false,
            claims: Vec::new(),
            group_blobs: Vec::new(),
            seen_index: None,
            receipt_filter: None,
            authority: None,
            blocklist: None,
        };
        store.load();
        store
    }

    fn load(&mut self) {
        let bytes = match read_snapshot(&self.path, self.limits) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                self.load_rejected = true;
                return;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // `persist` fsyncs a complete temporary image before the
                // Windows replace step. If the process dies after the old
                // path is removed but before rename completes, the primary
                // is absent while this candidate may still be recoverable.
                // Only a fully framed, semantically validated image may be
                // considered; a primary (even one containing corrupt records)
                // always wins and never falls back to `.tmp`.
                let temp = self.path.with_extension("tmp");
                let temp_bytes = match read_snapshot(&temp, self.limits) {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => {
                        self.load_rejected = true;
                        return;
                    }
                    Err(_) => return,
                };
                if !validate_complete_snapshot(&temp_bytes) {
                    return;
                }
                temp_bytes
            }
            Err(_) => return,
        };
        self.load_bytes(&bytes);
    }

    fn load_bytes(&mut self, bytes: &[u8]) {
        if bytes.len() < 8 || &bytes[..8] != MAGIC {
            return;
        }
        let mut pos: usize = 8;
        while let Some(length_end) = pos.checked_add(4) {
            // `.get()` + `?` rather than direct indexing: this is the one path
            // that reads potentially corrupted/truncated disk bytes, and the
            // handbook §0 policy is that no panic must be reachable from wire
            // or disk data, even where a bound is currently provably safe.
            let Some(len_bytes) = bytes.get(pos..length_end) else { break };
            let Ok(len_bytes) = <[u8; 4]>::try_from(len_bytes) else { break };
            let len = u32::from_le_bytes(len_bytes) as usize;
            pos = length_end;
            let Some(record_end) = pos.checked_add(len) else { break };
            let Some(checksum_end) = record_end.checked_add(32) else { break };
            if checksum_end > bytes.len() {
                break; // truncated tail: crash mid-write — drop it
            }
            let record = &bytes[pos..record_end];
            let sum = &bytes[record_end..checksum_end];
            pos = checksum_end;
            if blake3::hash(record).as_bytes() != sum {
                continue; // corrupt record: drop-and-redownload
            }
            let Some((&kind, payload)) = record.split_first() else { continue };
            match kind {
                RECORD_BUNDLE => {
                    if let Some((meta, body)) = decode_record(payload) {
                        self.mem.put(meta, &body);
                    }
                }
                RECORD_CLAIM => {
                    if let Ok(claim) = Claim::decode(payload) {
                        // Doc 04 §7 / handbook §4.3 step 1: a tampered claim
                        // is dropped on load exactly like a corrupt record —
                        // verify-before-trust is unconditional (golden rule 4
                        // applied here to claims, not just bundles).
                        if verify(SigDomain::Claim, &claim.owner, &claim.signed_object(), &claim.sig)
                            .is_ok()
                        {
                            self.claims.push(claim);
                        }
                    }
                }
                RECORD_GROUP => {
                    self.group_blobs.push(payload.to_vec());
                }
                RECORD_SEEN_INDEX => {
                    // A malformed record is dropped exactly like a corrupt
                    // record — the node just starts with an empty SeenIndex,
                    // same safe-direction posture as never having persisted
                    // one at all.
                    if let Some(seen) = SeenIndex::decode(payload) {
                        self.seen_index = Some(seen);
                    }
                }
                RECORD_RECEIPT_FILTER => {
                    if let Some(rf) = ReceiptFilter::decode(payload) {
                        self.receipt_filter = Some(rf);
                    }
                }
                // AuthorityStore::decode re-verifies certificates against the
                // restored pinned roots before this state can affect
                // admission after restart.
                RECORD_AUTHORITY if mmp_core::AuthorityStore::decode(payload).is_some() => {
                    self.authority = Some(payload.to_vec());
                }
                RECORD_BLOCKLIST if valid_blocklist(payload) => {
                    self.blocklist = Some(payload.to_vec());
                }
                _ => {} // unknown kind: forward-compat, skip rather than fail closed
            }
        }
    }

    /// Write the whole store atomically (temp + rename): bundles, then the
    /// claims/group blobs most recently staged via [`SnapshotStore::set_claims`]
    /// / [`SnapshotStore::set_group_blobs`].
    pub fn persist(&self) -> std::io::Result<()> {
        if self.load_rejected {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "snapshot exceeded configured file limit and was not loaded",
            ));
        }
        // `decode_record` rejects a metadata/body length mismatch on restart.
        // Validate the in-memory working set before creating a replacement
        // image so `persist` never reports success for a snapshot that would
        // silently discard a bundle at the next load. This can happen through
        // the legacy infallible `BundleStore::put` or `update_meta` seam,
        // which intentionally remains available during the E7 transaction
        // migration. The existing primary image is untouched on rejection.
        for meta in self.mem.all_meta() {
            let Some((stored_meta, body)) = self.mem.get(&meta.id) else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "snapshot working set changed while validating a bundle",
                ));
            };
            if stored_meta.size as usize != body.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle metadata size does not match stored bytes",
                ));
            }
        }
        let tmp = self.path.with_extension("tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(MAGIC)?;
            for meta in self.mem.all_meta() {
                if let Some((m, body)) = self.mem.get(&meta.id) {
                    let mut record = Vec::with_capacity(1 + 80 + body.len());
                    record.push(RECORD_BUNDLE);
                    record.extend_from_slice(&encode_record(&m, &body));
                    write_record(&mut f, &record)?;
                }
            }
            for claim in &self.claims {
                let mut record = Vec::with_capacity(1 + 32 + 1 + 4 + claim.body.len() + 64);
                record.push(RECORD_CLAIM);
                record.extend_from_slice(&claim.encode());
                write_record(&mut f, &record)?;
            }
            for blob in &self.group_blobs {
                let mut record = Vec::with_capacity(1 + blob.len());
                record.push(RECORD_GROUP);
                record.extend_from_slice(blob);
                write_record(&mut f, &record)?;
            }
            if let Some(seen) = &self.seen_index {
                let mut record = vec![RECORD_SEEN_INDEX];
                record.extend_from_slice(&seen.encode());
                write_record(&mut f, &record)?;
            }
            if let Some(rf) = &self.receipt_filter {
                let mut record = vec![RECORD_RECEIPT_FILTER];
                record.extend_from_slice(&rf.encode());
                write_record(&mut f, &record)?;
            }
            if let Some(authority) = &self.authority {
                let mut record = vec![RECORD_AUTHORITY];
                record.extend_from_slice(authority);
                write_record(&mut f, &record)?;
            }
            if let Some(blocklist) = &self.blocklist {
                let mut record = vec![RECORD_BLOCKLIST];
                record.extend_from_slice(blocklist);
                write_record(&mut f, &record)?;
            }
            f.sync_all()?;
            if let Some(limits) = self.limits {
                if f.metadata()?.len() > limits.max_file_bytes() {
                    drop(f);
                    let _ = std::fs::remove_file(&tmp);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "snapshot exceeds configured file limit",
                    ));
                }
            }
        }
        replace_snapshot(&tmp, &self.path)
    }

    /// Return the complete encoded snapshot size after replacing or inserting
    /// one bundle. This is the bounded receive-side admission calculation for
    /// [`BundleStore::try_put`]; it deliberately includes framing, checksums,
    /// and every staged auxiliary record because `SnapshotLimits` applies to
    /// the complete image rather than to bundle bytes alone.
    fn projected_image_size(&self, candidate: &BundleMeta, body: &[u8]) -> Option<u64> {
        if usize::try_from(candidate.size).ok()? != body.len() {
            return None;
        }

        let mut total = u64::try_from(MAGIC.len()).ok()?;
        for meta in self.mem.all_meta() {
            if meta.id == candidate.id {
                continue; // replacement delta: do not count the old body
            }
            let (stored_meta, stored_body) = self.mem.get(&meta.id)?;
            if usize::try_from(stored_meta.size).ok()? != stored_body.len() {
                return None;
            }
            total = total.checked_add(bundle_record_size(stored_body.len())?)?;
        }
        total = total.checked_add(bundle_record_size(body.len())?)?;

        for claim in &self.claims {
            total = total.checked_add(framed_record_size(1usize.checked_add(claim.encode().len())?)?)?;
        }
        for blob in &self.group_blobs {
            total = total.checked_add(framed_record_size(1usize.checked_add(blob.len())?)?)?;
        }
        if let Some(seen) = &self.seen_index {
            total = total.checked_add(framed_record_size(1usize.checked_add(seen.encode().len())?)?)?;
        }
        if let Some(filter) = &self.receipt_filter {
            total = total.checked_add(framed_record_size(1usize.checked_add(filter.encode().len())?)?)?;
        }
        if let Some(authority) = &self.authority {
            total = total.checked_add(framed_record_size(1usize.checked_add(authority.len())?)?)?;
        }
        if let Some(blocklist) = &self.blocklist {
            total = total.checked_add(framed_record_size(1usize.checked_add(blocklist.len())?)?)?;
        }
        Some(total)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether an explicitly bounded open rejected the existing image before
    /// parsing. A rejected store cannot be persisted until the host chooses a
    /// different path or removes the oversized image.
    pub fn load_rejected(&self) -> bool {
        self.load_rejected
    }

    /// Claims present in the store: after [`SnapshotStore::open`], every
    /// signature-verified claim record loaded from disk; after
    /// [`SnapshotStore::set_claims`], whatever was most recently staged.
    pub fn claims(&self) -> &[Claim] {
        &self.claims
    }

    /// Stage the full claim set to write on the next [`SnapshotStore::persist`]
    /// (the caller — `mmp-ffi` — owns the `ClaimStore`; this just holds what
    /// it's told to write, same pattern as `mem` holding bundle contents).
    pub fn set_claims(&mut self, claims: Vec<Claim>) {
        self.claims = claims;
    }

    /// Opaque group-state blobs present in the store (see the module-level
    /// docs for why this crate cannot know their internal shape).
    pub fn group_blobs(&self) -> &[Vec<u8>] {
        &self.group_blobs
    }

    /// Stage the full group-blob set to write on the next
    /// [`SnapshotStore::persist`] — mirrors [`SnapshotStore::set_claims`].
    pub fn set_group_blobs(&mut self, blobs: Vec<Vec<u8>>) {
        self.group_blobs = blobs;
    }

    /// The `SeenIndex` present in the store: after [`SnapshotStore::open`],
    /// whatever survived load (`None` if never persisted, or the persisted
    /// record was corrupt); after [`SnapshotStore::set_seen_index`], whatever
    /// was most recently staged.
    pub fn seen_index(&self) -> Option<&SeenIndex> {
        self.seen_index.as_ref()
    }

    /// Stage the `SeenIndex` to write on the next [`SnapshotStore::persist`]
    /// — mirrors [`SnapshotStore::set_claims`]'s staging discipline.
    pub fn set_seen_index(&mut self, seen: SeenIndex) {
        self.seen_index = Some(seen);
    }

    /// The `ReceiptFilter` present in the store — mirrors [`SnapshotStore::seen_index`].
    pub fn receipt_filter(&self) -> Option<&ReceiptFilter> {
        self.receipt_filter.as_ref()
    }

    /// Stage the `ReceiptFilter` to write on the next [`SnapshotStore::persist`]
    /// — mirrors [`SnapshotStore::set_seen_index`].
    pub fn set_receipt_filter(&mut self, rf: ReceiptFilter) {
        self.receipt_filter = Some(rf);
    }

    /// The validated authority-admission snapshot, if one was loaded or
    /// staged.
    pub fn authority(&self) -> Option<&[u8]> {
        self.authority.as_deref()
    }

    /// Stage a receiver-local authority snapshot. Invalid bytes are rejected
    /// before they can be written, and the decoder also validates every
    /// certificate against its restored pinned root.
    pub fn set_authority(&mut self, authority: Vec<u8>) {
        if mmp_core::AuthorityStore::decode(&authority).is_some() {
            self.authority = Some(authority);
        }
    }

    /// The validated local blocklist snapshot, if one was loaded or staged.
    pub fn blocklist(&self) -> Option<&[u8]> {
        self.blocklist.as_deref()
    }

    /// Stage a bounded, deterministic local blocklist snapshot. Invalid or
    /// unsorted entries are rejected before persistence.
    pub fn set_blocklist(&mut self, blocklist: Vec<u8>) {
        if valid_blocklist(&blocklist) {
            self.blocklist = Some(blocklist);
        }
    }
}

fn valid_blocklist(bytes: &[u8]) -> bool {
    if bytes.len() < 3 || bytes[0] != BLOCKLIST_VERSION {
        return false;
    }
    let count = u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
    if count > MAX_BLOCKLIST_ENTRIES
        || bytes.len() != 3usize.saturating_add(count.saturating_mul(16))
    {
        return false;
    }
    bytes[3..]
        .chunks_exact(16)
        .try_fold(None, |previous: Option<&[u8]>, current| {
            if previous.is_some_and(|old| old >= current) {
                None
            } else {
                Some(Some(current))
            }
        })
        .is_some()
}

/// Read one snapshot image, optionally enforcing a host-selected complete-file
/// bound before and during the read. `Ok(None)` means the file exists but was
/// rejected for exceeding the configured limit; `NotFound` remains an ordinary
/// `io::Error` so the caller can apply the primary-then-`.tmp` recovery rule.
fn read_snapshot(path: &Path, limits: Option<SnapshotLimits>) -> std::io::Result<Option<Vec<u8>>> {
    let Some(limits) = limits else {
        return std::fs::read(path).map(Some);
    };

    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > limits.max_file_bytes() {
        return Ok(None);
    }

    // Read at most one byte beyond the limit so a file that grows after the
    // metadata check is rejected without ever retaining an oversized image.
    let read_limit = limits.max_file_bytes().saturating_add(1);
    let mut bytes = Vec::new();
    file.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limits.max_file_bytes() {
        return Ok(None);
    }
    Ok(Some(bytes))
}

/// Validate a temporary snapshot as one complete image before using it as a
/// recovery source. The normal primary-file loader intentionally skips bad
/// records so one damaged bundle does not hide every other record; recovery
/// from a replacement candidate is stricter because accepting a truncated
/// image would silently replace a missing primary with partial state.
fn validate_complete_snapshot(bytes: &[u8]) -> bool {
    if bytes.len() < MAGIC.len() || &bytes[..MAGIC.len()] != MAGIC {
        return false;
    }

    let mut pos = MAGIC.len();
    while pos < bytes.len() {
        let Some(length_end) = pos.checked_add(4) else { return false };
        let Some(length_bytes) = bytes.get(pos..length_end) else { return false };
        let Ok(length_bytes) = <[u8; 4]>::try_from(length_bytes) else { return false };
        let length = u32::from_le_bytes(length_bytes) as usize;
        let Some(after_length) = pos.checked_add(4) else { return false };
        pos = after_length;
        let Some(record_end) = pos.checked_add(length) else { return false };
        let Some(checksum_end) = record_end.checked_add(32) else { return false };
        if checksum_end > bytes.len() {
            return false;
        }
        let record = &bytes[pos..record_end];
        let checksum = &bytes[record_end..checksum_end];
        if blake3::hash(record).as_bytes() != checksum {
            return false;
        }
        let Some((&kind, payload)) = record.split_first() else { return false };
        let semantically_valid = match kind {
            RECORD_BUNDLE => decode_record(payload).is_some(),
            RECORD_CLAIM => Claim::decode(payload).ok().is_some_and(|claim| {
                verify(SigDomain::Claim, &claim.owner, &claim.signed_object(), &claim.sig).is_ok()
            }),
            // GroupState is deliberately opaque at this layer; mmp-node
            // performs its strict decoder before adopting a group.
            RECORD_GROUP => true,
            RECORD_SEEN_INDEX => SeenIndex::decode(payload).is_some(),
            RECORD_RECEIPT_FILTER => ReceiptFilter::decode(payload).is_some(),
            RECORD_AUTHORITY => mmp_core::AuthorityStore::decode(payload).is_some(),
            RECORD_BLOCKLIST => valid_blocklist(payload),
            // Unknown kinds are forward-compatible as long as framing and
            // checksum are intact; the normal loader skips them too.
            _ => true,
        };
        if !semantically_valid {
            return false;
        }
        pos = checksum_end;
    }
    pos == bytes.len()
}

fn checked_record_len(len: usize) -> std::io::Result<u32> {
    u32::try_from(len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "snapshot record exceeds the u32 framing limit",
        )
    })
}

fn write_record(f: &mut std::fs::File, record: &[u8]) -> std::io::Result<()> {
    let len = checked_record_len(record.len())?;
    f.write_all(&len.to_le_bytes())?;
    f.write_all(record)?;
    f.write_all(blake3::hash(record).as_bytes())?;
    Ok(())
}

/// Replace the published image after the complete temporary image has been
/// synced. Unix `rename` replaces an existing destination atomically when both
/// paths are on the same filesystem (the temp path is derived from the
/// destination, so that condition holds), and syncing the parent directory
/// makes the rename durable across a metadata-only crash.
///
/// The standard library does not expose an atomic replace primitive for
/// Windows. Keep the existing remove-then-rename fallback there rather than
/// introducing unreviewed platform FFI; callers must treat a Windows crash in
/// this narrow interval as recoverable only when the complete `.tmp` image is
/// still present.
fn replace_snapshot(tmp: &Path, path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(tmp, path)?;
        return sync_parent_directory(path);
    }

    #[cfg(not(unix))]
    {
        let _ = std::fs::remove_file(path);
        std::fs::rename(tmp, path)
    }
}

/// Size of one framed record (`u32 length` + record + BLAKE3 checksum).
/// `None` means the record cannot be represented by the snapshot framing.
fn framed_record_size(record_len: usize) -> Option<u64> {
    let record_len = u32::try_from(record_len).ok()? as u64;
    4u64.checked_add(record_len)?.checked_add(32)
}

/// Size of a bundle record, including its kind byte and fixed metadata header.
/// The body length is the exact byte vector held by `MemStore`.
fn bundle_record_size(body_len: usize) -> Option<u64> {
    const BUNDLE_META_LEN: usize = 16 + 1 + 16 + 16 + 1 + 4 + 8 + 8 + 1 + 1 + 1;
    let record_len = 1usize.checked_add(BUNDLE_META_LEN)?.checked_add(body_len)?;
    framed_record_size(record_len)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)?.sync_all()
}

fn encode_record(m: &BundleMeta, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(80 + body.len());
    out.extend_from_slice(&m.id.0);
    out.push(m.class);
    out.extend_from_slice(&m.origin.0);
    out.extend_from_slice(&m.dest.0);
    out.push(m.priority);
    out.extend_from_slice(&m.size.to_le_bytes());
    out.extend_from_slice(&m.stored_at.to_le_bytes());
    out.extend_from_slice(&m.expires_at.to_le_bytes());
    out.push(m.budget);
    out.push(m.own_origin as u8);
    out.push(match m.slot {
        SlotState::Stored => 0,
        // I3 crash rule: an unconfirmed hand-off counts as completed —
        // under-spray, never over-spray. Persist it as such.
        SlotState::HandoffPending => 0,
    });
    out.extend_from_slice(body);
    out
}

fn decode_record(r: &[u8]) -> Option<(BundleMeta, Vec<u8>)> {
    const HDR: usize = 16 + 1 + 16 + 16 + 1 + 4 + 8 + 8 + 1 + 1 + 1;
    if r.len() < HDR {
        return None;
    }
    // `.get()` + `?` throughout rather than direct indexing/`.expect()`: this
    // decodes potentially corrupted disk bytes, so no panic may be reachable
    // here even where a bound is provably satisfied today (handbook §0).
    let id16 = |o: usize| -> Option<Id16> {
        let mut a = [0u8; 16];
        a.copy_from_slice(r.get(o..o + 16)?);
        Some(Id16(a))
    };
    let u32le = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(r.get(o..o + 4)?.try_into().unwrap())) };
    let u64le = |o: usize| -> Option<u64> { Some(u64::from_le_bytes(r.get(o..o + 8)?.try_into().unwrap())) };
    let class_byte = *r.get(16)?;
    let priority = *r.get(49)?;
    let meta = BundleMeta {
        id: id16(0)?,
        class: class_byte,
        origin: id16(17)?,
        dest: id16(33)?,
        priority,
        // Not persisted (RAM-only field, doc 18 §5.2 clamp — see mmp-node's
        // AuthorityStore/AuthorityBuckets, also RAM-only by design). Safe
        // fail-closed default on load: non-announcement bundles have no
        // clamp concept, so effective == declared; a restarted announcement
        // re-clamps to NORMAL until the authority state that justified any
        // prior elevation is re-established (it never persisted either).
        effective_priority: if class_byte == class::ANNOUNCEMENT { prio::NORMAL } else { priority },
        size: u32le(50)?,
        stored_at: u64le(54)?,
        expires_at: u64le(62)?,
        budget: *r.get(70)?,
        own_origin: *r.get(71)? != 0,
        slot: SlotState::Stored,
    };
    let body = r.get(HDR..)?;
    // `BundleMeta::size` is the stored wire-byte length and feeds host-side
    // quota/transfer accounting.  A checksummed record with a mismatched
    // length is still malformed: accepting it would restore metadata that
    // disagrees with the bytes the store actually holds.  Reject the record
    // rather than allowing inconsistent state to influence scheduling.
    if body.len() != usize::try_from(meta.size).ok()? {
        return None;
    }
    Some((meta, body.to_vec()))
}

impl BundleStore for SnapshotStore {
    fn put(&mut self, meta: BundleMeta, bytes: &[u8]) {
        self.mem.put(meta, bytes)
    }
    fn try_put(&mut self, meta: BundleMeta, bytes: &[u8]) -> Result<StorePutOutcome, StorePutError> {
        let replaced = self.mem.meta(&meta.id).is_some();
        let Some(limits) = self.limits else {
            self.mem.put(meta, bytes);
            return Ok(if replaced { StorePutOutcome::Replaced } else { StorePutOutcome::Inserted });
        };
        // An oversized image was intentionally not loaded. Do not let a
        // subsequent receive path appear to admit state that `persist` is
        // permanently forbidden to replace.
        if self.load_rejected
            || self
                .projected_image_size(&meta, bytes)
                .is_none_or(|size| size > limits.max_file_bytes())
        {
            return Err(StorePutError::Capacity);
        }
        self.mem.put(meta, bytes);
        Ok(if replaced { StorePutOutcome::Replaced } else { StorePutOutcome::Inserted })
    }
    fn get(&self, id: &Id16) -> Option<(BundleMeta, Vec<u8>)> {
        self.mem.get(id)
    }
    fn meta(&self, id: &Id16) -> Option<BundleMeta> {
        self.mem.meta(id)
    }
    fn update_meta(&mut self, meta: BundleMeta) {
        self.mem.update_meta(meta)
    }
    fn remove(&mut self, id: &Id16) -> bool {
        self.mem.remove(id)
    }
    fn ids_range(&self, lo: &Id16, hi: &Id16) -> Vec<Id16> {
        self.mem.ids_range(lo, hi)
    }
    fn all_meta(&self) -> Vec<BundleMeta> {
        self.mem.all_meta()
    }
    fn total_bytes(&self) -> u64 {
        self.mem.total_bytes()
    }
    fn len(&self) -> usize {
        self.mem.len()
    }
    fn expire(&mut self, now_ms: u64) -> Vec<Id16> {
        self.mem.expire(now_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_record_length_rejects_u32_overflow() {
        assert_eq!(checked_record_len(u32::MAX as usize).unwrap(), u32::MAX);
        #[cfg(target_pointer_width = "64")]
        assert!(checked_record_len(u32::MAX as usize + 1).is_err());
    }

    #[test]
    fn decode_record_rejects_metadata_body_size_mismatch() {
        let meta = BundleMeta {
            id: Id16([1; 16]),
            class: class::DIRECT,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 4,
            stored_at: 10,
            expires_at: 1000,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        };
        let encoded = encode_record(&meta, b"hello");
        assert!(decode_record(&encoded).is_none());
    }

    #[test]
    fn replacement_swaps_complete_image() {
        let dir = std::env::temp_dir().join(format!(
            "mmp-store-atomic-replace-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snapshot.mmp");
        let tmp = path.with_extension("tmp");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&path, b"old").unwrap();
        std::fs::write(&tmp, b"new").unwrap();

        replace_snapshot(&tmp, &path).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert!(!tmp.exists());
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn snapshot_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snap.mmp");
        let _ = std::fs::remove_file(&path);

        let mut s = SnapshotStore::open(&path);
        let meta = BundleMeta {
            id: Id16([1; 16]),
            class: 1,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: 1,
            effective_priority: 1,
            size: 5,
            stored_at: 10,
            expires_at: 1000,
            budget: 4,
            own_origin: true,
            slot: SlotState::HandoffPending,
        };
        s.put(meta, b"hello");
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        let (m2, body) = s2.get(&Id16([1; 16])).unwrap();
        assert_eq!(body, b"hello");
        assert_eq!(m2.budget, 4);
        // I3: pending hand-off resumed as completed (Stored, budget persisted).
        assert_eq!(m2.slot, SlotState::Stored);

        // Corrupt a byte mid-file → record dropped, no panic.
        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 40] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.get(&Id16([1; 16])).is_none());
    }

    #[test]
    fn persist_rejects_metadata_body_mismatch_without_replacing_primary() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mismatched-meta.mmp");
        let tmp = path.with_extension("tmp");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&tmp);

        let mut store = SnapshotStore::open(&path);
        let valid = BundleMeta {
            id: Id16([9; 16]),
            class: class::DIRECT,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 5,
            stored_at: 10,
            expires_at: 1000,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        };
        store.put(valid, b"hello");
        store.persist().unwrap();
        let primary = std::fs::read(&path).unwrap();

        // The compatibility `put` seam is infallible and can receive a
        // caller-supplied size that disagrees with the bytes. Such a record
        // would be dropped by `decode_record` after restart, so persistence
        // must fail before replacing the last known-good image.
        let mut invalid = valid;
        invalid.size = 4;
        store.put(invalid, b"hello");
        let error = store.persist().expect_err("invalid metadata must not be persisted");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&path).unwrap(), primary);
        assert!(!tmp.exists(), "validation must happen before creating a replacement image");

        let restored = SnapshotStore::open(&path);
        assert!(restored.get(&valid.id).is_some(), "the previous complete image remains recoverable");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn complete_temp_recovers_only_when_primary_is_absent() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tmp-recovery.mmp");
        let candidate_path = dir.join("tmp-recovery-candidate.mmp");
        let temp = path.with_extension("tmp");
        for file in [&path, &candidate_path, &temp] {
            let _ = std::fs::remove_file(file);
        }

        let meta = |id: u8| BundleMeta {
            id: Id16([id; 16]),
            class: 1,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: 1,
            effective_priority: 1,
            size: 3,
            stored_at: 10,
            expires_at: 1000,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        };

        let mut primary = SnapshotStore::open(&path);
        primary.put(meta(1), b"old");
        primary.persist().unwrap();
        let primary_bytes = std::fs::read(&path).unwrap();

        let mut candidate = SnapshotStore::open(&candidate_path);
        candidate.put(meta(2), b"new");
        candidate.persist().unwrap();
        let candidate_bytes = std::fs::read(&candidate_path).unwrap();
        std::fs::remove_file(&candidate_path).unwrap();

        // Model a crash after the destination was removed but before rename.
        std::fs::write(&temp, &candidate_bytes).unwrap();
        std::fs::remove_file(&path).unwrap();
        let recovered = SnapshotStore::open(&path);
        assert!(recovered.get(&Id16([2; 16])).is_some(), "complete temp image must be recovered");
        assert!(recovered.get(&Id16([1; 16])).is_none());

        // A valid primary always wins; an unvalidated temp must never shadow it.
        std::fs::write(&path, &primary_bytes).unwrap();
        std::fs::write(&temp, b"MMPSNAP2\x01\x02").unwrap();
        let retained = SnapshotStore::open(&path);
        assert!(retained.get(&Id16([1; 16])).is_some());
        assert!(retained.get(&Id16([2; 16])).is_none());
    }

    #[test]
    fn incomplete_or_corrupt_temp_is_rejected_when_primary_is_absent() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tmp-recovery-invalid.mmp");
        let temp = path.with_extension("tmp");
        for file in [&path, &temp] {
            let _ = std::fs::remove_file(file);
        }

        // A cut length prefix is not a complete candidate.
        let mut incomplete = MAGIC.to_vec();
        incomplete.extend_from_slice(&8u32.to_le_bytes());
        incomplete.extend_from_slice(&[0xaa, 0xbb]);
        std::fs::write(&temp, incomplete).unwrap();
        let empty = SnapshotStore::open(&path);
        assert_eq!(empty.len(), 0);

        // A complete frame with a bad checksum is also rejected, rather than
        // being fed to the normal drop-corrupt-record loader.
        let mut corrupt = MAGIC.to_vec();
        let record = [RECORD_BUNDLE, 0u8];
        corrupt.extend_from_slice(&(record.len() as u32).to_le_bytes());
        corrupt.extend_from_slice(&record);
        corrupt.extend_from_slice(&[0u8; 32]);
        std::fs::write(&temp, corrupt).unwrap();
        let empty = SnapshotStore::open(&path);
        assert_eq!(empty.len(), 0);

        // Framing/checksum alone is insufficient for a known record kind:
        // a semantically malformed bundle must also be rejected.
        let malformed_record = [RECORD_BUNDLE, 0u8];
        let mut malformed = MAGIC.to_vec();
        malformed.extend_from_slice(&(malformed_record.len() as u32).to_le_bytes());
        malformed.extend_from_slice(&malformed_record);
        malformed.extend_from_slice(blake3::hash(&malformed_record).as_bytes());
        std::fs::write(&temp, malformed).unwrap();
        let empty = SnapshotStore::open(&path);
        assert_eq!(empty.len(), 0);
    }

    /// E2-S6 acceptance: a file truncated mid-write (crash right after the
    /// magic, or right in the middle of a length prefix) must load with
    /// records skipped, never panic. Covers the anomaly classes the `.get()`
    /// hardening in `load`/`decode_record` targets, distinct from the
    /// mid-file byte-flip corruption covered by the other roundtrip tests.
    #[test]
    fn truncated_file_loads_with_no_records_and_no_panic() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();

        // Case 1: nothing but the magic — no records to read at all.
        let path1 = dir.join("truncated-magic-only.mmp");
        std::fs::write(&path1, MAGIC).unwrap();
        let s1 = SnapshotStore::open(&path1);
        assert_eq!(s1.claims().len(), 0);
        assert_eq!(s1.group_blobs().len(), 0);

        // Case 2: magic plus a length prefix cut short (2 of 4 bytes) —
        // exactly the anomaly the `bytes.get(pos..pos+4)` guard now covers.
        let path2 = dir.join("truncated-length-prefix.mmp");
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&[0x01, 0x02]);
        std::fs::write(&path2, &bytes).unwrap();
        let s2 = SnapshotStore::open(&path2);
        assert_eq!(s2.claims().len(), 0);
        assert_eq!(s2.group_blobs().len(), 0);

        // Case 3: a full length prefix claiming a record body that was never
        // written (crash between writing the length and the body/checksum).
        let path3 = dir.join("truncated-record-body.mmp");
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&100u32.to_le_bytes());
        std::fs::write(&path3, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path3);
        assert_eq!(s3.claims().len(), 0);
        assert_eq!(s3.group_blobs().len(), 0);
    }

    fn signed_claim(seed: [u8; 32], field: u8, seq: u32, body: Vec<u8>) -> Claim {
        let identity = mmp_crypto::Identity::from_seed(&seed);
        let mut claim = Claim { owner: identity.public(), field, seq, body, sig: mmp_types::ids::Sig64::ZERO };
        claim.sig = identity.sign(SigDomain::Claim, &claim.signed_object());
        claim
    }

    /// Mirrors `snapshot_roundtrip_and_corruption_drop`'s shape (handbook
    /// §4.3 step 1 acceptance): claims round-trip through open → persist →
    /// open, and a tampered claim record is dropped on load rather than
    /// trusted, exactly like a corrupt bundle record.
    #[test]
    fn claims_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claims.mmp");
        let _ = std::fs::remove_file(&path);

        let c1 = signed_claim([1; 32], 0x01, 1, b"alice".to_vec());
        let c2 = signed_claim([2; 32], 0x01, 1, b"bob".to_vec());

        let mut s = SnapshotStore::open(&path);
        s.set_claims(vec![c1.clone(), c2.clone()]);
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        assert_eq!(s2.claims().len(), 2, "both signed claims survive the round trip");
        assert!(s2.claims().contains(&c1));
        assert!(s2.claims().contains(&c2));

        // Corrupt a byte mid-file → the affected record is dropped, not trusted.
        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 40] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.claims().len() < 2, "the corrupted claim record must not survive load");
    }

    /// A claim whose signature does not verify (tampered body, valid framing
    /// and checksum) must be dropped on load — belt-and-braces on top of the
    /// checksum, per handbook §4.3 step 1 ("re-verify on load").
    #[test]
    fn tampered_claim_signature_is_dropped_even_with_a_valid_checksum() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claims-tampered-sig.mmp");
        let _ = std::fs::remove_file(&path);

        let mut c = signed_claim([3; 32], 0x01, 1, b"carol".to_vec());
        let mut s = SnapshotStore::open(&path);
        s.set_claims(vec![c.clone()]);
        s.persist().unwrap();

        // Flip the claim body post-signing and re-persist under a *correct*
        // checksum (simulating a bit-flip that happens to preserve framing —
        // the point is the signature check, not the checksum) by staging the
        // tampered claim directly rather than editing file bytes.
        c.body = b"mallory".to_vec();
        let mut s2 = SnapshotStore::open(&path);
        s2.set_claims(vec![c]);
        s2.persist().unwrap();

        let s3 = SnapshotStore::open(&path);
        assert!(s3.claims().is_empty(), "a claim whose signature no longer matches its body must be dropped");
    }

    /// Mirrors `snapshot_roundtrip_and_corruption_drop`'s shape for the
    /// group-state persistence lane (handbook §4.3 step 2 acceptance). This
    /// crate does not depend on `mmp-node`, so it cannot construct a real
    /// `GroupState` — it stores/returns opaque blobs, so the test uses
    /// stand-in bytes; the real `GroupState::encode`/`decode` round trip is
    /// covered end-to-end by the `mmp-ffi` restart test.
    #[test]
    fn group_blobs_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("groups.mmp");
        let _ = std::fs::remove_file(&path);

        let g1 = vec![1u8, 2, 3, 4, 5];
        let g2 = vec![9u8; 40];

        let mut s = SnapshotStore::open(&path);
        s.set_group_blobs(vec![g1.clone(), g2.clone()]);
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        assert_eq!(s2.group_blobs().len(), 2);
        assert!(s2.group_blobs().contains(&g1));
        assert!(s2.group_blobs().contains(&g2));

        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 20] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.group_blobs().len() < 2, "the corrupted group-blob record must not survive load");
    }

    /// E4-S4 acceptance (handbook §4.3 step 3): a previously-received bundle
    /// re-offered by a peer after restart must be deduped — proving the
    /// `SeenIndex` actually survived, not just that its bytes round-trip in
    /// isolation (already covered directly in `mmp-core::tests`).
    #[test]
    fn seen_index_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("seen.mmp");
        let _ = std::fs::remove_file(&path);

        let mut seen = mmp_core::SeenIndex::new();
        seen.insert(&Id16([7; 16]), class::DIRECT, 1000);

        let mut s = SnapshotStore::open(&path);
        s.set_seen_index(seen);
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        assert!(s2.seen_index().unwrap().contains(&Id16([7; 16])));

        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 10] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.seen_index().is_none(), "the corrupted SeenIndex record must not survive load");
    }

    /// Mirrors `seen_index_roundtrip_and_corruption_drop` for `ReceiptFilter`.
    #[test]
    fn receipt_filter_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("receipts.mmp");
        let _ = std::fs::remove_file(&path);

        let mut rf = mmp_core::ReceiptFilter::new();
        rf.note_verified_receipt(&Id16([8; 16]), 5000);

        let mut s = SnapshotStore::open(&path);
        s.set_receipt_filter(rf);
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        assert_eq!(
            s2.receipt_filter().unwrap().signal(&Id16([8; 16])),
            mmp_core::SuppressSignal::Purge
        );

        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 5] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.receipt_filter().is_none(), "the corrupted ReceiptFilter record must not survive load");
    }

    #[test]
    fn authority_snapshot_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("authority.mmp");
        let _ = std::fs::remove_file(&path);

        let issuer = mmp_crypto::Identity::from_seed(&[21; 32]);
        let subject = mmp_types::ids::Key32([22; 32]);
        let scope = Id16([23; 16]);
        let mut authority = mmp_core::AuthorityStore::new();
        authority.pin_root(scope, issuer.public());
        authority.subscribe(scope);
        let mut cert = mmp_types::authcert::AuthCert {
            suite: 1,
            subject,
            scope,
            caps: mmp_types::authcert::CAP_ANNOUNCE,
            priority_budget: 2,
            not_after: 10_000,
            issuer: issuer.public(),
            sig: mmp_types::ids::Sig64::ZERO,
        };
        cert.sig = issuer.sign(SigDomain::AuthCert, &cert.signed_object());
        authority.consider_cert(&cert, 0).unwrap();

        let mut s = SnapshotStore::open(&path);
        s.set_authority(authority.encode());
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        let restored = mmp_core::AuthorityStore::decode(s2.authority().unwrap()).unwrap();
        assert!(restored.is_subscribed(&scope));
        assert!(restored.grant_for(&subject, &scope, prio::OPERATIONAL, 0).is_some());

        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 40] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.authority().is_none(), "the corrupted authority record must not survive load");
    }

    #[test]
    fn blocklist_snapshot_roundtrip_and_corruption_drop() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("blocklist.mmp");
        let _ = std::fs::remove_file(&path);

        let mut blocklist = vec![BLOCKLIST_VERSION, 2, 0];
        blocklist.extend_from_slice(&[1; 16]);
        blocklist.extend_from_slice(&[2; 16]);
        let mut s = SnapshotStore::open(&path);
        s.set_blocklist(blocklist.clone());
        s.set_blocklist(vec![BLOCKLIST_VERSION, 0, 0, 0]);
        assert_eq!(s.blocklist(), Some(blocklist.as_slice()), "invalid staging must not clear valid state");
        s.persist().unwrap();

        let s2 = SnapshotStore::open(&path);
        assert_eq!(s2.blocklist(), Some(blocklist.as_slice()));

        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 40] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let s3 = SnapshotStore::open(&path);
        assert!(s3.blocklist().is_none(), "the corrupted blocklist record must not survive load");
    }
}
