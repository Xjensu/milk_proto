# Audit of the supplied MMP/Φ proposal

Date: 2026-08-08
Scope: supplied external dialogue, frozen MMP/1 documents, and the current Rust workspace.
Decision: MMP/Φ is a useful research backlog, but it is not evidence for changing the MMP/1 wire contract or for claiming a 2× production improvement.

## Executive conclusion

The dialogue correctly recognizes the strongest parts of MMP/1: delay-tolerant store-carry-forward, a hint-only beacon plane, verify-before-store/forward, bounded replication, and an explicit treatment of metadata and platform limitations. Those conclusions are consistent with the local specification and with the DTN/Noise/BLE references listed below.

It also identifies real issues, but several statements are too strong or mix a promising hypothesis with a demonstrated result. The most important confirmed issue is a normative contradiction about `ID_PROOF`: the crypto document says it is sent inside the encrypted Noise transport, while the message-format document calls it a pre-channel parser. The Rust session engine already implements the encrypted interpretation. This must be resolved as an erratum before independent implementations are generated; the frozen specification has not been edited in this pass.

The proposed energy, mobility-voucher, deadline-routing, and fountain-coding mechanisms should therefore be treated as experimentally gated extensions. They may improve utility or battery cost, but none currently establishes a production 2× result. The immediate implementation plan remains security/lifecycle hardening, Linux/BlueZ evidence, and conformance vectors.

## Claims confirmed against the project

### Strong architectural choices

1. **DTN rather than synchronous Bluetooth mesh.** MMP/1 explicitly models contacts as short, intermittent opportunities and uses store-carry-forward bundles. This is the right abstraction for mobile BLE; BPv7 likewise recommends bundle age for nodes without accurate clocks [A7].
2. **Beacon as hint plane.** The specification says beacons are unauthenticated hints and that durable behavior follows only after authenticated interaction. This is a sound separation of cheap discovery from expensive authenticated state.
3. **Verify before store/forward.** Inline origin keys plus canonical, domain-separated signatures make verification independent of prior claim gossip. The current `mmp-core` ingest path follows that order.
4. **Bounded replication.** Binary Spray-and-Wait with local utility avoids unconditional epidemic flooding. The design deliberately rejects unauthenticated third-party utility claims; that is a legitimate poisoning-resistance trade-off.
5. **Honest security boundaries.** The documents explicitly state that pseudonymity is not anonymity, that TOFU can be attacked, that metadata remains visible, and that RF jamming is out of scope. The dialogue correctly treats this honesty as a strength.

## Findings and corrections

