//! BLAKE3 Merkle tree, arity 4 (doc 08 §8): per-symbol verification against a
//! signed root BEFORE storage — pollution-immune multi-source blob transfer.

use alloc::vec::Vec;

use crate::error::CryptoError;

pub const ARITY: usize = 4;

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

/// Verify `leaf` connects to `root` via `path`.
pub fn verify_path(root: &[u8; 32], leaf: &[u8; 32], path: &MerklePath) -> Result<(), CryptoError> {
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
    if &acc == root {
        Ok(())
    } else {
        Err(CryptoError::Merkle)
    }
}

/// Blob-root claim object under `mmp1/blobroot` (doc 10 §3.1):
/// `root ‖ size ‖ k ‖ n` (rs_params folded into k/n for suite 0x01).
pub fn blobroot_object(root: &[u8; 32], size: u64, k: u8, n: u8) -> Vec<u8> {
    let mut o = Vec::with_capacity(32 + 8 + 2);
    o.extend_from_slice(root);
    o.extend_from_slice(&size.to_le_bytes());
    o.push(k);
    o.push(n);
    o
}
