//! Session engine (doc 11 §3): DIALING → HANDSHAKE → BINDING → ESTABLISHED →
//! CLOSING, sans-IO. The host shuttles wire bytes and implements the storage
//! seams; the engine owns phase grammar, the Noise channel, RBSR driving, and
//! transfer sequencing. Any frame violating the phase grammar ⇒ ERROR + CLOSE.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use mmp_crypto::{Identity, NoiseChannel, NoiseHandshake};
use mmp_types::consts::{HANDSHAKE_BUDGET_MS, L_MAX, SESSION_AIRTIME_CAP_MS, SESSION_IDLE_MS};
use mmp_types::frame::{
    errcode, read_envelope, write_envelope, Frame, Hello, Summary, FT_HS, MAX_ENVELOPE_PAYLOAD,
};
use mmp_types::ids::{Id16, Key32, NodeId};

use crate::rbsr::{IdSource, Reconciler, DOMAIN_REPLY_FLAG};

/// Reconciliation domains driven in priority order (doc 06 §2). D4 (blob
/// availability) is WANT-driven in the MVP rather than range-reconciled.
const DOMAINS: [u8; 4] = [0, 1, 2, 3];

/// Everything the engine needs from the node. All methods are synchronous and
/// cheap; heavy lifting stays in the node/store.
pub trait SessionHost {
    /// The host's [`IdSource`] implementation for RBSR range queries.
    type Ids: IdSource;

    /// Domain-scoped, offer-eligible sorted IDs + membership (RBSR seam).
    fn ids(&mut self, peer: &NodeId) -> &mut Self::Ids;
    /// SUMMARY to send this peer (receipt buckets, claim digest…).
    fn summary(&mut self, peer: &NodeId) -> Summary;
    /// A SUMMARY arrived from `peer` (receipt buckets, claim digest…).
    fn on_summary(&mut self, peer: &NodeId, summary: Summary);
    /// Load a bundle for hand-off: wire bytes with DECREMENTED lifetime and
    /// the budget to grant. `None` = no longer eligible (purged, expired…).
    /// Implementations MUST persist hand-off intent before returning (I3).
    fn load_for_handoff(&mut self, peer: &NodeId, id: &Id16) -> Option<(Vec<u8>, u8)>;
    /// Ingest a received bundle (full doc 03 §4.1.2 pipeline inside).
    fn on_bundle(&mut self, peer: &NodeId, bytes: &[u8]);
    /// Budget grant for a bundle we just received. Clamp to L_MAX inside.
    fn on_budget(&mut self, peer: &NodeId, id: &Id16, granted: u8);
    /// Outstanding WANTs beyond reconciliation (DAG gap repair, doc 06 §5.3).
    fn wants(&mut self, peer: &NodeId) -> Vec<Id16>;
    /// Admission on identity reveal: false ⇒ silent close (doc 07 §6.5).
    fn accept_peer(&mut self, identity: &Key32, node: &NodeId) -> bool;

    // --------------------------------------------------- friendship (§4.5) --
    // Default impls are a no-op so every existing `SessionHost` keeps
    // compiling unchanged (handbook §4.5) — the engine stays grammar-only,
    // opaque bodies pass straight through to whichever host cares to
    // interpret them (mmp-node's `NodeCore`, currently the only one that does).

    /// FRIEND_OFFER arrived from `peer`. `Some(ack body)` ⇒ the engine sends
    /// FRIEND_ACK with that body right back; `None` ⇒ silently ignored (e.g.
    /// not on our allowlist — doc 04 §4 "no auto-accept").
    fn on_friend_offer(&mut self, peer: &NodeId, body: &[u8]) -> Option<Vec<u8>> {
        let _ = (peer, body);
        None
    }
    /// FRIEND_ACK arrived from `peer`.
    fn on_friend_ack(&mut self, peer: &NodeId, body: &[u8]) {
        let _ = (peer, body);
    }
    /// PROXY_SUMMARY arrived from `peer`.
    fn on_proxy_summary(&mut self, peer: &NodeId, body: &[u8]) {
        let _ = (peer, body);
    }
    /// Outbound friendship frames (FRIEND_OFFER solicitation, PROXY_SUMMARY
    /// advertisement) to send `peer`, polled once right after Established
    /// (after SUMMARY).
    fn friend_frames(&mut self, peer: &NodeId) -> Vec<Frame> {
        let _ = peer;
        Vec::new()
    }

