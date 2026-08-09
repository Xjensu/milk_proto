//! Session engine (doc 11 §3): DIALING → HANDSHAKE → BINDING → ESTABLISHED →
//! CLOSING, sans-IO. The host shuttles wire bytes and implements the storage
//! seams; the engine owns phase grammar, the Noise channel, RBSR driving, and
//! transfer sequencing. Any frame violating the phase grammar ⇒ ERROR + CLOSE.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use mmp_crypto::{Identity, NoiseChannel, NoiseHandshake};
use mmp_types::consts::{
    HANDSHAKE_BUDGET_MS, L_MAX, MAX_FRAME_PAYLOAD, SESSION_AIRTIME_CAP_MS, SESSION_IDLE_MS,
};
use mmp_types::frame::{
    errcode, read_envelope, write_envelope, Frame, Hello, Range, Summary, FT_HS, MAX_ENVELOPE_PAYLOAD,
};
use mmp_types::ids::{Id16, Key32, NodeId};

use crate::rbsr::{IdSource, Reconciler, DOMAIN_REPLY_FLAG};

/// Reconciliation domains driven in priority order (doc 06 §2). D4 (blob
/// availability) is WANT-driven in the MVP rather than range-reconciled.
const DOMAINS: [u8; 4] = [0, 1, 2, 3];
const RX_REASSEMBLY_CAP: usize = MAX_ENVELOPE_PAYLOAD + 3;
/// Normative initial/replenishment window from specification/09 §5.2.
const FRAME_CREDIT_WINDOW: u8 = 8;
/// Keep enough receive credit for an atomic BUNDLE+BUDGET response pair;
/// replenishing only at zero can deadlock a sender holding one credit.
const FRAME_CREDIT_RESERVE: u8 = 2;
/// A bounded deferred-frame reserve prevents a slow host from turning the
/// sans-IO engine into an unbounded allocator while preserving frame order.
const MAX_PENDING_FRAMES: usize = 256;
/// A single authenticated request can name roughly 255 IDs; retaining one
/// bounded request window lets a peer retry after a CREDIT grant without
/// allowing repeated WANT/BLOB requests to grow memory without limit.
const MAX_PENDING_HANDOFFS: usize = 256;
/// Bound immediate wire frames retained by the sans-IO engine before the
/// host drains [`SessionEngine::poll_out`]. Two reserved slots ensure a
/// protocol-error path can still append ERROR+CLOSE before termination.
const MAX_OUT_FRAMES: usize = 256;
const OUT_CONTROL_RESERVE: usize = 2;

const fn frame_payload_within_cap(len: usize) -> bool {
    len <= MAX_FRAME_PAYLOAD
}

/// Check collection widths before calling the public infallible frame encoder.
///
/// `Frame::encode_payload` is intentionally kept as the existing API, but it
/// serializes several collection lengths with `u8`/`u16` casts.  A local host
/// (or a compromised host callback) must not be able to turn those casts into
/// a different wire value in an otherwise live session.  The aggregate byte
/// cap is checked after encoding; these checks cover only the narrower field
/// widths that could truncate before that cap is observed.
fn frame_wire_widths_valid(frame: &Frame) -> bool {
    match frame {
        Frame::Summary(summary) => {
            summary.receipt_buckets.len() <= u8::MAX as usize
                && summary.claim_digest.len() <= u8::MAX as usize
                && summary.group_hints.len() <= u8::MAX as usize
                && summary.want_heads.len() <= u8::MAX as usize
                && summary.receipt_buckets.iter().all(|bucket| bucket.bloom.len() <= u16::MAX as usize)
                && summary.want_heads.iter().all(|heads| heads.heads.len() <= u8::MAX as usize)
        }
        Frame::ReconRange { ranges, .. } => ranges.len() <= u8::MAX as usize,
        Frame::ReconIds { ids, .. } | Frame::Want(ids) => ids.len() <= u16::MAX as usize,
        Frame::BlobReq { bitmap, .. } | Frame::BlobHave { bitmap, .. } => bitmap.len() <= u16::MAX as usize,
        Frame::Error { detail, .. } => detail.len() <= 256,
        _ => true,
    }
}

const fn envelope_payload_within_cap(len: usize) -> bool {
    len <= MAX_ENVELOPE_PAYLOAD
}

fn valid_recon_domain(domain: u8, allow_reply_flag: bool) -> bool {
    let base = domain & !DOMAIN_REPLY_FLAG;
    DOMAINS.contains(&base) && (allow_reply_flag || domain & DOMAIN_REPLY_FLAG == 0)
}

fn valid_range(range: &Range) -> bool {
    range.lo <= range.hi
}

