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
pub mod energy;

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

/// Deterministic clustered-contact model for simulation experiments.
///
/// Nodes are partitioned into equal-sized communities. Most contacts stay
/// inside the currently active community; every `bridge_every`th contact
/// crosses to the next community. This is a deliberately small, explicit
/// approximation of clustered human mobility, not a claim that the model is
/// SLAW-calibrated. The seed still controls all choices, so a run is exactly
/// reproducible.
pub struct ClusteredMobility {
    communities: Vec<Vec<usize>>,
    bridge_every: usize,
    contact_index: usize,
}

impl ClusteredMobility {
    /// Build a plan for `node_count` nodes and `community_count` communities.
    /// Every community must contain at least two nodes so an intra-community
    /// contact is always possible; `bridge_every` must be non-zero.
    pub fn new(node_count: usize, community_count: usize, bridge_every: usize) -> Option<Self> {
        if community_count < 2
            || bridge_every == 0
            || node_count < community_count.saturating_mul(2)
        {
            return None;
        }
        let mut communities = vec![Vec::new(); community_count];
        for node in 0..node_count {
            communities[node % community_count].push(node);
        }
        Some(Self { communities, bridge_every, contact_index: 0 })
    }

    /// Select the next `(left, right, is_bridge)` contact.
    pub fn next_pair(&mut self, rng: &mut Rng) -> (usize, usize, bool) {
        let community = (self.contact_index / self.bridge_every) % self.communities.len();
        let is_bridge = self.contact_index.is_multiple_of(self.bridge_every);
        let pair = if is_bridge {
            let next = (community + 1) % self.communities.len();
            let left = self.pick(&self.communities[community], rng);
            let right = self.pick(&self.communities[next], rng);
            (left, right, true)
        } else {
            let members = &self.communities[community];
            let first = self.pick(members, rng);
            let offset = 1 + rng.below((members.len() - 1) as u64) as usize;
            let first_index = members.iter().position(|node| *node == first).expect("picked member");
            (first, members[(first_index + offset) % members.len()], false)
        };
        self.contact_index = self.contact_index.saturating_add(1);
        pair
    }

    fn pick(&self, members: &[usize], rng: &mut Rng) -> usize {
        members[rng.below(members.len() as u64) as usize]
    }
}

/// Deterministic social-cluster contact model for controlled experiments.
///
/// Unlike [`ClusteredMobility`]'s periodic bridge cadence, this model keeps a
/// latent active community and takes a seeded bridge transition with the
/// configured per-mille probability. A bridge moves the latent activity to a
/// different community; otherwise both endpoints are sampled from the active
/// community. This makes dwell/bridge distributions explicit without claiming
/// to reproduce SLAW, human traces, or any production routing behavior.
pub struct SocialClusteredMobility {
    communities: Vec<Vec<usize>>,
    bridge_probability_per_mille: u16,
    active_community: usize,
    bridge_count: usize,
}

impl SocialClusteredMobility {
    /// Build a seeded-model-independent plan for balanced social communities.
    ///
    /// `bridge_probability_per_mille` is in `0..=1000`; zero is a useful
    /// disconnected-cluster control and 1000 forces a bridge every contact.
    /// Every community contains at least two nodes so intra-community pairs
    /// are always valid.
    pub fn new(
        node_count: usize,
        community_count: usize,
        bridge_probability_per_mille: u16,
    ) -> Option<Self> {
        if community_count < 2
            || bridge_probability_per_mille > 1_000
            || node_count < community_count.saturating_mul(2)
        {
            return None;
        }
        let mut communities = vec![Vec::new(); community_count];
        for node in 0..node_count {
            communities[node % community_count].push(node);
        }
        Some(Self {
            communities,
            bridge_probability_per_mille,
            active_community: 0,
            bridge_count: 0,
        })
    }

    /// Configured probability of a bridge contact in per-mille units.
    pub fn bridge_probability_per_mille(&self) -> u16 {
        self.bridge_probability_per_mille
    }

    /// Number of bridge contacts emitted so far.
    pub fn bridge_count(&self) -> usize {
        self.bridge_count
    }

