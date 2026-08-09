//! Session handshake: Noise XX + ID_PROOF channel binding (doc 08 §3).
//!
//! Pattern is the *standard, formally analyzed* `Noise_XX_25519_ChaChaPoly_BLAKE2s`
//! via `snow` — BLAKE3 is NOT substituted inside Noise (doc 08 §1 rationale).
//! The Noise static is the identity's X25519 exchange key.
//!
//! Handshake payloads carry `identity_pubkey(32) ‖ HELLO` encrypted (responder
//! in msg2, initiator in msg3): the ID_PROOF preimage commits to BOTH identity
//! public keys (anti-UKS, doc 10 §2.2), so each side must learn the other's
//! identity key before either can sign its proof — carrying it inside the XX
//! payloads preserves identity hiding and breaks the ordering deadlock.

use alloc::vec::Vec;

use mmp_types::domain::SigDomain;
use mmp_types::frame::Hello;
use mmp_types::ids::{Key32, Sig64};

use crate::error::CryptoError;
use crate::identity::{verify, Identity};

/// The Noise protocol name this crate speaks — standard, formally analyzed
/// XX with X25519, ChaCha20-Poly1305, and BLAKE2s (see module docs for why
/// BLAKE3 is not substituted here).
pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// Snow message overhead headroom (two AEAD tags + two public keys).
const HS_OVERHEAD: usize = 128;
/// Fixed receive storage for one Noise XX handshake message.
///
/// The v1 HELLO decoder caps the suite vector at 16 bytes.  Together with the
/// 32-byte identity key, the largest v1 handshake payload is therefore 56
/// bytes; the largest XX message (the responder's `e, ee, s, es, payload`)
/// is 152 bytes after the two public-key/DH tokens and two AEAD tags.  A
/// 256-byte buffer leaves margin for the framing headroom while keeping
/// pre-auth receive memory independent of attacker-controlled input length.
/// Inputs above this bound are not valid v1 handshake messages and are
/// rejected before calling into `snow`.
pub(crate) const MAX_HANDSHAKE_MESSAGE: usize = 256;

/// Cipher suites this build can speak (doc 03 §8: 1-byte registry pinned in
/// the handshake). Only 0x01 has a real construction; 0x02 (hybrid ML-KEM)
/// is reserved and unimplemented (OQ-11) — advertising it here would be a lie.
pub const SUPPORTED_SUITES: &[u8] = &[0x01];

/// What a completed handshake yields alongside the transport channel.
#[derive(Debug, Clone)]
pub struct PeerIntro {
    /// Peer's claimed identity key — proven only after ID_PROOF verifies.
    pub identity: Key32,
    /// Peer's Noise static = its X25519 exchange key.
    pub exchange: Key32,
    /// The peer's HELLO carried inside the handshake payload.
    pub hello: Hello,
}

/// Incremental Noise XX driver: feed inbound HS frames, ship outbound replies.
pub struct NoiseHandshake {
    hs: Option<snow::HandshakeState>,
    initiator: bool,
    /// Number of inbound Noise handshake messages successfully consumed.
    /// Used to enforce the v1 XX first-message grammar on the responder side.
    read_messages: u8,
    local_payload: Vec<u8>,
    /// Reused fixed-size output for `snow::HandshakeState::read_message`.
    /// This must not be resized from the received message length: handshake
    /// input is unauthenticated and can be supplied by an active peer.
    read_buf: Vec<u8>,
    remote_intro: Option<(Key32, Hello)>,
}

/// Result of driving one step of [`NoiseHandshake::advance`].
pub struct HandshakeOutcome {
    /// Next HS frame payload to send, if any.
    pub reply: Option<Vec<u8>>,
    /// Present exactly once, when the handshake completes.
    pub channel: Option<NoiseChannel>,
}

fn payload_of(identity: &Identity, hello: &Hello) -> Vec<u8> {
    let mut p = Vec::with_capacity(32 + 16);
    p.extend_from_slice(&identity.public().0);
    p.extend_from_slice(&hello.encode());
    p
}

