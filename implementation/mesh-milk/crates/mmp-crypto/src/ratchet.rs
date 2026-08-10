//! Conversation ratchet (doc 08 §5): symmetric forward ratchet per direction,
//! skipped-key window for DTN reordering, opportunistic epoch bump at contact.
//!
//! `K_i = derive_key("mmp1 msg", CK_i)`, `CK_{i+1} = derive_key("mmp1 ck", CK_i)`,
//! CK erased after advance. PCS heals when (and only when) the parties actually
//! meet — stated plainly (doc 08 §5.2).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mmp_types::consts::RATCHET_WINDOW;
use mmp_types::ids::Key32;
use zeroize::{Zeroize, Zeroizing};

use crate::error::CryptoError;

/// Version of the provider-local send/receive ratchet snapshots.
///
/// These bytes are not MMP wire data. They are only a bounded plaintext
/// payload for a platform-keystore envelope; callers still have to provide
/// the keystore, encryption, freshness/TTL, and atomic persistence policy.
const SNAPSHOT_VERSION: u8 = 1;
const SEND_SNAPSHOT_LEN: usize = 1 + 32 + 2 + 4;
const RECV_SNAPSHOT_HEADER_LEN: usize = 1 + 32 + 2 + 4 + 4 + 2;
const SKIPPED_RECORD_LEN: usize = 2 + 4 + 32;

fn snapshot_error() -> CryptoError {
    CryptoError::Snapshot
}

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
    /// True after the final representable message index has been emitted.
    /// This distinguishes a chain ready to emit `u32::MAX` from one that has
    /// already consumed it; the wire field remains a full `u32`.
    exhausted: bool,
    /// The current epoch (bumped by [`SendChain::bump_epoch`]).
    pub epoch: u16,
    /// The index that will be assigned to the next message key.
    pub next_index: u32,
}

impl SendChain {
    /// Start a send chain at (`ck`, `epoch`), index 0.
    pub fn new(ck: [u8; 32], epoch: u16) -> SendChain {
        SendChain { ck, exhausted: false, epoch, next_index: 0 }
    }