    /// Select the next `(left, right, is_bridge)` contact.
    pub fn next_pair(&mut self, rng: &mut Rng) -> (usize, usize, bool) {
        let bridge = rng.below(1_000) < u64::from(self.bridge_probability_per_mille);
        let current = self.active_community;
        let (left, right, is_bridge) = if bridge {
            let offset = 1 + rng.below((self.communities.len() - 1) as u64) as usize;
            let destination = (current + offset) % self.communities.len();
            let left = self.pick(&self.communities[current], rng);
            let right = self.pick(&self.communities[destination], rng);
            self.active_community = destination;
            self.bridge_count = self.bridge_count.saturating_add(1);
            (left, right, true)
        } else {
            let members = &self.communities[current];
            let first = self.pick(members, rng);
            let offset = 1 + rng.below((members.len() - 1) as u64) as usize;
            let first_index = members.iter().position(|node| *node == first).expect("picked member");
            (first, members[(first_index + offset) % members.len()], false)
        };
        (left, right, is_bridge)
    }

    fn pick(&self, members: &[usize], rng: &mut Rng) -> usize {
        members[rng.below(members.len() as u64) as usize]
    }
}

/// Metrics returned by one social-cluster schedule run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocialClusteredRunMetrics {
    pub meetings: usize,
    pub bridge_meetings: usize,
    pub bridge_probability_per_mille: u16,
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

/// Contact-generator labels for research-only paired comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactGeneratorKind {
    UniformRandom,
    PeriodicClustered,
    SocialClustered,
}