| Priority | Dialogue observation | Verdict | Evidence and required action |
|---|---|---|---|
| P0 | `ID_PROOF` is both encrypted and pre-channel. | **Confirmed contradiction.** | `specification/08-crypto-spec.md §3.2` and the Rust `SessionEngine` send/verify it as a Noise transport frame. `specification/10-message-format.md §2` and §2.2 still describe it as a pre-channel parser. Noise itself distinguishes handshake messages from post-handshake AEAD transport messages [F1]. File an erratum: HS1/HS2/HS3 are the only pre-transport frames; `ID_PROOF` is the first encrypted transport frame and gates all application frames. Add an independent vector. |
| P0 | Per-origin buckets restore global copy-budget conservation. | **Not valid as stated.** | `mmp-router` conserves budget only in an honest local spray tree. `OriginBuckets` are receiver-local and keyed by origin; they do not create a globally spend-once capability. A Byzantine relay can present the same inflated grant to multiple honest receivers. Keep the clamp and local quotas, but narrow the security invariant to: “each conforming receiver locally bounds admitted origin bytes; global Byzantine copy conservation is not guaranteed.” This is an erratum candidate, not a router rewrite. |
| P0 | AEAD AD must normalize mutable lifetime. | **Confirmed and implemented in Rust; spec unchanged.** | `specification/10 §4.1` says AD is the bundle header, while `specification/03` defines lifetime-zeroed canonical bytes. `Bundle::canonical_aead_ad()` now binds the header prefix through `payload_len`, zeroes lifetime, and is used by direct/async and group seal/open paths. The exact boundary and residual vector work are recorded in `docs/AEAD_AD_CONFORMANCE.md`. |
| P1 | The time model is contradictory. | **Mostly a terminology problem, with a real boundary.** | Bundle aging and diagnostics use monotonic elapsed time. Authority/announcement `not_after` is intentionally receiver-local wall-clock validity (`mmp-types/src/authcert.rs`, doc 18). “No global clock” does not mean “no wall clock”; it means wall time must not be load-bearing for bundle aging or causal order. Add an explicit clock-trust table and rollback/forward-jump tests. |
| P1 | Missing `ROADMAP.md`, `AUDIT.md`, and council artifacts make the suite unreproducible. | **Branch/publication issue, not current-local absence.** | The current checkout contains tracked root `ROADMAP.md`, `CHANGELOG.md`, and `COUNCIL-REVIEW.md`; the public GitHub `master` URLs for those three files currently return 404. Release publication/branch parity is therefore a real gate. Add a release check that all cited artifacts exist in the published ref; do not duplicate them inside `specification/`. |
| P1 | “FINAL” conflicts with phase-draft headers. | **Confirmed status ambiguity.** | `specification/01` says FINAL while docs 03–14 retain “Phase N draft” and many constants remain ⚠ provisional. Use a manifest/status table to distinguish frozen wire rules, provisional constants, historical phase labels, and extension documents. This is documentation governance, not a wire change. |
| P1 | 64-bit RBSR fingerprints should become 128-bit. | **Plausible but unproven.** | The current spec explicitly budgets a 2⁻⁶⁴ random collision per comparison and uses BLAKE3 over an ID fold plus count. Whether 64 bits is adequate depends on the lifetime number of comparisons and recovery behavior. First measure comparison counts and add a collision/recovery simulation; upgrade only if the measured risk justifies the extra 8 bytes per fingerprint. |
| P1 | Published energy figures prove a 3.14× EDR gain. | **Not established.** | The dialogue treats daily totals (`0.12` mAh advertising and `4.5` mAh baseline scan) as if they were independent linear coefficients and assumes discovery hazard `h ∝ λρ`. The project ledger instead derives those totals from event current, scan duty, OS/system overhead `k_sys∈[4,12]`, collisions, batching, and burst scans (`specification/13 §1`). The square-root optimum is a useful controlled experiment, not a production result. |
| P1 | Bluetooth 6 Monitoring Advertisers/Decision-Based Filtering guarantees the EDR battery win. | **Capability confirmed; outcome not.** | Bluetooth Core 6.0 documents controller filtering and advertiser monitoring, but those features do not by themselves establish phone-level energy savings or cross-platform availability [I15]. Monitoring Advertisers is address/RSSI-threshold based, while MMP rotates ephemeral identifiers, so controller compatibility with the beacon privacy profile must be measured rather than assumed. Android 16 exposes a capability-discovery API for Bluetooth Channel Sounding [I11]; iOS and IoT support remain platform-dependent. Treat EDR as an optional profile with measured discovery probability and whole-phone energy. |
| P1 | Signed mobility vouchers safely restore transitive routing. | **Research idea, not a security proof.** | A voucher can prove that an issuer signed an encounter statement; it does not prove that the carrier will meet the destination before expiry. Collusion, stale vouchers, replay, privacy leakage, and issuer availability remain. Prototype only with explicit issuer trust, freshness, collusion, and privacy models; do not feed it into normative routing in v1. RFC 6693 confirms that PRoPHET uses encounter history and transitivity, but does not make those claims cryptographically trustworthy [C9]. |
| P1 | Evidence-Weighted Spray reduces 22 random carriers to four high-quality carriers. | **Mathematically conditional.** | `1−∏(1−p_i)` assumes calibrated probabilities and independent delivery paths. Human mobility is correlated, so the numerical 5.5× example is illustrative, not a measurement. Compare against current binary spray with trace-driven simulation and confidence intervals; keep copy budgets unchanged until the comparison is reproducible. |
| P1 | Fourier/non-homogeneous-Poisson mobility gives useful ETA. | **Plausible, high privacy/model risk.** | Periodicity may exist, but contact processes are bursty, non-stationary, and socially correlated. Any ETA must expose uncertainty and avoid leaking routines. Start with an on-device encounter-rate baseline and evaluate calibration before Fourier features. |
| P1 | RSSI slope and Channel Sounding can predict residual contact time. | **Strong research direction, not a guarantee.** | Bluetooth CS provides PBR/RTT measurements over a connected 1:1 link, but the SIG does not define an application distance algorithm and hardware support is required [I3]. RSSI is explicitly weak in multipath environments (the project’s H3 source). Implement a deadline scheduler that degrades to RSSI/history and benchmark against an oracle. |
| P2 | Replace RS symbols with “DoF Spray” fountain coding. | **Technically credible, not yet a win.** | RaptorQ is a standardized fountain code [E4] and NASA/CSU report DTN fountain-code benefits [K5]. MMP already has signed Merkle-rooted Reed–Solomon symbols. A switch changes CPU/RAM, symbol authentication, scheduling, and possible IPR exposure. Compare both schemes under the same contact traces before any wire allocation. |

## What is genuinely novel versus what is composition

The dialogue’s strongest research contribution is the composition of three existing ingredients: local energy-aware rendezvous, contact-window prediction, and probabilistic DTN forwarding. That composition may be publishable after a reproducible baseline comparison. It is not yet a “discovery,” and the proposed numerical gains are hypotheses.