    // -------------------------------------------------------- blob plane (E5-S1) --
    // Default impls are a no-op, mirroring the friendship seam above, so every
    // existing `SessionHost` keeps compiling unchanged — the engine stays
    // grammar-only, opaque bitmaps pass straight through to whichever host
    // cares to interpret them.

    /// BLOB_HAVE arrived: `peer`'s have-state for `root` — an implicit
    /// request for everything absent. Returns BundleIDs of symbol bundles to
    /// serve this peer (already capped by the host); the engine ships each
    /// via the same `load_for_handoff` → BUNDLE + BUDGET path the WANT arm
    /// uses (doc 16 §4.1, handbook §4.4 D4).
    fn on_blob_have(&mut self, peer: &NodeId, root: &Id16, block: u16, bitmap: &[u8]) -> Vec<Id16> {
        let _ = (peer, root, block, bitmap);
        Vec::new()
    }
    /// BLOB_REQ arrived: explicit want-bitmap. Same return contract as
    /// [`SessionHost::on_blob_have`].
    fn on_blob_req(&mut self, peer: &NodeId, root: &Id16, block: u16, bitmap: &[u8]) -> Vec<Id16> {
        let _ = (peer, root, block, bitmap);
        Vec::new()
    }
    /// Outbound blob frames (one BLOB_HAVE per active assembly) to send
    /// `peer`, polled once right after Established, immediately after
    /// [`SessionHost::friend_frames`], on both sides.
    fn blob_frames(&mut self, peer: &NodeId) -> Vec<Frame> {
        let _ = peer;
        Vec::new()
    }
}

/// The session FSM's current phase (doc 11 §3): DIALING/HANDSHAKE are
/// collapsed into `Handshake` here — Noise message exchange runs before any
/// `SessionEngine` phase transition fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Noise handshake in progress; no channel yet.
    Handshake,
    /// Channel established, waiting for the peer's ID_PROOF (doc 08 §3.2).
    Binding,
    /// Peer identity proven; SUMMARY/RBSR/bundle exchange may proceed.
    Established,
    /// Session torn down — terminal, no further frames are processed.
    Closed,
}

/// An event the engine surfaces to the host after processing wire bytes or a
/// timer tick, via [`SessionEngine::poll_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// Peer identity proven via ID_PROOF (doc 08 §3.2).
    Established {
        /// The peer's derived node ID.
        peer: NodeId,
        /// The peer's proven identity key.
        identity: Key32,
    },
    /// The session has torn down (graceful, silent, or error close alike).
    Closed,
}

/// The sans-IO per-link session FSM (doc 11 §3): drives the Noise handshake,
/// ID_PROOF binding, RBSR reconciliation across [`DOMAINS`] in priority
/// order, and bundle hand-off, from Handshake through Established to Closed.
/// Wire bytes are pushed in via [`SessionEngine::on_wire`] and pulled out via
/// [`SessionEngine::poll_out`]; the engine owns no socket, timer, or clock of
/// its own.
pub struct SessionEngine {
    phase: Phase,
    initiator: bool,
    hs: Option<NoiseHandshake>,
    chan: Option<NoiseChannel>,
    peer_node: Option<NodeId>,
    sent_proof: bool,
    /// Reassembly buffer — pre-allocated, NEVER grown from the wire (AUDIT F10).
    rx: Vec<u8>,
    out: VecDeque<Vec<u8>>,
    events: VecDeque<SessionEvent>,
    recon: Option<Reconciler>,
    domain_cursor: usize,
    started_at: u64,
    last_activity: u64,
    sync_started: bool,
}