    /// Yields (message key, epoch, index) and advances — the spent CK is erased.
    // Not an `Iterator`: this advances a keyed ratchet state, not a sequence
    // being consumed. Renaming would change the public API surface.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> (Zeroizing<[u8; 32]>, u16, u32) {
        self.try_next().expect("ratchet send counter exhausted")
    }

    /// Fallible form of [`SendChain::next`]. The final wire index may be
    /// emitted once; any subsequent call fails closed instead of wrapping to
    /// zero and reusing the AEAD context. Existing callers retain
    /// [`SendChain::next`] for source compatibility; new provider integrations
    /// should use this method to surface the terminal state without panicking.
    pub fn try_next(&mut self) -> Result<(Zeroizing<[u8; 32]>, u16, u32), CryptoError> {
        if self.exhausted {
            return Err(CryptoError::Ratchet);
        }
        let key = Zeroizing::new(msg_key(&self.ck));
        let (epoch, index) = (self.epoch, self.next_index);
        let new = next_ck(&self.ck);
        self.ck.zeroize();
        self.ck = new;
        if index == u32::MAX {
            self.exhausted = true;
        } else {
            self.next_index = index + 1;
        }
        Ok((key, epoch, index))
    }

    /// Derive the next message key and a candidate post-send chain without
    /// mutating this chain. Producers that must persist an encrypted bundle
    /// before committing ratchet state use this checkpoint-free staging seam.
    pub fn try_next_candidate(&self) -> Result<(Zeroizing<[u8; 32]>, SendChain), CryptoError> {
        if self.exhausted {
            return Err(CryptoError::Ratchet);
        }
        let key = Zeroizing::new(msg_key(&self.ck));
        let next_ck = next_ck(&self.ck);
        let next_index = if self.next_index == u32::MAX {
            u32::MAX
        } else {
            self.next_index + 1
        };
        let candidate = SendChain {
            ck: next_ck,
            exhausted: self.next_index == u32::MAX,
            epoch: self.epoch,
            next_index,
        };
        Ok((key, candidate))
    }

    /// Bump the epoch at live contact (doc 08 §5.2): folds `session_material`
    /// into the chain key, resets the index to 0, and increments `epoch`.
    pub fn bump_epoch(&mut self, session_material: &[u8; 32]) {
        self.try_bump_epoch(session_material).expect("ratchet epoch exhausted");
    }

    /// Fallible form of [`SendChain::bump_epoch`]. Epoch exhaustion must not
    /// wrap to zero because `(epoch, index)` is part of the authenticated
    /// message context and a wrap would permit state/key reuse.
    pub fn try_bump_epoch(&mut self, session_material: &[u8; 32]) -> Result<(), CryptoError> {
        let next_epoch = self.epoch.checked_add(1).ok_or(CryptoError::Ratchet)?;
        let new = bumped(&self.ck, session_material);
        self.ck.zeroize();
        self.ck = new;
        self.epoch = next_epoch;
        self.exhausted = false;
        self.next_index = 0;
        Ok(())
    }

    /// Encode this chain as a canonical provider-local keystore payload.
    ///
    /// The result contains only the current chain key and counters. It is not
    /// encrypted here, is not a wire format, and does not by itself provide
    /// restart continuity. In particular, no conversation root is present.
    pub fn snapshot(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        if self.epoch == u16::MAX || self.next_index == u32::MAX {
            return Err(snapshot_error());
        }
        let mut out = Zeroizing::new(Vec::with_capacity(SEND_SNAPSHOT_LEN));
        out.push(SNAPSHOT_VERSION);
        out.extend_from_slice(&self.ck);
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.next_index.to_le_bytes());
        Ok(out)
    }

    /// Restore a chain from a canonical provider-local snapshot.
    ///
    /// Parsing is strict: the version and exact length must match, and
    /// counters at their wrap boundaries are rejected because the live
    /// ratchet cannot safely advance them. The input is assumed to be a
    /// decrypted keystore payload; callers must authenticate it before use.
    pub fn restore(bytes: &[u8]) -> Result<SendChain, CryptoError> {
        if bytes.len() != SEND_SNAPSHOT_LEN || bytes.first().copied() != Some(SNAPSHOT_VERSION) {
            return Err(snapshot_error());
        }
        let epoch = u16::from_le_bytes([bytes[33], bytes[34]]);
        let next_index = u32::from_le_bytes([bytes[35], bytes[36], bytes[37], bytes[38]]);
        if epoch == u16::MAX || next_index == u32::MAX {
            return Err(snapshot_error());
        }
        let mut ck = Zeroizing::new([0u8; 32]);
        ck.copy_from_slice(&bytes[1..33]);
        Ok(SendChain { ck: *ck, exhausted: false, epoch, next_index })
    }
}

/// Receiving side of one direction, tolerant of DTN reordering within `window`.
pub struct RecvChain {
    ck: [u8; 32],
    /// True after the final representable message index has been consumed.
    exhausted: bool,
    epoch: u16,
    next_index: u32,
    window: u32,
    skipped: BTreeMap<(u16, u32), [u8; 32]>,
}

impl RecvChain {
    /// Start a receive chain at (`ck`, `epoch`), index 0, tolerating up to
    /// `window` messages of reordering ahead of the expected index.
    pub fn new(ck: [u8; 32], epoch: u16, window: u32) -> RecvChain {
        RecvChain { ck, exhausted: false, epoch, next_index: 0, window, skipped: BTreeMap::new() }
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
        // Once the final representable index has been consumed, no later
        // message key exists. Already-stored skipped keys above remain valid
        // and must still be released for out-of-order delivery.
        if self.exhausted {
            return Err(CryptoError::Ratchet);
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
            self.next_index = self.next_index.checked_add(1).ok_or(CryptoError::Ratchet)?;
        }
        let key = Zeroizing::new(msg_key(&self.ck));
        let new = next_ck(&self.ck);
        self.ck.zeroize();
        self.ck = new;
        if self.next_index == u32::MAX {
            self.exhausted = true;
        } else {
            self.next_index += 1;
        }
        Ok(key)
    }

