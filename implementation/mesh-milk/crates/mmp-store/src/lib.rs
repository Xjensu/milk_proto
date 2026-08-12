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

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use mmp_core::store::{
    BundleMeta, BundleStore, MemStore, SlotState, StoreBatchEntry, StoreBatchError,
    StoreBatchReservation, StoreBatchReservationKey, StoreCommitError, StorePutError,
    StorePutOutcome, StoreReservation, StoreReservationKey, StoreAbortError, StoreReserveError,
};
use mmp_core::{ReceiptFilter, SeenIndex};
use mmp_crypto::verify;
use mmp_types::bundle::{class, prio};
use mmp_types::consts::{class_max_lifetime_ms, L_MAX};
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
    /// Set when opening the primary image (or its crash-recovery candidate)
    /// failed closed — for example because it exceeded an explicit host limit
    /// or returned a non-`NotFound` I/O error. Refusing a later `persist` avoids
    /// replacing an unreadable image with an empty in-memory store and silently
    /// losing state.
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
    /// Runtime-only reservation metadata. It is deliberately excluded from
    /// snapshots; callers must explicitly commit or abort after restart.
    reservations: BTreeMap<StoreReservationKey, PendingReservation>,
    /// Runtime-only metadata for atomic batch reservations. Like individual
    /// reservations, this has no snapshot representation.
    batch_reservations: BTreeMap<StoreBatchReservationKey, Vec<PendingReservation>>,
}