/// Comparable simulator metrics captured after one contact-generator run.
///
/// This is an observation bundle for experiment reports, not a protocol or
/// routing input. The optional bridge count is populated only by generators
/// that expose bridge transitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactScenarioMetrics {
    pub generator: ContactGeneratorKind,
    pub meetings: u64,
    pub bridge_meetings: Option<usize>,
    pub simulated_elapsed_ms: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_delivered: u64,
    pub delivery_ratio: f64,
    pub p50_latency_ms: Option<u64>,
    pub p90_latency_ms: Option<u64>,
    pub store_high_water_bytes: u64,
    pub quota_ok: bool,
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
    /// `now_ms` at the moment each tracked id was sent (E6-S1, doc 13 §5) —
    /// needed to turn a `Delivered` event into an end-to-end latency sample.
    sent_at_ms: std::collections::HashMap<Id16, u64>,
    /// One sample per tracked id, pushed exactly once (`delivered_ids` above
    /// already gates the "exactly once" edge-triggering) at the moment its
    /// `Delivered` receipt is observed.
    delivery_latencies_ms: Vec<u64>,
    /// Running max of `store_len(i) * MAX_BUNDLE` across every node touched
    /// by a `meet()` so far — the same conservative per-node upper bound
    /// `quota_ok()` uses, tracked as a peak rather than a final snapshot
    /// (doc 13 §3's "store high-water" column is a peak-over-the-run figure,
    /// not an end-of-run one).
    store_high_water_bytes: u64,
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
            sent_at_ms: std::collections::HashMap::new(),
            delivery_latencies_ms: Vec::new(),
            store_high_water_bytes: 0,
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
    /// Whether the origin has seen `id`'s delivery receipt yet — per-message
    /// detail beneath [`Sim::delivery_ratio`]'s aggregate, for scenarios that
    /// need to compare delivery across subsets of tracked sends (E6-S3's
    /// near-carriage-vs-far-carriage misdeployment check, doc 17 §8 D-8).
    pub fn is_delivered(&self, id: &Id16) -> bool {
        self.delivered_ids.contains(id)
    }

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
        self.sent_at_ms.insert(id, self.now_ms);
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
                        if let Some(sent) = self.sent_at_ms.get(id) {
                            self.delivery_latencies_ms.push(self.now_ms.saturating_sub(*sent));
                        }
                    }
                    _ => {}
                }
                out.push((idx, ev));
            }
        }

        for i in [a_idx, b_idx] {
            let bytes = self.nodes[i].store_len() as u64 * MAX_BUNDLE as u64;
            self.store_high_water_bytes = self.store_high_water_bytes.max(bytes);
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

    /// Run a deterministic clustered-contact schedule. This is a simulation
    /// model for the open SLAW/clustered-mobility measurement gap, not a
    /// production routing behavior. Invalid community parameters leave the
    /// simulator unchanged and return `false`.
    pub fn run_clustered_meetings(
        &mut self,
        count: usize,
        min_gap_ms: u64,
        max_gap_ms: u64,
        community_count: usize,
        bridge_every: usize,
    ) -> bool {
        let Some(mut plan) = ClusteredMobility::new(self.nodes.len(), community_count, bridge_every) else {
            return false;
        };
        let span = max_gap_ms.saturating_sub(min_gap_ms).max(1);
        for _ in 0..count {
            let (i, j, _) = plan.next_pair(&mut self.rng);
            let dt = min_gap_ms + self.rng.below(span);
            self.meet(i, j, dt);
        }
        true
    }

    /// Run a deterministic social-cluster schedule and return only generator
    /// metrics. This is a research instrument for comparing contact traces;
    /// it leaves routing, copy budgets, and wire behavior untouched.
    pub fn run_social_clustered_meetings(
        &mut self,
        count: usize,
        min_gap_ms: u64,
        max_gap_ms: u64,
        community_count: usize,
        bridge_probability_per_mille: u16,
    ) -> Option<SocialClusteredRunMetrics> {
        let mut plan = SocialClusteredMobility::new(
            self.nodes.len(),
            community_count,
            bridge_probability_per_mille,
        )?;
        let span = max_gap_ms.saturating_sub(min_gap_ms).max(1);
        for _ in 0..count {
            let (i, j, _) = plan.next_pair(&mut self.rng);
            let dt = min_gap_ms + self.rng.below(span);
            self.meet(i, j, dt);
        }
        Some(SocialClusteredRunMetrics {
            meetings: count,
            bridge_meetings: plan.bridge_count(),
            bridge_probability_per_mille: plan.bridge_probability_per_mille(),
        })
    }

    /// One conductor round (doc 17 §3.2, E6-S3): `conductor` meets each node
    /// in `stops` **in order**, full `meet()` contacts (not random pairing) —
    /// the scheduled, deterministic topology a physically-walking conductor
    /// imposes, as opposed to [`Sim::run_random_meetings`]'s uniform-random
    /// pairing. `travel_ms` advances the clock after each stop (time to walk
    /// to the next carriage); does not loop — call again for another round.
    pub fn run_conductor_round(&mut self, conductor: usize, stops: &[usize], travel_ms: u64) {
        for &stop in stops {
            self.meet(conductor, stop, travel_ms);
        }
    }

    /// End-to-end delivery latency (send → origin sees the receipt) for
    /// every tracked message delivered so far, in simulated milliseconds —
    /// the raw samples doc 13 §5's p50/p90 latency bands are computed over.
    pub fn delivery_latencies_ms(&self) -> &[u64] {
        &self.delivery_latencies_ms
    }

    /// The `p`-th percentile (`0.0..=1.0`) of [`Sim::delivery_latencies_ms`],
    /// nearest-rank on a sorted copy. `None` with no delivered samples yet.
    pub fn latency_percentile_ms(&self, p: f64) -> Option<u64> {
        if self.delivery_latencies_ms.is_empty() {
            return None;
        }
        let mut sorted = self.delivery_latencies_ms.clone();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1).min(sorted.len() - 1);
        Some(sorted[idx])
    }

    /// Peak, over every `meet()` so far, of `store_len * MAX_BUNDLE` for any
    /// single node — doc 13 §3's "store high-water" figure (the same
    /// conservative per-record upper bound `quota_ok()` uses, tracked as a
    /// running max rather than an end-of-run snapshot).
    pub fn store_high_water_bytes(&self) -> u64 {
        self.store_high_water_bytes
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

    /// Capture a deterministic, comparable metric block for a research run.
    /// `bridge_meetings` is supplied by a generator such as
    /// [`SocialClusteredMobility`] and is otherwise `None`.
    pub fn research_metrics(
        &self,
        generator: ContactGeneratorKind,
        bridge_meetings: Option<usize>,
    ) -> ContactScenarioMetrics {
        ContactScenarioMetrics {
            generator,
            meetings: self.counters.meetings_run,
            bridge_meetings,
            simulated_elapsed_ms: self.now_ms,
            messages_sent: self.counters.messages_sent,
            messages_received: self.counters.messages_received,
            messages_delivered: self.counters.messages_delivered,
            delivery_ratio: self.delivery_ratio(),
            p50_latency_ms: self.latency_percentile_ms(0.50),
            p90_latency_ms: self.latency_percentile_ms(0.90),
            store_high_water_bytes: self.store_high_water_bytes,
            quota_ok: self.quota_ok(),
        }
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
#[cfg(test)]
mod tests_scale;
#[cfg(test)]
mod tests_deployment;
