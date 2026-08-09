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

### Orchestration continuation VI (2026-08-09)

The roadmap was reconciled against the current implementation evidence. Its
protocol-completeness section now distinguishes shipped class-4 symbol
transfer, receive-side `BLOB_REQ`/`BLOB_HAVE` serving, friendship buffering,
advisory proxy summaries, and generalized identity revocation from their still
deferred origin-targeted request scheduling, offline patron reconciliation,
and native keystore persistence. The 1k-node entry now records the existing
measurement scenario and the remaining clustered-mobility qualification. This
is a documentation correction only; no wire or specification file changed.

The own-origin storage transaction handoff remains the authoritative blocker:
capacity rejection cannot safely be threaded through `store_own` until
reserve/commit or ratchet snapshot/restore semantics are approved and tested
for pairwise, group, receipt, claim, announcement, and file-symbol producers.

The follow-up gate-preparation pass closed two reproducibility mismatches and
one deliberately preserved transactional blocker. `0514fed` extends the
embedded CI dependency-closure check to include `mmp-store` under
`--no-default-features`; this verifies that its host filesystem graph cannot
leak into the target profile, but still does not claim a fixed-capacity store.
`0bcbffe` corrects the manual PC/BLE procedure so the bootstrap `connect` log
and delivery-producing `send` log are retained separately and concatenated
into the single central evidence log required by the validator. `c875c72`
audits own-origin writes and records why replacing `BundleStore::put` with
`try_put` immediately would be unsafe: pairwise/group ratchets, pending
first-contact state, and file-symbol indexes can advance before storage. The
handoff defines the required reserve/commit or snapshot/restore contract
instead of introducing a partial failure path.

The post-cycle verification remains green: 420 listed Rust tests (414 active,
6 intentionally ignored), 93 Python tests with 6 skips, the focused node
suite, Clippy, the exact embedded target check including `mmp-store`, fuzz
seed validation, and the prior full workspace matrix all pass. No file under
`specification/` and no pre-existing dirty path was modified or staged.

### Orchestration continuation VII (2026-08-09)

The transaction-readiness audit added three bounded, wire-neutral safeguards.
`be61df4` and `07b57ed` document and test the existing zeroizing
`SendChain::snapshot`/`restore` seam: a rejected candidate can replay the same
message key, epoch, and index, but only for the same in-memory chain and only
as provider-local material. `230d80b` rejects a revoked file recipient before
class-4 symbols are minted or indexed, closing a deterministic partial-state
path. `864833a` performs an exact placeholder `Bundle` width check before a
group sender chain advances and proves that a rejected oversized group send
does not consume index zero; it deliberately does not add a group-key rollback
API because duplicating chain and signing-key state would risk key reuse.

These changes do not make own-origin storage transactional. A provider-capacity
failure after encryption or symbol minting still requires the reserve/commit
contract recorded in `docs/OWN_BUNDLE_STORE_TRANSACTION_HANDOFF.md`. The full
workspace matrix is green at 426 listed tests (420 active, 6 intentionally
ignored); focused crypto/node suites, Clippy, the embedded no-default target
check, and layer-check also pass. Physical BlueZ exchange, native mobile radio
and keystore behavior, fixed-capacity embedded storage, external cryptographic
review, and the normative RBSR/D3 decision remain open. No file under
`specification/` was modified.

### Orchestration continuation VIII (2026-08-09)

The next delegated audit found two state-ordering hazards in group admission.
`40dcc11` sends the authenticated sender-key distribution before mutating the
membership OR-Set, so `UnknownPeer` and `ConversationPending` failures leave
membership unchanged. It also extracts a pure, exact group-size preflight and
runs it before restored-group distribution; an oversized request therefore
does not queue a distribution, clear `needs_distribution`, or consume a group
sender index. The node suite now has 84 passing tests.

The Linux/peripheral audit added `a6250ff`: the BlueZ advertiser rejects any
payload other than the shared 24-byte v1 beacon shape before registration. This
is host-side wire-shape validation, not a physical BlueZ result; the Linux
pkg-config/sysroot and real adapter gate remain external. The release audit
also made the tagged artifact build use `cargo build -p mmp-cli --release
--locked`, with a Python regression protecting that reproducibility invariant
(`6def96b`). The storage audit deliberately added no advisory capacity check:
the reserve/commit provider contract remains required because `total_bytes`
cannot reserve an unknown final ciphertext or BundleID.

The post-cycle matrix is green: 429 Rust tests listed (423 active, 6
intentionally ignored), 94 Python tests with 6 skips, focused Clippy, the
embedded no-default target check, and the full workspace test run. No file under
`specification/` was modified, and pre-existing dirty paths remain unstaged.
Physical Linux/BlueZ exchange, native mobile radio and keystore behavior,
fixed-capacity embedded storage, external cryptographic review, and the
normative RBSR/D3 decision remain open.

### Orchestration continuation IX (2026-08-09)

The storage audit produced `ffe4975`, a fail-closed snapshot-integrity fix.
`SnapshotStore` now rejects checksummed bundle records whose persisted
`BundleMeta::size` disagrees with the body length, preventing inconsistent
quota/transfer accounting. Snapshot framing also checks the `usize` to `u32`
conversion rather than truncating an oversized record. The mmp-store unit and
integration suites pass, including malformed-record and boundary regressions;
its no-default dependency-fence check remains green.

The peripheral audit produced `e8ded79`: after `shutdown_all`, advertising is
terminally fenced with `PeriphError::ShuttingDown`, and the Linux backend drops
a newly registered handle if shutdown wins during the asynchronous BlueZ
registration race. This is host-side lifecycle evidence only; Linux/pkg-config,
BlueZ, and physical-adapter validation remain external.

The release audit produced `b3a442c`, pinning every Cargo build/run invocation
in the Android AAR and iOS XCFramework scripts to `Cargo.lock`, with a Python
regression covering all invocations. The full matrix now passes at 432 Rust
tests listed (426 active, 6 intentionally ignored); the Python suite and
focused Clippy/embedded checks are green. A separate SnapshotStore
remove-before-rename crash window remains documented in
`docs/OWN_BUNDLE_STORE_TRANSACTION_HANDOFF.md` and requires platform-specific
atomic-replace design. No file under `specification/` was modified, and all
pre-existing dirty paths remain protected.

### Orchestration continuation X (2026-08-09)