impl SessionEngine {
    // Deliberately erases the handshake-failure reason (`()`): the caller's
    // only valid reaction is to drop the attempt, and a richer error type
    // would be a public-API redesign out of scope here.
    /// Starts a new session as the dialing side: begins the Noise handshake
    /// against `hello` and queues msg1 for immediate send. `Err(())` on a
    /// handshake-construction failure (nothing to salvage — drop the attempt).
    #[allow(clippy::result_unit_err)]
    pub fn initiator(identity: &Identity, hello: &Hello, now_ms: u64) -> Result<SessionEngine, ()> {
        let (msg1, hs) = NoiseHandshake::initiator(identity, hello).map_err(|_| ())?;
        let mut s = SessionEngine::new(true, now_ms);
        s.hs = Some(hs);
        s.out.push_back(write_envelope(FT_HS, &msg1));
        Ok(s)
    }

    /// Starts a new session as the accepting side: awaits the initiator's
    /// handshake message against `hello`. `Err(())` on a handshake-
    /// construction failure (nothing to salvage — drop the attempt).
    #[allow(clippy::result_unit_err)]
    pub fn responder(identity: &Identity, hello: &Hello, now_ms: u64) -> Result<SessionEngine, ()> {
        let hs = NoiseHandshake::responder(identity, hello).map_err(|_| ())?;
        let mut s = SessionEngine::new(false, now_ms);
        s.hs = Some(hs);
        Ok(s)
    }

    fn new(initiator: bool, now_ms: u64) -> SessionEngine {
        SessionEngine {
            phase: Phase::Handshake,
            initiator,
            hs: None,
            chan: None,
            peer_node: None,
            sent_proof: false,
            rx: Vec::with_capacity(MAX_ENVELOPE_PAYLOAD + 3),
            out: VecDeque::new(),
            events: VecDeque::new(),
            recon: None,
            domain_cursor: 0,
            started_at: now_ms,
            last_activity: now_ms,
            sync_started: false,
        }
    }

    /// The engine's current FSM phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The peer's derived node ID, available from `Binding` onward once
    /// ID_PROOF has been accepted.
    pub fn peer(&self) -> Option<NodeId> {
        self.peer_node
    }

    /// Fresh session-unique entropy for the conversation-ratchet opportunistic
    /// epoch bump (doc 08 §5.2/§5.3, E4-S4 bump_epoch wiring) — `None` before
    /// `Established` fires. The host reads this at session teardown (`chan`
    /// outlives `Established` all the way to `Closed`/link-down — only `hs`
    /// and `recon` are cleared then), alongside `peer_identity`, to bump any
    /// existing `Conv` with this peer once the contact's own message
    /// exchange has fully drained.
    pub fn session_material(&self) -> Option<[u8; 32]> {
        self.chan.as_ref().map(|c| c.session_material())
    }

    /// The peer's proven identity key, available from `Established` onward
    /// (same lifetime as [`SessionEngine::session_material`]) — `conv_id`
    /// needs this, not just the `NodeId` [`SessionEngine::peer`] returns.
    pub fn peer_identity(&self) -> Option<Key32> {
        self.chan.as_ref().map(|c| c.peer.identity)
    }

    /// Wire bytes ready to send (drain in order).
    pub fn poll_out(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }

    /// The next queued [`SessionEvent`] for the host to react to, if any
    /// (drain in order).
    pub fn poll_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    /// Feed raw wire bytes from the link.
    pub fn on_wire<H: SessionHost>(
        &mut self,
        bytes: &[u8],
        now_ms: u64,
        identity: &Identity,
        host: &mut H,
    ) {
        if self.phase == Phase::Closed {
            return;
        }
        self.last_activity = now_ms;
        self.rx.extend_from_slice(bytes);
        loop {
            let (typ, payload, consumed) = match read_envelope(&self.rx) {
                Ok(Some(x)) => (x.0, x.1.to_vec(), x.2),
                Ok(None) => break,
                Err(_) => {
                    self.protocol_error(errcode::OVERSIZE);
                    return;
                }
            };
            self.rx.drain(..consumed);
            self.on_envelope(typ, &payload, now_ms, identity, host);
            if self.phase == Phase::Closed {
                return;
            }
        }
        // Reassembly bound (AUDIT F10): after draining complete frames, a
        // retained partial frame can never legitimately exceed one envelope —
        // the length prefix over the cap was already rejected by read_envelope,
        // so this bounds attacker-pinned memory per session.
        if self.rx.len() > MAX_ENVELOPE_PAYLOAD + 3 {
            self.protocol_error(errcode::OVERSIZE);
        }
    }