The mobility-voucher and phase-coherence ideas are hypotheses with interesting experimental questions. They should be labelled as such in future research notes. The project should avoid claiming “new” or “2×” until a pre-registered simulator/field benchmark reports confidence intervals against the current MMP/1 profile.

## Three-pass validation record

The amended plan was checked in three separate passes, with the plan revised
after each pass:

1. **Repository pass.** The normative documents, Rust session/router/transport
   code, tests, and current handoff were compared field by field. This found
   the `ID_PROOF` and AEAD-associated-data mismatches, the receiver-local
   nature of copy quotas, the actual 64-bit RBSR choice, and the real Linux
   evidence boundary.
2. **External-evidence pass.** Noise, BPv7, PRoPHET, Bluetooth Core 6.0 and
   Channel Sounding, Android ranging, iOS background BLE, RaptorQ, and DTN
   fountain-code references were checked from authoritative URLs. Capability
   claims were retained, while unmeasured energy and delivery claims were
   downgraded to hypotheses.
3. **Adversarial implementation pass.** The proposed Byzantine, replay,
   collusion, correlation, privacy, multipath, OS-throttling, and resource
   failure modes were applied to the amended work order. The result keeps the
   v1 wire contract unchanged, puts conformance/security before optimization,
   and requires reproducible baselines and confidence intervals before any
   “2×” or production-readiness claim.

## Amended implementation plan

The current production plan is amended in this order:

1. **Conformance/security P0:** resolve the encrypted `ID_PROOF` wording, define canonical AEAD AD, and narrow the copy-budget invariant. Add independent vectors and Byzantine double-grant tests without changing the wire format.
2. **Lifecycle/resource P0:** retain the completed ingress hardening; next add transport-level link admission/close feedback so Node quota denial cannot leave a physical BLE link alive. Keep stream-end sweeps and bounded external BLE awaits covered by tests.
3. **Evidence P1:** run Linux/BlueZ two-PC interoperability, capture negotiated MTU/RSSI/reconnect/receipt logs, and close the manual test gate. Mobile remains artifact/API-only until a native consuming shell exists.
4. **Measurement P1:** build an `mmp-sim` experiment matrix for (a) current binary spray, (b) local evidence-weighted scheduling with unchanged budgets, (c) EDR duty-cycle points, and (d) residual-contact deadlines. Report delivery, p50/p90 latency, radio/controller energy, whole-phone energy, airtime, and storage—not one aggregate “2×” number.
5. **Research P1:** prototype mobility vouchers and calibrated ETA behind feature flags; include collusion, replay, privacy, correlated-contact, and stale-model scenarios. No v1 wire change.
6. **File-plane P2:** compare Reed–Solomon and fountain/DoF coding under real BLE contact traces and embedded resource limits. Preserve the existing signed Merkle root until a complete replacement contract and vectors exist.
7. **Release governance P1:** add a specification manifest/status table and a published-ref link check for `ROADMAP.md`, `CHANGELOG.md`, and `COUNCIL-REVIEW.md`.

8. **Host resource policy P0/P1:** expose complete-snapshot quotas to FFI
   consumers, keep parser and beacon-population allocations bounded under
   unauthenticated input, and test fail-closed persistence behavior. These are
   receiver-local controls; they must not be presented as global Byzantine
   copy-budget enforcement.
9. **Platform drive-loop P1:** keep the native radio adapters outside the Rust
   wire/core layers, but provide a host-testable Kotlin coordinator for
   monotonic ticking, callback ordering, MTU-minus-three fragmentation,
   back-pressure, stale-link filtering, and foreground relay demotion. Mark
   source-only validation separately from Kotlin compilation and physical BLE
   evidence.
10. **Callback backpressure P0:** bound every application-event producer behind
    one FIFO helper and expose saturation to FFI hosts. The current safe
    interim policy preserves queued FIFO entries and drops newest callbacks;
    it must remain an explicitly documented overload condition, not a claim of
    lossless user-event delivery. A future lossless design requires a reviewed
    admission/close contract rather than an arbitrary queue enlargement.
11. **Sans-IO output backpressure P0:** cap immediate `SessionEngine` wire
    output as well as node transport queues, including handshake/error paths,
    and fail closed before a single aggregate input can retain unbounded
    responses. Preserve the existing pending-frame cap and add an aggregate
    regression; this is a resource policy, not a wire change.

### Implemented from the amended plan

The conformance/security work is now executable without changing the wire
contract: `mmp-core` rejects plaintext/pre-proof application records and
accepts the encrypted `ID_PROOF` transport record; `mmp-sim` exercises a
Byzantine relay reusing one inflated grant across two honest receivers; and the
Rust host seams propagate bounded transport-close actions. The test deliberately
proves only receiver-local admission bounds; it does not claim global copy
conservation. `Bundle::canonical_aead_ad()` aligns direct and group ciphertext
with the frozen header-binding rule, with details in
`docs/AEAD_AD_CONFORMANCE.md`. The remaining document contradiction is recorded
as an implementation erratum in `docs/ID_PROOF_ERRATUM.md`; the `specification/`
files remain untouched.