#[derive(Debug, Clone, Copy)]
struct PendingReservation {
    id: Id16,
    declared_size: u32,
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
    /// candidate, including when it is rejected for size or another read
    /// error. A later `persist` returns an error while that rejection is
    /// recorded, preventing silent replacement of state the host chose not to
    /// load.
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
            reservations: std::collections::BTreeMap::new(),
            batch_reservations: std::collections::BTreeMap::new(),
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
                let temp = temporary_path(&self.path);
                let temp_bytes = match read_snapshot(&temp, self.limits) {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => {
                        self.load_rejected = true;
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(_) => {
                        // A primary missing together with an unreadable
                        // candidate is not equivalent to an empty store: the
                        // candidate may contain the only recoverable image.
                        // Refuse replacement until the host repairs/removes
                        // that candidate explicitly.
                        self.load_rejected = true;
                        return;
                    }
                };
                if !validate_complete_snapshot(&temp_bytes) {
                    self.load_rejected = true;
                    return;
                }
                temp_bytes
            }
            Err(_) => {
                // A primary that exists but cannot be read is authoritative:
                // do not fall back to a candidate or later replace it with an
                // empty in-memory store. `NotFound` is the only condition that
                // enters the crash-recovery branch above.
                self.load_rejected = true;
                return;
            }
        };
        self.load_bytes(&bytes);
    }

    fn load_bytes(&mut self, bytes: &[u8]) {
        if bytes.len() < 8 || &bytes[..8] != MAGIC {
            // A primary path that exists but is not an MMPSNAP2 image is not
            // equivalent to an empty store.  In particular, this protects
            // legacy/foreign files from being silently replaced by a fresh
            // snapshot on the next persist.  A recognized MMPSNAP2 image may
            // still contain a truncated or corrupt tail: the documented
            // skip-corrupt-record policy intentionally permits compaction of
            // that tail, so this guard is deliberately limited to the magic.
            self.load_rejected = true;
            return;
        }
        // `persist` emits at most one record for each of these singleton
        // auxiliary states.  Keep the first valid instance if a damaged or
        // locally rewritten image contains duplicates: last-valid-wins would
        // let an appended stale record roll back receiver-local state.
        let mut seen_index_loaded = false;
        let mut receipt_filter_loaded = false;
        let mut authority_loaded = false;
        let mut blocklist_loaded = false;
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
                        // The persisted image is supposed to contain one
                        // record per BundleID.  If a crash, append-only repair,
                        // or local attacker supplies duplicate valid records,
                        // keep the first complete state rather than allowing
                        // a later stale metadata record to roll back expiry,
                        // budget, or handoff accounting.
                        if self.mem.meta(&meta.id).is_none() {
                            self.mem.put(meta, &body);
                        }
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
                    if seen_index_loaded {
                        continue;
                    }
                    // A malformed record is dropped exactly like a corrupt
                    // record — the node just starts with an empty SeenIndex,
                    // same safe-direction posture as never having persisted
                    // one at all.
                    if let Some(seen) = SeenIndex::decode(payload) {
                        self.seen_index = Some(seen);
                        seen_index_loaded = true;
                    }
                }
                RECORD_RECEIPT_FILTER => {
                    if receipt_filter_loaded {
                        continue;
                    }
                    if let Some(rf) = ReceiptFilter::decode(payload) {
                        self.receipt_filter = Some(rf);
                        receipt_filter_loaded = true;
                    }
                }
                // AuthorityStore::decode re-verifies certificates against the
                // restored pinned roots before this state can affect
                // admission after restart.
                RECORD_AUTHORITY => {
                    if authority_loaded {
                        continue;
                    }
                    if mmp_core::AuthorityStore::decode(payload).is_some() {
                        self.authority = Some(payload.to_vec());
                        authority_loaded = true;
                    }
                }
                RECORD_BLOCKLIST => {
                    if blocklist_loaded {
                        continue;
                    }
                    if valid_blocklist(payload) {
                        self.blocklist = Some(payload.to_vec());
                        blocklist_loaded = true;
                    }
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
                "snapshot was not loaded safely and cannot be replaced",
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
            if !valid_persisted_metadata(
                stored_meta.class,
                stored_meta.priority,
                stored_meta.budget,
                stored_meta.own_origin as u8,
                stored_meta.stored_at,
                stored_meta.expires_at,
            ) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "snapshot metadata is outside its canonical domain",
                ));
            }
            if stored_meta.size as usize != body.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle metadata size does not match stored bytes",
                ));
            }
        }
        let tmp = temporary_path(&self.path);
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

    /// Return the complete encoded snapshot size for a prepared in-memory
    /// image. `SnapshotLimits` bounds the complete snapshot rather than only
    /// bundle payload bytes, so auxiliary records are included here too.
    fn image_size_for_mem(&self, mem: &MemStore) -> Option<u64> {
        let mut total = u64::try_from(MAGIC.len()).ok()?;
        for meta in mem.all_meta() {
            let (stored_meta, stored_body) = mem.get(&meta.id)?;
            if usize::try_from(stored_meta.size).ok()? != stored_body.len() {
                return None;
            }
            total = total.checked_add(bundle_record_size(stored_body.len())?)?;
        }

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

    /// Calculate a complete snapshot image after applying one optional
    /// candidate and all still-pending runtime reservations. Reservations are
    /// counted as future bundle records so several successful preflights cannot
    /// collectively exceed `SnapshotLimits`. `skip` is the token being
    /// committed and is therefore removed from the pending set before the
    /// candidate is added. No payload bytes are allocated here.
    fn image_size_for_reservation_plan(
        &self,
        candidate: Option<&BundleMeta>,
        skip: Option<&StoreReservationKey>,
    ) -> Option<u64> {
        let mut total = u64::try_from(MAGIC.len()).ok()?;
        for stored_meta in self.mem.all_meta() {
            let replaced = candidate.is_some_and(|meta| meta.id == stored_meta.id)
                || self.reservations.iter().any(|(key, pending)| {
                    Some(key) != skip && pending.id == stored_meta.id
                })
                || self.batch_reservations.values().any(|batch| {
                    batch.iter().any(|pending| pending.id == stored_meta.id)
                });
            if replaced {
                continue;
            }
            let (_, body) = self.mem.get(&stored_meta.id)?;
            if usize::try_from(stored_meta.size).ok() != Some(body.len()) {
                return None;
            }
            total = total.checked_add(bundle_record_size(body.len())?)?;
        }
        for (key, pending) in &self.reservations {
            if Some(key) == skip || candidate.is_some_and(|meta| meta.id == pending.id) {
                continue;
            }
            total = total.checked_add(bundle_record_size(usize::try_from(pending.declared_size).ok()?)?)?;
        }
        for batch in self.batch_reservations.values() {
            for pending in batch {
                total = total.checked_add(bundle_record_size(
                    usize::try_from(pending.declared_size).ok()?,
                )?)?;
            }
        }
        if let Some(meta) = candidate {
            total = total.checked_add(
                bundle_record_size(usize::try_from(meta.size).ok()?)?,
            )?;
        }

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

    fn image_size_for_batch_reservation_plan(
        &self,
        candidates: &[BundleMeta],
        skip: Option<&StoreBatchReservationKey>,
    ) -> Option<u64> {
        let mut candidate_ids = BTreeSet::new();
        for meta in candidates {
            if !candidate_ids.insert(meta.id) {
                return None;
            }
        }
        let mut total = u64::try_from(MAGIC.len()).ok()?;
        for stored_meta in self.mem.all_meta() {
            let replaced = candidate_ids.contains(&stored_meta.id)
                || self.reservations.values().any(|pending| pending.id == stored_meta.id)
                || self.batch_reservations.iter().any(|(key, batch)| {
                    Some(key) != skip && batch.iter().any(|pending| pending.id == stored_meta.id)
                });
            if !replaced {
                let (_, body) = self.mem.get(&stored_meta.id)?;
                if usize::try_from(stored_meta.size).ok()? != body.len() {
                    return None;
                }
                total = total.checked_add(bundle_record_size(body.len())?)?;
            }
        }
        for pending in self.reservations.values() {
            total = total.checked_add(bundle_record_size(
                usize::try_from(pending.declared_size).ok()?,
            )?)?;
        }
        for (key, batch) in &self.batch_reservations {
            if Some(key) == skip {
                continue;
            }
            for pending in batch {
                total = total.checked_add(bundle_record_size(
                    usize::try_from(pending.declared_size).ok()?,
                )?)?;
            }
        }
        for meta in candidates {
            total = total.checked_add(bundle_record_size(usize::try_from(meta.size).ok()?)?)?;
        }
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

    /// Whether opening the existing image failed closed before parsing. A
    /// rejected store cannot be persisted until the host chooses a different
    /// path or repairs/removes the unreadable image.
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

/// Derive the crash-recovery sibling used for atomic snapshot replacement.
///
/// Keep the historical `<stem>.tmp` name for ordinary snapshot paths so
/// existing recovery candidates remain usable.  A caller may nevertheless
/// choose a primary path that already ends in `.tmp`; `Path::with_extension`
/// would then return the primary itself, allowing a failed persist to truncate
/// or remove the last complete image.  Use a distinct suffix in that one case.
fn temporary_path(path: &Path) -> PathBuf {
    let candidate = path.with_extension("tmp");
    #[cfg(windows)]
    let collision = candidate.as_path() == path
        || path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"));
    #[cfg(not(windows))]
    let collision = candidate.as_path() == path;
    if !collision {
        return candidate;
    }

    let mut fallback = path.as_os_str().to_os_string();
    fallback.push(".part");
    PathBuf::from(fallback)
}

/// Validate metadata fields that are persisted outside the signed bundle.
///
/// The snapshot checksum is an integrity check for accidental corruption, not
/// an authenticity check: a local attacker who can rewrite the file can also
/// recompute BLAKE3.  These fields therefore need their own canonical-domain
/// checks before they influence routing, eviction, or lifetime decisions.
fn valid_persisted_metadata(
    class_byte: u8,
    priority: u8,
    budget: u8,
    own_origin: u8,
    stored_at: u64,
    expires_at: u64,
) -> bool {
    // The low nibble is the complete wire class domain.  Classes 6..=15 are
    // intentionally reserved for forward-compatible store-and-forward: a v1
    // node must retain and relay a signed class it does not interpret.
    if class_byte > 0x0f || priority > prio::BULK || budget > L_MAX
        || own_origin > 1
    {
        return false;
    }

    let Some(lifetime) = expires_at.checked_sub(stored_at) else { return false };
    lifetime <= class_max_lifetime_ms(class_byte)
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
    let size = u32le(50)?;
    let stored_at = u64le(54)?;
    let expires_at = u64le(62)?;
    let budget = *r.get(70)?;
    let own_origin = *r.get(71)?;
    if !valid_persisted_metadata(class_byte, priority, budget, own_origin, stored_at, expires_at) {
        return None;
    }
    // Snapshot bytes are local input, not an authenticated wire bundle. Keep
    // restored metadata inside the same priority domain enforced by the
    // canonical bundle decoder.
    if priority > prio::BULK {
        return None;
    }
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
        size,
        stored_at,
        expires_at,
        budget,
        own_origin: own_origin != 0,
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
        // A checked write has no reservation-aware accounting here. Refuse it
        // while any runtime capability is outstanding so it cannot consume
        // bytes/image space promised to a later single or atomic batch commit.
        if !self.reservations.is_empty() || !self.batch_reservations.is_empty() {
            return Err(StorePutError::Capacity);
        }
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
    fn try_put_batch(
        &mut self,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreBatchError> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        // Replacing the working image would discard MemStore's runtime-only
        // reservation table. Require callers to commit or abort those tokens
        // first rather than silently making them unusable.
        if !self.reservations.is_empty() || !self.batch_reservations.is_empty() {
            return Err(StoreBatchError::Invalid);
        }
        // Match the legacy bounded write's fail-closed behavior: an image
        // that was not safely loaded cannot be replaced by a new working set.
        if self.load_rejected {
            return Err(StoreBatchError::Capacity);
        }

        // Validate the aggregate request before allocating a staged image.
        // A duplicate ID or declared-length mismatch must never leave an
        // earlier batch entry visible.
        let mut ids = BTreeSet::new();
        for entry in entries {
            let byte_len = u64::try_from(entry.bytes.len())
                .map_err(|_| StoreBatchError::Capacity)?;
            if byte_len != u64::from(entry.meta.size) || !ids.insert(entry.meta.id) {
                return Err(StoreBatchError::Invalid);
            }
        }

        // Build a private image of the current working set. The provider's
        // batch implementation then validates and applies the complete
        // request to that image; the live image is published only after the
        // optional complete-snapshot limit check succeeds.
        let mut staged = MemStore::new();
        let mut existing_bytes = 0u64;
        for meta in self.mem.all_meta() {
            let (stored_meta, stored_body) = self
                .mem
                .get(&meta.id)
                .ok_or(StoreBatchError::Invalid)?;
            if usize::try_from(stored_meta.size).ok() != Some(stored_body.len()) {
                return Err(StoreBatchError::Invalid);
            }
            existing_bytes = existing_bytes
                .checked_add(
                    u64::try_from(stored_body.len())
                        .map_err(|_| StoreBatchError::Capacity)?,
                )
                .ok_or(StoreBatchError::Capacity)?;
            staged.put(stored_meta, &stored_body);
        }
        if existing_bytes != self.mem.total_bytes() {
            return Err(StoreBatchError::Invalid);
        }

        let outcomes = staged.try_put_batch(entries)?;
        if let Some(limits) = self.limits {
            let image_size = self
                .image_size_for_mem(&staged)
                .ok_or(StoreBatchError::Capacity)?;
            if image_size > limits.max_file_bytes() {
                return Err(StoreBatchError::Capacity);
            }
        }

        self.mem = staged;
        Ok(outcomes)
    }
    fn try_reserve(&mut self, meta: BundleMeta) -> Result<StoreReservation, StoreReserveError> {
        if self.load_rejected {
            return Err(StoreReserveError::Capacity);
        }
        if let Some(limits) = self.limits {
            let image_size = self
                .image_size_for_reservation_plan(Some(&meta), None)
                .ok_or(StoreReserveError::Invalid)?;
            if image_size > limits.max_file_bytes() {
                return Err(StoreReserveError::Capacity);
            }
        }
        let reservation = self.mem.try_reserve(meta)?;
        self.reservations.insert(
            reservation.key(),
            PendingReservation {
                id: meta.id,
                declared_size: meta.size,
            },
        );
        Ok(reservation)
    }
    fn try_reserve_batch(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<Vec<StoreReservation>, StoreReserveError> {
        if self.load_rejected {
            return Err(StoreReserveError::Capacity);
        }
        if metas.is_empty() {
            return Err(StoreReserveError::Invalid);
        }
        let mut ids = BTreeSet::new();
        for meta in metas {
            if !ids.insert(meta.id)
                || self.reservations.values().any(|pending| pending.id == meta.id)
                || self.batch_reservations.values().any(|batch| {
                    batch.iter().any(|pending| pending.id == meta.id)
                })
            {
                return Err(StoreReserveError::Invalid);
            }
        }
        if let Some(limits) = self.limits {
            // Reserve the aggregate plan in one image calculation. The
            // MemStore call below performs the corresponding byte/slot
            // accounting and rejects the whole request before mutation.
            let mut total = self
                .image_size_for_reservation_plan(None, None)
                .ok_or(StoreReserveError::Invalid)?;
            for meta in metas {
                let delta = bundle_record_size(
                    usize::try_from(meta.size).map_err(|_| StoreReserveError::Capacity)?,
                )
                .ok_or(StoreReserveError::Capacity)?;
                let old = match self.mem.get(&meta.id) {
                    Some((_, body)) => {
                        bundle_record_size(body.len()).ok_or(StoreReserveError::Capacity)?
                    }
                    None => 0,
                };
                total = total
                    .checked_sub(old)
                    .and_then(|value| value.checked_add(delta))
                    .ok_or(StoreReserveError::Capacity)?;
            }
            if total > limits.max_file_bytes() {
                return Err(StoreReserveError::Capacity);
            }
        }
        let reservations = self.mem.try_reserve_batch(metas)?;
        for (reservation, meta) in reservations.iter().zip(metas) {
            self.reservations.insert(
                reservation.key(),
                PendingReservation {
                    id: meta.id,
                    declared_size: meta.size,
                },
            );
        }
        Ok(reservations)
    }
    fn try_reserve_batch_atomic(
        &mut self,
        metas: &[BundleMeta],
    ) -> Result<StoreBatchReservation, StoreReserveError> {
        if self.load_rejected {
            return Err(StoreReserveError::Capacity);
        }
        if metas.is_empty() {
            return Err(StoreReserveError::Invalid);
        }
        let mut ids = BTreeSet::new();
        for meta in metas {
            if !ids.insert(meta.id)
                || self.reservations.values().any(|pending| pending.id == meta.id)
                || self.batch_reservations.values().any(|batch| {
                    batch.iter().any(|pending| pending.id == meta.id)
                })
            {
                return Err(StoreReserveError::Invalid);
            }
        }
        if let Some(limits) = self.limits {
            let image_size = self
                .image_size_for_batch_reservation_plan(metas, None)
                .ok_or(StoreReserveError::Capacity)?;
            if image_size > limits.max_file_bytes() {
                return Err(StoreReserveError::Capacity);
            }
        }
        let batch = self.mem.try_reserve_batch_atomic(metas)?;
        self.batch_reservations.insert(
            batch.key(),
            metas
                .iter()
                .map(|meta| PendingReservation {
                    id: meta.id,
                    declared_size: meta.size,
                })
                .collect(),
        );
        Ok(batch)
    }
    fn commit_reserved(
        &mut self,
        reservation: &StoreReservation,
        meta: BundleMeta,
        bytes: &[u8],
    ) -> Result<StorePutOutcome, StoreCommitError> {
        let key = reservation.key();
        let pending = self
            .reservations
            .get(&key)
            .copied()
            .ok_or(StoreCommitError::Invalid)?;
        if pending.id != meta.id
            || pending.declared_size != meta.size
            || usize::try_from(meta.size).ok() != Some(bytes.len())
        {
            return Err(StoreCommitError::Invalid);
        }
        if let Some(limits) = self.limits {
            let image_size = self
                .image_size_for_reservation_plan(Some(&meta), Some(&key))
                .ok_or(StoreCommitError::Conflict)?;
            if image_size > limits.max_file_bytes() {
                return Err(StoreCommitError::Capacity);
            }
        }
        let outcome = self.mem.commit_reserved(reservation, meta, bytes)?;
        self.reservations.remove(&key);
        Ok(outcome)
    }
    fn commit_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
        entries: &[StoreBatchEntry<'_>],
    ) -> Result<Vec<StorePutOutcome>, StoreCommitError> {
        let key = batch.key();
        let pending = self
            .batch_reservations
            .get(&key)
            .ok_or(StoreCommitError::Invalid)?;
        if pending.len() != entries.len()
            || pending.iter().zip(entries).any(|(pending, entry)| {
                pending.id != entry.meta.id
                    || pending.declared_size != entry.meta.size
                    || usize::try_from(entry.meta.size).ok() != Some(entry.bytes.len())
            })
        {
            return Err(StoreCommitError::Invalid);
        }
        if let Some(limits) = self.limits {
            let metas: Vec<BundleMeta> = entries.iter().map(|entry| entry.meta).collect();
            let image_size = self
                .image_size_for_batch_reservation_plan(&metas, Some(&key))
                .ok_or(StoreCommitError::Conflict)?;
            if image_size > limits.max_file_bytes() {
                return Err(StoreCommitError::Capacity);
            }
        }
        let outcomes = self.mem.commit_reserved_batch(batch, entries)?;
        self.batch_reservations.remove(&key);
        Ok(outcomes)
    }
    fn abort_reservation(
        &mut self,
        reservation: &StoreReservation,
    ) -> Result<(), StoreAbortError> {
        let key = reservation.key();
        let result = self.mem.abort_reservation(reservation);
        if result.is_ok() {
            self.reservations.remove(&key);
        }
        result
    }
    fn abort_reserved_batch(
        &mut self,
        batch: &StoreBatchReservation,
    ) -> Result<(), StoreAbortError> {
        let key = batch.key();
        let result = self.mem.abort_reserved_batch(batch);
        if result.is_ok() {
            self.batch_reservations.remove(&key);
        }
        result
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
    fn for_each_meta(&self, visitor: &mut dyn FnMut(BundleMeta)) {
        self.mem.for_each_meta(visitor)
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

    fn batch_meta(id: u8, size: usize) -> BundleMeta {
        BundleMeta {
            id: Id16([id; 16]),
            class: class::DIRECT,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: u32::try_from(size).unwrap(),
            stored_at: 10,
            expires_at: 1000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        }
    }

    #[test]
    fn batch_insert_and_replace_publishes_all_outcomes() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-batch-replace-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let mut store = SnapshotStore::open(&path);
        store.try_put(batch_meta(1, 3), b"old").unwrap();
        let replacement = batch_meta(1, 3);
        let inserted = batch_meta(2, 3);
        let entries = [
            StoreBatchEntry::new(replacement, b"new"),
            StoreBatchEntry::new(inserted, b"add"),
        ];

        assert_eq!(
            store.try_put_batch(&entries).unwrap(),
            vec![StorePutOutcome::Replaced, StorePutOutcome::Inserted]
        );
        assert_eq!(store.get(&Id16([1; 16])).unwrap().1, b"new");
        assert_eq!(store.get(&Id16([2; 16])).unwrap().1, b"add");
        assert_eq!(store.total_bytes(), 6);

        // The batch seam is working-set atomic; persistence is still the
        // caller's separate operation. Verify that an explicitly persisted
        // complete batch reopens as a complete image.
        store.persist().unwrap();
        let reopened = SnapshotStore::open(&path);
        assert_eq!(reopened.get(&Id16([1; 16])).unwrap().1, b"new");
        assert_eq!(reopened.get(&Id16([2; 16])).unwrap().1, b"add");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_batch_duplicate_or_size_has_no_prefix_mutation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-batch-invalid-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let mut store = SnapshotStore::open(&path);
        store.try_put(batch_meta(1, 3), b"old").unwrap();
        let duplicate_entries = [
            StoreBatchEntry::new(batch_meta(2, 3), b"new"),
            StoreBatchEntry::new(batch_meta(2, 3), b"dup"),
        ];
        assert_eq!(
            store.try_put_batch(&duplicate_entries),
            Err(StoreBatchError::Invalid)
        );
        assert!(store.meta(&Id16([2; 16])).is_none());
        assert_eq!(store.get(&Id16([1; 16])).unwrap().1, b"old");

        let size_mismatch = [StoreBatchEntry::new(batch_meta(3, 4), b"bad")];
        assert_eq!(
            store.try_put_batch(&size_mismatch),
            Err(StoreBatchError::Invalid)
        );
        assert!(store.meta(&Id16([3; 16])).is_none());
        assert_eq!(store.total_bytes(), 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn batch_image_limit_rejection_has_no_mutation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-batch-limit-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let mut writer = SnapshotStore::open(&path);
        writer.try_put(batch_meta(1, 3), b"old").unwrap();
        writer.persist().unwrap();
        let limit = std::fs::metadata(&path).unwrap().len();

        let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(limit));
        let entries = [StoreBatchEntry::new(batch_meta(2, 3), b"new")];
        assert_eq!(
            bounded.try_put_batch(&entries),
            Err(StoreBatchError::Capacity)
        );
        assert!(bounded.meta(&Id16([2; 16])).is_none());
        assert_eq!(bounded.get(&Id16([1; 16])).unwrap().1, b"old");
        assert_eq!(bounded.total_bytes(), 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn batch_limit_accounts_replacement_delta_and_insert_as_one_image() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-batch-exact-fit-{}.mmp",
            std::process::id()
        ));
        let probe_path = std::env::temp_dir().join(format!(
            "mmp-store-batch-exact-fit-probe-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&probe_path);

        let old = batch_meta(1, 3);
        let replacement = StoreBatchEntry::new(old, b"new");
        let inserted = StoreBatchEntry::new(batch_meta(2, 3), b"add");
        let entries = [replacement, inserted];

        let mut writer = SnapshotStore::open(&path);
        writer.try_put(old, b"old").unwrap();
        writer.persist().unwrap();

        // Derive the exact complete-image bound from the same replacement +
        // insertion, rather than guessing record framing overhead.
        let mut probe = SnapshotStore::open(&probe_path);
        probe.try_put(old, b"old").unwrap();
        probe.try_put_batch(&entries).unwrap();
        probe.persist().unwrap();
        let exact_limit = std::fs::metadata(&probe_path).unwrap().len();

        let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(exact_limit));
        assert_eq!(bounded.try_put_batch(&entries).unwrap().len(), 2);
        assert_eq!(bounded.get(&Id16([1; 16])).unwrap().1, b"new");
        assert_eq!(bounded.get(&Id16([2; 16])).unwrap().1, b"add");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(probe_path);
    }

    #[test]
    fn batch_load_rejected_fails_closed_without_mutation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-batch-rejected-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();

        let mut store = SnapshotStore::open(&path);
        assert!(store.load_rejected());
        let entries = [StoreBatchEntry::new(batch_meta(1, 3), b"new")];
        assert_eq!(
            store.try_put_batch(&entries),
            Err(StoreBatchError::Capacity)
        );
        assert!(store.meta(&Id16([1; 16])).is_none());
        assert_eq!(store.total_bytes(), 0);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn reservation_honors_complete_image_limit_and_replacement_delta() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-reservation-limit-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let old = batch_meta(1, 3);
        let mut writer = SnapshotStore::open(&path);
        writer.try_put(old, b"old").unwrap();
        writer.persist().unwrap();
        let exact_limit = std::fs::metadata(&path).unwrap().len();

        let mut bounded =
            SnapshotStore::open_with_limits(&path, SnapshotLimits::new(exact_limit));
        let replacement = batch_meta(1, 3);
        let token = bounded.try_reserve(replacement).unwrap();
        // A second insert would exceed the complete image even though the
        // current payload quota still has room; reservations account framing.
        assert_eq!(
            bounded.try_reserve(batch_meta(2, 1)),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(
            bounded.commit_reserved(&token, replacement, b"new"),
            Ok(StorePutOutcome::Replaced)
        );
        assert_eq!(bounded.get(&Id16([1; 16])).unwrap().1, b"new");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reservation_load_rejected_fails_closed_without_mutation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-reservation-rejected-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();

        let mut store = SnapshotStore::open(&path);
        assert!(store.load_rejected());
        let result = store.try_reserve(batch_meta(1, 3));
        assert_eq!(result, Err(StoreReserveError::Capacity));
        assert_eq!(store.try_reserve_batch(&[batch_meta(2, 3)]), Err(StoreReserveError::Capacity));
        assert_eq!(store.total_bytes(), 0);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn atomic_batch_reservation_rejects_complete_image_over_limit_without_prefix() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-atomic-batch-limit-{}.mmp",
            std::process::id()
        ));
        let probe_path = std::env::temp_dir().join(format!(
            "mmp-store-atomic-batch-limit-probe-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&probe_path);

        let metas = [batch_meta(31, 2), batch_meta(32, 2)];
        let entries = [
            StoreBatchEntry::new(metas[0], b"ab"),
            StoreBatchEntry::new(metas[1], b"cd"),
        ];
        let mut probe = SnapshotStore::open(&probe_path);
        let batch = probe.try_reserve_batch_atomic(&metas).unwrap();
        probe.commit_reserved_batch(&batch, &entries).unwrap();
        probe.persist().unwrap();
        let too_small = std::fs::metadata(&probe_path).unwrap().len() - 1;

        let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(too_small));
        assert_eq!(
            bounded.try_reserve_batch_atomic(&metas),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(bounded.total_bytes(), 0);
        assert!(bounded.meta(&metas[0].id).is_none());
        assert!(bounded.meta(&metas[1].id).is_none());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&probe_path);
    }

    #[test]
    fn atomic_batch_reservation_load_rejected_fails_closed_without_mutation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-atomic-batch-rejected-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();

        let mut store = SnapshotStore::open(&path);
        assert!(store.load_rejected());
        assert_eq!(
            store.try_reserve_batch_atomic(&[batch_meta(33, 2)]),
            Err(StoreReserveError::Capacity)
        );
        assert_eq!(store.total_bytes(), 0);

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn checked_write_cannot_bypass_an_outstanding_atomic_batch_reservation() {
        let path = std::env::temp_dir().join(format!(
            "mmp-store-atomic-batch-checked-write-{}.mmp",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut store = SnapshotStore::open(&path);
        let reserved = batch_meta(35, 2);
        let batch = store.try_reserve_batch_atomic(&[reserved]).unwrap();
        assert_eq!(
            store.try_put(batch_meta(36, 2), b"no"),
            Err(StorePutError::Capacity)
        );
        let entries = [StoreBatchEntry::new(reserved, b"ok")];
        assert_eq!(
            store.commit_reserved_batch(&batch, &entries),
            Ok(vec![StorePutOutcome::Inserted])
        );
        assert_eq!(store.get(&reserved.id).unwrap().1, b"ok");

        let _ = std::fs::remove_file(path);
    }

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
    fn decode_record_rejects_unrepresentable_priority() {
        let meta = BundleMeta {
            id: Id16([4; 16]),
            class: class::DIRECT,
            origin: Id16([5; 16]),
            dest: Id16([6; 16]),
            priority: prio::BULK + 1,
            effective_priority: prio::BULK + 1,
            size: 0,
            stored_at: 10,
            expires_at: 1000,
            budget: 4,
            own_origin: true,
            slot: SlotState::Stored,
        };
        let encoded = encode_record(&meta, &[]);
        assert!(decode_record(&encoded).is_none());
    }

    #[test]
    fn decode_record_rejects_noncanonical_persisted_metadata() {
        let meta = BundleMeta {
            id: Id16([7; 16]),
            class: class::DIRECT,
            origin: Id16([8; 16]),
            dest: Id16([9; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 0,
            stored_at: 10,
            expires_at: 1000,
            budget: L_MAX,
            own_origin: false,
            slot: SlotState::Stored,
        };

        let mut unknown_class = encode_record(&meta, &[]);
        unknown_class[16] = 6;
        assert!(decode_record(&unknown_class).is_some(), "unknown wire classes remain relayable");

        let mut unrepresentable_class = encode_record(&meta, &[]);
        unrepresentable_class[16] = 0x10;
        assert!(decode_record(&unrepresentable_class).is_none(), "class bytes above the low nibble are invalid");

        let mut oversized_budget = encode_record(&meta, &[]);
        oversized_budget[70] = L_MAX.saturating_add(1);
        assert!(decode_record(&oversized_budget).is_none(), "restored spray budget must stay within L_MAX");

        let mut noncanonical_bool = encode_record(&meta, &[]);
        noncanonical_bool[71] = 2;
        assert!(decode_record(&noncanonical_bool).is_none(), "boolean metadata must have one wire encoding");

        let mut reversed_time = encode_record(&meta, &[]);
        reversed_time[62..70].copy_from_slice(&9u64.to_le_bytes());
        assert!(decode_record(&reversed_time).is_none(), "expiry before storage must be rejected");

        let mut overlong_time = encode_record(&meta, &[]);
        let overlong = 10u64 + class_max_lifetime_ms(class::DIRECT) + 1;
        overlong_time[62..70].copy_from_slice(&overlong.to_le_bytes());
        assert!(decode_record(&overlong_time).is_none(), "persisted lifetime must respect class maximum");
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

        let mut invalid_priority = valid;
        invalid_priority.priority = prio::BULK.saturating_add(1);
        store.update_meta(invalid_priority);
        let error = store.persist().expect_err("out-of-domain priority must not be persisted");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&path).unwrap(), primary);
        assert!(!tmp.exists(), "priority validation must happen before creating a replacement image");

        let restored = SnapshotStore::open(&path);
        assert!(restored.get(&valid.id).is_some(), "the previous complete image remains recoverable");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn persist_rejects_noncanonical_metadata_without_replacing_primary() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("noncanonical-meta.mmp");
        let tmp = path.with_extension("tmp");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&tmp);

        let mut store = SnapshotStore::open(&path);
        let valid = BundleMeta {
            id: Id16([10; 16]),
            class: class::DIRECT,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 5,
            stored_at: 10,
            expires_at: 1000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };
        store.put(valid, b"hello");
        store.persist().unwrap();
        let primary = std::fs::read(&path).unwrap();

        let mut invalid = valid;
        invalid.budget = L_MAX.saturating_add(1);
        store.update_meta(invalid);
        let error = store.persist().expect_err("noncanonical metadata must not be persisted");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&path).unwrap(), primary);
        assert!(!tmp.exists(), "validation must happen before creating a replacement image");

        let restored = SnapshotStore::open(&path);
        assert!(restored.get(&valid.id).is_some(), "the previous complete image remains recoverable");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn tmp_suffix_primary_uses_distinct_recovery_path_on_failed_persist() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snapshot.tmp");
        let temp = temporary_path(&path);
        assert_ne!(temp, path, "a .tmp primary must never alias its recovery path");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&temp);

        let meta = BundleMeta {
            id: Id16([11; 16]),
            class: class::DIRECT,
            origin: Id16([2; 16]),
            dest: Id16([3; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 5,
            stored_at: 10,
            expires_at: 1000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };

        let mut writer = SnapshotStore::open(&path);
        writer.put(meta, b"hello");
        writer.persist().unwrap();
        let primary = std::fs::read(&path).unwrap();

        // Load the complete image at its exact limit, then add a second bundle
        // so the replacement image is rejected after the temporary file has
        // been written.  A colliding temp path would delete the primary here.
        let mut bounded = SnapshotStore::open_with_limits(&path, SnapshotLimits::new(primary.len() as u64));
        let mut second = meta;
        second.id = Id16([12; 16]);
        bounded.put(second, b"world");
        let error = bounded.persist().expect_err("the expanded image must exceed its complete-file limit");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&path).unwrap(), primary, "failed replacement must preserve the primary");
        assert!(!temp.exists(), "the failed temporary candidate must be removed");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp);
    }

    #[cfg(windows)]
    #[test]
    fn case_insensitive_tmp_suffix_primary_uses_distinct_recovery_path() {
        let path = PathBuf::from("snapshot.TMP");
        assert_eq!(path.with_extension("tmp"), PathBuf::from("snapshot.tmp"));
        assert_ne!(temporary_path(&path), path.with_extension("tmp"));
    }

    #[test]
    fn primary_read_error_fails_closed_before_persist() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("read-error-directory");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();

        let store = SnapshotStore::open(&path);
        assert!(store.load_rejected(), "a non-file primary must not load as an empty store");
        let error = store.persist().expect_err("an unreadable primary must not be replaced");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(path.is_dir(), "the original primary path remains intact");

        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn unknown_primary_format_fails_closed_before_persist() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("unknown-primary-format.mmp");
        let temp = temporary_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&temp);

        // A legacy/foreign image must not be mistaken for an empty snapshot.
        // Persisting an empty working set here would silently destroy the only
        // copy of data the host may still be able to migrate or inspect.
        let original = b"MMPSNAP1\x00legacy";
        std::fs::write(&path, original).unwrap();

        let store = SnapshotStore::open(&path);
        assert!(store.load_rejected(), "an unknown primary format must be rejected");
        let error = store.persist().expect_err("unknown primary must not be replaced");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read(&path).unwrap(), original);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn unreadable_temp_candidate_fails_closed_when_primary_is_missing() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("missing-primary.mmp");
        let temp = temporary_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&temp);
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir(&temp).unwrap();

        let store = SnapshotStore::open(&path);
        assert!(store.load_rejected(), "an unreadable recovery candidate must block replacement");
        let error = store.persist().expect_err("the missing primary must not mask an unreadable candidate");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(temp.is_dir(), "the recovery candidate remains available for host repair");
        assert!(!path.exists(), "no new primary is created while recovery is ambiguous");

        let _ = std::fs::remove_dir_all(temp);
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

        let candidate_before_persist = std::fs::read(&temp).unwrap();
        let error = empty
            .persist()
            .expect_err("an invalid recovery candidate must block replacement");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!path.exists(), "ambiguous recovery must not create a primary");
        assert_eq!(
            std::fs::read(&temp).unwrap(),
            candidate_before_persist,
            "the candidate must remain available for host repair"
        );

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

    #[test]
    fn duplicate_singleton_records_keep_first_valid_state() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("duplicate-singletons.mmp");
        let _ = std::fs::remove_file(&path);

        let mut first_seen = SeenIndex::new();
        first_seen.insert(&Id16([41; 16]), class::DIRECT, 1000);
        let mut second_seen = SeenIndex::new();
        second_seen.insert(&Id16([42; 16]), class::DIRECT, 1000);

        let mut first_receipts = ReceiptFilter::new();
        first_receipts.note_verified_receipt(&Id16([43; 16]), 5000);
        let mut second_receipts = ReceiptFilter::new();
        second_receipts.note_verified_receipt(&Id16([44; 16]), 5000);

        let first_authority = mmp_core::AuthorityStore::new().encode();
        let mut second_authority_state = mmp_core::AuthorityStore::new();
        second_authority_state.subscribe(Id16([45; 16]));
        let second_authority = second_authority_state.encode();

        let first_blocklist = {
            let mut bytes = vec![BLOCKLIST_VERSION, 1, 0];
            bytes.extend_from_slice(&[46; 16]);
            bytes
        };
        let second_blocklist = {
            let mut bytes = vec![BLOCKLIST_VERSION, 1, 0];
            bytes.extend_from_slice(&[47; 16]);
            bytes
        };

        let mut store = SnapshotStore::open(&path);
        store.set_seen_index(first_seen.clone());
        store.set_receipt_filter(first_receipts.clone());
        store.set_authority(first_authority.clone());
        store.set_blocklist(first_blocklist.clone());
        store.persist().unwrap();

        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        let mut append_record = |kind: u8, payload: &[u8]| {
            let mut record = vec![kind];
            record.extend_from_slice(payload);
            write_record(&mut file, &record).unwrap();
        };
        append_record(RECORD_SEEN_INDEX, &second_seen.encode());
        append_record(RECORD_RECEIPT_FILTER, &second_receipts.encode());
        append_record(RECORD_AUTHORITY, &second_authority);
        append_record(RECORD_BLOCKLIST, &second_blocklist);
        drop(file);

        let restored = SnapshotStore::open(&path);
        assert_eq!(restored.seen_index().unwrap().encode(), first_seen.encode());
        assert_eq!(restored.receipt_filter().unwrap().encode(), first_receipts.encode());
        assert_eq!(restored.authority(), Some(first_authority.as_slice()));
        assert_eq!(restored.blocklist(), Some(first_blocklist.as_slice()));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn duplicate_bundle_records_keep_first_valid_metadata() {
        let dir = std::env::temp_dir().join("mmp-store-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("duplicate-bundles.mmp");
        let _ = std::fs::remove_file(&path);

        let id = Id16([51; 16]);
        let first = BundleMeta {
            id,
            class: class::DIRECT,
            origin: Id16([52; 16]),
            dest: Id16([53; 16]),
            priority: prio::NORMAL,
            effective_priority: prio::NORMAL,
            size: 5,
            stored_at: 10,
            expires_at: 1000,
            budget: 1,
            own_origin: false,
            slot: SlotState::Stored,
        };
        let mut stale = first;
        stale.stored_at = 20;
        stale.expires_at = 2000;
        stale.budget = 32;

        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(MAGIC).unwrap();
        for meta in [first, stale] {
            let mut record = vec![RECORD_BUNDLE];
            record.extend_from_slice(&encode_record(&meta, b"hello"));
            write_record(&mut file, &record).unwrap();
        }
        drop(file);

        let restored = SnapshotStore::open(&path);
        let meta = restored.meta(&id).expect("duplicate bundle was loaded");
        assert_eq!(meta.stored_at, first.stored_at);
        assert_eq!(meta.expires_at, first.expires_at);
        assert_eq!(meta.budget, first.budget);

        let _ = std::fs::remove_file(path);
    }
}