The platform-atomicity audit produced `0f2f433`. On Unix,
`SnapshotStore::persist` now replaces the same-filesystem destination with an
atomic `rename` and synchronizes the parent directory after the temporary image
has been fsynced. This removes the prior remove-before-rename crash window.
Windows retains the standard-library fallback because unsafe platform FFI is
forbidden and no portable `ReplaceFile` primitive exists; that residual and
its recovery boundary are explicit in the storage handoff. The Unix path was
cross-compiled and the replacement regression passed, but runtime crash
injection on a real Unix filesystem and Windows kernel-level replacement
testing remain external.

The release/CI audit produced `bba5da2`, adding `--locked` to all three
`mobile-bindings` Cargo invocations and asserting that invariant in the
workflow tests. The peripheral evidence audit produced `1f41735`, bounding the
secondary manifest-commit extraction in `validate_release_preflight` so an
oversized evidence package cannot trigger a second unbounded read after the
primary validator rejects it. The script suite passes with 97 tests and 6
expected symlink skips.

After these changes the full workspace remains green at 432 Rust tests listed
(426 active, 6 intentionally ignored), with the embedded no-default target and
focused Clippy checks passing. No file under `specification/` was modified and
all pre-existing dirty paths remain unstaged. Real Linux/BlueZ exchange, native
mobile radio/keystore validation, fixed-capacity own-origin storage, external
cryptographic review, and the RBSR/D3 normative decision remain open.

### Orchestration continuation XI (2026-08-09)

The delegated RBSR/D3 audit produced `b89a54c`, a wire-neutral regression that
locks the currently executable mapping: control/claim inventory remains D0,
direct traffic remains D1, group traffic remains D2, and relay/class-5 traffic
remains D3. It also proves that receiver-local class-5 effective-priority
clamping cannot silently become a reconciliation partition key. The test is
coverage of the present safe subset, not a claim that the frozen priority-class
partition or D3 admission weighting has been resolved. The normative blocker
therefore remains the missing partition encoding/legacy behavior and weighting
rule.

The delegated storage audit found no safe implementation-only fix for the
fixed-capacity own-origin gap. `BundleStore::put` is still infallible, while
`try_put` only protects receive-side admission; making local producers reject
atomically requires a provider reservation/commit contract that can roll back
producer-specific ratchet, conversation, group, and file-index state. The
existing E7 and own-storage handoffs remain the authoritative next-step design;
no advisory preflight was added.

The requested manual test artifact is now committed as
`772582b` ([INTEROPERABILITY_TEST_GUIDE.md](./INTEROPERABILITY_TEST_GUIDE.md)).
It separates deterministic loopback, real Linux/BlueZ two-PC evidence, and
Android/iOS artifact/API-bridge checks; records exact commands, logs, MTU/RSSI,
reconnect, store-carry-forward, and callback-overload observations; and states
the hardware and consuming-app limits explicitly. It does not turn a build or
manifest-validation pass into radio or mobile evidence.

The current cycle therefore advances implementation-side conformance and
operator reproducibility without changing the frozen specification. The next
meaningful gates remain real Linux/BlueZ exchange, native Android/iOS runtime
and keystore validation, a fixed-capacity transaction API, external crypto
review, and an approved RBSR/D3 normative decision.

### Orchestration continuation XII (2026-08-09)

The fuzz-input audit produced `55acf0d`. The deterministic seed recipe now
rejects symlinked vector files, caps each supplied JSON file at 1 MiB, caps
decoded seed wires at the CI/libFuzzer `-max_len=8192` boundary, and limits
frame/bundle/negative-case arrays to 512 entries before iterating them. This
is a tooling/resource hardening boundary, not a protocol-wire limit. The
recipe still reproduces 46 seeds and its focused hostile-input tests pass.

The E7 storage audit produced `96da3bd` and the new
[`E7_FIXED_STORE_DESIGN.md`](./E7_FIXED_STORE_DESIGN.md). It defines an
additive, provider-local reservation/commit/abort seam, replacement-delta
accounting, batch reservations for files, producer rollback invariants, and
host/embedded/HIL acceptance criteria. It deliberately leaves the current
`BundleStore` behavior unchanged until a provider can prove atomicity; no
specification or wire behavior was guessed.

The release audit produced `f241ea8`: iOS XCFramework verification now rejects
symlinked `Info.plist` metadata instead of reading outside the artifact root,
with a regression fixture. The script suite remains green, and the focused
fuzz suite is 12 tests with one Windows symlink skip. These changes close a
concrete artifact-boundary issue without claiming a successful Xcode or device
build.

The MMP-Φ proposals remain research hypotheses. No EDR, proof-carrying
mobility, channel-sounding scheduler, or DoF coding change has been promoted
into v1 implementation or specification without measurements and an approved
wire contract.

### Orchestration continuation XIII (2026-08-09)

The mobile audit produced `930dac9`. The Android-SDK-free Kotlin `MeshDrive`
reference now exposes the UniFFI transport-local close actions and saturation
diagnostics, drains `pollClose()` before `pollSend()`, purges pending fragments
for a closing link, and requires the native adapter to report physical
disconnect through `onLinkDown`. This closes the previously documented stale-
send lifecycle gap at the reference-drive-loop boundary. Twenty focused source
and binding/build tests pass; no Android SDK, Kotlin compiler, CoreBluetooth
runtime, or physical-device evidence exists yet.

The release-security audit produced `a678f5f`. Release manifests now reject
case-insensitive filename collisions, preventing two distinct textual paths
from referring to one artifact on Windows/macOS and creating ambiguous digest
or role evidence. The change is tooling-only and has no protocol effect.

The transport handoff was refreshed to distinguish the completed Kotlin seam
from the still-external native Android/iOS adapter and hardware gates. The
frozen specification remains untouched, and no MMP-Φ research hypothesis was
promoted into v1 behavior.

### Orchestration continuation XIV (2026-08-09)

The simulation audit produced `402dbee`. The existing deterministic social
cluster control now reports realized intra-community contacts, maximum
consecutive intra-community streak, and realized bridge probability, including
an explicit zero-meeting case. These are reproducibility metrics for comparing
contact traces; they are not SLAW calibration, human-mobility evidence, or
production routing inputs. The focused simulator tests and Clippy pass, while
external trace calibration remains open.

The transactional-store review produced `63f791e` and
[`E7_TRANSACTIONAL_API_DECISION.md`](./E7_TRANSACTIONAL_API_DECISION.md).
The proposed public trait is intentionally not exposed yet: current storage is
held as `Box<dyn BundleStore + Send>`, opaque reservation ownership has no safe
provider construction path, and no implementation proves batch/unknown-commit
recovery. The existing fixed-store design remains the target, while legacy
receive and own-origin behavior stays unchanged.

