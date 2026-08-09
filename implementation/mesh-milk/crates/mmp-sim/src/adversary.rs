//! `Adversary` — a raw-frame/bundle-level attacker (E5-S7, handbook §4.8).
//!
//! Every other actor in this simulator is an [`mmp_node::Node`]: it enforces
//! the wire protocol on itself (WANT-gated hand-off, canonical encoding, its
//! own admission/quota policy) simply because that is the only API surface it
//! exposes. An `Adversary` is not a `Node` at all — it never imports it — and
//! deliberately re-implements just enough of `mmp_core::session::SessionEngine`
//! (Noise XX handshake, ID_PROOF, frame envelopes) by hand, directly against
//! `mmp-types` and `mmp-crypto`, so that a test can violate every assumption a
//! `Node` enforces on itself: send `Frame::Bundle` unsolicited (no WANT), byte-
//! replay a frame it captured, forge a receipt under its own signature, gossip
//! a stale claim, or hand a poisoned summary bloom to an honest peer.
//!
//! The honest side of every scenario in `tests_adversarial.rs` is a real
//! `mmp_node::Node` obtained from `Sim::node_mut` — the adversary is the only
//! bypass; the victim runs unmodified production code.

use std::collections::VecDeque;

use mmp_crypto::{Identity, NoiseChannel, NoiseHandshake};
use mmp_node::Node;
use mmp_tal::{LinkId, LinkQuality, TalEvent};
use mmp_types::bundle::Bundle;
use mmp_types::claim::Claim;
use mmp_types::domain::SigDomain;
use mmp_types::frame::{read_envelope, write_envelope, Frame, Hello, FT_HS};
use mmp_types::ids::{node_id_of, Id16, Key32, NodeId, Sig64};

/// One adversarial identity. Cheap to construct — Sybil scenarios just make
/// several.
pub struct Adversary {
    identity: Identity,
    hello: Hello,
}

impl Adversary {
    pub fn new(seed: [u8; 32]) -> Adversary {
        Adversary {
            identity: Identity::from_seed(&seed),
            // Mirrors `mmp_node::Node::new`'s HELLO shape (doc 10 §2.1) —
            // nothing about the attacks below depends on it being anything
            // other than a well-formed HELLO.
            hello: Hello { ver: 1, suites: vec![0x01], tier: 2, flags: 0, credits: 8, psm: 0 },
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }

    pub fn public_key(&self) -> Key32 {
        self.identity.public()
    }

    /// Sign a claim as this adversary's own identity (owner = us).
    pub fn sign_claim(&self, field: u8, seq: u32, body: Vec<u8>) -> Claim {
        signed_claim(&self.identity, field, seq, body)
    }

    /// Build + sign a bundle with this adversary as `origin` — the common
    /// case (flood / quota-abuse junk, self-authored claim gossip). Use
    /// [`signed_bundle`] directly when the scenario needs a bundle signed by
    /// a *different* (e.g. synthetic third-party honest) identity — the
    /// eclipse scenario's forged-receipt test needs exactly that, since a
    /// receipt's purge gate keys on whose signature the CARRIER bundle
    /// (`Frame::Bundle`'s `origin`) carries, not who is attacking.
    #[allow(clippy::too_many_arguments)]
    pub fn bundle(
        &self,
        class: u8,
        dest: Id16,
        priority: u8,
        lifetime_ms: u64,
        flags: u8,
        payload: Vec<u8>,
    ) -> Bundle {
        signed_bundle(&self.identity, class, dest, priority, lifetime_ms, flags, payload)
    }

    /// Open a fresh session as the dialing party (initiator). Every scenario
    /// in this suite has the adversary dial in — a sim contact only needs one
    /// side able to speak, and "initiator" is what lets it also self-issue a
    /// `ReconInit`/`Want` on its own schedule rather than waiting to be asked.
    pub fn dial(&self, now_ms: u64) -> AdversarySession {
        let _ = now_ms; // no wall-clock dependency in the raw driver itself
        let (msg1, hs) =
            NoiseHandshake::initiator(&self.identity, &self.hello).expect("adversary noise init");
        let mut s = AdversarySession {
            identity: self.identity.clone(),
            hs: Some(hs),
            chan: None,
            out: VecDeque::new(),
            rx: Vec::new(),
            sent_id_proof: false,
            established: false,
            peer: None,
            inbox: Vec::new(),
        };
        s.out.push_back(write_envelope(FT_HS, &msg1));
        s
    }
}

fn signed_claim(identity: &Identity, field: u8, seq: u32, body: Vec<u8>) -> Claim {
    let mut c = Claim { owner: identity.public(), field, seq, body, sig: Sig64::ZERO };
    c.sig = identity.sign(SigDomain::Claim, &c.signed_object());
    c
}

/// Build + sign a bundle under an arbitrary identity (doc 10 §3). Free
/// function (not `Adversary`-scoped) because the eclipse scenario needs a
/// bundle signed by a *synthetic honest third party* the adversary is merely
/// relaying — the point of that test is that forging the RECEIPT is what
/// fails, not the underlying (honestly signed) carried bundle.
#[allow(clippy::too_many_arguments)]
pub fn signed_bundle(
    identity: &Identity,
    class: u8,
    dest: Id16,
    priority: u8,
    lifetime_ms: u64,
    flags: u8,
    payload: Vec<u8>,
) -> Bundle {
    let mut b = Bundle {
        class,
        flags,
        origin: identity.public(),
        dest,
        lifetime_ms,
        priority,
        exts: Vec::new(),
        payload,
        signature: Sig64::ZERO,
    };
    b.signature = identity.sign(SigDomain::Bundle, &b.canonical_zeroed());
    b
}

/// One raw session with a victim `Node`. Everything past the Noise handshake
/// and ID_PROOF exchange is scripted by the caller via [`AdversarySession::send`]
/// — unlike `mmp_core::session::SessionEngine`, this driver enforces NOTHING
/// about frame ordering, WANT-gating, or reconciliation state; that absence
/// of self-restraint is the entire point of an adversary harness.
pub struct AdversarySession {
    identity: Identity,
    hs: Option<NoiseHandshake>,
    chan: Option<NoiseChannel>,
    out: VecDeque<Vec<u8>>,
    /// Reassembly buffer for inbound bytes — unbounded here (unlike the real
    /// `SessionEngine`'s pre-allocated cap, AUDIT F10): this is the ATTACKER
    /// side of the harness, not code under test.
    rx: Vec<u8>,
    sent_id_proof: bool,
    established: bool,
    peer: Option<NodeId>,
    /// Every application frame (post-handshake, post-ID_PROOF) the victim
    /// sent us, in arrival order — the "observed traffic" a replay/relay
    /// attack harvests from.
    inbox: Vec<Frame>,
}

impl AdversarySession {
    pub fn established(&self) -> bool {
        self.established
    }

