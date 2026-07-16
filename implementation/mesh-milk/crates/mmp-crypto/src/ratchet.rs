//! Conversation ratchet (doc 08 §5): symmetric forward ratchet per direction,
//! skipped-key window for DTN reordering, opportunistic epoch bump at contact.
//!
//! `K_i = derive_key("mmp1 msg", CK_i)`, `CK_{i+1} = derive_key("mmp1 ck", CK_i)`,
//! CK erased after advance. PCS heals when (and only when) the parties actually
//! meet — stated plainly (doc 08 §5.2).

use alloc::collections::BTreeMap;

use mmp_types::ids::Key32;
use zeroize::{Zeroize, Zeroizing};

use crate::error::CryptoError;

/// Deterministic per-direction chain seed: both parties derive the same value
/// for a given sender.
pub fn direction_chain(ck0: &[u8; 32], sender_identity: &Key32) -> Zeroizing<[u8; 32]> {
    let mut material = Zeroizing::new([0u8; 64]);
    material[..32].copy_from_slice(ck0);
    material[32..].copy_from_slice(&sender_identity.0);
    Zeroizing::new(blake3::derive_key("mmp1 conv chain", &material[..]))
}

fn next_ck(ck: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key("mmp1 ck", ck)
}

fn msg_key(ck: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key("mmp1 msg", ck)
}

/// Epoch bump at live contact (doc 08 §5.2): `CK' = derive("mmp1 epoch", CK ‖ material)`.
fn bumped(ck: &[u8; 32], session_material: &[u8; 32]) -> [u8; 32] {
    let mut input = Zeroizing::new([0u8; 64]);
    input[..32].copy_from_slice(ck);
    input[32..].copy_from_slice(session_material);
    blake3::derive_key("mmp1 epoch", &input[..])
}

/// Sending side of one direction.
pub struct SendChain {
    ck: [u8; 32],
    pub epoch: u16,
    pub next_index: u32,
}

impl SendChain {
    pub fn new(ck: [u8; 32], epoch: u16) -> SendChain {
        SendChain { ck, epoch, next_index: 0 }
    }

    /// Yields (message key, epoch, index) and advances — the spent CK is erased.
    // Not an `Iterator`: this advances a keyed ratchet state, not a sequence
    // being consumed. Renaming would change the public API surface.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> (Zeroizing<[u8; 32]>, u16, u32) {
        let key = Zeroizing::new(msg_key(&self.ck));
        let (epoch, index) = (self.epoch, self.next_index);
        let new = next_ck(&self.ck);
        self.ck.zeroize();
        self.ck = new;
        self.next_index += 1;
        (key, epoch, index)
    }

    pub fn bump_epoch(&mut self, session_material: &[u8; 32]) {
        let new = bumped(&self.ck, session_material);
        self.ck.zeroize();
        self.ck = new;
        self.epoch += 1;
        self.next_index = 0;
    }
}

/// Receiving side of one direction, tolerant of DTN reordering within `window`.
pub struct RecvChain {
    ck: [u8; 32],
    epoch: u16,
    next_index: u32,
    window: u32,
    skipped: BTreeMap<(u16, u32), [u8; 32]>,
}

impl RecvChain {
    pub fn new(ck: [u8; 32], epoch: u16, window: u32) -> RecvChain {
        RecvChain { ck, epoch, next_index: 0, window, skipped: BTreeMap::new() }
    }

    /// Message key for (epoch, index). Old-epoch or already-consumed keys only
    /// exist in the skipped store; everything else advances the chain.
    pub fn key_for(&mut self, epoch: u16, index: u32) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
        if epoch < self.epoch || (epoch == self.epoch && index < self.next_index) {
            return self
                .skipped
                .remove(&(epoch, index))
                .map(Zeroizing::new)
                .ok_or(CryptoError::Ratchet);
        }
        if epoch > self.epoch {
            // The chain only crosses epochs via bump_epoch (a live session);
            // a from-the-future epoch is undecryptable until then.
            return Err(CryptoError::Ratchet);
        }
        if index - self.next_index >= self.window {
            return Err(CryptoError::Ratchet);
        }
        while self.next_index < index {
            self.store_skipped(self.epoch, self.next_index, msg_key(&self.ck));
            let new = next_ck(&self.ck);
            self.ck.zeroize();
            self.ck = new;
            self.next_index += 1;
        }
        let key = Zeroizing::new(msg_key(&self.ck));
        let new = next_ck(&self.ck);
        self.ck.zeroize();
        self.ck = new;
        self.next_index += 1;
        Ok(key)
    }

    pub fn bump_epoch(&mut self, session_material: &[u8; 32]) {
        let new = bumped(&self.ck, session_material);
        self.ck.zeroize();
        self.ck = new;
        self.epoch += 1;
        self.next_index = 0;
    }

    pub fn epoch(&self) -> u16 {
        self.epoch
    }

    fn store_skipped(&mut self, epoch: u16, index: u32, key: [u8; 32]) {
        if self.skipped.len() >= self.window as usize {
            // Evict the oldest skipped key — a message that far behind has
            // outlived its bundle lifetime anyway (doc 08 §5.1).
            if let Some(k) = self.skipped.keys().next().copied() {
                if let Some(mut v) = self.skipped.remove(&k) {
                    v.zeroize();
                }
            }
        }
        self.skipped.insert((epoch, index), key);
    }
}

impl Drop for SendChain {
    fn drop(&mut self) {
        self.ck.zeroize();
    }
}

impl Drop for RecvChain {
    fn drop(&mut self) {
        self.ck.zeroize();
        for (_, v) in self.skipped.iter_mut() {
            v.zeroize();
        }
    }
}