The release-security audit produced `c9cc6d8`. Release-manifest parsing now
limits the input to 1 MiB plus one-byte overflow detection before JSON parsing,
closing an attacker-controlled memory-amplification path. The host-selected
bounded `SnapshotStore` policy and vault generation fences were retained; no
global persistence cap was guessed.

### Orchestration continuation XV (2026-08-09)

The evidence-manifest audit produced `a931e70`. A host entry is no longer
accepted merely because its command string is non-empty: the peripheral role
must invoke `mmp-cli listen`, and the central delivery evidence must invoke
`mmp-cli send`. A bootstrap-only `connect` command, including one with a free-
form argument containing the word `send`, is rejected. The validator accepts
the documented direct executable and `cargo run -p mmp-cli -- ...` forms. This
improves reproducibility and prevents a manifest from passing with commands
that could not have produced the claimed role-specific evidence; it does not
prove that a physical adapter ran.

The delegated cryptographic audit found no new high-confidence Rust-only defect:
the Noise XX channel binding and encrypted `ID_PROOF` gate, prekey replay
boundary, ratchet snapshot checks, and snapshot AEAD/rollback tests match the
current implementation. The focused crypto suite reports 41 passing tests,
`--no-default-features` checks, and all-target Clippy are clean. The remaining
composition review is external and must precede a v1.0 security claim. The
keystore audit likewise found no provider-neutral patch: durable generation
floors, trusted platform time, opaque DEK wrapping, atomic crash recovery, and
Android Keystore/iOS Keychain/Linux adapters remain external gates.

### Three-pass re-audit of the supplied MMP/Φ dialogue

This addendum repeats the requested validation cycle rather than promoting the
dialogue's proposed optimizations into MMP/1:

1. **Repository pass.** The current Rust/session/router code confirms the
   encrypted `ID_PROOF` interpretation, canonical lifetime-zeroed AEAD AD,
   receiver-local (not globally spend-once) quotas, and the deliberate
   no-change cryptographic/keystore gates. The EDR experiment seam records
   measurements only; it is not a scheduler or a 2× result.
2. **Authoritative-source pass.** RFC 9171 supports store-carry-forward and
   bundle-age handling but leaves route computation out of scope; RFC 6693
   defines encounter-history transitivity but does not make peer claims
   trustworthy. Noise separates handshake messages from post-handshake
   transport ciphertext. Bluetooth Core 6.0 confirms advertiser monitoring and
   decision-based filtering as controller capabilities, not phone-level energy
   guarantees. Android's Ranging API confirms capability discovery and also
   documents permission/background restrictions. RaptorQ and the NASA DTN study
   support fountain coding as a credible experiment, not an automatic win.
3. **Adversarial/model pass.** The dialogue's proposed vouchers and
   evidence-weighted routing were tested against collusion, stale proofs,
   correlated mobility, privacy leakage, and probability miscalibration. The
   square-root EDR calculation was reclassified as a controlled toy model
   because daily mAh totals are not independent linear TX/RX coefficients and
   omit OS wakeups, collisions, and burst-scan effects. The illustrative
   22-to-4 carrier reduction remains conditional on calibrated, independent
   paths. The amended work order therefore keeps these ideas research-only,
   requires trace-driven confidence intervals and whole-device measurements,
   and preserves the frozen wire contract.

The implementation plan is consequently extended with two gates: (a) require
role-correct, delivery-producing commands in every future radio evidence
manifest, and (b) require an external cryptographic/provider review plus
platform measurements before any MMP/Φ mechanism can alter routing, timing, or
wire semantics. No file under `specification/` was modified.

### Orchestration continuation XVI (2026-08-09)

The mobile-artifact audit produced `c887492`. The iOS XCFramework verifier now
rejects any `AvailableLibraries` slice outside the two explicitly built
arm64 targets. This prevents an untested architecture from being accepted as
release evidence. The validator regression and related mobile artifact/API
tests pass; macOS/Xcode, native bindings, and physical-device interoperability
remain external.

The storage audit produced `6fa7700`. `SnapshotStore::persist` now validates
every in-memory record's declared `BundleMeta::size` against its actual body
before creating a replacement image. The legacy infallible `BundleStore::put`
surface can still receive inconsistent metadata, but persistence now fails
closed with `InvalidData`, leaves the last primary image intact, and creates no
temporary replacement. This closes a concrete false-durability path without
pretending to implement the E7 reservation/commit API.

The embedded/no-std audit made no code change. The target dependency closure
and layer checks pass, while the following remain genuinely external: a
fixed-capacity flash-log store, Embassy executor integration, nRF52/ESP32 TAL
backends, board/linker configuration, stripped-image RAM/flash measurements,
Reed–Solomon target benchmarking, and phone-to-sensor hardware-in-the-loop
recovery tests. No file under `specification/` or any protected dirty path was
modified.

### Orchestration continuation XVII (2026-08-09)

The release-governance audit produced `8dda7d4`. Release manifest creation now
rejects empty artifacts, signatures, and certificates, while verification
requires strictly positive recorded sizes. This closes a package-integrity
case where an empty file could otherwise pass a digest/schema check. The CI
release audit also produced `780ba6f`: Linux, Windows, and macOS artifacts now
install, build, and copy from explicit declared Rust targets instead of relying
on the runner's default architecture. Workflow assertions pin the matrix and
target-specific output paths.

The fixed-store audit produced `2d25072`. When `SnapshotLimits` is configured,
`SnapshotStore::try_put` now calculates the exact projected complete image:
magic, bundle framing/checksum, replacement delta, and currently staged claims,
groups, SeenIndex, ReceiptFilter, authority, and blocklist records. It rejects
over-limit, load-rejected, or malformed-size candidates before mutating memory.
`mmp-ffi::SharedStore` forwards this bounded admission method instead of
silently inheriting the unbounded default. Exact-fill, replacement, and
rejection regressions pass.

This does not close the E7 producer transaction. FFI auxiliary state is staged
at `persist_now`; if it grows after a successful receive admission, a later
flush can still fail. A complete fix requires the deferred reservation/commit
or producer snapshot protocol, so no public transactional trait or wire change
was introduced. The frozen `specification/` tree and protected dirty paths
remain untouched.

### Orchestration continuation XVIII (2026-08-09)

The embedded audit produced `e53bb2e`. The Reed–Solomon decoder now rejects
symbol index `255`: with `MAX_N = 255`, valid zero-based indices stop at `254`.
A maximum-`k` parity-only recovery regression also exercises the upper host
parameter boundary. The target check passes, but linked image size, RAM, and
named-board HIL evidence remain external E7 gates.

