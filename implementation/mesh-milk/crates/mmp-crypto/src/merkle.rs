//! BLAKE3 Merkle tree, arity 4 (doc 08 §8): per-symbol verification against a
//! signed root BEFORE storage — pollution-immune multi-source blob transfer.

use alloc::vec::Vec;

use crate::error::CryptoError;

pub const ARITY: usize = 4;

/// Maximum inclusion-path depth the wire codec accepts (E5-S1 decision D1).
/// `4^8 = 65_536 ≥ FILE_MAX_STRIPES(128) * RS_TOTAL_N(40) = 5_120` — a
/// property of the path codec (any depth an 8 MiB file's leaf count could
/// need), not a protocol constant, hence local to this module rather than
/// `mmp-types/src/consts.rs`.
pub const MERKLE_PATH_MAX_LEVELS: usize = 8;

pub fn hash_block(block: &[u8]) -> [u8; 32] {
    *blake3::hash(block).as_bytes()
}

fn hash_children(children: &[[u8; 32]]) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("mmp1 merkle node");
    for c in children {
        h.update(c);
    }
    *h.finalize().as_bytes()
}

/// Root over source-block hashes. A single leaf is its own root; an empty
/// tree is the hash of nothing (degenerate, blobs always have ≥1 block).
pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return hash_children(&[]);
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        level = level.chunks(ARITY).map(hash_children).collect();
    }
    level[0]
}

/// One level of an inclusion path: our position within the group and the
/// sibling hashes in group order (excluding ours).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLevel {
    pub pos: u8,
    pub siblings: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MerklePath {
    pub levels: Vec<PathLevel>,
}

/// Inclusion path for `index` (≤ 5 levels at avatar scales, doc 08 §8).
pub fn merkle_path(leaves: &[[u8; 32]], index: usize) -> MerklePath {
    let mut levels = Vec::new();
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    let mut idx = index;
    while level.len() > 1 {
        let group_start = (idx / ARITY) * ARITY;
        let group = &level[group_start..level.len().min(group_start + ARITY)];
        let pos = (idx - group_start) as u8;
        let siblings = group
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != pos as usize)
            .map(|(_, h)| *h)
            .collect();
        levels.push(PathLevel { pos, siblings });
        level = level.chunks(ARITY).map(hash_children).collect();
        idx /= ARITY;
    }
    MerklePath { levels }
}

/// Recompute the root `leaf` connects to via `path` (E5-S1 decision D1) —
/// `verify_path` is a thin wrapper comparing this against an expected root.
pub fn compute_root(leaf: &[u8; 32], path: &MerklePath) -> Result<[u8; 32], CryptoError> {
    let mut acc = *leaf;
    for level in &path.levels {
        if level.siblings.len() >= ARITY || usize::from(level.pos) > level.siblings.len() {
            return Err(CryptoError::Merkle);
        }
        let mut group: Vec<[u8; 32]> = Vec::with_capacity(level.siblings.len() + 1);
        group.extend_from_slice(&level.siblings[..level.pos as usize]);
        group.push(acc);
        group.extend_from_slice(&level.siblings[level.pos as usize..]);
        acc = hash_children(&group);
    }
    Ok(acc)
}

/// Verify `leaf` connects to `root` via `path`.
pub fn verify_path(root: &[u8; 32], leaf: &[u8; 32], path: &MerklePath) -> Result<(), CryptoError> {
    if &compute_root(leaf, path)? == root {
        Ok(())
    } else {
        Err(CryptoError::Merkle)
    }
}

impl MerklePath {
    /// Wire encoding (E5-S1 decision D1): `u8 depth ‖ depth × { u8 pos, u8
    /// nsib, nsib × [u8;32] siblings }`. Max size at `MERKLE_PATH_MAX_LEVELS`
    /// depth and full `ARITY-1` siblings per level: `1 + 8*(2 + 3*32) = 785` B.
    pub fn encode(&self) -> Vec<u8> {
        debug_assert!(self.levels.len() <= MERKLE_PATH_MAX_LEVELS);
        let mut out = Vec::with_capacity(1 + self.levels.len() * (2 + (ARITY - 1) * 32));
        out.push(self.levels.len() as u8);
        for level in &self.levels {
            debug_assert!(level.siblings.len() < ARITY && usize::from(level.pos) <= level.siblings.len());
            out.push(level.pos);
            out.push(level.siblings.len() as u8);
            for s in &level.siblings {
                out.extend_from_slice(s);
            }
        }
        out
    }

    /// Strict decode: rejects depth over the cap, over-wide sibling groups,
    /// an out-of-range `pos`, truncation, and trailing bytes (crate-wide
    /// canonical-decode discipline). Any rejection collapses to
    /// `CryptoError::Merkle` — a malformed path is exactly as untrusted as
    /// one that fails to verify.
    pub fn decode(bytes: &[u8]) -> Result<MerklePath, CryptoError> {
        let mut r = mmp_types::wire::Reader::new(bytes);
        let depth = r.u8().map_err(|_| CryptoError::Merkle)? as usize;
        if depth > MERKLE_PATH_MAX_LEVELS {
            return Err(CryptoError::Merkle);
        }
        let mut levels = Vec::with_capacity(depth);
        for _ in 0..depth {
            let pos = r.u8().map_err(|_| CryptoError::Merkle)?;
            let nsib = r.u8().map_err(|_| CryptoError::Merkle)? as usize;
            if nsib >= ARITY || usize::from(pos) > nsib {
                return Err(CryptoError::Merkle);
            }
            let mut siblings = Vec::with_capacity(nsib);
            for _ in 0..nsib {
                let s = r.take(32).map_err(|_| CryptoError::Merkle)?;
                let mut a = [0u8; 32];
                a.copy_from_slice(s);
                siblings.push(a);
            }
            levels.push(PathLevel { pos, siblings });
        }
        r.finish().map_err(|_| CryptoError::Merkle)?;
        Ok(MerklePath { levels })
    }

    /// The leaf index this path proves: `Σ levels[i].pos · 4^i`, `i` counted
    /// from the leaf-adjacent (first) level upward — position binding (E5-S1
    /// decision D1 check b): a valid-but-repositioned path must not verify
    /// against a `sym_no` other than the one it actually proves.
    pub fn leaf_index(&self) -> u64 {
        let mut idx: u64 = 0;
        let mut mul: u64 = 1;
        for level in &self.levels {
            idx += u64::from(level.pos) * mul;
            mul *= ARITY as u64;
        }
        idx
    }
}

/// Blob-root claim object under `mmp1/blobroot` (doc 10 §3.1):
/// `root ‖ size ‖ k ‖ n` (rs_params folded into k/n for suite 0x01).
///
/// Reserved for the public-blob (`avatar_root`, doc 06 §4.1) claim path; the
/// doc 16 file-transfer plane (E5-S1) deliberately does NOT use this — a
/// `FileManifest`'s `file_root`/`thumb_root` are AEAD-interior (doc 16 §3.1),
/// and minting a public `mmp1/blobroot` claim for one would leak exactly the
/// metadata (who shared what, when) that design exists to hide. Left dead on
/// purpose; do not wire file transfer through it.
pub fn blobroot_object(root: &[u8; 32], size: u64, k: u8, n: u8) -> Vec<u8> {
    let mut o = Vec::with_capacity(32 + 8 + 2);
    o.extend_from_slice(root);
    o.extend_from_slice(&size.to_le_bytes());
    o.push(k);
    o.push(n);
    o
}