    fn on_envelope<H: SessionHost>(
        &mut self,
        typ: u8,
        payload: &[u8],
        now_ms: u64,
        identity: &Identity,
        host: &mut H,
    ) {
        if typ == FT_HS {
            self.on_hs(payload, identity);
            return;
        }
        // Everything after HS is inside the AEAD channel (doc 10 §2).
        let Some(chan) = self.chan.as_mut() else {
            self.protocol_error(errcode::PROTOCOL);
            return;
        };
        let Ok(plain) = chan.decrypt(payload) else {
            self.protocol_error(errcode::PROTOCOL);
            return;
        };
        let Ok(frame) = Frame::decode_payload(typ, &plain) else {
            // Unknown frame type or malformed: ERROR(unsupported) + CLOSE.
            self.protocol_error(errcode::UNSUPPORTED);
            return;
        };
        self.on_frame(frame, now_ms, identity, host);
    }

    fn on_hs(&mut self, payload: &[u8], identity: &Identity) {
        if self.phase != Phase::Handshake {
            self.protocol_error(errcode::PROTOCOL);
            return;
        }
        let Some(hs) = self.hs.as_mut() else {
            self.protocol_error(errcode::PROTOCOL);
            return;
        };
        let Ok(outcome) = hs.advance(payload) else {
            self.close_silently();
            return;
        };
        if let Some(reply) = outcome.reply {
            self.out.push_back(write_envelope(FT_HS, &reply));
        }
        if let Some(chan) = outcome.channel {
            self.hs = None;
            self.chan = Some(chan);
            self.phase = Phase::Binding;
            // Both sides send ID_PROOF immediately (doc 08 §3.2).
            let chan = self.chan.as_ref().expect("just set");
            let sig = chan.make_id_proof(identity, self.initiator);
            self.send_frame(&Frame::IdProof { identity: identity.public(), sig });
            self.sent_proof = true;
        }
    }

    fn on_frame<H: SessionHost>(
        &mut self,
        frame: Frame,
        _now_ms: u64,
        identity: &Identity,
        host: &mut H,
    ) {
        match (self.phase, frame) {
            (Phase::Binding, Frame::IdProof { identity: claimed, sig }) => {
                let chan = self.chan.as_ref().expect("binding implies channel");
                // The claimed identity must be the one carried in the handshake
                // payload, and the proof must verify against it.
                if claimed != chan.peer.identity
                    || chan.verify_id_proof(&identity.public(), &sig).is_err()
                {
                    self.close_silently();
                    return;
                }
                let peer_node = mmp_types::node_id_of(&claimed);
                // Inbound deny is silent — same shape as a timeout (doc 07 §6.5).
                if !host.accept_peer(&claimed, &peer_node) {
                    self.close_silently();
                    return;
                }
                self.peer_node = Some(peer_node);
                self.phase = Phase::Established;
                self.events.push_back(SessionEvent::Established { peer: peer_node, identity: claimed });
                // Phase 2: SUMMARY first — maximizes value of short contacts
                // (doc 03 §5.2).
                let summary = host.summary(&peer_node);
                self.send_frame(&Frame::Summary(summary));
                // Friendship offers/proxy summaries piggyback right after
                // SUMMARY, both sides (handbook §4.5) — polled once per
                // contact, not gated on `self.initiator`.
                for f in host.friend_frames(&peer_node) {
                    self.send_frame(&f);
                }
                // Blob-plane BLOB_HAVE advertisement (handbook §4.4 D4),
                // polled once right after friendship frames, both sides.
                for f in host.blob_frames(&peer_node) {
                    self.send_frame(&f);
                }
                if self.initiator {
                    self.start_domain(host);
                }
            }
            (Phase::Established, frame) => self.on_established(frame, host),
            (_, Frame::Close) => self.close_silently(),
            (_, Frame::Error { .. }) => self.close_silently(),
            _ => self.protocol_error(errcode::PROTOCOL),
        }
    }