The manual interoperability audit produced `5cef266`. The PC procedure now
distinguishes Linux/BlueZ peripheral support from Linux, Windows, and macOS
central paths, uses native executable/path examples per host, and names the
Debian/Ubuntu BlueZ and D-Bus prerequisites. It explicitly retains the
Android/iOS consuming-app and physical-radio boundary instead of presenting a
host-only build as mobile interoperability. The documented in-memory CLI
smoke test passed with both receiver and delivery assertions.

The E7 storage audit produced `0ab41c8`, an isolated integration-test-only
fixed-capacity transaction double. It exercises exact-fill and replacement
delta accounting, outstanding reservations, token mismatch and idempotent
abort behavior, injected commit failure, and recovery to the last durable
generation. It intentionally adds no public API, producer wiring, FFI seam,
wire behavior, or specification change. Its clone-based recovery model is
acceptance evidence only; filesystem crash consistency, embedded allocation,
producer rollback, and file batch transactions remain open.

No file under `specification/` or any protected dirty path was modified.

### Orchestration continuation XIX (2026-08-09)

The BLE evidence audit produced `4d8c0ea`. Evidence manifests now reject
symlinked log/store leaves and symlinked intermediate path components, even
when the resolved target stays inside the package. This prevents an evidence
archive from changing meaning through mutable links between validation and
review. The validator remains structural: it cannot authenticate text logs or
prove that a physical adapter produced them. The focused and complete Python
test suites pass.

The research-simulation audit produced `380bc3b`. Paired EDR measurements now
expose Wilson 95% intervals for discovery and delivery proportions, while
zero-sample traces are represented as unknown (`None`) rather than as evidence
of a zero rate. The interval documentation explicitly limits the result to
binomial sampling uncertainty within one trace; correlated contacts,
between-trace variance, and whole-device energy still require repeated field
measurements. No production routing, scheduler, wire, or specification code
was changed.

The production-compliance audit produced `a296494` and
`docs/E8-S3_STORE_COMPLIANCE.md`. The dossier records official Android, Google
Play, and Apple sources accessed on 2026-08-09, separates platform facts from
engineering inference and store/legal approval, and defines evidence needed
from a real consuming application. It highlights the iOS 26
AccessorySetupKit/state-restoration condition and Android API-34 foreground
service declaration/Doze gates. E8-S3 is therefore documentation-complete,
not mobile-interoperability-complete.

No file under `specification/` or any protected dirty path was modified.

### Orchestration continuation XX (2026-08-09)

The fuzz-workflow audit produced `ba980a5`. The backlog, roadmap, and security
dossier now distinguish the tracked deterministic `seed_corpus.py` recipe and
its CI check/write/verify gates from the intentionally ignored libFuzzer
discovered corpus. The recipe currently validates 46 seeds; the measured
CPU-hour hardening run and sanitizer-linked Linux/CI execution remain open.

The mobile lifecycle audit produced `5fcad7f`. `MeshDrive.start()` now rolls
back partially started advertiser/scanner state and clears `isRunning` when
clock, radio, or initial drain setup fails. `stop()` now releases protocol
sessions and explicitly requests native GATT teardown for every owned link,
preventing physical links from remaining open after coordinator shutdown.
Source-level lifecycle tests pass; Android SDK, CoreBluetooth, and device
behavior remain external.

The cryptographic audit produced `edc18cf`. `NoiseChannel::make_id_proof`
derives initiator/responder identity ordering from the completed Noise channel
instead of accepting a caller-controlled role flag. Correct wire output is
unchanged, while an integration mistake can no longer create a deterministic
identity-proof ordering failure. The remaining composition risks—Noise channel
binding, serverless prekey suppression/replay, opportunistic PCS, and group
sender-key epochs—still require external cryptographic review.

No file under `specification/` or any protected dirty path was modified.

### Orchestration continuation XXI (2026-08-09)

This implementation cycle delegated three bounded tracks. The embedded track
produced `659da7a`: the host fixed-capacity transaction acceptance double is
now a named CI step, with exact-fill, replacement-delta, reservation/abort,
stale-token, failed-commit, and durable-generation recovery coverage. The
target dependency closure passes, but this remains preflight evidence; the
fixed-capacity provider, Embassy executor, board TAL, linked-image budget,
silicon benchmark, and phone-to-sensor HIL are still open.

The interoperability track produced `c2b743d`. The guide now matches the
implemented capability boundary: Linux/BlueZ is the peripheral path and
Linux, Windows, and macOS are central paths through `btleplug`; Windows/macOS
peripheral roles and real hardware execution remain explicitly unverified. A
source-level regression prevents the guide from drifting back to a Linux-only
central claim.

The security track produced `ce7ab18`, `ba76d4a`, `b8d7dfc`, `f63ef82`,
`3cc195c`, and handoff update `2b290aa`. Ratchet counters now use checked
transitions and additive fallible APIs, preserve the full `u32` wire index once
at the terminal value, retain already-skipped keys after terminal-index-first
delivery, and reset exhaustion on a new epoch. Legacy wrappers fail closed via
an explicit panic at exhaustion to preserve their public signatures; production
node/group callers have not yet migrated to the fallible API, which remains an
external application-recovery/API decision. Prekey tests now bind both
identities, sender exchange context, and optional OTK inclusion; the bespoke
serverless X3DH composition and replay/suppression semantics still require
external cryptographic review.

Verification for this continuation: full workspace tests and Clippy pass;
49 crypto tests, 85 node tests, 113 Python tests (8 skipped), 46 deterministic
fuzz seeds, the complete embedded no-std target closure, layer-check, the
transactional acceptance double, and the CLI delivery demo all pass. No file
under `specification/` or any protected dirty path was modified.

### Orchestration continuation XXII (2026-08-09)

The follow-up implementation cycle closed the production-call-site residual
identified in continuation XXI. Commit `a039351` adds fallible direct and
group send paths: `SendChain::try_next` and
`LocalSenderKey::try_encrypt_with_ad` now map ratchet exhaustion to the public
`SendError::RatchetExhausted`, and no bundle is emitted after that failure.
At repeated-contact teardown, `try_bump_epoch` is used for both directional
chains; a terminal or asymmetric rollover retires the conversation and clears
pending markers instead of invoking a panic-compatible wrapper or leaving
desynchronized state. The FFI surface intentionally maps this local lifecycle
condition to its existing coarse `Internal` error, preserving generated mobile
API compatibility. The compatibility wrappers remain for external Rust users,
but production node paths no longer depend on them.