The newly completed non-wire work is tracked separately: `bcd5288` adds
fixed-width count prechecks to three codecs; `fe0aee9` caps the router's rolling
ephemeral/resolved beacon windows; `74194e5` exposes
`MmpNode::new_with_store_limit` while preserving the legacy constructor; and
`79dc056` adds the Android-SDK-free Kotlin `MeshDrive` source seam with seven
source-level tests. `5aa2626` updates the manual PC/mobile procedure with
these boundaries. The full workspace test/clippy gates and 41 Python tests pass,
but Kotlin/JVM compilation, Android/iOS background behavior, and Linux/BlueZ
two-PC exchange remain external gates.

The latest delegated audits extend the evidence without changing the v1 wire
contract: `766a741` rejects non-representable EDR model inputs and records a
platform-measurement handoff; `4f11d5d` reopens the RBSR/D3 gate with exact
missing normative decisions; and `09629bb` bounds the application callback
FIFO with a sticky Rust/FFI saturation counter. These changes deliberately
preserve the distinction between protocol delivery state and host callback
delivery: a saturated callback queue can lose a user-visible event, so hosts
must poll promptly and treat the counter as an operational failure signal.
`e8ffcac` closes the corresponding sans-IO `SessionEngine::out` backlog with a
256-frame bound, handshake coverage, reserved `ERROR+CLOSE` capacity, and a
fail-closed aggregate-input regression. `26938a0` keeps the generated Kotlin/
Swift API manifest aligned with the new FFI observability method.

The continuation resource audit adds two further wire-neutral bounds in
`4b0c8eb`: advisory `proxy_hints` are capped at 256 dependents per peer and
256 peer entries, while `blob_served` keeps at most 256 distinct peer counters
and fails closed for a new peer after saturation. These policies affect only
optional spray suppression or local blob service; they do not change bundle
validity or delivery semantics. Follow-up `5a50e43` gives `known_identities` a
session-scoped lifetime: entries are released after the peer's last live link,
without imposing an arbitrary count or wall-clock TTL that could break delayed
friendship acknowledgements. Group `remotes`/`dists` still receive no local
epoch cap because sender-key replay semantics need a normative retention rule
first. `pending_offers`, `offered_since`, and `patrons` are already bounded by
`MAX_PATRONS`; `friends` is allowlist-gated and TTL-pruned. The remaining
map-growth backlog is therefore group epoch retention, not an unreviewed
known-identity denial-of-service claim.

The EDR research seam is also now executable as an aggregate-only input in
`e0cee7c`. `RendezvousMeasurement` and its paired comparison validate trace
identity, counts, finite/non-negative energy and latency values, and produce
deterministic rates, deltas, energy channels, and optional latency means.
This is deliberately not a production scheduler, not a wire field, and not
evidence for the dialogue's claimed 2× gain; host/controller harnesses must
provide the measurements and report whole-device energy separately from radio
and host channels.

The final adversarial review found and closed one additional local DoS edge in
`9615929`: an empty or non-matching blob request no longer inserts a zero-valued
peer counter. Only a response containing at least one candidate consumes a
slot, so a Sybil stream cannot exhaust the 256-peer table without causing any
blob work. Matching requests remain subject to the same per-contact budget and
the focused node suite remains green.

### Continuation cycle (2026-08-09)

Three delegated follow-up audits produced two implementation-only safeguards
and one explicit research boundary. `473c298` adds an RBSR regression that
locks the current D3 transcript to base domain `3` (with only the existing
reply flag variation); it prevents an implementation from inventing priority
partitioning before an approved normative erratum. `daca0ad` adds a bounded,
deterministic EDR experiment matrix with transmit multipliers `0.25, 0.50,
0.75, 1, 2, 4, 8`; it preserves the toy hazard product and labels the `1.0`
point as the unchanged baseline, but does not alter routing or scheduling.
Finally, `5a50e43` makes `known_identities` session-scoped: entries are
released when the peer's last live link is removed and retained across
multiple concurrent links. This closes the process-lifetime map-growth issue
without arbitrary TTLs or delayed-FRIEND_ACK breakage. Group epoch maps remain
uncapped because their retention semantics still require a normative anchor;
the handoff now records the exact conformance tests needed before such a policy
can be enabled.