    pub fn peer(&self) -> Option<NodeId> {
        self.peer
    }

    /// Wire bytes ready to hand to the victim's `on_tal_event(LinkData)`.
    pub fn poll_out(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }

    /// Feed wire bytes received from the victim.
    pub fn on_wire(&mut self, bytes: &[u8]) {
        self.rx.extend_from_slice(bytes);
        loop {
            let (typ, payload, consumed) = match read_envelope(&self.rx) {
                Ok(Some(x)) => (x.0, x.1.to_vec(), x.2),
                Ok(None) => break,
                Err(_) => {
                    self.rx.clear();
                    return;
                }
            };
            self.rx.drain(..consumed);
            self.on_envelope(typ, &payload);
        }
    }

    fn on_envelope(&mut self, typ: u8, payload: &[u8]) {
        if typ == FT_HS {
            self.on_hs(payload);
            return;
        }
        let Some(chan) = self.chan.as_mut() else { return };
        let Ok(plain) = chan.decrypt(payload) else { return };
        let Ok(frame) = Frame::decode_payload(typ, &plain) else { return };
        self.on_frame(frame);
    }

    fn on_hs(&mut self, payload: &[u8]) {
        let Some(hs) = self.hs.as_mut() else { return };
        let Ok(outcome) = hs.advance(payload) else { return };
        if let Some(reply) = outcome.reply {
            self.out.push_back(write_envelope(FT_HS, &reply));
        }
        if let Some(chan) = outcome.channel {
            self.hs = None;
            self.chan = Some(chan);
            if !self.sent_id_proof {
                let sig = self.chan.as_ref().expect("just set").make_id_proof(&self.identity);
                self.send(Frame::IdProof { identity: self.identity.public(), sig });
                self.sent_id_proof = true;
            }
        }
    }

    fn on_frame(&mut self, frame: Frame) {
        if let Frame::IdProof { identity, sig } = &frame {
            if let Some(chan) = self.chan.as_ref() {
                let ok = *identity == chan.peer.identity
                    && chan.verify_id_proof(&self.identity.public(), sig).is_ok();
                if ok {
                    self.peer = Some(node_id_of(identity));
                    self.established = true;
                }
            }
            return;
        }
        self.inbox.push(frame);
    }

    /// Send one frame, encrypted under the established Noise channel. A no-op
    /// before the channel exists — a scripting error in a test, not a
    /// protocol case worth modeling.
    pub fn send(&mut self, frame: Frame) {
        let payload = frame.encode_payload();
        let Some(chan) = self.chan.as_mut() else { return };
        if let Ok(ct) = chan.encrypt(&payload) {
            self.out.push_back(write_envelope(frame.typ(), &ct));
        }
    }

    /// Drain every application frame the victim has sent since the last call.
    pub fn drain_inbox(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.inbox)
    }
}

/// Drive one meeting between an [`AdversarySession`] (always the dialer) and
/// a victim `Node` — the adversary-harness analog of `Sim::meet`'s pump loop.
/// Pumps wire bytes until quiet or the iteration cap. `on_established` fires
/// exactly once, the instant the session reaches `Established`, so the caller
/// can script its attack frames from inside the closure instead of
/// front-loading them (which would just queue up before the peer identity is
/// even confirmed — harmless here, but it keeps the call site reading in
/// wire order).
pub fn adversary_contact(
    victim: &mut Node,
    session: &mut AdversarySession,
    link: LinkId,
    now_ms: u64,
    mut on_established: impl FnMut(&mut AdversarySession),
) {
    victim.tick(now_ms);
    victim.on_tal_event(TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() });
    let mut fired = false;
    for _ in 0..300 {
        let mut quiet = true;
        while let Some(bytes) = session.poll_out() {
            quiet = false;
            victim.on_tal_event(TalEvent::LinkData { link, bytes });
        }
        while let Some((_, bytes)) = victim.poll_send() {
            quiet = false;
            session.on_wire(&bytes);
        }
        if session.established() && !fired {
            fired = true;
            on_established(session);
            quiet = false; // give any frames the script just queued a chance to flush
        }
        if quiet {
            break;
        }
    }
    victim.on_tal_event(TalEvent::LinkDown { link });
}