The E7 audit produced `0c441d3`. It records why a no-std flash provider was not
invented: the current `BundleStore` contract has no transactional producer
reservation, and board capacity, allocator, and flash-recovery policy are not
specified. The shipped receive-side `try_put`, bounded snapshot framing, and
transactional acceptance double are therefore separated from the still-open
provider and HIL requirements.

The release-evidence audit produced `b54f4e3`. Security, roadmap, and backlog
documents now distinguish workflow-wired checks from unobserved remote CI,
declared MSRV from an unpinned rolling stable toolchain, durable non-secret
authority snapshots from still-open keystore secrets, and the 17 committed
vectors from deferred X3DH deterministic-RNG coverage. A documentation
regression suite prevents those closed-gap claims from becoming stale again.

Final verification for this continuation: full workspace tests, workspace
Clippy, 50 crypto / 87 node / 20 FFI focused tests, 115 Python tests (8
skipped), 46 deterministic fuzz seeds, the complete embedded no-std target
closure, layer-check, the six-test transactional acceptance double, and the
release CLI delivery demo all pass. No file under `specification/` or any
protected dirty path was modified.

### Orchestration continuation XXIII (2026-08-09)

The delegated RBSR review produced `484e392` and the new
[`RBSR_D3_CONFORMANCE_HANDOFF.md`](./RBSR_D3_CONFORMANCE_HANDOFF.md). The
review confirms that the supplied MMP/Φ dialogue correctly identified a real
conformance concern, but it does not justify silently implementing priority
partitions. The frozen documents require RBSR per declared priority class while
also requiring class-5 traffic to reuse D3; the wire carries only the base
domain byte, and no partition encoding, negotiation, legacy downgrade, or D3
admission-weight formula is defined. The implementation therefore keeps the
executable D0–D3 mapping. A new regression proves that class-5 bundles with
declared priorities 0 and 1 remain in D3 and that receiver-local effective
priority is not treated as a wire key.

The dialogue review also separates validated building blocks from research
hypotheses. PRoPHET and fountain coding are established experimental/standard
references, and Bluetooth Core 6.0 specifies decision-based advertising
filtering, advertiser monitoring, and channel-sounding primitives. None of
those sources proves the proposed EDR 3.14x battery gain, signed mobility
vouchers, Fourier encounter forecasts, contact-survival hazard model, or a
2x end-to-end MMP improvement. Those proposals remain simulation and hardware
measurement work, with privacy, collision, controller/OS support, and
adversarial cost treated as first-class acceptance criteria.

Focused verification for this continuation: `mmp-core` and `mmp-node` library
tests pass (68 and 87 respectively), with the RBSR priority-boundary and
session-order regressions green. No file under `specification/` or any
protected dirty path was modified. The next plan revision adds a research-only
EDR/PCM/contact-survival/DoF evaluation track after the frozen-wire
conformance gates, not before them.

### Orchestration continuation XXIV (2026-08-09)

Three bounded readiness tracks advanced the implementation without changing
the frozen wire. Commit `3d52a87` makes Android artifact validation reject ZIP
members that differ only by case, preventing host-dependent Kotlin source
collisions during extraction. Commit `367aec8` adds an identity-to-conversation
binding fence to the raw Rust secret-state restore seam and preserves atomic
failure behavior. Commit `cc5dcc0` makes `mmp-cli` return a non-success status
when exit-time `SnapshotStore` persistence fails, including successful-delivery
and normal-shutdown paths that previously logged the error but could exit 0.

The changes are intentionally narrower than native platform integration: they
do not claim Android Keystore, iOS Keychain, filesystem crash consistency,
Android/Xcode artifact builds, or real BLE exchange. Focused evidence is 34
`mmp-cli` tests, 87 `mmp-node` tests, and 12 mobile-artifact tests; the full
Python suite and workspace matrix remain the next verification gate. Existing
dirty paths and the entire `specification/` tree remain untouched.

### Orchestration continuation XXV (2026-08-09)

The next bounded platform cycle produced `ef37de0` and `371127d`. The
embedded track added a source-level regression which keeps the Embassy and
embedded TAL shells unconditionally `#![no_std]`; this guards the dependency
boundary but does not invent an allocator, flash provider, executor, radio
backend, linked image, or hardware result.

The mobile track added a deterministic iOS lifecycle-contract checker to CI.
It verifies that both integration guides retain restoration identifiers and
callbacks, monotonic node timing, Keychain seed ownership, serial drive-queue
ownership, link lifecycle forwarding, and ATT back-pressure/flush markers. The
iOS artifact verifier now rejects case-folded slice-identifier collisions and
non-canonical packaged path casing. These are source/artifact integrity gates,
not Swift/CoreBluetooth compilation or physical-device evidence; native iOS
restoration, Keychain accessibility, ATT behavior, and relaunch tests remain
external gates.

The host Kotlin coordinator now queues a polled envelope before resolving ATT
MTU. A transient MTU callback failure therefore leaves the frame retryable
instead of losing it. The change is still a source-level coordinator result;
Android SDK compilation, controller behavior, and real radio exchange remain
unverified.

Focused evidence for this continuation is 32 iOS/Kotlin tests (three
platform-dependent skips), a passing iOS lifecycle checker, the embedded
no-std target closure, and the previously recorded full workspace matrix. No
file under `specification/` or any pre-existing protected dirty path was
modified. The next implementation step remains a real native-platform/HIL
gate, not a new wire or cryptographic claim.

### Orchestration continuation XXVI (2026-08-09)

Two delegated readiness reviews found concrete local resource/lifecycle gaps.
Commit `e3262ec` bounds `mmp-core::ReceiptFilter`'s exact purge map to the
existing 8,192-entry Standard-tier budget, rejects persisted exact counts above
that cap before allocation, and evicts the earliest-expiring entry
deterministically when the authenticated map is full. The Bloom bit remains a
hint, so eviction can only delay an optional purge; it cannot authorize unsafe
deletion. This is a local-memory safeguard and does not alter receipt encoding,
RBSR, quotas, or the frozen wire.

Commit `02dc789` shares the btleplug central's terminal shutdown fence with its
adapter-event watcher. If the watcher cannot be opened or its stream ends,
future `start_scan` and `dial` admission fails closed before the link teardown
sweep. This closes a race in which a physical link could otherwise be admitted
after lifecycle observation had already failed. The change is host-side only;
Linux/BlueZ execution and real adapter exchange remain external evidence.