The next delegated gate-preparation cycle added three non-wire artifacts. The
`SocialClusteredMobility` control in `71ff465` provides seeded per-mille bridge
transitions and realized bridge metrics for paired simulator experiments; its
handoff explicitly rejects SLAW or human-mobility claims until trace
calibration. `7a4eb6e` hardens the iOS XCFramework verifier so every expected
slice declares exactly `arm64`, matching the build targets, with a synthetic
negative test. `b4e3039` adds `validate_ble_evidence.py`, a path-safe
`mmp-ble-evidence-v1` manifest checker for the future two-host Linux/BlueZ
run. It verifies internal evidence completeness—roles, logs, markers, stores,
delivery, receipt, reconnect, MTU/RSSI availability, and gate flags—but does
not authenticate logs or claim hardware success.

The next delegated cycle closed three additional non-wire gaps. `4f6351e`
adds `validate_release_preflight.py`, which combines the BLE evidence checks
with repository-side verification of the recorded commit and a clean
`specification/` tree; unrelated dirty paths are intentionally ignored.
`847758e` adds a high-level vault regression proving that an unavailable
provider fails closed before commit and leaves the restore target unchanged;
native keystore, lock/invalidation, and crash-recovery behavior remain external
platform gates. `1e2fdef` adds a reproducible research matrix comparing random,
periodic-clustered, and social-clustered contact generators with delivery,
latency, elapsed time, storage, quota, and bridge metrics. It is a control
experiment only and does not support a human-mobility or 2x claim.

The current delegated security/readiness cycle recorded three further results.
`67183c9` confirms that group `(member, epoch)` retention cannot receive a
wire-neutral cap until an authenticated retention anchor and partition-merge
rule exist; pruning now would change delayed decrypt/replay behavior. `9525e14`
hardens the generated fuzz corpus against traversal, target mismatch, and
root/target symlink attacks, while preserving the developer-owned corpus.
`b439ce9` adds writer-generation fencing coverage for the peripheral ingress
and a BlueZ lifecycle handoff; host tests are green, but D-Bus, ATT-MTU,
reconnect, and two-machine evidence remain hardware-only.

The latest delegated gate cycle added two scoped artifacts and one explicit
no-change result. `057b43b` adds a wire-neutral RBSR regression: an established
responder closes when a syntactically valid D3 `RECON_INIT` arrives while D0
remains unresolved, so the implementation cannot silently invent priority
partitioning that the frozen documents do not define. The accompanying RBSR
handoff records the remaining normative blocker: the specification still lacks
the byte-level `(domain, declared_priority)` negotiation/key, legacy behavior,
D3 weighting, starvation/abuse invariants, and golden transcripts required for
cross-implementation priority-class reconciliation. `6ffbb21` adds the E7
embedded handoff. It confirms that `mmp-tal-embedded` is currently a
cross-compilable `no_std` placeholder and that `mmp-node-embassy` still fails
the embedded target build because the dependency closure enables `std`; no
MCU-linked image, RAM/flash budget, IRQ/GATT bridge, or hardware-in-the-loop
claim is made. The FFI/mobile secret-surface audit made no code change: the
existing API-completeness and generated Kotlin/Swift denylist checks cover the
Rust-internal vault/ratchet identifiers, while native Keystore/Keychain,
JNI/Swift compilation, React Native runtime, and background-radio validation
remain external gates. No specification or dirty `mmp-types` file was touched.

The next orchestration cycle closed three additional evidence gaps without
changing the wire contract. `2dc2d2b` corrected the fuzz-build and fuzz-run
paths relative to the workflow-wide `implementation/mesh-milk` working
directory; the static CI test now prevents the nested-path regression.
`0338903` makes signed release uploads fail closed when any binary, signature,
or certificate is missing. `7353363` adds a research-only BLAKE3 trace ID to
`mmp-sim` metrics, covering the seed, node count, ordered meeting pairs, and
virtual time advances. This prevents baseline/candidate reports with equal
aggregate counters from being paired as if they used the same contact trace;
it is not a protocol credential and does not replace the full seed/config
record. `6efbc41` adds a hostile persistence regression: oversized authenticated
provider envelopes and contexts fail closed without mutating the target secret
state. The native keystore/backend, durable CAS/floor, and complete-image
quota remain platform/provider gates. Local cargo-audit could not refresh its
RustSec database (HTTP 403), and cargo-deny is not installed; those checks
remain remote-CI evidence rather than local claims. No specification,
`mmp-types`, or pre-existing dirty path was modified.

The latest orchestration cycle added three more bounded implementation gates.
`750c807` fences `mmp-node-embassy` behind `default-features = false`, removes
the filesystem-backed std-only store from the empty Embassy shell, and makes
the `thumbv7em-none-eabihf` dependency closure genuinely no-std while clearly
leaving executor, TAL, flash-store, and Noise/session wiring as future work.
`d6e81f8` adds a host-side mobile-drive regression for callback injection and
drain ordering; the audit also records that `pollClose`, `closeActionsDropped`,
and `eventQueueDropped` are not yet modeled by the Kotlin/Swift host adapters.
`92f0ec8` independently recomputes the committed RBSR fingerprint vector from
raw IDs, XOR fold, count encoding, and BLAKE3 without calling production
helpers. It improves transcript evidence but does not define priority
partitioning or D3 admission weighting. No specification, `mmp-types`, or
pre-existing dirty path was modified.