    fn on_established<H: SessionHost>(&mut self, frame: Frame, host: &mut H) {
        let peer = self.peer_node.expect("established implies peer");
        match frame {
            Frame::Summary(s) => host.on_summary(&peer, s),
            Frame::Credit(_) => {} // link-layer flow control: mem/GATT hosts differ
            Frame::ReconInit { domain, range, .. } => {
                let recon = self.recon_for(domain & !DOMAIN_REPLY_FLAG);
                let replies = recon.on_range(&range, false, host.ids(&peer));
                self.send_frames(replies);
            }
            Frame::ReconRange { domain, ranges } => {
                let is_reply = domain & DOMAIN_REPLY_FLAG != 0;
                let recon = self.recon_for(domain & !DOMAIN_REPLY_FLAG);
                let mut replies = Vec::new();
                for r in &ranges {
                    replies.extend(recon.on_range(r, is_reply, host.ids(&peer)));
                }
                self.send_frames(replies);
                self.drain_wants();
                self.maybe_advance_domain(host);
            }
            Frame::ReconIds { domain, lo, hi, ids } => {
                let recon = self.recon_for(domain & !DOMAIN_REPLY_FLAG);
                let replies = recon.on_ids(&lo, &hi, &ids, host.ids(&peer));
                self.send_frames(replies);
                self.drain_wants();
                self.maybe_advance_domain(host);
            }
            Frame::Want(ids) => {
                for id in &ids {
                    if let Some((bytes, granted)) = host.load_for_handoff(&peer, id) {
                        self.send_frame(&Frame::Bundle(bytes));
                        // BUDGET immediately follows its BUNDLE (doc 10 §2.5).
                        self.send_frame(&Frame::Budget { id: *id, granted });
                    }
                }
            }
            Frame::Bundle(bytes) => host.on_bundle(&peer, &bytes),
            Frame::Budget { id, granted } => {
                // Receive-side clamp (AUDIT F3) — silent, never an error.
                host.on_budget(&peer, &id, granted.min(L_MAX));
            }
            Frame::BlobReq { root, block, bitmap } => {
                for id in host.on_blob_req(&peer, &root, block, &bitmap) {
                    if let Some((bytes, granted)) = host.load_for_handoff(&peer, &id) {
                        self.send_frame(&Frame::Bundle(bytes));
                        // BUDGET immediately follows its BUNDLE (doc 10 §2.5).
                        self.send_frame(&Frame::Budget { id, granted });
                    }
                }
            }
            Frame::BlobHave { root, block, bitmap } => {
                for id in host.on_blob_have(&peer, &root, block, &bitmap) {
                    if let Some((bytes, granted)) = host.load_for_handoff(&peer, &id) {
                        self.send_frame(&Frame::Bundle(bytes));
                        self.send_frame(&Frame::Budget { id, granted });
                    }
                }
            }
            Frame::FriendOffer(body) => {
                if let Some(ack_body) = host.on_friend_offer(&peer, &body) {
                    self.send_frame(&Frame::FriendAck(ack_body));
                }
            }
            Frame::FriendAck(body) => host.on_friend_ack(&peer, &body),
            Frame::ProxySummary(body) => host.on_proxy_summary(&peer, &body),
            Frame::Close => self.close_silently(),
            Frame::Error { .. } => self.close_silently(),
            Frame::Hs(_) | Frame::IdProof { .. } => self.protocol_error(errcode::PROTOCOL),
        }
    }

