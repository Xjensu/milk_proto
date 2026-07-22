//! mmp-sim — discrete-event simulator (OQ-4/OQ-5, doc 12 §8).
//!
//! Owns N [`mmp_node::Node`]s over in-memory storage and drives them through
//! randomized pairwise "meetings": a contact opens a link, pumps wire bytes
//! back and forth until quiet (mirrors `mmp-node`'s own test harness in
//! `crates/mmp-node/src/tests.rs::contact`), then closes it and advances a
//! virtual millisecond clock. No radio, no wall-clock sleeps — a whole
//! multi-hour scenario runs in milliseconds of real time.
//!
//! The PRNG is a ~10-line xorshift64* — deterministic from a seed, no `rand`
//! dependency needed at this layer (doc 12 §8: simulator determinism is a
//! goal in itself, not just an implementation convenience).

use std::collections::HashSet;

use mmp_core::MemStore;
pub use mmp_node::{Event, Node, Tier};
use mmp_tal::{LinkQuality, TalEvent};
use mmp_types::consts::{MAX_BUNDLE, QUOTA_ANCHOR, QUOTA_STANDARD};
use mmp_types::frame::FT_BUNDLE;
use mmp_types::ids::{Id16, NodeId};

pub mod adversary;

/// Deterministic xorshift64* PRNG — good enough statistical spread for
/// picking meeting pairs and jittering the clock; not cryptographic.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1) // xorshift needs a nonzero state
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform value in `[0, bound)`.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

/// A whole-scenario counter block, exposed so scenario tests can assert on
/// outcomes beyond pass/fail (delivery ratio, quota headroom, ...).
#[derive(Debug, Default, Clone, Copy)]
pub struct Counters {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_delivered: u64,
    pub meetings_run: u64,
}

/// The simulator: N nodes, a virtual clock, and a contact schedule.
pub struct Sim {
    nodes: Vec<Node>,
    now_ms: u64,
    rng: Rng,
    next_link: u64,
    counters: Counters,
    /// BundleIDs of our own outgoing sends, to detect their `MessageReceived`
    /// / `Delivered` echoes exactly once each — event delivery is edge
    /// triggered (it does not replay on a later contact), so counters must
    /// be updated inside `meet()` itself rather than by a caller inspecting
    /// the returned event list after the fact.
    tracked: HashSet<Id16>,
    received_ids: HashSet<Id16>,
    delivered_ids: HashSet<Id16>,
}