The following gate cycle added three more scoped safeguards. `567e719` proves
that a valid sealed secret snapshot from one node cannot be replayed into a
different identity/node, and that the failed restore leaves the target state
byte-identical. `d1e55a7` rejects stale generated fuzz-seed members while
preserving unmanaged developer corpus files; deterministic `check`/`verify`
still validates all 44 seeds. `48df24f` rejects symlinked Android ZIP members
and symlink/junction escapes in XCFramework slice, library, and header paths.
These are artifact and persistence boundaries, not protocol or platform
interop claims. Nightly `cargo fuzz build` reaches the Windows linker but
cannot link because the MSVC sanitizer runtime
`clang_rt.asan_dynamic_runtime_thunk-x86_64.lib` is unavailable; the Linux CI
sanitizer job remains the authoritative fuzz-build/run gate. No specification,
`mmp-types`, or pre-existing dirty path was modified.

The latest focused security/mobile/embedded cycle added three further bounded
gates. `3190a54` proves that authenticated snapshot generation metadata cannot
be forged backwards: a lower-generation header fails both the durable floor
check and the AEAD/context binding, without changing the wire contract.
`5bd10be` records the mobile host backpressure handoff: native adapters still
need explicit `pollClose`, `closeActionsDropped`, and `eventQueueDropped`
handling, with drain-before-send, exactly-once link-down, and fail-closed
overload acceptance tests. `5ff2032` adds a CI regression for the complete
`thumbv7em-none-eabihf` dependency closure of `mmp-node-embassy` and
`mmp-tal-embedded`; the check now passes with `default-features = false`, but
this remains only a no-std compile-closure proof. It does not claim an Embassy
executor, flash store, radio backend, linked image, or hardware-in-the-loop
result. No specification, `mmp-types`, or pre-existing dirty path was touched.

The subsequent interop/release/session cycle added three more bounded records.
`0323deb` updates the manual PC/mobile procedure with the FFI close-drain
ordering, sticky overload counters, Android NDK/zip prerequisites, and the
required evidence fields; it does not substitute for a consuming app or real
radio. `c15d6ff` makes signed release uploads auditable by adding atomic
SHA-256/size manifests for each binary, signature, and certificate, bound to
the GitHub repository, commit, and ref. This improves artifact integrity but
does not prove a remote Actions run or replace Sigstore verification.
`55054f2` adds the exact Noise-tag envelope-boundary regression and records the
remaining clear envelope-type authentication, infallible public frame
encoder, and pre-auth allocation gaps. No specification, `mmp-types`, or
pre-existing dirty path was touched.

`ba736d1` closes a separate operator-safety issue found while executing the
manual procedure: every documented CLI subcommand now handles help before
performing I/O, `keygen --help` cannot create an identity file, and only
option-looking path values are rejected. Free-form message text and ordinary
absolute/relative paths remain valid. Focused CLI tests and process-level help
checks pass; this is usability/release hardening, not a protocol change.

### Verification checkpoint (2026-08-09)

The post-audit implementation gate is green on the Windows host: the complete
workspace test matrix passes, including 33 CLI/conformance tests, 64 core unit
tests plus 5 decoder-limit integration tests, 37 crypto tests, 19 FFI tests,
79 node tests, 24 active simulator tests (6 scale/deployment tests remain
ignored by design), 11 router tests, 3 Reed–Solomon tests, 11 store tests, 2
bounded-snapshot tests, 1 hostile-snapshot test, 22 mobile-transport tests, 38
BLE adapter tests, 43 type/codec tests, and three additional 3-test conformance
integration suites. Workspace Clippy with all targets, features, and
`-D warnings` also passes. The Python harness reports 66 tests,
compileall is clean, the deterministic fuzz recipe validates 44 seeds,
layer-check passes, and the CLI demo observes both receiver delivery and
sender receipt completion.

This is still not physical interoperability evidence: Windows has no usable
WSL/Bash launcher in the current environment, Linux/BlueZ two-PC exchange has
not been run here, and Android/iOS background behavior, keystore persistence,
and true RBSR/D3 cross-domain conformance remain external gates. The working
tree's pre-existing application/docs changes and audit scratch directories
were intentionally not staged. No file under `specification/` changed.

The latest serial rerun supersedes the earlier component inventory above: it
listed 411 tests (405 active, 6 intentionally ignored) with zero failures.
The Python harness now reports 89 tests (six platform-specific skips), while the
46-seed fuzz check, compileall, layer-check, and CLI delivery/receipt smoke
evidence remain green.