Focused evidence: five ReceiptFilter tests, 23 btleplug tests, and targeted
Clippy pass. Full workspace tests, workspace Clippy, 124 Python tests (8
platform-dependent skips), 46 deterministic fuzz seeds, the embedded
`thumbv7em-none-eabihf` closure, layer-check, release CLI delivery demo, and
six transactional-store tests all pass. No file under `specification/` or any
pre-existing protected dirty path was modified.

### Orchestration continuation XXVII (2026-08-09)

The vault security review produced `8223f13`. `restore_from_vault` now parses
and validates the bounded identity, node, and conversation context before it
asks a provider to unwrap any DEK. A regression provider counts unwrap calls
and proves that a malformed context returns `VaultError::Invalid` with zero
provider key-unwrapping activity. This enforces the keystore design's
validate-before-decryption boundary without implementing a native backend or
changing the wire contract.

Focused vault evidence is 18 passing tests with targeted Clippy clean. The
complete workspace matrix also passes: Rust tests and Clippy, 124 Python tests
(8 platform-dependent skips), 46 deterministic fuzz seeds, the embedded
`thumbv7em-none-eabihf` closure, layer-check, release CLI delivery demo, and
six transactional-store tests. Native keystore/Keychain/DPAPI adapters,
durable provider recovery, and physical platform evidence remain explicitly
open. No file under `specification/` or any pre-existing protected dirty path
was modified.

### Orchestration continuation XXVIII (2026-08-09)

The delegated AuthorityStore audit produced `52b7fc2`. Runtime mutation now
honors the same local bounds already enforced by snapshot decoding: new
subscriptions and pinned roots are ignored once their fail-closed capacities
are full, while existing entries may still be refreshed. New certificates
return an explicit `CertReject::Capacity` instead of growing beyond the
documented limit. `expire()` now removes revocations after their own
not-after horizon as well as expired certificates. These changes preserve
legitimate admission and refresh behavior without silently dropping
security-critical revocation-list entries.

The parallel secret-surface review was `no_change`: the Rust-internal vault
and secret-state symbols are already excluded from the exported `MmpNode`
surface and covered by the existing generated Kotlin/Swift denylist and API
surface checks. No redundant public API or generated-binding patch was added.

Focused evidence: 14 authority tests and Clippy pass. Full workspace tests,
workspace Clippy, 124 Python tests (8 platform-dependent skips), 46
deterministic fuzz seeds, the embedded `thumbv7em-none-eabihf` closure,
layer-check, release CLI delivery demo, and six transactional-store tests all
pass. Revocation-list overflow policy, native keystore adapters, and physical
platform evidence remain open. No file under `specification/` or any
pre-existing protected dirty path was modified.

### Orchestration continuation XXIX (2026-08-09)

The revocation-overflow gap is now closed by `31da872`. The earlier proposal
to persist a global `authority_disabled` marker was rejected: one exhausted
authority could otherwise deny unrelated scopes and create a cross-scope
denial-of-service condition. The implementation instead adds the additive
`AuthorityStore::try_merge_revocation_list` result while retaining the legacy
`merge_revocation_list` wrapper for existing callers.

Verified entries are preflighted as a unique set before mutation. If the
bounded sticky union would exceed `MAX_AUTH_REVOKED`, no new tombstone is
silently dropped and no older tombstone is evicted. Every affected scope is
quarantined atomically by removing its pinned root and cached certificates;
unrelated roots and certificates remain usable. Existing tombstones remain in
the bounded map and may outlive the quarantined root, so a later explicit
re-pin cannot erase them. Re-pinning is an operator recovery action, not an
automatic overflow response. Unknown suites and already-retained tombstones
retain their prior behavior.

The private authority snapshot remains version 1 and wire-compatible. Decoder
validation now permits a bounded tombstone whose root was deliberately
quarantined; certificates still require a currently pinned root and are
re-verified on restore. This keeps the fail-closed state durable without
introducing a new wire field or a global disable bit. Regression coverage
checks atomic overflow, unrelated-scope availability, root/certificate
removal, rootless-tombstone persistence, and explicit re-pin recovery.

Focused evidence for this continuation is 15 `mmp-core` authority tests and
targeted Clippy. The complete post-commit gate is also green: workspace tests
and Clippy, 124 Python tests (8 skipped), 46 deterministic fuzz seeds, the
embedded `thumbv7em-none-eabihf` closure, layer-check, release CLI delivery
demo, and six transactional-store tests. No file under `specification/` was
modified, no AI-specific artifact was added, and all pre-existing dirty paths
remain outside the commit.

### Orchestration continuation XXX (2026-08-09)

The delegated node-state audit found two concrete runtime lifecycle defects and
closed them without changing the frozen wire. First, authenticated async
first-contact traffic could create one permanent `Conv`, `established_convs`
marker, and ratchet allocation per pseudonymous identity. `MAX_CONVERSATION_STATE`
now bounds admission at the receiver and at every outbound first-contact path;
the check runs before X3DH/sealed work, one-time-prekey consumption, Reed-Solomon
expansion, or symbol minting. Existing conversations remain usable, while a
new over-cap contact fails with `SendError::ResourceLimit`. A link-down race
also retires untracked provisional state instead of allowing the marker table to
exceed the bound. The FFI adapter keeps its deliberately coarse error surface
and maps this local condition to `Internal`.

Second, the runtime `sent` delivery-history map was previously bounded only by
secret-state serialization. It now has the same 4096-entry runtime ceiling:
only `Delivered`/`Expired` terminal history is evictable, and a full table of
`Queued`/`InMesh` messages rejects a new send before bundle construction or
ratchet mutation. This preserves active delivery semantics while ensuring a
long-lived node cannot grow past the snapshot admissibility limit.

The same audit repaired the holder-side blob indexes. Receipt purge now
deindexes removed symbols; replacing a duplicate `(root, symbol)` removes only
the old reverse mapping; and stale reverse cleanup cannot delete a newer forward
mapping. `send_file` uses the single indexing helper for locally minted symbols,
so insertion and replacement follow one invariant.

Focused evidence is 93 `mmp-node` tests and targeted `mmp-node`/`mmp-ffi`
Clippy with `-D warnings`. The post-change full gates are green: workspace
tests and Clippy, 124 Python tests (8 skipped), 46 deterministic fuzz seeds,
the embedded `thumbv7em-none-eabihf` closure, layer-check, release CLI delivery
demo, and six transactional-store tests. Rustfmt could not be invoked because
the current toolchain lacks the `rustfmt` component. No file under
`specification/`, no AI-specific artifact, and no pre-existing dirty path was
modified by this continuation.

### Orchestration continuation XXXI (2026-08-09)