impl Sim {
    /// Build `n` `Tier::Relay` nodes over independent `MemStore`s, seeded
    /// deterministically from `seed` (each node's identity seed is derived
    /// from the sim seed and its index, so the whole scenario is
    /// reproducible end to end).
    pub fn new(n: usize, seed: u64) -> Sim {
        let mut rng = Rng::new(seed);
        let mut nodes = Vec::with_capacity(n);
        for i in 0..n {
            let mut node_seed = [0u8; 32];
            let a = seed.wrapping_add(i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let b = rng.next_u64();
            node_seed[..8].copy_from_slice(&a.to_le_bytes());
            node_seed[8..16].copy_from_slice(&b.to_le_bytes());
            node_seed[16..24].copy_from_slice(&(i as u64).to_le_bytes());
            node_seed[24..32].copy_from_slice(&seed.to_le_bytes());
            nodes.push(Node::new(node_seed, Box::new(MemStore::new()), Tier::Relay));
        }
        Sim {
            nodes,
            now_ms: 0,
            rng,
            next_link: 1,
            counters: Counters::default(),
            tracked: HashSet::new(),
            received_ids: HashSet::new(),
            delivered_ids: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub fn node_id(&self, i: usize) -> NodeId {
        self.nodes[i].node_id()
    }

    pub fn store_len(&self, i: usize) -> usize {
        self.nodes[i].store_len()
    }

    /// Mutable access to one node — the adversary harness (E5-S7, handbook
    /// §4.8) needs this to enable diagnostics, tick a node directly outside a
    /// `meet()` pump, and drive `adversary::adversary_contact` against it.
    pub fn node_mut(&mut self, i: usize) -> &mut Node {
        &mut self.nodes[i]
    }

    /// Read-only access to one node (diagnostics inspection, id queries).
    pub fn node(&self, i: usize) -> &Node {
        &self.nodes[i]
    }

    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// Fraction of tracked sends whose `Delivered` receipt has been seen by
    /// the origin. `0.0` when nothing has been sent yet.
    pub fn delivery_ratio(&self) -> f64 {
        if self.counters.messages_sent == 0 {
            0.0
        } else {
            self.counters.messages_delivered as f64 / self.counters.messages_sent as f64
        }
    }

    /// Out-of-band claim exchange (QR ceremony equivalent) — mirrors
    /// `mmp-node`'s `tests.rs::introduce`.
    pub fn introduce(&mut self, a: usize, b: usize) {
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let (left, right) = self.nodes.split_at_mut(hi);
        let (na, nb) = (&mut left[lo], &mut right[0]);
        for c in na.own_claims() {
            nb.ingest_claim(c, true);
        }
        for c in nb.own_claims() {
            na.ingest_claim(c, true);
        }
        // Introduction ticks + events are plumbing, not scenario signal.
        na.tick(self.now_ms);
        nb.tick(self.now_ms);
        drain(na);
        drain(nb);
    }

    /// Send a text message `from` node `i` `to` node `j`; tracked for
    /// `delivery_ratio()`. Returns the BundleID so callers can also assert
    /// directly on `Event::Delivered { id }`.
    pub fn send_text(&mut self, from: usize, to: usize, text: &str) -> Id16 {
        let peer = self.nodes[to].node_id();
        let id = self.nodes[from].send_text(&peer, text).expect("peer claims known (introduce first)");
        self.tracked.insert(id);
        self.counters.messages_sent += 1;
        id
    }

    /// Run one meeting between nodes `i` and `j`: open a link, pump wire
    /// bytes until quiet (bounded — matches the node-level test harness),
    /// close the link, then advance the virtual clock by `dt_ms`. Returns
    /// the events both sides emitted, in `(node_index, event)` form, so
    /// scenario tests can inspect anything beyond the built-in counters.
    pub fn meet(&mut self, i: usize, j: usize, dt_ms: u64) -> Vec<(usize, Event)> {
        assert_ne!(i, j, "a node cannot meet itself");
        let link = self.next_link;
        self.next_link += 1;
        let now = self.now_ms;

        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let (left, right) = self.nodes.split_at_mut(hi);
        let (a, b) = (&mut left[lo], &mut right[0]);
        let (a_idx, b_idx) = if i < j { (i, j) } else { (j, i) };

        a.tick(now);
        b.tick(now);
        a.on_tal_event(TalEvent::LinkUp { link, initiator: true, quality: LinkQuality::default() });
        b.on_tal_event(TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() });
        for _ in 0..300 {
            let mut quiet = true;
            while let Some((l, bytes)) = a.poll_send() {
                quiet = false;
                b.on_tal_event(TalEvent::LinkData { link: l, bytes });
            }
            while let Some((l, bytes)) = b.poll_send() {
                quiet = false;
                a.on_tal_event(TalEvent::LinkData { link: l, bytes });
            }
            if quiet {
                break;
            }
        }
        a.on_tal_event(TalEvent::LinkDown { link });
        b.on_tal_event(TalEvent::LinkDown { link });

        let mut out = Vec::new();
        for (idx, n) in [(a_idx, &mut *a), (b_idx, &mut *b)] {
            for ev in drain(n) {
                match &ev {
                    Event::MessageReceived { id, .. }
                        if self.tracked.contains(id) && self.received_ids.insert(*id) =>
                    {
                        self.counters.messages_received += 1;
                    }
                    Event::Delivered { id }
                        if self.tracked.contains(id) && self.delivered_ids.insert(*id) =>
                    {
                        self.counters.messages_delivered += 1;
                    }
                    _ => {}
                }
                out.push((idx, ev));
            }
        }

        self.counters.meetings_run += 1;
        self.now_ms += dt_ms;
        out
    }

    /// Like [`Sim::meet`], but the contact can be cut short — simulating a
    /// radio link that drops mid-transfer — the moment a `Frame::Bundle`
    /// envelope would be sent, with probability `drop_pct/100` evaluated
    /// once per such envelope (E5-S1 blob-plane acceptance test, handbook
    /// §4.4). The frame TYPE byte (`bytes[2]`) sits outside the AEAD channel
    /// by construction (doc 10 §2) — the same wire-level inspection the
    /// handbook's bug ledger recommends for a test pump — so this can target
    /// `FT_BUNDLE` without decrypting.
    ///
    /// **Why "cut short" rather than "skip this one frame and keep going"**
    /// (an interpretation call, found while building this test): the
    /// session's post-handshake channel (`mmp_crypto::noise::NoiseChannel`,
    /// wrapping `snow::TransportState`) uses a strict monotonic AEAD nonce
    /// counter per direction — standard Noise transport behavior, not
    /// specific to this crate. Silently discarding an already-encrypted
    /// envelope while continuing to feed LATER envelopes on the same
    /// direction desyncs the receiver's expected nonce from the sender's
    /// actual one, so EVERY subsequent `decrypt()` on that direction fails
    /// and the session protocol-errors closed almost immediately — not a
    /// realistic packet-loss model, and not what any real BLE transport
    /// would do either (ATT writes/notifies are locally reliable; loss there
    /// manifests as a dropped or shortened CONNECTION, not a silently
    /// skipped in-stream message.) Ending the whole contact at the drop
    /// point — both directions, no more envelopes exchanged — mirrors a
    /// dropped connection instead, which is both realistic and leaves the
    /// Noise channel invariant untouched. It cannot distinguish class-4
    /// SYMBOL bundles from other classes riding the same frame type
    /// (manifest, receipts): at file-transfer scale that is a close proxy
    /// for "class-4 loss specifically" anyway (a multi-hundred-symbol
    /// transfer dwarfs the handful of other bundles in the same contact).
    pub fn meet_with_bundle_loss(&mut self, i: usize, j: usize, dt_ms: u64, drop_pct: u64) -> Vec<(usize, Event)> {
        assert_ne!(i, j, "a node cannot meet itself");
        assert!(drop_pct <= 100);
        let link = self.next_link;
        self.next_link += 1;
        let now = self.now_ms;

        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let (left, right) = self.nodes.split_at_mut(hi);
        let (a, b) = (&mut left[lo], &mut right[0]);
        let (a_idx, b_idx) = if i < j { (i, j) } else { (j, i) };

        a.tick(now);
        b.tick(now);
        a.on_tal_event(TalEvent::LinkUp { link, initiator: true, quality: LinkQuality::default() });
        b.on_tal_event(TalEvent::LinkUp { link, initiator: false, quality: LinkQuality::default() });
        'contact: for _ in 0..1200 {
            let mut quiet = true;
            while let Some((l, bytes)) = a.poll_send() {
                quiet = false;
                if Self::drop_bundle_frame(&bytes, drop_pct, &mut self.rng) {
                    break 'contact;
                }
                b.on_tal_event(TalEvent::LinkData { link: l, bytes });
            }
            while let Some((l, bytes)) = b.poll_send() {
                quiet = false;
                if Self::drop_bundle_frame(&bytes, drop_pct, &mut self.rng) {
                    break 'contact;
                }
                a.on_tal_event(TalEvent::LinkData { link: l, bytes });
            }
            if quiet {
                break;
            }
        }
        a.on_tal_event(TalEvent::LinkDown { link });
        b.on_tal_event(TalEvent::LinkDown { link });

        let mut out = Vec::new();
        for (idx, n) in [(a_idx, &mut *a), (b_idx, &mut *b)] {
            for ev in drain(n) {
                out.push((idx, ev));
            }
        }
        self.counters.meetings_run += 1;
        self.now_ms += dt_ms;
        out
    }

    fn drop_bundle_frame(envelope: &[u8], drop_pct: u64, rng: &mut Rng) -> bool {
        envelope.get(2) == Some(&FT_BUNDLE) && rng.below(100) < drop_pct
    }

    /// Run `count` meetings between uniformly random distinct node pairs,
    /// each followed by a `min_gap_ms..max_gap_ms` clock advance. Discards
    /// events (use `meet` directly when the scenario needs to inspect a
    /// specific contact).
    pub fn run_random_meetings(&mut self, count: usize, min_gap_ms: u64, max_gap_ms: u64) {
        let n = self.nodes.len() as u64;
        assert!(n >= 2, "need at least two nodes to meet");
        for _ in 0..count {
            let i = self.rng.below(n) as usize;
            let mut j = self.rng.below(n) as usize;
            while j == i {
                j = self.rng.below(n) as usize;
            }
            let span = max_gap_ms.saturating_sub(min_gap_ms).max(1);
            let dt = min_gap_ms + self.rng.below(span);
            self.meet(i, j, dt);
        }
    }

    /// Conservative per-node quota check: `store_len * MAX_BUNDLE` is an
    /// upper bound on `total_bytes` (every stored record is ≤ `MAX_BUNDLE`
    /// wire bytes), and `Node::tick` already self-enforces the real byte
    /// quota internally (doc 04 §2) — `Node` has no public `total_bytes()`
    /// getter, so this is the tightest check available from outside.
    pub fn quota_ok(&self) -> bool {
        self.nodes.iter().all(|n| {
            let quota = if n.tier() == Tier::Anchor { QUOTA_ANCHOR } else { QUOTA_STANDARD };
            (n.store_len() as u64) * (MAX_BUNDLE as u64) <= quota
        })
    }
}

fn drain(n: &mut Node) -> Vec<Event> {
    std::iter::from_fn(|| n.poll_event()).collect()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_adversarial;
#[cfg(test)]
mod tests_blob;