### Security/release continuation (2026-08-09)

The delegated pre-auth audit produced two related implementation commits. `d2a29f1`
replaced attacker-sized Noise receive storage with a fixed 256-byte buffer and an
early input bound. The follow-up initially explored a strict empty-payload rule
for Noise message 1, but the frozen documents do not state that rule as a
standalone MUST. `e17243e` therefore keeps interoperability broad: a bounded
message-1 payload is consumed by Noise and ignored; only the encrypted message-2/
message-3 introductions can populate the peer identity/HELLO. The regression
injects an invalid cleartext payload and completes the handshake, proving that it
cannot influence provisional identity state. This closes the unauthenticated
provisional-HELLO injection path without changing the wire specification. The
remaining Noise profile and ID_PROOF wording still require a reviewed normative
erratum before independent implementations are called frozen.

The release audit added `df0a92c` and `3d560d5`: CI now verifies strict signed
artifact manifests against exact repository, commit, and ref values, with
rejection of malformed JSON, schema drift, traversal/absolute paths, symlinks,
duplicate entries, and digest/size mismatches. The BLE evidence gate `de8d68e`
now requires distinct host seed fingerprints and role-specific advertising,
dialing, establishment, and delivery markers. These are structural evidence
guards; they do not claim a physical Linux/BlueZ exchange or authenticate
operator-supplied logs.

Focused verification after this continuation passed: `mmp-crypto` (39 tests),
workspace tests (411 listed / 405 active / 6 ignored), workspace Clippy with all
features, Python tests (89 / 6 skipped), compileall, 46 deterministic fuzz seeds,
the embedded target check, and layer-check. No `specification/` file or
pre-existing dirty path was staged.

### Orchestration continuation (2026-08-09)

Three independent follow-up audits produced additional non-wire hardening. The
BLE evidence validator `25c28fa` now canonicalizes all four evidence paths and
rejects reused or aliased log/store files, preventing one-sided evidence from
passing the two-host gate. The release checker `2d69a6a`/`53fecae` verifies that
`ROADMAP.md`, `CHANGELOG.md`, and `COUNCIL-REVIEW.md` are non-empty regular files
both in the checkout and in the recorded Git tree, including blob-mode checks;
the tag-release workflow runs it before signing. Historical `AUDIT.md` references
remain provenance only; no missing transcript is fabricated.

The session audit `8debea6`/`4aba52b`/`b2c1d02` adds pre-encoding width checks for
every narrow summary, reconciliation, WANT, blob, and error field. Invalid local
frames now fail closed before either the pending queue or encrypted output, even
when public `Frame` encoders remain infallible for compatibility. The focused
`mmp-core` suite is 66 tests; frame-type authentication remains a deliberately
recorded future wire-revision residual because adding Noise associated data or a
duplicate encrypted type would change frozen v1 bytes.

The next delegated cycle added three scoped release gates. `a01a867` pins the
Noise seed corpus at the 256/257-byte pre-auth boundary, and CI now materializes
and verifies the 46 deterministic seeds before building the four fuzz targets.
`a09e6cb` makes the Android artifact verifier parse the manifest XML, require the
`org.meshmilk.ffi` package identity, and tests that both mobile packaging jobs
invoke their structural verifiers. `e708c49` fences the embedded TAL dependency
with `default-features = false`, adds a source-level regression for the Embassy
dependency graph, and corrects the E7 handoff's historical failure wording.
These changes are implementation/release hardening only: the specification tree
and all pre-existing dirty application/docs paths remain untouched. The complete
Python suite is now 89 tests with six intentional skips; the Rust and target
checks above remain green.

### Orchestration continuation II (2026-08-09)

The next delegated cycle added three scoped outcomes. `d6f4e24` makes the BLE
evidence validator require the exact command line for each host, matching the
existing manual evidence checklist; it does not turn operator-supplied logs
into cryptographic proof. `17906a4` records the E7-S1 fixed-store blocker:
`BundleStore::put` cannot report capacity rejection, read-side APIs allocate,
and the current `mmp-store` filesystem graph is `std`-only. The handoff defines
the required API split, board-derived capacities, and HIL evidence instead of
adding a misleading embedded store. `abcfe67` adds checked EDR gain reporting
for finite model inputs whose quotient overflows and documents why no raw-log
parser is justified until provenance, calibration, and confidence-interval
fields are agreed.

The post-cycle verification is green: 412 Rust tests are listed (406 active,
6 intentionally ignored), including 25 active simulator tests; the Python
harness reports 90 tests with 6 skips; the deterministic fuzz corpus validates
46 seeds; focused BLE and EDR suites pass; the embedded target closure passes;
and `specification/` remains unchanged. These are host-side or structural
results only. Physical BlueZ exchange, native mobile radio behavior, linked
embedded images, and normative RBSR/D3 decisions remain separate gates.