    /// Derive a message key without advancing this receive chain.
    ///
    /// This is used by authenticated encryption consumers that must validate
    /// the ciphertext before committing replay-window state.  The checks and
    /// derived key are identical to [`RecvChain::key_for`], but skipped keys
    /// and the forward chain remain untouched until the caller subsequently
    /// commits the same `(epoch, index)` with [`RecvChain::key_for`].
    pub fn peek_key_for(&self, epoch: u16, index: u32) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
        if epoch < self.epoch || (epoch == self.epoch && index < self.next_index) {
            return self
                .skipped
                .get(&(epoch, index))
                .copied()
                .map(Zeroizing::new)
                .ok_or(CryptoError::Ratchet);
        }
        if self.exhausted {
            return Err(CryptoError::Ratchet);
        }
        if epoch > self.epoch {
            return Err(CryptoError::Ratchet);
        }
        if index - self.next_index >= self.window {
            return Err(CryptoError::Ratchet);
        }

        let mut ck = Zeroizing::new(self.ck);
        let mut cursor = self.next_index;
        while cursor < index {
            ck = Zeroizing::new(next_ck(&ck));
            cursor = cursor.checked_add(1).ok_or(CryptoError::Ratchet)?;
        }
        Ok(Zeroizing::new(msg_key(&ck)))
    }

    /// Bump the epoch at live contact (doc 08 §5.2), mirroring
    /// [`SendChain::bump_epoch`] on the receive side.
    pub fn bump_epoch(&mut self, session_material: &[u8; 32]) {
        self.try_bump_epoch(session_material).expect("ratchet epoch exhausted");
    }

    /// Fallible form of [`RecvChain::bump_epoch`]. Epoch exhaustion must not
    /// wrap to zero because `(epoch, index)` is part of the authenticated
    /// message context and a wrap would permit state/key reuse.
    pub fn try_bump_epoch(&mut self, session_material: &[u8; 32]) -> Result<(), CryptoError> {
        let next_epoch = self.epoch.checked_add(1).ok_or(CryptoError::Ratchet)?;
        let new = bumped(&self.ck, session_material);
        self.ck.zeroize();
        self.ck = new;
        self.epoch = next_epoch;
        self.exhausted = false;
        self.next_index = 0;
        Ok(())
    }

    /// The current epoch.
    pub fn epoch(&self) -> u16 {
        self.epoch
    }

    /// Encode this chain, including its bounded skipped-key window, as a
    /// canonical provider-local keystore payload. The bytes are not MMP wire
    /// data and are intentionally left to the platform keystore layer for
    /// authenticated encryption and persistence policy.
    pub fn snapshot(&self) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let count = self.skipped.len();
        if self.epoch == u16::MAX
            || self.next_index == u32::MAX
            || self.window == 0
            || self.window > RATCHET_WINDOW
            || count > self.window as usize
            || count > u16::MAX as usize
        {
            return Err(snapshot_error());
        }
        let capacity = RECV_SNAPSHOT_HEADER_LEN + count * SKIPPED_RECORD_LEN;
        let mut out = Zeroizing::new(Vec::with_capacity(capacity));
        out.push(SNAPSHOT_VERSION);
        out.extend_from_slice(&self.ck);
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.next_index.to_le_bytes());
        out.extend_from_slice(&self.window.to_le_bytes());
        out.extend_from_slice(&(count as u16).to_le_bytes());
        for ((epoch, index), key) in &self.skipped {
            out.extend_from_slice(&epoch.to_le_bytes());
            out.extend_from_slice(&index.to_le_bytes());
            out.extend_from_slice(key);
        }
        Ok(out)
    }

    /// Restore a receive chain from a canonical provider-local snapshot.
    ///
    /// The parser validates every bound before allocating the skipped-key
    /// map, requires strictly sorted records, rejects duplicate keys and
    /// future/current out-of-range indexes, and consumes the entire input.
    /// The input is assumed to be a decrypted keystore payload; callers must
    /// authenticate it before use.
    pub fn restore(bytes: &[u8]) -> Result<RecvChain, CryptoError> {
        if bytes.len() < RECV_SNAPSHOT_HEADER_LEN
            || bytes.first().copied() != Some(SNAPSHOT_VERSION)
        {
            return Err(snapshot_error());
        }

        let epoch = u16::from_le_bytes([bytes[33], bytes[34]]);
        let next_index = u32::from_le_bytes([bytes[35], bytes[36], bytes[37], bytes[38]]);
        let window = u32::from_le_bytes([bytes[39], bytes[40], bytes[41], bytes[42]]);
        let count = u16::from_le_bytes([bytes[43], bytes[44]]) as usize;
        if epoch == u16::MAX
            || next_index == u32::MAX
            || window == 0
            || window > RATCHET_WINDOW
            || count > window as usize
            || count > RATCHET_WINDOW as usize
        {
            return Err(snapshot_error());
        }
        let records_len = count.checked_mul(SKIPPED_RECORD_LEN).ok_or_else(snapshot_error)?;
        let expected_len = RECV_SNAPSHOT_HEADER_LEN.checked_add(records_len).ok_or_else(snapshot_error)?;
        if bytes.len() != expected_len {
            return Err(snapshot_error());
        }

        let mut ck = Zeroizing::new([0u8; 32]);
        ck.copy_from_slice(&bytes[1..33]);
        let mut skipped = BTreeMap::new();
        let mut previous: Option<(u16, u32)> = None;
        let mut pos = RECV_SNAPSHOT_HEADER_LEN;
        for _ in 0..count {
            let entry_epoch = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]);
            let entry_index = u32::from_le_bytes([
                bytes[pos + 2],
                bytes[pos + 3],
                bytes[pos + 4],
                bytes[pos + 5],
            ]);
            let key_start = pos + 6;
            let key_end = key_start + 32;
            let key_id = (entry_epoch, entry_index);
            if entry_epoch > epoch
                || (entry_epoch == epoch && entry_index >= next_index)
                || previous.is_some_and(|old| old >= key_id)
            {
                return Err(snapshot_error());
            }
            let mut key = Zeroizing::new([0u8; 32]);
            key.copy_from_slice(&bytes[key_start..key_end]);
            skipped.insert(key_id, *key);
            previous = Some(key_id);
            pos = key_end;
        }
        // `expected_len` above and the exact loop count prove full input
        // consumption; keep this assertion local so future format edits do
        // not accidentally reintroduce a trailing-byte acceptance path.
        if pos != bytes.len() {
            return Err(snapshot_error());
        }
        Ok(RecvChain { ck: *ck, exhausted: false, epoch, next_index, window, skipped })
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
        for v in self.skipped.values_mut() {
            v.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{msg_key, RecvChain, SendChain};
    use crate::error::CryptoError;

    #[test]
    fn staged_send_candidate_does_not_mutate_the_live_chain() {
        let mut live = SendChain::new([0x31; 32], 4);
        let (staged_key, candidate) = live.try_next_candidate().unwrap();
        assert_eq!(live.next_index, 0);
        assert_eq!(live.epoch, 4);

        let (live_key, epoch, index) = live.try_next().unwrap();
        assert_eq!(*staged_key, *live_key);
        assert_eq!((epoch, index), (4, 0));
        assert_eq!(candidate.epoch, live.epoch);
        assert_eq!(candidate.next_index, live.next_index);
    }

    #[test]
    fn receive_chain_accepts_final_index_once() {
        let mut recv = RecvChain::new([0x99; 32], 0, 8);
        // Reach the final representable index without iterating through the
        // whole u32 space; this models the terminal state after index
        // `u32::MAX - 1` was consumed.
        recv.next_index = u32::MAX;
        let expected = msg_key(&recv.ck);
        assert_eq!(*recv.key_for(0, u32::MAX).unwrap(), expected);
        assert_eq!(recv.key_for(0, u32::MAX), Err(CryptoError::Ratchet));
    }

    #[test]
    fn receive_epoch_bump_reopens_exhausted_index_space() {
        let mut recv = RecvChain::new([0x12; 32], 3, 8);
        recv.next_index = u32::MAX;
        recv.key_for(3, u32::MAX).unwrap();
        assert_eq!(recv.key_for(3, u32::MAX), Err(CryptoError::Ratchet));
        recv.try_bump_epoch(&[0x42; 32]).unwrap();
        assert!(recv.key_for(4, 0).is_ok());
    }

    #[test]
    fn final_index_does_not_discard_an_older_skipped_key() {
        let mut recv = RecvChain::new([0x23; 32], 0, 8);
        recv.next_index = u32::MAX - 2;
        let skipped = msg_key(&recv.ck);
        recv.key_for(0, u32::MAX - 1).unwrap();
        recv.key_for(0, u32::MAX).unwrap();
        assert_eq!(*recv.key_for(0, u32::MAX - 2).unwrap(), skipped);
    }
}
