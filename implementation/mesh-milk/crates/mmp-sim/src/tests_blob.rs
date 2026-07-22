//! E5-S1 blob-plane / file-transfer acceptance test (handbook §4.4 decision
//! D8's acceptance paragraph): A shares a file with B via relay R, over
//! contacts that can be cut short by class-4-carrying frames (see
//! `Sim::meet_with_bundle_loss`'s doc comment for exactly what "class-4
//! specifically" means at the wire-observable level, and why loss here means
//! "the contact ends early" rather than "this one frame vanishes and the
//! session carries on" — the latter breaks the Noise transport's AEAD nonce
//! sequencing). B reassembles the file bit-exact via the doc 11 §6 FSM
//! despite the loss; store quotas are respected throughout; a second
//! identical `send_file` mints no new class-4 bundles (dedup by content
//! address, decision D6).
//!
//! **Three interpretation calls (flagged per this repo's methodology), each
//! discovered by actually running this scenario rather than assumed:**
//!
//! 1. **Loss mechanism.** The decision doc's acceptance paragraph says "30%
//!    random drop of class-4 bundle frames." Implemented literally (silently
//!    discard a chosen `Frame::Bundle` envelope but keep pumping later
//!    frames on the same direction), every contact protocol-errored closed
//!    within a few frames, every time: `NoiseChannel` (`mmp-crypto`, wrapping
//!    `snow::TransportState`) uses a strict monotonic AEAD nonce counter per
//!    direction, standard Noise transport behavior — skipping one encrypted
//!    envelope while continuing to feed later ones desyncs the receiver's
//!    expected nonce from what the sender actually used, so every subsequent
//!    `decrypt()` on that direction fails. This is not a realistic loss model
//!    for THIS transport anyway: a real BLE ATT write/notify is locally
//!    reliable, so packet loss there manifests as a shortened or dropped
//!    CONNECTION, not a silently-skipped in-stream message.
//!    `Sim::meet_with_bundle_loss` therefore ends the WHOLE contact (both
//!    directions) the moment a would-be-dropped class-4-carrying frame is
//!    chosen, simulating a link that drops mid-transfer — realistic, and it
//!    never touches the Noise channel invariant.
//! 2. **Which hop carries the loss.** `Frame::Want`'s hand-off (doc 05 §2,
//!    `mmp-core/src/session.rs`) commits a bundle's spray-budget mutation
//!    for an ENTIRE incoming WANT batch synchronously, before any of the
//!    resulting `Frame::Bundle`s are actually drained onto the wire — so a
//!    contact ending mid-batch still leaves every batched item's budget
//!    spent, whether or not its bytes crossed the wire. At the router's
//!    `origin_budget` floor (4 — this sim's tiny population estimate never
//!    exceeds it) that is two halvings, i.e. two real "attempts" total,
//!    before the FOCUS rule's strict-superiority requirement
//!    (`u_peer > u_self + 0.15`) permanently stops retries — and `u_peer`
//!    for a same-utility, non-destination relay never clears that bar (no
//!    transitivity, by design, doc 05 §4). Applying any nonzero cutoff
//!    chance to the A→R hop this way strands the large majority of a
//!    160-symbol batch permanently within two contacts, regardless of how
//!    small the per-frame chance is or how many further contacts run. The
//!    R→B hop has no such ceiling: B is the bundle's actual destination, so
//!    `handoff_decision` takes its `peer_is_dest` branch unconditionally
//!    (doc 05 §2.5) and never touches the spray budget at all — retries
//!    there are genuinely unlimited. This test therefore keeps the A→R hop
//!    loss-free (it still needs to happen over the wire, subject to the
//!    same admission bucket as everything else) and applies the cutoff
//!    model to the R→B hop, which is where "B assembles the file across
//!    several contacts despite loss" is actually demonstrated. Both the
//!    WANT-batch-eager-budget-spend behavior and the two-attempt ceiling are
//!    real, pre-existing `mmp-router`/`mmp-core` characteristics surfaced by
//!    building this test, not E5-S1 bugs — distinct from the one rbsr.rs bug
//!    that WAS this story's to find and fix (`on_ids`, an unbounded terminal
//!    ID list that produced a real, session-killing oversized envelope,
//!    fixed in this same change).
//! 3. **File size.** 250 KB keeps the whole symbol set within R's
//!    single-contact admission-bucket burst (~196 symbols at the default
//!    WoT×4 rate, doc 05 §8), so the A→R hop's one loss-free contact is
//!    sufficient and bucket capacity never competes with the dynamic above.

