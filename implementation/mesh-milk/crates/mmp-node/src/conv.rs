//! Per-conversation cryptographic state: X3DH-established root + symmetric
//! ratchet per direction + the causal DAG. One `Conv` per peer identity.

use mmp_crypto::ratchet::{direction_chain, RecvChain, SendChain};
use mmp_crypto::receipt::receipt_key;
use mmp_types::consts::RATCHET_WINDOW;
use mmp_types::ids::{Id16, Key32};
use zeroize::Zeroizing;

use mmp_core::ConversationDag;

pub struct Conv {
    // Identity fields kept for diagnostics/future lookups; not read on the
    // hot path today (the conv map is already keyed by `id`).
    #[allow(dead_code)]
    pub id: Id16,
    #[allow(dead_code)]
    pub peer: Key32,
    /// Receipt MAC key derived once from CK₀. CK₀ itself is not retained after
    /// the directional chains are initialized.
    pub(crate) receipt_key: Zeroizing<[u8; 32]>,
    pub send: SendChain,
    pub recv: RecvChain,
    pub dag: ConversationDag,
}

impl Conv {
    /// Build both direction chains from a shared CK₀ (doc 08 §5). `me` is our
    /// identity key, `peer` theirs — `direction_chain` makes each side's send
    /// chain equal the other's recv chain.
    pub fn new(id: Id16, me: &Key32, peer: &Key32, ck0: [u8; 32]) -> Conv {
        let ck0 = Zeroizing::new(ck0);
        let send_seed = direction_chain(&ck0, me);
        let recv_seed = direction_chain(&ck0, peer);
        let receipt_key = receipt_key(&ck0);
        Conv {
            id,
            peer: *peer,
            receipt_key,
            send: SendChain::new(*send_seed, 0),
            recv: RecvChain::new(*recv_seed, 0, RATCHET_WINDOW),
            dag: ConversationDag::new(),
        }
    }
}

/// AEAD context-commitment input (doc 16 §4.2, CR-S2): a valid tag implies a
/// unique intended (conversation, direction, epoch, index) — and binds the
/// routing origin/dest so a re-headered ciphertext cannot decrypt (doc 10 §4.1).
pub fn msg_context(origin: &Key32, dest: &Id16, conv: &Id16, epoch: u16, index: u32) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("mmp1 msg-context");
    h.update(&origin.0);
    h.update(&dest.0);
    h.update(&conv.0);
    h.update(&epoch.to_le_bytes());
    h.update(&index.to_le_bytes());
    *h.finalize().as_bytes()
}
