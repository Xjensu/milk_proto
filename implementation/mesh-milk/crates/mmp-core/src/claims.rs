//! Claims: single-writer LWW registers (doc 06 §4).
//!
//! Higher `seq` wins; equal `seq` + different body resolves deterministically
//! (smaller BLAKE3(body)) AND raises a compromise signal (doc 07 §6.4).
//! Signature verification happens at ingest — the store only merges verified
//! claims.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mmp_types::claim::Claim;
use mmp_types::consts::LIFETIME_CLAIM_MS;
use mmp_types::frame::ClaimDigestEntry;
use mmp_types::ids::{node_id_of, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimMerge {
    /// New or newer — adopted.
    Adopted,
    /// Lower or equal seq, identical body — nothing to do.
    Stale,
    /// Equal seq, different body: deterministic winner chosen; surface a
    /// CompromiseWarning for `owner` (doc 06 §4.2). Field says if we switched.
    Conflict { adopted: bool },
}

struct Stored {
    claim: Claim,
    received_at: u64,
    /// ContactGraph peers' claims never expire (doc 06 §4.2).
    pinned: bool,
}

#[derive(Default)]
pub struct ClaimStore {
    map: BTreeMap<(NodeId, u8), Stored>,
}

impl ClaimStore {
    pub fn new() -> ClaimStore {
        ClaimStore::default()
    }

    /// Merge a signature-verified claim.
    pub fn merge(&mut self, claim: Claim, now_ms: u64, pinned: bool) -> ClaimMerge {
        let key = (node_id_of(&claim.owner), claim.field);
        match self.map.get_mut(&key) {
            None => {
                self.map.insert(key, Stored { claim, received_at: now_ms, pinned });
                ClaimMerge::Adopted
            }
            Some(cur) => {
                cur.pinned |= pinned;
                if claim.seq > cur.claim.seq {
                    cur.claim = claim;
                    cur.received_at = now_ms;
                    ClaimMerge::Adopted
                } else if claim.seq < cur.claim.seq || claim.body == cur.claim.body {
                    ClaimMerge::Stale
                } else {
                    // Equal seq, different body: smaller BLAKE3(body) wins.
                    let ours = blake3::hash(&cur.claim.body);
                    let theirs = blake3::hash(&claim.body);
                    let adopted = theirs.as_bytes() < ours.as_bytes();
                    if adopted {
                        cur.claim = claim;
                        cur.received_at = now_ms;
                    }
                    ClaimMerge::Conflict { adopted }
                }
            }
        }
    }

    pub fn get(&self, node: &NodeId, field: u8) -> Option<&Claim> {
        self.map.get(&(*node, field)).map(|s| &s.claim)
    }

    pub fn pin(&mut self, node: &NodeId) {
        for ((n, _), s) in self.map.iter_mut() {
            if n == node {
                s.pinned = true;
            }
        }
    }

    /// Claim digest for SUMMARY, scoped to the given nodes (doc 06 §4.3 —
    /// non-WoT sessions digest only the two session identities).
    pub fn digest_for(&self, nodes: &[NodeId]) -> Vec<ClaimDigestEntry> {
        let mut out = Vec::new();
        for node in nodes {
            for ((n, field), s) in self.map.range((*node, 0)..=(*node, 0xff)) {
                debug_assert_eq!(n, node);
                out.push(ClaimDigestEntry {
                    node_prefix: u32::from_le_bytes([node.0[0], node.0[1], node.0[2], node.0[3]]),
                    field: *field,
                    seq: s.claim.seq,
                });
            }
        }
        out
    }

    /// Strangers' claims expire at 7 d; pinned (contact) claims are retained.
    pub fn expire(&mut self, now_ms: u64) {
        self.map.retain(|_, s| s.pinned || s.received_at + LIFETIME_CLAIM_MS > now_ms);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Claim> {
        self.map.values().map(|s| &s.claim)
    }
}