### Orchestration continuation III (2026-08-09)

The next security-focused cycle converted the E7 design blocker into an
additive API seam. `5a85b7b` adds `BundleStore::try_put` with explicit
insert/replacement outcomes and capacity rejection; `ingest` now returns
`DropReason::StoreRejected` instead of reporting delivery when a future fixed
store refuses a write. Existing `put` implementations and wire behavior remain
unchanged. `bcdfa8c` bounds direct `NoiseChannel` transport allocations to the
frozen 4096-byte envelope cap, rejects oversized inputs before nonce/state
advance, and preserves valid v1 behavior. `3a434fc` closes the corresponding
session path on encryption errors; the pre-existing envelope-boundary
regression caught this integration issue before release. `4b451e3` adds a
checked mobile ATT chunk helper (`23`-byte minimum MTU, 3-byte ATT header)
while preserving the saturating compatibility helper.

The post-fix workspace lists 418 Rust tests (412 active, 6 intentionally
ignored), the Python suite remains 90 tests with 6 skips, and focused mmp-core
(68), mmp-crypto (40), and mobile TAL (3) suites pass. Target/layer checks
remain required before the next handoff; physical native-platform behavior,
embedded fixed-store implementation, and external cryptographic review remain
open.

### Orchestration continuation IV (2026-08-09)

The next delegated implementation cycle added three scoped, non-wire gates.
`0444d36` fences `mmp-store` behind an explicit default `std` feature: the
host `SnapshotStore` behavior is unchanged, while its no-default-features
profile now cross-checks on `thumbv7em-none-eabihf` without exporting a
misleading filesystem API. The fixed-capacity embedded store is still not
implemented, so this is dependency-closure evidence only. `e4fcaa1` adds a
node-level capacity-rejection regression proving that an authenticated bundle
is not retained or delivered when the provider refuses storage, while
first-sight dedup remains recorded. `af62144` adds an FFI regression proving
that an oversized host string maps to `BadArg` without poisoning first-contact
state and that a valid retry succeeds immediately.

The post-cycle verification is green: 420 Rust tests are listed (414 active,
6 intentionally ignored), workspace Clippy with all targets/features passes,
the Python suite reports 90 tests with 6 skips, 46 deterministic fuzz seeds
check and verify, the embedded no-default-features target check passes, and
layer-check remains clean. These results are host-side and structural only;
Linux/BlueZ two-PC exchange, native Android/iOS radio and keystore behavior,
an embedded linked image/HIL run, external cryptographic review, and the
normative RBSR/D3 decision remain open. No file under `specification/` and no
pre-existing dirty path was modified or staged.

### Orchestration continuation V (2026-08-09)

The subsequent delegated cycle closed three additional implementation and
release boundaries. `4961ff5` bounds BLE evidence manifests to 1 MiB and each
referenced log/store artifact to 8 MiB, rejecting oversized files before JSON
or UTF-8 materialization; this hardens the offline evidence checker without
pretending that operator-supplied logs prove a physical exchange. `8b46dad`
rejects impossible application payloads at the `Node::send_body` boundary
before copying host/FFI input, while retaining exact bundle-size preflight and
proving that a rejected first-contact send leaves no conversation or pending
ratchet state. `b5eac2b` makes the release workflow run `cosign verify-blob`
with repository/workflow/ref and GitHub OIDC identity constraints before
creating the digest manifest. The actual keyless verification still requires
a GitHub Actions tag run; local execution has no OIDC context.

The post-cycle verification is green: the workspace remains at 420 listed
Rust tests (414 active, 6 intentionally ignored), Clippy with all
targets/features passes, the Python suite reports 93 tests with 6 skips, 46
deterministic fuzz seeds check and verify, the embedded no-default-features
target check passes, and layer-check remains clean. No file under
`specification/` and no pre-existing dirty path was modified or staged.

## Source register

- [A7] RFC 9171, Bundle Protocol v7: https://www.rfc-editor.org/rfc/rfc9171.html
- [C9] RFC 6693, PRoPHET: https://www.rfc-editor.org/rfc/rfc6693.html
- [E4] RFC 6330, RaptorQ: https://www.rfc-editor.org/rfc/rfc6330.html
- [F1] Noise Protocol Framework: https://noiseprotocol.org/noise.html
- [I3] Bluetooth SIG Channel Sounding: https://www.bluetooth.com/learn-about-bluetooth/feature-enhancements/channel-sounding/
- [I11] Android RangingManager: https://developer.android.com/develop/connectivity/ranging
- [I15] Bluetooth Core 6.0 feature overview: https://www.bluetooth.com/core-specification-6-feature-overview/
- [K5] NASA/CSU fountain coding for DTN: https://ntrs.nasa.gov/citations/20230007643