The delegated blob lifecycle track closed the remaining terminal assembly leak
without introducing a wire field or changing holder behavior. `BlobAssembly`
now records a monotonic `terminal_at` when it becomes `Abandoned` or
`Assembled`. `NodeCore::sweep_blob_ttl` retains terminal progress for one
additional `BLOB_INTEREST_TTL_MS` window, then removes only the requester-side
in-memory assembly. Verified partial symbols remain in `blob_index` and the
bundle store, so they remain eligible for normal serving and repair. Active
`Wanted`/`Fetching` assemblies are not evicted by this cleanup. A regression
covers active-state retention, terminal aging, and preservation of the holder
index.

The independent group-state review confirmed that no arbitrary local cap is
safe yet: authenticated future epochs and delayed older epochs can legitimately
cross partitions, while `SenderKeyDist` has no authenticated floor or expiry.
The required next gate is an authenticated `(gid, floor_epoch)` retention
anchor, conservative merge rule, tombstone/floor persistence, and only then a
measured future-epoch budget plus safe pruning. `sent_dists` can be bounded as a
retransmission cache only after that anchor is available.

The mobile review found one remaining consumer-facing contradiction in the
pre-existing dirty `docs/API_REFERENCE.md`: an illustrative `Date.now()` tick
snippet conflicts with the authoritative same-boot monotonic tick contract.
That file was deliberately not edited; the correction remains a handoff item
for the owner of the existing change. The committed manual procedure already
contains the mandatory initial tick ordering.

Focused evidence for this continuation is the new terminal-GC regression plus
strict `mmp-node` Clippy. The post-change full gates are green: workspace tests
and Clippy, 124 Python tests (8 skipped), 46 deterministic fuzz seeds, the
embedded `thumbv7em-none-eabihf` closure, layer-check, release CLI delivery
demo, and six transactional-store tests. No file under `specification/`, no
AI-specific artifact, and no pre-existing dirty path was modified.

### Orchestration continuation XXXII (2026-08-09)

Three delegated audits completed with no additional code patch. The production
store review confirmed that `SnapshotStore::open()` still uses an unbounded
`std::fs::read` path and can retain all valid claims/group blobs from an
unbounded image. `open_with_limits()` bounds the complete image and its hostile
length tests pass, while blocklist validation is already strict and capped.
Changing the default is not safe without a tier-aware policy (standard versus
anchor storage) and a streaming/early-reject parser; this remains a production
host integration gate.

The simulator audit confirmed that `mmp-sim::AdversarySession` buffers,
metrics, and schedules are intentionally caller/test-driver-owned. They are
not imported by production `Node` code, and iteration/payload limits constrain
the adversarial scenarios. No production memory claim is based on those
buffers, so no simulator patch was warranted.

The mobile contract audit found documentation mismatches that remain in
pre-existing dirty files: the FFI error enum omits `ConversationPending` in the
API reference; `pollClose`, `closeActionsDropped`, and `eventQueueDropped` are
missing from that reference; example ticks still use `Date.now()`; one mobile
signature table groups `groupInvite/groupSend/groupRemove` under the wrong
return type; and RN/iOS snippets do not visibly retain destructive `pollSend`
frames across ATT back-pressure or show the `pollClose -> native close ->
linkDown` ordering. Those files were deliberately not overwritten. The
authoritative Rust FFI and committed manual procedure already express the
correct monotonic-time and close-queue requirements; updating the user-owned
dirty documentation is the next human-owned integration action.

Focused evidence: `mmp-store` unit/hostile/bounded/transactional tests and
strict Clippy pass; `mmp-sim` has 27 passing tests and 6 ignored research/HIL
cases. No file under `specification/`, no AI-specific artifact, and no
pre-existing dirty path was modified.

### Orchestration continuation XXXIII (2026-08-09)

The final delegated runtime pass covered the session engine, both BLE
transport adapters, and the generated mobile boundary. `SessionEngine` keeps
remote-driven reassembly, pending frames, handoffs, outbound frames, and RBSR
ledgers within explicit caps; malformed aggregate counts are rejected before
allocation and ledger exhaustion closes the session. Its event queue has only
the bounded lifecycle transitions (`Established` and `Closed`), so no
attacker-controlled growth path was found. The production node supplies
quota-backed host data; custom `SessionHost` implementations remain
responsible for local collection limits.

The btleplug central and peripheral adapters already use bounded control/data
event lanes, generation fencing, stale-callback rejection, bounded disconnect
feedback, and fail-closed tombstone exhaustion. Active-link maps and
`known_peers()` materialization remain adapter-policy surfaces: adding an
arbitrary cap could reject legitimate concurrent physical links and needs an
explicit configurable admission contract. Peripheral stream pre-announcement
also means that recording every unknown generation in `set_streams` would be
incorrect; this is tracked as an internal-invariant follow-up rather than a
one-line patch.

The FFI review found no source-level error mismatch. A regression now locks the
intentional compatibility policy that `ResourceLimit` and `RatchetExhausted`
map to the coarse `FfiError::Internal`; exposing new variants requires a
versioned Kotlin/Swift API decision. Existing user-owned API/mobile documents
still need reconciliation for restore semantics, `ConversationPending`, close
feedback, monotonic ticks, group return types, ATT-write retention, and
`pollClose -> native close -> linkDown` ordering. They were not edited.

Focused evidence: `mmp-core` 79 tests, `mmp-tal-btleplug` 23 tests,
`mmp-tal-peripheral` 40 tests, and `mmp-ffi` 21 tests pass; strict Clippy is
green for the touched crates. No file under `specification/` or AI-specific
artifact was modified.

### Orchestration continuation XXXIV (2026-08-09)

The store, blob, and routing audits were re-run against the current tree. The
store's explicit `open_with_limits` path remains bounded and fail-closed, but
`open()` cannot receive a safe default cap because snapshots combine bundles,
claims, groups, indexes, authority state, and blocklists while standard and
anchor tiers have different policies. The blob plane already caps per-contact
symbols, peer counters, offers, sessions, and output; streaming send and group
file payload preservation remain cross-surface work. Priority-aware transfer
ordering is intentionally split between signed declared priority and
receiver-local effective priority; RBSR/D3 ordering is still a normative
ambiguity and was not guessed.

One concrete implementation defect was found and fixed. The local
`publish_announcement` API accepted priority values above the wire-defined
`0..=3` domain, allowing release builds to sign/store a bundle that every
canonical decoder would reject. The API now rejects this input before signing
or storage with `SendError::InvalidPriority`, and the FFI maps it to the
existing `BadArg` variant. Snapshot restore also rejects raw priority bytes
outside the same domain, preventing a tampered-but-checksummed local image
from influencing ordering with an unrepresentable value. Regression tests
cover both admission paths and prove no bundle is stored on invalid input.