fn parse_payload(bytes: &[u8]) -> Result<(Key32, Hello), CryptoError> {
    if bytes.len() < 32 {
        return Err(CryptoError::Noise);
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&bytes[..32]);
    let hello = Hello::decode(&bytes[32..]).map_err(|_| CryptoError::Noise)?;
    Ok((Key32(pk), hello))
}

fn builder() -> snow::Builder<'static> {
    // Parse of a compile-time constant cannot fail.
    let params = NOISE_PARAMS.parse().expect("valid noise params");
    snow::Builder::new(params)
}

impl NoiseHandshake {
    /// Start as initiator; returns msg1 (`-> e`, empty payload) to send.
    pub fn initiator(identity: &Identity, hello: &Hello) -> Result<(Vec<u8>, Self), CryptoError> {
        let secret = identity.exchange_secret_bytes();
        let mut hs = builder()
            .local_private_key(&secret[..])
            .build_initiator()
            .map_err(|_| CryptoError::Noise)?;
        let mut msg1 = alloc::vec![0u8; HS_OVERHEAD];
        let n = hs.write_message(&[], &mut msg1).map_err(|_| CryptoError::Noise)?;
        msg1.truncate(n);
        Ok((
            msg1,
            NoiseHandshake {
                hs: Some(hs),
                initiator: true,
                read_messages: 0,
                local_payload: payload_of(identity, hello),
                read_buf: alloc::vec![0u8; MAX_HANDSHAKE_MESSAGE],
                remote_intro: None,
            },
        ))
    }

    /// Start as responder; waits for msg1.
    pub fn responder(identity: &Identity, hello: &Hello) -> Result<Self, CryptoError> {
        let secret = identity.exchange_secret_bytes();
        let hs = builder()
            .local_private_key(&secret[..])
            .build_responder()
            .map_err(|_| CryptoError::Noise)?;
        Ok(NoiseHandshake {
            hs: Some(hs),
            initiator: false,
            read_messages: 0,
            local_payload: payload_of(identity, hello),
            read_buf: alloc::vec![0u8; MAX_HANDSHAKE_MESSAGE],
            remote_intro: None,
        })
    }

    /// Drive one inbound HS message. Read → (maybe) write → (maybe) complete.
    pub fn advance(&mut self, incoming: &[u8]) -> Result<HandshakeOutcome, CryptoError> {
        // Do this check before taking the state or invoking Snow.  The input
        // is still unauthenticated at this point; accepting arbitrary Noise
        // message lengths would otherwise force a receive allocation tied to
        // attacker input (the session envelope cap is larger than the valid
        // v1 XX message bound).
        if incoming.len() > MAX_HANDSHAKE_MESSAGE {
            return Err(CryptoError::Noise);
        }
        // XX message 1 is `-> e` with an empty payload in MMP/1.  Snow quite
        // correctly permits an arbitrary Noise payload here, but accepting it
        // would let an unauthenticated initiator inject a provisional HELLO
        // into `remote_intro` before the encrypted payloads arrive.  Reject
        // the non-conforming shape before touching the Noise state.  Only the
        // responder receives message 1; the initiator's first inbound message
        // is encrypted message 2 and is parsed below as usual.
        if !self.initiator && self.read_messages == 0 && incoming.len() != 32 {
            return Err(CryptoError::Noise);
        }
        let mut hs = self.hs.take().ok_or(CryptoError::Noise)?;

        let n = hs
            .read_message(incoming, &mut self.read_buf)
            .map_err(|_| CryptoError::Noise)?;
        self.read_messages = self.read_messages.saturating_add(1);
        if n > 0 {
            let (peer_pk, hello) = parse_payload(&self.read_buf[..n])?;
            // Cipher-agility gate (doc 03 §8): refuse rather than silently
            // fall back if the peer names no suite this build can speak.
            if !hello.suites.iter().any(|s| SUPPORTED_SUITES.contains(s)) {
                return Err(CryptoError::Noise);
            }
            self.remote_intro = Some((peer_pk, hello));
        }

        if hs.is_handshake_finished() {
            // Responder after reading msg3.
            return Ok(HandshakeOutcome { reply: None, channel: Some(self.finish(hs)?) });
        }

        let mut out = alloc::vec![0u8; self.local_payload.len() + HS_OVERHEAD];
        let n = hs.write_message(&self.local_payload, &mut out).map_err(|_| CryptoError::Noise)?;
        out.truncate(n);

        if hs.is_handshake_finished() {
            // Initiator after writing msg3.
            return Ok(HandshakeOutcome { reply: Some(out), channel: Some(self.finish(hs)?) });
        }
        self.hs = Some(hs);
        Ok(HandshakeOutcome { reply: Some(out), channel: None })
    }