use mmp_node::BlobState;

use crate::{Event, Sim};

#[test]
fn file_transfer_survives_lossy_relay_and_dedups() {
    const A: usize = 0;
    const R: usize = 1;
    const B: usize = 2;

    let mut sim = Sim::new(3, 0xF11E_7A1E_5EED_0001);

    // A's identity claim ingested VERIFIED at R and B, and vice versa (a
    // QR-style first-contact bootstrap) — needed both so A can encrypt to B
    // at all (the manifest is an ordinary class-1 message) and so every hop's
    // per-origin token bucket gets the WoT ×4 multiplier admitting A's
    // symbol flood (doc 05 §8).
    sim.introduce(A, R);
    sim.introduce(R, B);
    sim.introduce(A, B);

    // 250 KB: 4 stripes (STRIPE_DATA_BYTES = 64 KiB), a non-multiple length,
    // 160 symbols total (see the module doc comment, interpretation call 3).
    // Deterministic, non-trivial content (not all-zero).
    let data: Vec<u8> = (0..250_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8).collect();

    let b_id = sim.node_id(B);
    let sent = sim.node_mut(A).send_file(&b_id, "vacation.jpg", "image/jpeg", &data).expect("send_file");

    let dir = std::env::temp_dir().join("mmp-sim-blob-test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let dest = dir.join(format!("recv-{}.bin", std::process::id()));
    let _ = std::fs::remove_file(&dest);

    // A-R: loss-free (interpretation call 2). R-B: `CUTOFF_PCT` chance per
    // candidate class-4-carrying frame that the contact ends right there,
    // simulating a link dropping mid-transfer — small enough that a typical
    // contact still carries many symbols, but nonzero enough that this
    // 160-symbol file routinely needs several R-B contacts to fully arrive.
    const CUTOFF_PCT: u64 = 5;
    const ROUNDS: usize = 30;
    const DT_MS: u64 = 10 * 60_000;
    let mut fetch_started = false;
    let mut assembled = false;

    for round in 0..ROUNDS {
        sim.meet_with_bundle_loss(A, R, DT_MS, 0);
        let events = sim.meet_with_bundle_loss(R, B, DT_MS, CUTOFF_PCT);

        if !fetch_started {
            let offered =
                events.iter().any(|(idx, ev)| *idx == B && matches!(ev, Event::FileOffered { .. }));
            if offered {
                sim.node_mut(B).fetch_file(&sent.root, &dest).expect("fetch_file");
                fetch_started = true;
            }
        }

        assert!(sim.quota_ok(), "no node exceeded its store quota at round {round}");

        if let Some(p) = sim.node(B).file_progress(&sent.root) {
            if p.state == BlobState::Assembled {
                assembled = true;
                break;
            }
        }
    }

    assert!(fetch_started, "B never received the file offer across {ROUNDS} lossy rounds");
    assert!(
        assembled,
        "B never finished assembling the file across {ROUNDS} lossy rounds; final progress: {:?}",
        sim.node(B).file_progress(&sent.root)
    );

    let written = std::fs::read(&dest).expect("assembled file exists");
    assert_eq!(written, data, "reassembled bytes are bit-exact despite lossy, multi-contact relay");

    // Dedup by content address (decision D6): a second `send_file` of the
    // identical bytes mints exactly one new bundle in A's own store (a fresh
    // manifest message — content-addressed, so its ciphertext/BundleID still
    // differ per send) and zero new class-4 symbol bundles.
    let before = sim.store_len(A);
    let resent = sim.node_mut(A).send_file(&b_id, "vacation.jpg", "image/jpeg", &data).expect("resend");
    let after = sim.store_len(A);
    assert_eq!(resent.root, sent.root, "identical content ⇒ same Merkle root");
    assert_eq!(after, before + 1, "resend mints exactly one new bundle (the manifest), zero symbols");

    let _ = std::fs::remove_file(&dest);
}