Focused evidence: mmp-node, mmp-store, and mmp-ffi regressions pass; strict
Clippy is green for all three crates. No specification file or pre-existing
dirty path was modified.

### Orchestration continuation XXXV (2026-08-09)

The delegated group, vault, and blob audits produced one native-platform
boundary confirmation and two concrete wire-neutral hardenings. The ratchet
vault seam is already complete inside Rust: it seals the typed secret aggregate
before provider access, validates identity/context before unwrapping, enforces
trusted-clock TTL and durable generation fences, and swaps state atomically.
Native Android Keystore, iOS Keychain, and DPAPI providers remain the real
integration gate; no plaintext persistence path was invented.

Group retention maps remain intentionally uncapped until the protocol defines
authenticated epoch floors and partition-merge semantics. However, malformed
local group snapshots are now rejected when `epoch == 0` or the admin byte is
not canonical `0/1`. Group epoch rotation also uses checked arithmetic and
returns `RatchetExhausted` at `u16::MAX`, preventing wraparound key reuse and
leaving membership unchanged on failure.

The blob audit found that shape-valid but Merkle-invalid class-4 bundles were
indexed before verification when no local fetch was active. Such entries could
later be served through `BLOB_HAVE/BLOB_REQ` without another proof check. The
receive path now verifies every symbol before it remains in the holder index,
removes invalid durable entries, and counts failures only for active fetches.
Two regressions cover unsolicited and active invalid symbols.

Focused evidence: 99 `mmp-node` tests, 22 FFI tests, and 16 + 3 + 1 + 6
store tests pass; strict Clippy is green. No specification file or protected
pre-existing dirty path was modified.

### Orchestration continuation XXXVI (2026-08-09)

The delegated snapshot/crypto audit found one narrow hygiene issue in the
Rust-internal restore decoder. A decoded 32-byte conversation receipt key was
temporarily held in a plain stack array while parsing the remaining components.
It is now held in `Zeroizing<[u8; 32]>` until ownership moves into the live
conversation object. This does not change the wire format, key derivation, or
restore semantics; it removes a non-zeroizing transient copy on rejected or
malformed restores. The snapshot envelope, AEAD associated-data binding,
generation/freshness fences, component limits, and prekey bounds were otherwise
confirmed strict and fail closed.

The vault preflight review found no evidence-based envelope or wrapped-key cap
that can be added safely at the provider seam without duplicating the snapshot
format contract. `snapshot::open` already rejects malformed sizes before
decrypting, while a native provider must still enforce its own opaque-handle
policy. Native keystore/Keychain/DPAPI adapters and their provider-specific
resource limits remain integration gates.

Focused evidence: the three `mmp-node` secret-state tests pass after the
zeroization change; the existing snapshot/vault tests cover malformed lengths,
context binding, freshness, generation rollback, and atomic restore. No
specification file, AI-specific artifact, or pre-existing dirty path was
modified.

### Orchestration continuation XXXVII (2026-08-09)

The next delegated security pass found two distinct checksum-only persistence
risks and one mobile lifecycle gap. `mmp-store` now canonicalizes restored
metadata before it can affect routing or eviction: class bytes above the
reserved 4-bit domain, spray budgets above `L_MAX`, noncanonical boolean bytes,
reversed/overlong lifetimes, and invalid metadata on persist are rejected
before replacement. Forward-compatible classes `6..=15` remain relayable.

At the node boundary, persisted bundle bytes are now canonical-decoded,
content-address checked, bound to their persisted metadata, and signature-
verified before forwarding. Invalid records are removed rather than emitted.
Startup blob-index rebuild applies the same checks and additionally verifies
Merkle membership for symbols, so a locally rewritten checksum cannot create a
servable invalid symbol after restart. Regression coverage includes tampered
handoff bytes and signed-but-Merkle-invalid startup state.

The Android-SDK-free Kotlin coordinator now exposes `refreshBeacon(nowMs)`.
Long-lived hosts can replace a rotated ephemeral advertisement without a
second protocol tick; timestamps remain monotonic, unchanged payloads avoid
redundant restarts, and defensive byte-array copies protect the native adapter
boundary. Kotlin compilation and physical mobile tests remain external gates.

No wire field or frozen specification text changed. Existing user-owned dirty
integration documents and temporary artifacts were not touched.

### Orchestration continuation XXXVIII (2026-08-09)

The persistence audit found a crash-recovery filename edge case. When the
primary snapshot path itself ended in `.tmp`, `Path::with_extension("tmp")`
could select the primary image as its own temporary replacement. The store now
uses a distinct `.part` sibling in that case, including Windows' case-
insensitive `.TMP` alias, while preserving the historical `.tmp` recovery name
for ordinary paths. Regressions prove failed bounded writes preserve the last
complete image and remove only the candidate.

The group audit found a real local memory-growth path rather than guessing a
cap for authenticated remote epochs: `sent_dists` retained every historical
`(group, peer, epoch)` marker. Successful epoch rotation now prunes stale
markers for that group while retaining the new epoch; failed sends remain
retryable. Remote distribution maps remain intentionally uncapped pending an
authenticated epoch-floor/partition-merge design.

The mobile lifecycle audit found that callbacks can admit links synchronously
while scanner startup or the initial drain later fails. `MeshDrive` startup
rollback now sends protocol `linkDown`, clears ownership before native GATT
close callbacks, closes admitted links, and clears pending frames/cache. A
source-level regression covers the failure ordering. Kotlin compilation and
physical device tests remain external gates.

No wire field or frozen specification text changed.

## Source register

- [A7] RFC 9171, Bundle Protocol v7: https://www.rfc-editor.org/rfc/rfc9171.html
- [C9] RFC 6693, PRoPHET: https://www.rfc-editor.org/rfc/rfc6693.html
- [E4] RFC 6330, RaptorQ: https://www.rfc-editor.org/rfc/rfc6330.html
- [F1] Noise Protocol Framework: https://noiseprotocol.org/noise.html
- [I3] Bluetooth SIG Channel Sounding: https://www.bluetooth.com/learn-about-bluetooth/feature-enhancements/channel-sounding/
- [I11] Android RangingManager: https://developer.android.com/develop/connectivity/ranging
- [I15] Bluetooth Core 6.0 feature overview: https://www.bluetooth.com/core-specification-6-feature-overview/
- [K5] NASA/CSU fountain coding for DTN: https://ntrs.nasa.gov/citations/20230007643