    fn finish(&mut self, hs: snow::HandshakeState) -> Result<NoiseChannel, CryptoError> {
        let mut handshake_hash = [0u8; 32];
        handshake_hash.copy_from_slice(hs.get_handshake_hash());
        let remote_static = hs.get_remote_static().ok_or(CryptoError::Noise)?;
        let mut exchange = [0u8; 32];
        exchange.copy_from_slice(remote_static);
        let (identity, hello) = self.remote_intro.take().ok_or(CryptoError::Noise)?;
        let transport = hs.into_transport_mode().map_err(|_| CryptoError::Noise)?;
        Ok(NoiseChannel {
            transport,
            handshake_hash,
            initiator: self.initiator,
            peer: PeerIntro { identity, exchange: Key32(exchange), hello },
        })
    }
}

/// AEAD transport channel after a completed XX handshake.
pub struct NoiseChannel {
    transport: snow::TransportState,
    /// Noise handshake hash — session-unique transcript binding, reused as
    /// the ID_PROOF preimage input and as ratchet epoch-bump material.
    pub handshake_hash: [u8; 32],
    /// Whether the local side was the Noise XX initiator.
    pub initiator: bool,
    /// The peer's proven-after-ID_PROOF identity and exchange keys.
    pub peer: PeerIntro,
}

impl NoiseChannel {
    /// Encrypt one transport message (adds the Noise AEAD tag).
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut out = alloc::vec![0u8; plaintext.len() + 16];
        let n = self.transport.write_message(plaintext, &mut out).map_err(|_| CryptoError::Noise)?;
        out.truncate(n);
        Ok(out)
    }

    /// Decrypt one transport message.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut out = alloc::vec![0u8; ciphertext.len()];
        let n = self.transport.read_message(ciphertext, &mut out).map_err(|_| CryptoError::Noise)?;
        out.truncate(n);
        Ok(out)
    }

    /// The ID_PROOF signature this side must send (doc 08 §3.2).
    pub fn make_id_proof(&self, identity: &Identity, local_identity_is_initiator: bool) -> Sig64 {
        let (init_pk, resp_pk) = if local_identity_is_initiator {
            (identity.public(), self.peer.identity)
        } else {
            (self.peer.identity, identity.public())
        };
        identity.sign(SigDomain::ChanBind, &chanbind_object(&self.handshake_hash, &init_pk, &resp_pk))
    }

    /// Verify the peer's ID_PROOF against its claimed identity key.
    pub fn verify_id_proof(&self, local: &Key32, sig: &Sig64) -> Result<(), CryptoError> {
        let (init_pk, resp_pk) = if self.initiator {
            (*local, self.peer.identity)
        } else {
            (self.peer.identity, *local)
        };
        verify(
            SigDomain::ChanBind,
            &self.peer.identity,
            &chanbind_object(&self.handshake_hash, &init_pk, &resp_pk),
            sig,
        )
    }

    /// Fresh entropy for the conversation-ratchet epoch bump (doc 08 §5.2):
    /// session-unique, both-party material.
    pub fn session_material(&self) -> [u8; 32] {
        self.handshake_hash
    }
}

/// `noise_handshake_hash ‖ initiator_pubkey ‖ responder_pubkey` (doc 10 §2.2).
pub fn chanbind_object(h: &[u8; 32], init_pk: &Key32, resp_pk: &Key32) -> Vec<u8> {
    let mut o = Vec::with_capacity(96);
    o.extend_from_slice(h);
    o.extend_from_slice(&init_pk.0);
    o.extend_from_slice(&resp_pk.0);
    o
}