    fn recon_for(&mut self, domain: u8) -> &mut Reconciler {
        let fresh = match &self.recon {
            Some(r) => r.domain != domain,
            None => true,
        };
        if fresh {
            self.recon = Some(Reconciler::new(domain));
        }
        self.recon.as_mut().expect("just ensured")
    }

    fn start_domain<H: SessionHost>(&mut self, host: &mut H) {
        let peer = self.peer_node.expect("established");
        let domain = DOMAINS[self.domain_cursor];
        self.recon = Some(Reconciler::new(domain));
        self.sync_started = true;
        let frame = self
            .recon
            .as_mut()
            .expect("just set")
            .init_frame(host.ids(&peer));
        self.send_frame(&frame);
    }

    fn maybe_advance_domain<H: SessionHost>(&mut self, host: &mut H) {
        if !self.initiator {
            return;
        }
        let done = self.recon.as_ref().map(|r| r.is_done()).unwrap_or(false);
        if !done {
            return;
        }
        if self.domain_cursor + 1 < DOMAINS.len() {
            self.domain_cursor += 1;
            self.start_domain(host);
        } else if let Some(peer) = self.peer_node {
            // All domains reconciled: flush DAG-repair WANTs (doc 06 §5.3).
            let wants = host.wants(&peer);
            if !wants.is_empty() {
                self.send_frame(&Frame::Want(wants));
            }
        }
    }

    /// Move accumulated RBSR `missing` IDs into WANT frames.
    fn drain_wants(&mut self) {
        let Some(recon) = self.recon.as_mut() else { return };
        if recon.missing.is_empty() {
            return;
        }
        let ids: Vec<Id16> = core::mem::take(&mut recon.missing).into_iter().collect();
        for chunk in ids.chunks(200) {
            self.send_frame(&Frame::Want(chunk.to_vec()));
        }
    }

    fn send_frames(&mut self, frames: Vec<Frame>) {
        for f in frames {
            self.send_frame(&f);
        }
    }

    fn send_frame(&mut self, f: &Frame) {
        let payload = f.encode_payload();
        let Some(chan) = self.chan.as_mut() else {
            return; // pre-channel non-HS send is a programming error; drop
        };
        if let Ok(ct) = chan.encrypt(&payload) {
            self.out.push_back(write_envelope(f.typ(), &ct));
        }
    }

    /// Timers (doc 11 §7): handshake budget 5 s, idle 10 s, airtime cap 60 s.
    pub fn tick(&mut self, now_ms: u64) {
        if self.phase == Phase::Closed {
            return;
        }
        let handshaking = matches!(self.phase, Phase::Handshake | Phase::Binding);
        if handshaking && now_ms.saturating_sub(self.started_at) > HANDSHAKE_BUDGET_MS {
            self.close_silently();
        } else if now_ms.saturating_sub(self.last_activity) > SESSION_IDLE_MS
            || now_ms.saturating_sub(self.started_at) > SESSION_AIRTIME_CAP_MS
        {
            self.close(errcode::PROTOCOL, true);
        }
    }

    /// Graceful close: CLOSE frame then Closed event.
    pub fn close_gracefully(&mut self) {
        if self.phase != Phase::Closed {
            self.send_frame(&Frame::Close);
            self.finish_closed();
        }
    }

    fn close(&mut self, _code: u8, graceful: bool) {
        if graceful {
            self.send_frame(&Frame::Close);
        }
        self.finish_closed();
    }

    fn protocol_error(&mut self, code: u8) {
        self.send_frame(&Frame::Error { code, detail: Vec::new() });
        self.send_frame(&Frame::Close);
        self.finish_closed();
    }

    /// Silent disconnect: no ERROR, no CLOSE — indistinguishable from radio
    /// loss (doc 07 §6.5, doc 11 §3 admission deny).
    fn close_silently(&mut self) {
        self.finish_closed();
    }

    fn finish_closed(&mut self) {
        self.phase = Phase::Closed;
        self.hs = None;
        self.recon = None;
        self.events.push_back(SessionEvent::Closed);
    }
}