/// Read an envelope length from the bytes already buffered plus the next
/// transport chunk, without copying either side.  A length is returned only
/// once the two-byte prefix is available; this lets [`SessionEngine::on_wire`]
/// reject an oversized prefix before appending attacker-controlled bytes.
fn declared_len(buffered: &[u8], incoming: &[u8]) -> Option<usize> {
    match buffered.len() {
        n if n >= 2 => Some(u16::from_le_bytes([buffered[0], buffered[1]]) as usize),
        1 if !incoming.is_empty() => Some(u16::from_le_bytes([buffered[0], incoming[0]]) as usize),
        0 if incoming.len() >= 2 => Some(u16::from_le_bytes([incoming[0], incoming[1]]) as usize),
        _ => None,
    }
}

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
    pending_out: VecDeque<Frame>,
    pending_handoffs: VecDeque<Id16>,
    events: VecDeque<SessionEvent>,
    tx_credits: u8,
    rx_credits: u8,
    recon: Option<Reconciler>,
    domain_cursor: usize,
    started_at: u64,
    last_activity: u64,
    sync_started: bool,
    final_wants_sent: bool,
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
        let mut s = SessionEngine::new(true, now_ms, hello.credits);
        s.hs = Some(hs);
        s.queue_raw(write_envelope(FT_HS, &msg1));
        Ok(s)
    }

    /// Starts a new session as the accepting side: awaits the initiator's
    /// handshake message against `hello`. `Err(())` on a handshake-
    /// construction failure (nothing to salvage — drop the attempt).
    #[allow(clippy::result_unit_err)]
    pub fn responder(identity: &Identity, hello: &Hello, now_ms: u64) -> Result<SessionEngine, ()> {
        let hs = NoiseHandshake::responder(identity, hello).map_err(|_| ())?;
        let mut s = SessionEngine::new(false, now_ms, hello.credits);
        s.hs = Some(hs);
        Ok(s)
    }

    fn new(initiator: bool, now_ms: u64, local_credits: u8) -> SessionEngine {
        SessionEngine {
            phase: Phase::Handshake,
            initiator,
            hs: None,
            chan: None,
            peer_node: None,
            sent_proof: false,
            rx: Vec::with_capacity(MAX_ENVELOPE_PAYLOAD + 3),
            out: VecDeque::with_capacity(MAX_OUT_FRAMES),
            pending_out: VecDeque::new(),
            pending_handoffs: VecDeque::new(),
            events: VecDeque::new(),
            tx_credits: 0,
            rx_credits: local_credits.min(FRAME_CREDIT_WINDOW),
            recon: None,
            domain_cursor: 0,
            started_at: now_ms,
            last_activity: now_ms,
            sync_started: false,
            final_wants_sent: false,
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
        let mut offset = 0usize;
        loop {
            // Validate a complete two-byte length prefix before appending any
            // transport bytes.  This matters for both a fresh oversized frame
            // and a prefix split across two transport callbacks: malformed
            // input must not count as activity or inflate `rx` first.
            if let Some(len) = declared_len(&self.rx, &bytes[offset..]) {
                if len > MAX_ENVELOPE_PAYLOAD {
                    self.protocol_error(errcode::OVERSIZE);
                    return;
                }
            }

            if offset < bytes.len() {
                let room = RX_REASSEMBLY_CAP.saturating_sub(self.rx.len());
                if room == 0 {
                    self.protocol_error(errcode::OVERSIZE);
                    return;
                }
                let take = core::cmp::min(room, bytes.len() - offset);
                self.rx.extend_from_slice(&bytes[offset..offset + take]);
                offset += take;
                // Only accepted bytes are activity.  A rejected length prefix
                // above leaves the timer untouched, so a peer cannot keep a
                // malformed session alive by repeatedly sending junk.
                self.last_activity = now_ms;
            }

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

            if offset >= bytes.len() {
                break;
            }
        }

        // Reassembly bound (AUDIT F10): after draining complete frames, a
        // retained partial frame can never exceed one envelope.  Keep the
        // check as a final defence if a future parser change violates that
        // invariant.
        if self.rx.len() > RX_REASSEMBLY_CAP {
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
        if plain.len() > MAX_FRAME_PAYLOAD {
            self.protocol_error(errcode::OVERSIZE);
            return;
        }
        let Ok(frame) = Frame::decode_payload(typ, &plain) else {
            // Unknown frame type or malformed: ERROR(unsupported) + CLOSE.
            self.protocol_error(errcode::UNSUPPORTED);
            return;
        };
        if !matches!(&frame, Frame::Credit(_) | Frame::IdProof { .. }) {
            if self.rx_credits == 0 {
                self.protocol_error(errcode::PROTOCOL);
                return;
            }
            self.rx_credits -= 1;
            if self.rx_credits <= FRAME_CREDIT_RESERVE {
                let grant = FRAME_CREDIT_WINDOW - self.rx_credits;
                self.rx_credits = FRAME_CREDIT_WINDOW;
                self.send_frame(&Frame::Credit(grant));
            }
        }
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
            self.queue_raw(write_envelope(FT_HS, &reply));
        }
        if let Some(chan) = outcome.channel {
            self.hs = None;
            self.tx_credits = chan.peer.hello.credits.min(FRAME_CREDIT_WINDOW);
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
            Frame::Credit(granted) => {
                self.tx_credits = self
                    .tx_credits
                    .saturating_add(granted)
                    .min(FRAME_CREDIT_WINDOW);
                self.flush_pending_frames();
                self.flush_pending_handoffs(host);
            }
            Frame::ReconInit { domain, range, .. } if self.accepts_recon_init(domain) && valid_range(&range) => {
                let domain = domain & !DOMAIN_REPLY_FLAG;
                if self.recon.as_ref().is_none_or(|recon| recon.domain != domain) {
                    self.recon = Some(Reconciler::new(domain));
                }
                let (replies, exhausted) = {
                    let recon = self.recon.as_mut().expect("accepted init creates reconciler");
                    let replies = recon.on_range(&range, false, host.ids(&peer));
                    (replies, recon.resource_exhausted())
                };
                if exhausted {
                    self.protocol_error(errcode::PROTOCOL);
                    return;
                }
                self.send_frames(replies);
            }
            Frame::ReconRange { domain, ranges }
                if self.active_recon_matches(domain) && !ranges.is_empty() && ranges.iter().all(valid_range) =>
            {
                let is_reply = domain & DOMAIN_REPLY_FLAG != 0;
                let mut replies = Vec::new();
                let exhausted = {
                    let recon = self.recon.as_mut().expect("active domain guard");
                    for r in &ranges {
                        replies.extend(recon.on_range(r, is_reply, host.ids(&peer)));
                    }
                    recon.resource_exhausted()
                };
                if exhausted {
                    self.protocol_error(errcode::PROTOCOL);
                    return;
                }
                self.send_frames(replies);
                self.drain_wants();
                self.maybe_advance_domain(host);
            }
            Frame::ReconIds { domain, lo, hi, ids }
                if self.active_recon_matches(domain)
                    && domain & DOMAIN_REPLY_FLAG == 0
                    && lo <= hi
                    && ids.iter().all(|id| *id >= lo && *id <= hi) =>
            {
                let (replies, exhausted) = {
                    let recon = self.recon.as_mut().expect("active domain guard");
                    let replies = recon.on_ids(&lo, &hi, &ids, host.ids(&peer));
                    (replies, recon.resource_exhausted())
                };
                if exhausted {
                    self.protocol_error(errcode::PROTOCOL);
                    return;
                }
                self.send_frames(replies);
                self.drain_wants();
                self.maybe_advance_domain(host);
            }
            Frame::ReconInit { .. } | Frame::ReconRange { .. } | Frame::ReconIds { .. } => {
                self.protocol_error(errcode::UNSUPPORTED)
            }
            Frame::Want(ids) => {
                self.queue_handoffs(ids);
                self.flush_pending_handoffs(host);
            }
            Frame::Bundle(bytes) => host.on_bundle(&peer, &bytes),
            Frame::Budget { id, granted } => {
                // Receive-side clamp (AUDIT F3) — silent, never an error.
                host.on_budget(&peer, &id, granted.min(L_MAX));
            }
            Frame::BlobReq { root, block, bitmap } => {
                self.queue_handoffs(host.on_blob_req(&peer, &root, block, &bitmap));
                self.flush_pending_handoffs(host);
            }
            Frame::BlobHave { root, block, bitmap } => {
                self.queue_handoffs(host.on_blob_have(&peer, &root, block, &bitmap));
                self.flush_pending_handoffs(host);
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

    fn accepts_recon_init(&self, domain: u8) -> bool {
        if self.initiator || !valid_recon_domain(domain, false) {
            return false;
        }
        let Some(current) = self.recon.as_ref() else {
            return domain == DOMAINS[0];
        };
        if current.domain == domain {
            return !current.is_done();
        }
        let Some(current_index) = DOMAINS.iter().position(|candidate| *candidate == current.domain) else {
            return false;
        };
        DOMAINS.get(current_index + 1).copied() == Some(domain) && current.is_done()
    }

    fn active_recon_matches(&self, domain: u8) -> bool {
        valid_recon_domain(domain, true)
            && self
                .recon
                .as_ref()
                .is_some_and(|recon| recon.domain == (domain & !DOMAIN_REPLY_FLAG))
    }

    fn start_domain<H: SessionHost>(&mut self, host: &mut H) {
        let peer = self.peer_node.expect("established");
        let domain = DOMAINS[self.domain_cursor];
        self.recon = Some(Reconciler::new(domain));
        self.final_wants_sent = false;
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
        } else if !self.final_wants_sent {
            self.final_wants_sent = true;
            if let Some(peer) = self.peer_node {
                // All domains reconciled: flush DAG-repair WANTs (doc 06 §5.3).
                let wants = host.wants(&peer);
                if !wants.is_empty() {
                    self.send_frame(&Frame::Want(wants));
                }
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

    fn queue_handoffs<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = Id16>,
    {
        for id in ids {
            if self.pending_handoffs.len() >= MAX_PENDING_HANDOFFS {
                break;
            }
            if !self.pending_handoffs.contains(&id) {
                self.pending_handoffs.push_back(id);
            }
        }
    }

    fn flush_pending_handoffs<H: SessionHost>(&mut self, host: &mut H) {
        let Some(peer) = self.peer_node else { return };
        while self.phase != Phase::Closed && self.tx_credits >= 2 {
            let Some(id) = self.pending_handoffs.pop_front() else { break };
            if let Some((bytes, granted)) = host.load_for_handoff(&peer, &id) {
                self.send_frame(&Frame::Bundle(bytes));
                // BUDGET immediately follows its BUNDLE (doc 10 §2.5).
                self.send_frame(&Frame::Budget { id, granted });
            }
        }
    }

    /// Terminate locally when the host has not drained immediate wire output.
    /// Clearing the queues before publishing `Closed` is fail-closed and
    /// prevents a recursive ERROR/CLOSE attempt from exceeding the bound.
    fn output_overflow(&mut self) {
        self.out.clear();
        self.pending_out.clear();
        self.finish_closed();
    }

    /// Queue a pre-encoded envelope (the two Noise handshake messages) under
    /// the same immediate-output bound as encrypted frames.
    fn queue_raw(&mut self, bytes: Vec<u8>) {
        if self.phase == Phase::Closed {
            return;
        }
        if self.out.len() >= MAX_OUT_FRAMES {
            self.output_overflow();
            return;
        }
        self.out.push_back(bytes);
    }

    fn send_frame(&mut self, f: &Frame) {
        if self.phase == Phase::Closed {
            return;
        }
        if !frame_wire_widths_valid(f) {
            // The public encoder is infallible for API compatibility, so
            // reject values whose collection lengths would truncate before
            // they can enter either pending or immediate output queues.
            self.protocol_error(errcode::PROTOCOL);
            return;
        }
        let control = matches!(f, Frame::Hs(_) | Frame::IdProof { .. } | Frame::Credit(_) | Frame::Error { .. } | Frame::Close);
        if self.out.len() >= MAX_OUT_FRAMES {
            self.output_overflow();
            return;
        }
        // Keep two slots available for a protocol-error ERROR+CLOSE pair.
        // Data that would consume the reserve fails closed instead of
        // allowing a later control response to grow the queue past its cap.
        if !control && self.out.len().saturating_add(OUT_CONTROL_RESERVE) >= MAX_OUT_FRAMES {
            self.protocol_error(errcode::PROTOCOL);
            return;
        }
        if !control && self.tx_credits == 0 {
            if self.pending_out.len() >= MAX_PENDING_FRAMES {
                self.pending_out.clear();
                self.protocol_error(errcode::PROTOCOL);
            } else {
                self.pending_out.push_back(f.clone());
            }
            return;
        }
        self.emit_frame(f, control);
    }

    fn emit_frame(&mut self, f: &Frame, control: bool) {
        if self.phase == Phase::Closed {
            return;
        }
        if self.out.len() >= MAX_OUT_FRAMES {
            self.output_overflow();
            return;
        }
        if !control && self.out.len().saturating_add(OUT_CONTROL_RESERVE) >= MAX_OUT_FRAMES {
            self.protocol_error(errcode::PROTOCOL);
            return;
        }
        let payload = f.encode_payload();
        if !frame_payload_within_cap(payload.len()) {
            self.protocol_error(errcode::OVERSIZE);
            return;
        }
        let Some(chan) = self.chan.as_mut() else {
            return; // pre-channel non-HS send is a programming error; drop
        };
        if let Ok(ct) = chan.encrypt(&payload) {
            if envelope_payload_within_cap(ct.len()) {
                self.queue_raw(write_envelope(f.typ(), &ct));
                if !control {
                    self.tx_credits = self.tx_credits.saturating_sub(1);
                }
            } else {
                self.protocol_error(errcode::OVERSIZE);
            }
        }
    }

    fn flush_pending_frames(&mut self) {
        while self.phase != Phase::Closed && self.tx_credits > 0 {
            let Some(frame) = self.pending_out.pop_front() else { break };
            let control = matches!(
                &frame,
                Frame::Hs(_) | Frame::IdProof { .. } | Frame::Credit(_) | Frame::Error { .. } | Frame::Close
            );
            self.emit_frame(&frame, control);
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
        if self.phase == Phase::Closed {
            return;
        }
        self.phase = Phase::Closed;
        self.hs = None;
        self.recon = None;
        self.events.push_back(SessionEvent::Closed);
    }
}

#[cfg(test)]
mod wire_budget_tests {
    use super::{
        envelope_payload_within_cap, frame_payload_within_cap, frame_wire_widths_valid, valid_recon_domain, Phase,
        SessionEngine, SessionEvent,
    };
    use mmp_types::consts::MAX_FRAME_PAYLOAD;
    use mmp_types::frame::{read_envelope, Frame, Hello, Range, MAX_ENVELOPE_PAYLOAD, FT_BUNDLE, FT_FRIEND_OFFER};
    use mmp_types::ids::Id16;
    use mmp_types::WireError;

    #[test]
    fn outbound_payload_budget_rejects_release_oversize_inputs() {
        assert!(frame_payload_within_cap(MAX_FRAME_PAYLOAD));
        assert!(!frame_payload_within_cap(MAX_FRAME_PAYLOAD + 1));
        assert!(envelope_payload_within_cap(MAX_ENVELOPE_PAYLOAD));
        assert!(!envelope_payload_within_cap(MAX_ENVELOPE_PAYLOAD + 1));
    }

    #[test]
    fn receive_caps_accept_exact_boundary_and_reject_one_byte_over() {
        let mut at_cap = vec![0u8; 3 + MAX_FRAME_PAYLOAD];
        at_cap[..2].copy_from_slice(&(MAX_FRAME_PAYLOAD as u16).to_le_bytes());
        at_cap[2] = FT_BUNDLE;
        assert!(matches!(read_envelope(&at_cap), Ok(Some(_))));

        let over = (MAX_FRAME_PAYLOAD + 1) as u16;
        let mut one_over = vec![0u8; 3 + over as usize];
        one_over[..2].copy_from_slice(&over.to_le_bytes());
        one_over[2] = FT_BUNDLE;
        assert_eq!(read_envelope(&one_over), Err(WireError::Oversize));
    }

    #[test]
    fn receive_caps_reject_aggregate_plaintext_before_frame_decode() {
        let payload = vec![0u8; MAX_FRAME_PAYLOAD + 1];
        assert_eq!(Frame::decode_payload(FT_FRIEND_OFFER, &payload), Err(WireError::Oversize));
    }

    #[test]
    fn outbound_encoder_width_guard_rejects_truncating_lengths() {
        let ranges = Frame::ReconRange {
            domain: 0,
            ranges: vec![Range { lo: Id16::MIN, hi: Id16::MAX, count: 0, fp: 0 }; 256],
        };
        assert!(!frame_wire_widths_valid(&ranges));

        let ids = vec![Id16::MIN; u16::MAX as usize + 1];
        assert!(!frame_wire_widths_valid(&Frame::ReconIds {
            domain: 0,
            lo: Id16::MIN,
            hi: Id16::MAX,
            ids: ids.clone(),
        }));
        assert!(!frame_wire_widths_valid(&Frame::Want(ids)));
        assert!(!frame_wire_widths_valid(&Frame::BlobReq {
            root: Id16::MIN,
            block: 0,
            bitmap: vec![0u8; u16::MAX as usize + 1],
        }));
        assert!(!frame_wire_widths_valid(&Frame::Error { code: 1, detail: vec![0u8; 257] }));
    }

    #[test]
    fn outbound_encoder_width_guard_closes_before_queueing_truncated_frame() {
        let local = mmp_crypto::Identity::from_seed(&[7u8; 32]);
        let hello = Hello { ver: 1, suites: vec![0x01], tier: 2, flags: 0, credits: 8, psm: 0 };
        let mut session = SessionEngine::responder(&local, &hello, 0).unwrap();
        session.phase = Phase::Established;
        session.tx_credits = 0;
        session.send_frame(&Frame::ReconRange {
            domain: 0,
            ranges: vec![Range { lo: Id16::MIN, hi: Id16::MAX, count: 0, fp: 0 }; 256],
        });

        assert_eq!(session.phase, Phase::Closed);
        assert!(matches!(session.poll_event(), Some(SessionEvent::Closed)));
        assert!(session.pending_out.is_empty());
        assert!(session.poll_out().is_none());
    }

    #[test]
    fn recon_domain_validation_rejects_unknown_and_misflagged_domains() {
        for domain in 0..4 {
            assert!(valid_recon_domain(domain, false));
            assert!(valid_recon_domain(domain | super::DOMAIN_REPLY_FLAG, true));
            assert!(!valid_recon_domain(domain | super::DOMAIN_REPLY_FLAG, false));
        }
        assert!(!valid_recon_domain(4, false));
        assert!(!valid_recon_domain(4 | super::DOMAIN_REPLY_FLAG, true));
        assert!(!valid_recon_domain(0x7f, true));
    }
}

#[cfg(test)]
mod rx_bound_tests {
    use super::*;
    use alloc::vec;
    use mmp_types::frame::{FT_CREDIT, FT_HS, FT_ID_PROOF, FT_SUMMARY};

    struct EmptyIds {
        ids: Vec<Id16>,
    }

    impl IdSource for EmptyIds {
        fn ids_in(&mut self, _domain: u8, _lo: &Id16, _hi: &Id16) -> Vec<Id16> {
            self.ids.iter().filter(|id| **id >= *_lo && **id <= *_hi).copied().collect()
        }

        fn has(&mut self, id: &Id16) -> bool {
            self.ids.binary_search(id).is_ok()
        }
    }

    struct EmptyHost {
        ids: EmptyIds,
        loads: usize,
        load_bytes: Option<Vec<u8>>,
        wants_calls: usize,
    }

    impl SessionHost for EmptyHost {
        type Ids = EmptyIds;

        fn ids(&mut self, _peer: &NodeId) -> &mut Self::Ids {
            &mut self.ids
        }

        fn summary(&mut self, _peer: &NodeId) -> Summary {
            Summary::default()
        }

        fn on_summary(&mut self, _peer: &NodeId, _summary: Summary) {}

        fn load_for_handoff(&mut self, _peer: &NodeId, _id: &Id16) -> Option<(Vec<u8>, u8)> {
            self.loads += 1;
            self.load_bytes.clone().map(|bytes| (bytes, 1))
        }

        fn on_bundle(&mut self, _peer: &NodeId, _bytes: &[u8]) {}

        fn on_budget(&mut self, _peer: &NodeId, _id: &Id16, _granted: u8) {}

        fn wants(&mut self, _peer: &NodeId) -> Vec<Id16> {
            self.wants_calls += 1;
            Vec::new()
        }

        fn accept_peer(&mut self, _identity: &Key32, _node: &NodeId) -> bool {
            true
        }
    }

    fn hello() -> Hello {
        Hello { ver: 1, suites: vec![0x01], tier: 2, flags: 0, credits: 8, psm: 0 }
    }

    fn identity(seed: u8) -> Identity {
        Identity::from_seed(&[seed; 32])
    }

    fn host() -> EmptyHost {
        EmptyHost { ids: EmptyIds { ids: Vec::new() }, loads: 0, load_bytes: None, wants_calls: 0 }
    }

    fn host_with_ids(ids: Vec<Id16>) -> EmptyHost {
        EmptyHost { ids: EmptyIds { ids }, loads: 0, load_bytes: None, wants_calls: 0 }
    }

    #[test]
    fn recon_init_is_role_and_order_fenced() {
        let initiator = identity(1);
        let responder = identity(2);
        let hello = hello();
        let a = SessionEngine::initiator(&initiator, &hello, 1000).unwrap();
        let mut b = SessionEngine::responder(&responder, &hello, 1000).unwrap();

        assert!(!a.accepts_recon_init(0), "only the negotiated initiator may open a domain");
        assert!(b.accepts_recon_init(0));
        b.recon = Some(Reconciler::new(0));
        b.recon.as_mut().expect("reconciler installed").open = 1;
        assert!(!b.accepts_recon_init(1), "a later domain cannot replace an unresolved domain");
        b.recon.as_mut().expect("reconciler installed").open = 0;
        assert!(b.accepts_recon_init(1), "the next domain is accepted after completion");
    }

    #[test]
    fn recon_init_for_later_domain_closes_without_priority_partition() {
        let local = identity(7);
        let peer = identity(8);
        let mut session = SessionEngine::responder(&local, &hello(), 1000).unwrap();
        session.phase = Phase::Established;
        session.peer_node = Some(peer.node_id());

        // A responder must finish the active D0 reconciliation before it can
        // accept the next base domain.  In particular, a peer cannot use a
        // later domain as an implicit declared-priority partition: the frozen
        // executable wire has no such negotiation or key.
        let mut active = Reconciler::new(DOMAINS[0]);
        active.open = 1;
        session.recon = Some(active);
        let mut host = host();
        session.on_established(
            Frame::ReconInit {
                domain: DOMAINS[3],
                window: 4,
                range: Range { lo: Id16::MIN, hi: Id16::MAX, count: 0, fp: 0 },
            },
            &mut host,
        );

        assert_eq!(session.phase, Phase::Closed);
        assert!(matches!(session.poll_event(), Some(SessionEvent::Closed)));
    }

    #[test]
    fn session_waits_for_terminal_child_before_advancing_domain() {
        let local = identity(3);
        let peer = identity(4);
        let hello = hello();
        let local_id = Id16([0x42; 16]);
        let mut session = SessionEngine::initiator(&local, &hello, 1000).unwrap();
        let mut host = host_with_ids(vec![local_id]);
        session.phase = Phase::Established;
        session.peer_node = Some(peer.node_id());

        let mut recon = Reconciler::new(DOMAINS[0]);
        let Frame::ReconInit { range: full, .. } = recon.init_frame(host.ids(&peer.node_id())) else {
            panic!("the initial frame must be a range");
        };
        session.recon = Some(recon);

        let partial = Range { lo: local_id, hi: local_id, count: 0, fp: crate::fingerprint(&[]) };
        session.on_established(
            Frame::ReconRange { domain: DOMAINS[0], ranges: vec![partial] },
            &mut host,
        );
        assert_eq!(session.domain_cursor, 0, "a partial child must not advance the domain");
        assert_eq!(session.recon.as_ref().expect("reconciler").open, 2);

        session.on_established(
            Frame::ReconIds { domain: DOMAINS[0], lo: local_id, hi: local_id, ids: Vec::new() },
            &mut host,
        );
        assert_eq!(session.domain_cursor, 0, "the parent remains incomplete after one terminal child");
        assert_eq!(session.recon.as_ref().expect("reconciler").open, 1);

        session.on_established(
            Frame::ReconRange { domain: DOMAINS[0] | DOMAIN_REPLY_FLAG, ranges: vec![full] },
            &mut host,
        );
        assert_eq!(session.domain_cursor, 1, "the next domain starts only after the child and parent resolve");
    }

    #[test]
    fn high_volume_asymmetric_reconciliation_keeps_session_alive() {
        let ia = identity(5);
        let ib = identity(6);
        let mut ids = Vec::with_capacity(500);
        for value in 0u16..500 {
            let mut bytes = [0u8; 16];
            bytes[14..].copy_from_slice(&value.to_be_bytes());
            ids.push(Id16(bytes));
        }
        let mut ha = host_with_ids(Vec::new());
        let mut hb = host_with_ids(ids);
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..1_000 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }

        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);
        assert_eq!(a.domain_cursor, DOMAINS.len() - 1, "all domains should be reached");
        assert!(a.final_wants_sent);
    }

    #[test]
    fn rbsr_resource_exhaustion_closes_the_session() {
        let local = identity(11);
        let peer = identity(12);
        let ids: Vec<Id16> = (0..25).map(|value| {
            let mut bytes = [0x80; 16];
            bytes[14..].copy_from_slice(&(value as u16).to_be_bytes());
            Id16(bytes)
        }).collect();

        // Valid-size descriptors with unique bounds force repeated child
        // splits. The reconciler must reject the bounded-resource violation
        // instead of retaining an unbounded authenticated-peer worklist.
        let mut reconciler = Reconciler::new(DOMAINS[0]);
        let mut source = host_with_ids(ids.clone());
        for value in 0..2_000u16 {
            let mut hi = [0xff; 16];
            hi[14..].copy_from_slice(&value.to_be_bytes());
            let range = Range { lo: Id16::MIN, hi: Id16(hi), count: 0, fp: 0 };
            reconciler.on_range(&range, false, &mut source.ids);
            if reconciler.resource_exhausted() {
                break;
            }
        }
        assert!(reconciler.resource_exhausted());

        let mut session = SessionEngine::initiator(&local, &hello(), 1000).unwrap();
        session.phase = Phase::Established;
        session.peer_node = Some(peer.node_id());
        session.recon = Some(reconciler);
        let mut host = host_with_ids(ids);
        session.on_established(
            Frame::ReconRange {
                domain: DOMAINS[0],
                ranges: vec![Range { lo: Id16::MIN, hi: Id16::MAX, count: 0, fp: 0 }],
            },
            &mut host,
        );

        assert_eq!(session.phase, Phase::Closed);
        assert!(matches!(session.poll_event(), Some(SessionEvent::Closed)));
    }

    #[test]
    fn final_wants_are_polled_once_per_session() {
        let local = identity(1);
        let peer = identity(2);
        let hello = hello();
        let mut session = SessionEngine::initiator(&local, &hello, 1000).unwrap();
        session.peer_node = Some(peer.node_id());
        session.domain_cursor = DOMAINS.len() - 1;
        session.recon = Some(Reconciler::new(DOMAINS[session.domain_cursor]));
        let mut host = host();

        session.maybe_advance_domain(&mut host);
        session.maybe_advance_domain(&mut host);
        assert_eq!(host.wants_calls, 1);
        assert!(session.final_wants_sent);
    }

    /// Drive only the Noise XX messages, leaving both engines in Binding with
    /// their encrypted ID_PROOF records still queued.  The proof itself is
    /// deliberately not delivered so boundary tests can inject a plaintext
    /// or pre-proof application record.
    fn drive_to_binding(
        a: &mut SessionEngine,
        b: &mut SessionEngine,
        ia: &Identity,
        ib: &Identity,
        ha: &mut EmptyHost,
        hb: &mut EmptyHost,
    ) {
        let msg1 = a.poll_out().expect("initiator must queue Noise msg1");
        assert_eq!(msg1.get(2), Some(&FT_HS));
        b.on_wire(&msg1, 1000, ib, hb);

        let msg2 = b.poll_out().expect("responder must queue Noise msg2");
        assert_eq!(msg2.get(2), Some(&FT_HS));
        a.on_wire(&msg2, 1000, ia, ha);

        let msg3 = a.poll_out().expect("initiator must queue Noise msg3");
        assert_eq!(msg3.get(2), Some(&FT_HS));
        b.on_wire(&msg3, 1000, ib, hb);

        assert_eq!(a.phase, Phase::Binding);
        assert_eq!(b.phase, Phase::Binding);
    }

    #[test]
    fn id_proof_is_noise_transport_not_plaintext() {
        let ia = identity(1);
        let ib = identity(2);
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();
        let mut ha = host();
        let mut hb = host();
        drive_to_binding(&mut a, &mut b, &ia, &ib, &mut ha, &mut hb);

        // A plaintext ID_PROOF envelope is not a valid post-handshake record;
        // the receiver must attempt Noise AEAD first and close on failure.
        let sig = a.chan.as_ref().expect("Noise channel exists").make_id_proof(&ia, true);
        let plaintext = Frame::IdProof { identity: ia.public(), sig }.encode();
        assert_eq!(plaintext.get(2), Some(&FT_ID_PROOF));
        b.on_wire(&plaintext, 1000, &ib, &mut hb);
        assert_eq!(b.phase, Phase::Closed);
        assert!(!b.events.iter().any(|event| matches!(event, SessionEvent::Established { .. })));

        // The actual queued proof is encrypted by the Noise transport and is
        // accepted on a fresh Binding pair.
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();
        let mut ha = host();
        let mut hb = host();
        drive_to_binding(&mut a, &mut b, &ia, &ib, &mut ha, &mut hb);
        let proof_a = a.poll_out().expect("initiator must queue encrypted ID_PROOF");
        assert_eq!(proof_a.get(2), Some(&FT_ID_PROOF));
        b.on_wire(&proof_a, 1000, &ib, &mut hb);
        assert_eq!(b.phase, Phase::Established);
        assert!(b.events.iter().any(|event| matches!(event, SessionEvent::Established { .. })));
    }

    #[test]
    fn application_frames_are_gated_until_id_proof() {
        let ia = identity(3);
        let ib = identity(4);
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();
        let mut ha = host();
        let mut hb = host();
        drive_to_binding(&mut a, &mut b, &ia, &ib, &mut ha, &mut hb);

        // Remove the queued proof but do not deliver it.  A Summary encrypted
        // under the Noise transport must still be rejected while Binding.
        let _proof_a = a.poll_out().expect("proof must be queued first");
        a.send_frame(&Frame::Summary(Summary::default()));
        let summary = a.poll_out().expect("encrypted pre-proof application frame");
        assert_eq!(summary.get(2), Some(&FT_SUMMARY));
        b.on_wire(&summary, 1000, &ib, &mut hb);

        assert_eq!(b.phase, Phase::Closed);
        assert!(!b.events.iter().any(|event| matches!(event, SessionEvent::Established { .. })));
    }

    #[test]
    fn oversized_transport_input_is_rejected_before_rx_growth() {
        let identity = identity(1);
        let mut engine = SessionEngine::initiator(&identity, &hello(), 1000).unwrap();
        let mut bytes = vec![0u8; MAX_ENVELOPE_PAYLOAD + 64];
        bytes[..2].copy_from_slice(&((MAX_ENVELOPE_PAYLOAD + 1) as u16).to_le_bytes());
        bytes[2] = FT_CREDIT;
        let mut peer = host();

        engine.on_wire(&bytes, 2000, &identity, &mut peer);

        assert_eq!(engine.phase, Phase::Closed);
        assert_eq!(engine.last_activity, 1000);
        assert!(engine.rx.is_empty());
        assert!(engine.rx.capacity() <= MAX_ENVELOPE_PAYLOAD + 3);
        assert!(matches!(engine.poll_event(), Some(SessionEvent::Closed)));
    }

    #[test]
    fn aggregate_input_cannot_grow_outbound_queue_without_bound() {
        let ia = identity(21);
        let ib = identity(22);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        // Build one bounded transport callback containing many valid,
        // independently encrypted frames. The receiver sends CREDIT replies
        // while consuming them; without an intrinsic `out` cap those replies
        // accumulated until the callback returned to the host.
        let mut aggregate = Vec::new();
        for _ in 0..8 {
            b.tx_credits = u8::MAX;
            for _ in 0..200 {
                b.send_frame(&Frame::Summary(Summary::default()));
            }
            while let Some(bytes) = b.poll_out() {
                aggregate.extend_from_slice(&bytes);
            }
        }
        a.on_wire(&aggregate, 2000, &ia, &mut ha);

        assert_eq!(a.phase, Phase::Closed, "output saturation must fail closed");
        assert!(a.out.len() <= MAX_OUT_FRAMES);
        assert!(a.events.iter().any(|event| matches!(event, SessionEvent::Closed)));
    }

    #[test]
    fn aggregate_frames_are_processed_without_growing_rx() {
        let ia = identity(1);
        let ib = identity(2);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        a.tx_credits = u8::MAX;
        for _ in 0..200 {
            a.send_frame(&Frame::Summary(Summary::default()));
        }
        let mut aggregate = Vec::new();
        while let Some(bytes) = a.poll_out() {
            aggregate.extend_from_slice(&bytes);
        }
        assert!(aggregate.len() > MAX_ENVELOPE_PAYLOAD + 3);

        b.on_wire(&aggregate, 2000, &ib, &mut hb);

        assert_eq!(b.phase, Phase::Established);
        assert!(b.rx.is_empty());
        assert!(b.rx.capacity() <= MAX_ENVELOPE_PAYLOAD + 3);
    }

    #[test]
    fn outbound_frames_stop_after_peer_credit_is_spent() {
        let ia = identity(3);
        let ib = identity(4);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        a.tx_credits = FRAME_CREDIT_WINDOW;
        for _ in 0..9 {
            a.send_frame(&Frame::Summary(Summary::default()));
        }
        let mut sent = 0;
        while a.poll_out().is_some() {
            sent += 1;
        }
        assert_eq!(sent, 8);
        assert_eq!(a.phase, Phase::Established);
    }

    #[test]
    fn received_credit_reopens_a_spent_send_window() {
        let ia = identity(5);
        let ib = identity(6);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        a.tx_credits = FRAME_CREDIT_WINDOW;
        b.rx_credits = FRAME_CREDIT_WINDOW;
        for _ in 0..8 {
            a.send_frame(&Frame::Summary(Summary::default()));
        }
        let mut wire = Vec::new();
        while let Some(bytes) = a.poll_out() {
            wire.extend_from_slice(&bytes);
        }

        a.send_frame(&Frame::Summary(Summary::default()));
        assert!(a.poll_out().is_none());

        b.on_wire(&wire, 2000, &ib, &mut hb);

        while let Some(bytes) = b.poll_out() {
            a.on_wire(&bytes, 2000, &ia, &mut ha);
        }
        a.send_frame(&Frame::Summary(Summary::default()));
        assert!(a.poll_out().is_some());
    }

    #[test]
    fn receive_window_replenishes_before_atomic_pair_deadlock() {
        let ia = identity(9);
        let ib = identity(10);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        a.tx_credits = 1;
        b.rx_credits = 3;
        a.send_frame(&Frame::Summary(Summary::default()));
        let bytes = a.poll_out().expect("one frame fits the remaining credit");
        b.on_wire(&bytes, 2000, &ib, &mut hb);
        while let Some(bytes) = b.poll_out() {
            a.on_wire(&bytes, 2000, &ia, &mut ha);
        }
        assert!(a.tx_credits >= 6, "reserve replenishment must arrive before zero");
    }

    #[test]
    fn noise_tag_is_counted_at_envelope_boundary() {
        let ia = identity(13);
        let ib = identity(14);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        // Complete the handshake and drain the ordinary SUMMARY/RBSR traffic
        // before exercising the boundary in isolation.
        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        // Noise transport adds a 16-byte tag.  A plaintext of 4080 bytes is
        // therefore exactly one maximum 4096-byte envelope payload and must
        // remain sendable.
        a.tx_credits = 1;
        a.send_frame(&Frame::FriendOffer(vec![0xa5; MAX_FRAME_PAYLOAD - 16]));
        let at_cap = a.poll_out().expect("4080-byte plaintext fits one envelope");
        assert_eq!(u16::from_le_bytes([at_cap[0], at_cap[1]]) as usize, MAX_ENVELOPE_PAYLOAD);
        b.on_wire(&at_cap, 2000, &ib, &mut hb);
        assert_eq!(b.phase, Phase::Established);

        // One additional plaintext byte would make the encrypted payload
        // exceed the frozen envelope cap; the sender must fail closed rather
        // than emit a truncated length or oversized record.
        a.tx_credits = 1;
        a.send_frame(&Frame::FriendOffer(vec![0xa5; MAX_FRAME_PAYLOAD - 15]));
        assert_eq!(a.phase, Phase::Closed);
        assert!(a.events.iter().any(|event| matches!(event, SessionEvent::Closed)));
    }

    #[test]
    fn bundle_requests_wait_for_two_frame_credits_before_loading() {
        let ia = identity(7);
        let ib = identity(8);
        let mut ha = host();
        let mut hb = host();
        let mut a = SessionEngine::initiator(&ia, &hello(), 1000).unwrap();
        let mut b = SessionEngine::responder(&ib, &hello(), 1000).unwrap();

        for _ in 0..200 {
            let mut moved = false;
            while let Some(bytes) = a.poll_out() {
                moved = true;
                b.on_wire(&bytes, 1000, &ib, &mut hb);
            }
            while let Some(bytes) = b.poll_out() {
                moved = true;
                a.on_wire(&bytes, 1000, &ia, &mut ha);
            }
            if !moved {
                break;
            }
        }
        assert_eq!(a.phase, Phase::Established);
        assert_eq!(b.phase, Phase::Established);

        ha.load_bytes = Some(vec![0x42, 0x24]);
        a.tx_credits = 1;
        a.on_established(Frame::Want(vec![Id16([9; 16])]), &mut ha);
        assert_eq!(ha.loads, 0);

        a.tx_credits = 2;
        a.on_established(Frame::Want(vec![Id16([9; 16])]), &mut ha);
        assert_eq!(ha.loads, 1);
        assert!(a.poll_out().is_some());
        assert!(a.poll_out().is_some());
    }
}
