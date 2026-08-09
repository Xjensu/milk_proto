# E5-S4 Conformance Handoff

## Checkpoint

The original transport checkpoint was commit `0343cf5` (`fix(mesh-milk): reserve control capacity for BLE lifecycle`).
The current continuation checkpoint is commit `a931122` (`fix(mesh-milk): implement sealed async fallback safely`).
All commits in this handoff leave the frozen `specification/` directory unchanged.

### 2026-08-08 continuation update

The supplied MMP/Φ dialogue has now been audited against the local documents,
Rust implementation, adversarial tests, and authoritative protocol/platform
references. `docs/PROPOSAL_AUDIT_MMP_PHI.md` is the evidence record and the
amended work order. Its central conclusion is deliberately conservative:
MMP/Φ's EDR, mobility-voucher, contact-survival, and DoF ideas are research
hypotheses, not evidence for a v1 wire change or a production 2× claim.

Three high-risk conformance points are now pinned without editing the frozen
specification: ID_PROOF is implemented as the first encrypted Noise transport
record and gates application frames; bundle AEAD uses one lifetime-zeroed
canonical header AD helper for direct, async, and group paths; and copy-budget
security is documented as a receiver-local admission bound rather than a
Byzantine global conservation law. The ID_PROOF erratum also distinguishes
protocol-error `ERROR + CLOSE` handling from intentional silent close after a
valid encrypted proof fails identity or admission checks.

The stale fuzz documentation was corrected accordingly: ID_PROOF payload
grammar remains fuzzed, but it is not described as a pre-channel wire parser.
The existing `frames.json` ID_PROOF entry remains a decoded-payload grammar
fixture, not a deterministic encrypted Noise transcript. Do not invent a new
transcript-vector schema until test-only deterministic Noise ephemeral inputs
and an interoperable artifact format are agreed.

The sealed-box baseline is now wired through the Rust node path. A direct
first-contact send uses a peer's exchange-key claim even when its optional
prekey batch is absent, malformed, or suppressed; the sender and recipient
derive the same sealed conversation root for receipts and subsequent ratchet
traffic. Async decryption uses candidate conversation state and commits it
only after AEAD verification, so a valid-signature forged ciphertext cannot
consume ratchet state. Established conversations reject replacement sealed
roots deliberately; this preserves receipt and ratchet integrity, while
restart/rekey recovery remains an availability gate for a future live-session
implementation.

The first-contact ordering boundary is now explicit in commit `c585978`:
after an async (prekey or sealed) root-establishing send, application sends for
that conversation return `ConversationPending` until the authenticated
delivery receipt arrives. This prevents a later direct bundle from overtaking
the root in a DTN queue. The pending marker expires with the first bundle, and
receipt handling also clears markers for first-contact bundles that do not have
ordinary `SentMessage` tracking (for example, an own sender-key distribution).
Oversized admission is preflighted against the canonical wire length before
ephemeral material, sender-chain indices, or conversation state are consumed;
the regression test proves that a rejected oversized first send can be retried
as a valid prekey bundle. File resend/dedup coverage now performs the required
receipt contact before minting its second manifest. This is an availability
tradeoff, not a wire change: callers should retry `ConversationPending` after
delivery evidence or bundle expiry.

Verification at this checkpoint:

```text
cargo test --workspace --all-targets -- --test-threads=1   # pass; 4 ignored by design
cargo clippy --workspace --all-targets -- -D warnings      # pass
cargo run -p layer-check                                  # pass
cargo run -p mmp-cli -- demo                              # pass; receipt observed
```

## 2026-08-08 MMP/Φ research implementation

The simulator now contains an explicitly non-normative
`mmp_sim::energy::RendezvousEnergyModel` (`ecf25a2`). It implements the
square-root optimum for the controlled hypothesis `h ∝ λρ` and tests the
fixed-hazard invariant and illustrative baseline gain. The accompanying
`docs/MMP_PHI_SIMULATION.md` records units, assumptions, and the required
measurement plan. This does not alter MMP/1 wire behavior and does not claim a
2× battery result; collisions, controller scheduling, OS wakeups, and
whole-device energy still require platform measurements.

Current verification after `ecf25a2`:

```text
cargo test --workspace --all-targets -- --test-threads=1   # pass; 2 EDR tests included
cargo clippy --workspace --all-targets -- -D warnings      # pass
cargo test -p mmp-sim edr_ -- --nocapture                 # pass
git diff --name-only -- specification                    # empty
```

The follow-up snapshot audit (`ff37a15`) also rejects the reserved
`u64::MAX` written-at timestamp during both sealing and opening. This closes a
freshness bypass that would otherwise exist only when a faulty provider clock
and envelope timestamp reached the same sentinel value. The focused crypto and
node suites remain green; the native trusted-clock implementation is still an
external provider responsibility.

## 2026-08-08 lifecycle and mobile-surface follow-up

Two delegated audits closed additional implementation gaps without touching
the frozen specification:

- `a959f8a` requires an exact successor generation for every vault commit;
  gaps, forks, and overflow are rejected before provider mutation.
- `70fa9b7` rejects the reserved maximum prekey generation and makes OTK
  tombstone/expiry updates transactional on generation overflow.
- `d6c0d88` adds a CI check that the generated Kotlin and Swift bindings expose
  the complete intended `MmpNode` API surface, alongside the existing secret-
  surface check.

The focused evidence is 55 `mmp-node` tests and 35 `mmp-crypto` tests passing,
plus six mobile-binding checker tests. The remaining mobile documentation
discrepancies are intentionally not staged because the integration guides
already contain unrelated user edits; they are listed here for the next clean
documentation pass. Native radio shells, keystore providers, and physical
Linux/mobile runs remain external gates.

The remaining exit-gate evidence is physical: two Linux/BlueZ PCs for real
BLE, followed by a native Android/iOS consuming shell. Windows-only builds
are not evidence for the Linux peripheral backend or mobile background
behavior.

The current continuation after this checkpoint keeps the public 2048-event
FIFO bounded while splitting lifecycle admission into a 64-record normal
control lane plus a bounded 64-entry emergency `LinkDown` tombstone lane and a
1,984-record data lane. Duplicate teardown is coalesced, stale per-link data is
purged before admission, and an `Up` cannot be admitted after an emergency
`Down` for the same link. Central and peripheral stream EOF now trigger link
sweeps; peripheral endpoint retirement is capped at 256 identities per
direction and fails closed rather than evicting history. External BLE
disconnects are bounded and occur outside lifecycle locks. These changes are
implementation hardening only; the frozen specification remains unchanged.
The supplied MMP/Φ proposal audit and its amended research plan are recorded
in `docs/PROPOSAL_AUDIT_MMP_PHI.md`.

Commit `2357e6f` closes the remaining host-side lifecycle feedback gap. Both
BLE event queues now retain a sticky count when their normal and emergency
control lanes are simultaneously full; central and Linux peripheral roles
expose that count plus an explicit `shutdown_all()` sweep. The CLI drains
ordinary `poll_close()` actions, fails closed on either node or transport
lifecycle saturation, and invokes the sweep on all loop exit paths. The
transport queue tests exercise the full-lane rejection and sticky diagnostic.
Physical adapter teardown and the Linux/BlueZ interoperability gate remain
unverified on this Windows host.

Full verification was rerun after these changes on 2026-08-08: the serial
workspace matrix passed (including 40 `mmp-node`, 14 `mmp-ffi`, 16 central TAL,
28 peripheral TAL, 26 CLI, and 10 non-ignored simulator tests), workspace
clippy passed with `-D warnings`, `layer-check` passed, and `mmp-cli demo`
observed the expected two-contact delivery receipt. The four ignored simulator
tests remain deliberate scale/deployment drills, not failures.

The Linux-target check was also retried from this Windows host and stopped
before Rust backend compilation at `libdbus-sys`: no Linux pkg-config/sysroot
is configured (`PKG_CONFIG_SYSROOT_DIR`/`PKG_CONFIG_PATH` are absent). This is
an environment gate, not evidence against the Linux implementation; perform
the check on a Linux/BlueZ host before the real two-PC exchange.

The continued hardening commits are `47e3cfc`, `8a83621`, `b6dcea7`, and
`d96ca5c`. `send_file` now performs peer/conversation admission before
minting content-addressed symbol bundles, so `ConversationPending` and
`UnknownPeer` failures have no storage side effects. The sealed async rollback
regression is covered by `556dfd0`. Central shutdown now marks a terminal
closing state before draining, rejects late dial insertion, cleans up every
post-connect setup failure, removes logical links before bounded write-gate
waits, and disconnects stream-abort paths. The Linux peripheral role signals
shutdown to its control task, which aborts and joins spawned reader tasks before
returning. These changes narrow lifecycle races without changing the wire
contract; real adapter behavior remains hardware-gated.

The implementation audit also found and closed a file-plane admission leak:
`send_file` now requires both the peer identity and exchange-key claims before
RS/Merkle symbol generation, and checks `ConversationPending` first. A rejected
manifest therefore does not mint symbols or consume local blob quota. The
post-change workspace verification passed with 42 `mmp-node` tests, including
the new sealed rollback and file-side-effect regressions, 17 central TAL tests,
28 peripheral TAL tests, 14 FFI tests, and the existing simulator/adversarial
matrix. Workspace clippy, `layer-check`, and the CLI demo also passed.

The final lifecycle pass added four additional fences. `341496f` requires an
exchange-key claim before file encoding; `f9d588d` moves peripheral write-gate
waits outside the event lifecycle lock and bounds them; `8ad903b` stops central
scanning and drops the peripheral advertisement during terminal shutdown; and
`8b9b4a8` rejects scan startup that races with shutdown. The central focused
suite now covers 17 lifecycle cases and the peripheral suite 28 cases. These
changes close host-side late-start and lock-order hazards, but cannot prove
BlueZ adapter teardown or mobile OS behavior without the corresponding hosts.

The subsequent continuation closes the remaining peripheral lifecycle lock
ordering hole in `c3d8c47`: `PeripheralRole::close_link` now removes/fences
logical state and emits `LinkDown` before a bounded writer-gate wait outside
`event_gate`. Its regression covers a permanently held writer gate; the
peripheral suite now has 29 passing cases. Commit `de9b06b` adds a bounded,
strictly decoded `AuthorityStore` snapshot record. Pinned roots, verified
public certificates, subscriptions, and sticky revocations now survive
`SnapshotStore`/UniFFI restart, with signature re-validation and a full FFI
restart test. Secret ratchet/prekey state remains intentionally excluded until
a platform-keystore envelope exists.

Commit `ebb2045` also removes retained `CK0` from `mmp-node::Conv`: the
conversation derives and zeroizes a dedicated receipt-MAC key while creating
the directional chains, and receipt emission/verification uses that stored
zeroizing key. The sealed-root replacement regression and the full 42-test
node suite remain green. This reduces in-memory root exposure but does not
claim restart continuity; keystore-wrapped chain/prekey persistence is still
an explicit Phase B/native-platform gate.

Commit `299a496` extends the same local-state boundary to the explicit
blocklist: entries are sorted, capped at 1,024, strictly decoded, and restored
before sessions can be admitted. A restart regression drives a real FFI contact
and confirms that a previously blocked peer cannot establish again. This is a
local policy record, not a protocol or identity-revocation replacement.
Follow-up `8c4834b` makes invalid authority/blocklist staging atomic: a bad
candidate is ignored and cannot erase the last valid snapshot.

The post-change workspace matrix passes with 43 `mmp-node`, 16 `mmp-ffi`, and
9 `mmp-store` tests, plus the existing four intentionally ignored simulator
deployment/scale drills; workspace Clippy, `layer-check`, and the CLI demo are
clean.

### 2026-08-09 ratchet caller migration

Production node paths no longer call the panic-compatible ratchet wrappers.
Direct pairwise sends use `SendChain::try_next`; group sender-key encryption
uses `LocalSenderKey::try_encrypt_with_ad`. Both paths map exhaustion to the
new public `mmp_node::SendError::RatchetExhausted` and emit no bundle after the
failure. The old `next`/`bump_epoch` and `encrypt`/`encrypt_with_ad` methods
remain as additive compatibility wrappers for existing Rust consumers and
unit tests; they retain their fail-closed panic behavior if called after a
terminal counter or epoch.

At `LinkDown`, epoch rollover is also fallible. If either directional chain is
already at the terminal `u16` epoch (or an unexpected asymmetric rollover
failure occurs), the node retires the conversation, clears its established and
pending markers, and requires a fresh conversation rather than risking chain
desynchronization. This is a local availability policy and does not alter the
wire format or specification.

The FFI error enum remains deliberately coarse: `SendError::RatchetExhausted`
maps to `FfiError::Internal`, so generated mobile bindings do not gain a new
wire/API enum value. Native/Rust callers that need remediation detail should
match the new `SendError` arm and re-establish or re-key the conversation/group.
Focused regression coverage now includes direct-send no-side-effect behavior,
LinkDown retirement at epoch exhaustion, and fallible group encryption; the
crypto/node suites pass 50 and 87 tests respectively.

The delegated documentation audit identified stale examples in the API,
Android/iOS, React Native, and handbook files (missing `poll_close`, sticky
saturation handling, and `ConversationPending` retry semantics). Those files
contain pre-existing user edits and were intentionally not staged. The
authoritative close contract remains [`docs/TRANSPORT_CLOSE_FEEDBACK.md`]; the
next documentation pass should merge its lifecycle wording into those host
guides with a user-approved clean diff.

Recent focused commits, oldest first:

- `7d3c39e` — independent envelope and outer-bundle reader;
- `01a4014` — HELLO and all 17 registered frame vectors;
- `c34d7eb` — outer-bundle vectors for classes 0 through 5;
- `89a2185` — strict Ed25519 non-canonical `S + L` rejection vector;
- `2bb6d81` — complete 9-domain signature matrix (9 accepts, 72 rejects);
- `66786fb` — envelope-cap and malformed-TLV negative vectors.
- `c34069b` — independent Beacon and all-registered-frame payload grammars;
- `b07a0b7` — machine-readable coverage manifest;
- `45ccdf1` — group-control security boundary and remediation order.
- `a73f747` — CLI snapshot restoration/staging and a restart regression test;
  the manual `connect` → `Ctrl-C` → `send --store` workflow now preserves
  claims, group blobs, replay state, and receipt suppression state.
- `7f29d7d` — runtime outbound frame/envelope cap checks in `SessionEngine`,
  preventing release builds from emitting locally generated oversized frames;
  The receive framing, post-decrypt validation, and `Frame::decode_payload`
  paths now reject payloads above the frozen 4096-byte cap. The 16-byte Noise
  AEAD tag therefore leaves a 4080-byte maximum encrypted application payload.
- follow-up group hardening binds each direct sender-key `member` to the
  authenticated origin, rejects stale/non-member distributions, and suppresses
  duplicate `(member, epoch)` receiver-chain resets. Admin forwarding was
  removed because the frozen sender-key record has no provenance proof for
  another member.
- `8f40af4` — strict cap enforcement, sender-key provenance/stale/duplicate
  guards, simulator boundary correction, generated-vector refresh, and mobile
  packaging preflight checks.
- `c929c70` — parser memory-amplification and file-transfer admission
  hardening: `RECON_IDS`/`WANT` counts are checked against remaining bytes before
  allocation, and `FileManifest` rejects every entry above the normative
  8 MiB `FILE_MAX` before any receiver-side offer or blob assembly can use it.
- `e24637d` — local snapshot hardening: Bloom restore rejects zero/oversized
  moduli with checked slice arithmetic; SeenIndex and ReceiptFilter reject
  impossible bucket/exact counts before allocation, while preserving the
  fixed 12-bucket and 1024-entry limits.
- `d420934` — group snapshot hardening: OrSet member/tag/tombstone counts are
  checked against remaining bytes before loops, and GroupState length arithmetic
  is checked before slicing, including the 32-bit overflow path.
- `31a1106` — corrected the GroupState trailing-byte regression fixture to use
  the valid eight-byte empty OR-Set encoding, so the test exercises the exact
  record-length check rather than an earlier malformed-inner-record path.
- `aa9daa9` — bounded SessionEngine transport reassembly: oversized prefixes
  are rejected before append/activity, valid batched envelopes are drained
  incrementally, and `rx` never grows beyond the frozen envelope cap. RED/GREEN
  coverage includes split-prefix rejection and a 300-frame aggregate.
- `dc47c72` — normative HELLO/CREDIT flow control: peer credits are enforced,
  deferred frames are bounded, and atomic BUNDLE+BUDGET handoff waits for two
  credits so receive-window replenishment cannot deadlock.
- `d82606b` — bounded `mmp-node` outbound queue: each admitted session is
  limited to eight data-credit frames plus four control-reserve frames, with a
  tier-derived global frame/byte budget. Overflow clears only the affected link
  and its session, preserving unrelated links and avoiding partial encrypted-
  frame drops that would make the Noise nonce stream ambiguous.
- `db84453` — bounded the `mmp-tal-btleplug` event channel to 2048 events.
  Notification, beacon, link-up, and link-down producers now await capacity;
  no encrypted notification is dropped on a full host queue, and the dial task
  emits `LinkUp` before it begins consuming RX notifications.
- `31ccfd5` — preserved the central link lifecycle ordering while adding the
  bounded channel: `dial` returns without self-deadlocking a full event loop,
  but its per-link producer still sends `LinkUp` before reading notifications.
- `d57657c` — applied the same 2048-event bounded FIFO/backpressure policy to
  the Linux peripheral API and aligned the unsupported-platform receiver type.
  ATT read/control producers await capacity rather than silently dropping
  ordered bytes; Linux runtime compilation remains BlueZ/pkg-config gated here.
- `e106b9a` — added a per-link async write gate to the central `send` path and
  held it across the complete ATT chunk loop, so concurrent callers cannot
  interleave encrypted frame chunks. The regression test exercises the gate
  under two concurrent Tokio tasks.
- `6813a77` — acquired that same write gate before central `hangup` and adapter
  disconnect removal, so teardown waits for an in-flight write sequence instead
  of disconnecting halfway through an encrypted frame.
- `7226f1f` — added monotonic central link generations and an event-gated
  lifecycle transition. Reconnect, hangup, adapter disconnect, and RX EOF now
  revalidate the captured generation before removal; stale RX bytes and stale
  teardown callbacks cannot affect a replacement with the same stable
  `LinkId`. Replacement ordering is explicitly `LinkDown` then `LinkUp`, and
  the bounded queue remains the backpressure boundary.
- `a4ecfb8` — added direction-specific monotonic BlueZ stream tokens backed by
  endpoint identities. Linux reader and writer callbacks now reject stale
  data/teardown, serialize notification writes against same-direction socket
  replacement, and retire delayed opposite-direction callbacks. The
  hardware-independent lifecycle tests run on Windows; Linux compilation and
  real BlueZ execution remain environment-gated.

- `a931122` — implemented sealed-box async fallback, candidate-before-commit
  async decryption, sealed-root derivation, FFI error wording, and node/crypto
  regressions for delivery, receipts, malformed ciphertext, and established
  conversation protection.

- `0343cf5` — replaced both transport-facing FIFO producers with bounded
  synchronous ingress plus a dispatcher. Each backend reserves 64 lifecycle
  control records and 1,984 data records. Central data is fenced by link
  generation; peripheral data is fenced by reader/writer stream generation.
  Data overflow retires only the affected link and purges its stale bytes;
  control overflow fails closed after removing live state. The public
  `events()` receiver type is unchanged, and central dial failure now tears
  down a link if its initial control record cannot be admitted.

The latest session-admission slice maps the public `Node` API's Leaf/Relay/
Anchor tiers to one/two/eight concurrent sessions. Duplicate or over-quota
`LinkUp` events are silently denied, and slots are released on link-down or
engine closure. The embedded T0/Core profile is not represented by this `std`
API and remains a separate implementation gate.

The current frame-credit slice implements specification/09 §5.2: peer HELLO
credits are clamped to the normative eight-frame window, every established
data frame consumes one credit, CREDIT grants replenish the window, and a
two-frame receive reserve prevents BUNDLE+BUDGET deadlock. Deferred frames are
bounded at 256 per session; bundle requests are retained until two credits are
available, so `load_for_handoff` is never called for an unsendable pair.

The manual desktop/mobile installation and interoperability procedure is in
`docs/MANUAL_INTEROP_TESTING.md` (committed earlier). It accurately states
that Linux BlueZ PC↔PC BLE is executable today, while Android/iOS currently
have artifact/API bridge checks rather than a turnkey consuming application.

## What is verified

`implementation/mesh-milk/vectors/` now contains deterministic artifacts for:

- HELLO, the 17 registered frame tags, and envelope framing;
- outer Bundle classes 0–5 with production-generated signatures;
- strict Ed25519 canonical-S rejection (`S + L`);
- all nine registered signing domains and their full cross-verification matrix;
- lifetime-reset and budget-clamp invariants;
- unknown-critical, out-of-order, and duplicate TLVs;
- the normative 4096-byte logical frame and encrypted-envelope cap, with
  4080-byte maximum application plaintext before the Noise tag.
- strict negative coverage for hostile `u16::MAX` reconciliation/request
  counts, plus the exact `FILE_MAX` boundary and an oversized-manifest reject.
- persistence decoder regression coverage for zero/oversized Bloom filters,
  more-than-12 bucket snapshots, and exact-count claims with no remaining
  records.
- group persistence regression coverage for hostile OR-Set counts and
  overflowed/trailing GroupState records.
- independent Beacon and frame-payload grammar checks;
- `coverage.json`, mapping all committed artifacts to requirement IDs, source
  sections, and verification evidence.

The independent reader intentionally does not call production frame, bundle,
TLV, or payload decoders. Production tests separately perform semantic
round-trips and cryptographic checks.

## Verification evidence

An earlier serial workspace matrix passed, including 26 `mmp-cli`, 33
`mmp-core`, 26 `mmp-node`, 40 `mmp-types` unit tests, and 3 new parser/manifest
integration tests; `mmp-sim` passed 9 active tests with 4 intentionally ignored
scale/deployment cases. Workspace clippy with `-D warnings`, `cargo run -p
layer-check`, and `cargo run -p mmp-cli -- demo` also passed. Rustfmt could not
be run because the rustfmt component is not installed in the environment.
The persistence slice additionally passed 33 `mmp-core` unit tests, 4 focused
integration tests, and `mmp-store`'s 7 tests after the build cache was cleaned
to recover 7.1 GiB of disk space.
The group-decoder follow-up passed 34 `mmp-core` unit tests, 27 `mmp-node`
unit tests, and focused clippy for both crates.
The final post-follow-up workspace matrix passed with 34 `mmp-core`, 4
`mmp-core` persistence integration, 27 `mmp-node`, 40 `mmp-types`, and 3
`mmp-types` parser/manifest integration tests; `mmp-sim` again passed 9 active
tests with 4 intentionally ignored cases. Workspace clippy and `layer-check`
also passed.
After `aa9daa9`, the serial workspace matrix passed with 36 `mmp-core` unit,
4 persistence integration, 27 `mmp-node`, 40 `mmp-types`, and 3 parser/manifest
integration tests; `mmp-sim` passed 9 active tests with 4 ignored deployment or
scale cases. Workspace clippy, `layer-check`, and the CLI demo all passed.
After `344794b`, the full serial workspace matrix passed with 36 `mmp-core`
unit, 4 persistence integration, 31 `mmp-node`, 40 `mmp-types`, and 3
parser/manifest integration tests; `mmp-sim` passed 9 active tests with 4
intentionally ignored deployment or scale cases. The workspace inventory lists
221 active test items. Workspace clippy with `-D warnings`, `layer-check`, and
the CLI demo all passed again.
After `dc47c72`, the full serial workspace matrix passed with 40 `mmp-core`
unit, 4 persistence integration, 31 `mmp-node`, 40 `mmp-types`, and 3
parser/manifest integration tests; `mmp-sim` again passed 9 active tests with
4 intentionally ignored deployment or scale cases. The workspace inventory
lists 225 active test items. Workspace clippy with `-D warnings`,
`layer-check`, and the CLI demo all passed again.
After `d82606b`, the focused `mmp-node` suite passed 34 tests, including three
outbound-queue overflow regressions. Workspace clippy with `-D warnings`,
`layer-check`, and the CLI demo passed again. The subsequent full workspace
matrix passed with 228 active test items and 4 intentionally ignored simulator
cases.
After `db84453`, `mmp-tal-btleplug` passed 8 tests (including the bounded FIFO
backpressure regression) and package clippy passed with `-D warnings`. The
full workspace matrix remained at 228 active test items with 4 intentionally
ignored simulator cases.
After `d57657c`, `mmp-tal-peripheral` passed 15 Windows-target tests,
including its bounded FIFO regression, and package clippy passed with
`-D warnings`. A Linux-target check was attempted but stopped in
`libdbus-sys` before compiling the backend because this Windows host lacks a
configured Linux pkg-config/sysroot; the real BlueZ build/hardware gate is
unchanged.
After `e106b9a`, `mmp-tal-btleplug` passed 9 tests, including the concurrent
write-gate regression, and package clippy passed with `-D warnings`.
After `6813a77`, the full workspace matrix passed with 228 active test items
and 4 intentionally ignored simulator cases; workspace clippy and
`layer-check` remained clean.
After `7226f1f`, `mmp-tal-btleplug` passed 12 focused tests (including
monotonic-generation and replacement-order regressions), package clippy
passed with `-D warnings`, and the full workspace matrix again passed with
228 active test items and 4 intentionally ignored simulator cases.
Workspace clippy and `layer-check` remained clean. Rustfmt was not available
because the installed toolchain lacks the rustfmt component.
The lifecycle regressions exercise generation allocation and replacement signal
ordering without BLE hardware; direct callback scheduling remains a real-radio
and Linux/BlueZ validation gate.
The peripheral lifecycle tests additionally cover duplicate endpoints,
direction-specific replacement, reconnect generation non-reuse, and delayed
opposite-direction callbacks. They do not substitute for a Linux BlueZ build.
After `a4ecfb8`, the full workspace test matrix completed with no failures
(four simulator/deployment cases intentionally ignored), workspace clippy and
`layer-check` remained clean, and the Linux-target check stopped in
`libdbus-sys` before Rust compilation because this Windows host has no
configured Linux pkg-config/sysroot.
After `0343cf5`, central passed 14 focused tests and peripheral passed 24
Windows-target tests; both package clippy runs passed with `-D warnings`.
The serial workspace matrix passed with 9 active simulator tests and 4
intentionally ignored deployment/scale cases; workspace clippy and
`layer-check` remained clean. The Linux-target check reproduced the same
`libdbus-sys` pkg-config/sysroot failure before backend compilation. Rustfmt
could not run because the installed toolchain has no rustfmt component.

The current continuation passes 15 central focused tests and 27 peripheral
Windows-target tests, including emergency-down ordering, retired-link draining,
stream-EOF sweeps, and the retired-endpoint cap. Workspace tests pass with 9
active simulator cases and 4 intentionally ignored deployment/scale cases;
workspace clippy, `layer-check`, and `mmp-cli -- demo` are clean. The Linux
target check reproduces the same `libdbus-sys` pkg-config/sysroot failure before
backend compilation. Rustfmt remains unavailable on this host.

The conformance/security continuation added two `mmp-core` RX-bound tests:
plaintext `ID_PROOF` is rejected after Noise handshake completion, and an
application frame cannot establish a session before the encrypted identity
proof. It also added the active `mmp-sim` Byzantine double-grant scenario,
which demonstrates that copy-budget admission is bounded independently at
each honest receiver but is not a globally conserved Byzantine capability.
The focused `mmp-core` suite passes 42 unit tests; `mmp-sim` passes 10 active
tests and 4 intentionally ignored deployment/scale cases.

After `a931122`, the sealed baseline and candidate-state regressions pass in
the focused suites: 20 `mmp-crypto` tests, 38 `mmp-node` tests, and 12 FFI
tests. The serial workspace matrix passes with four intentionally ignored
simulator/deployment cases; workspace clippy, `layer-check`, and the CLI demo
remain clean. Rustfmt is still unavailable because the pinned toolchain lacks
the component.

Regenerate vectors with:

```text
cargo run -p mmp-cli -- vectors
```

Then run the focused suite before committing:

```text
cargo test -p mmp-cli --all-targets -- --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p layer-check
```

## Deliberate specification boundaries

These are implementation/specification discrepancies, not silent protocol
changes:

1. `SigDomain::GroupMsg` is used by the current class-2 implementation but is
   not one of the nine registered document-10 domains. It is excluded from the
   9×9 matrix and remains provisional.
2. Class-4 Merkle-path framing is an implementation extension beyond the
   frozen class-4 description.
3. Class-5 `AnnouncementBody` carries a `suite` byte required by the current
   document-18 signing preimage but omitted from the frozen document-10
    listing. The vector is explicitly marked provisional.
4. Group bootstrap still uses the authenticated peer as a trust-on-first-use
   inviter because the frozen sender-key record has no admin-signed membership
   envelope. The candidate control schema remains quarantined; this is a
   documented residual risk, not a specification edit.

Do not “fix” these by editing `specification/` during routine implementation.
Record evidence and obtain a protocol decision first.

### 2026-08-08 continuation update (lifecycle and host persistence)

Commit `7031c79` closes two additional host-side lifecycle races. The central
adapter watcher now removes and fences a disconnected link under the lifecycle
gate, emits `LinkDown`, then performs only a bounded writer-gate wait outside
that gate. The Linux peripheral role now has an atomic shutdown admission
fence, prioritizes shutdown handling, aborts/joins reader tasks, and rejects a
link admission racing with `shutdown_all()`. Focused suites are 18 central and
30 peripheral tests; both suites and their warning-deny clippy checks pass.
Linux/BlueZ compilation is still blocked on this Windows host by the missing
Linux pkg-config/sysroot for `libdbus-sys`.

The CLI persistence path is now aligned with UniFFI: `mmp-cli --store` restores
and stages validated authority admission snapshots and the bounded explicit
blocklist in addition to claims, groups, `SeenIndex`, and `ReceiptFilter`.
The restart regression covers pinned/subscribed authority state and a blocked
peer. The `MMPSNAP2` file remains a checksum/integrity container, not a secret
store; identity, ratchet, prekey, sender-key, and conversation secrets are not
written there. The implementation/backlog/roadmap status now explicitly calls
this non-secret persistence complete while retaining the keystore gate.

### Ratchet-persistence design boundary (do not overclaim)

The CK₀ retirement in `ebb2045` is intentionally an in-memory hardening step:
`Conv` keeps only a zeroizing derived receipt-MAC key plus directional chain
state. A future keystore-backed snapshot must therefore persist the derived
receipt key as secret state; CK₀ itself must never return. A complete restart
restore also needs the send/receive chain indices and skipped-key window,
prekey secret/rotation state, conversation DAG heads, the pending first-contact
marker, and a restart-safe mapping for outgoing BundleIDs to receipt handling.
Restoring only chain keys would silently lose causal ordering or leave queued
messages unable to transition to `Delivered`.

The safe next seam is a Rust-only provider owned by Android Keystore/iOS
Keychain (and a desktop equivalent where appropriate), with an authenticated
sealed envelope containing version, identity-bound AAD, durable written-at
metadata, and an explicit 72-hour freshness check. Do not derive a wrapping key
from the identity seed, expose raw ratchet bytes through UniFFI/JS, or reuse
`mmp-crypto::aead::seal/open`'s single-use zero nonce for repeated snapshots.
`SnapshotStore` must receive at most an opaque sealed record after that envelope
has a reviewed format and atomic save/load semantics. Until then, restart
continuity is an availability gap and must remain documented as such.

Commit `51e00eb` adds the first safe implementation slice for that future
boundary: strict provider-local codecs for `SendChain`, `RecvChain` (including
bounded skipped keys), and `PrekeyStore`. The codecs reject unsupported
versions, truncation/trailing bytes, counter wrap, oversized windows/batches,
duplicate or unsorted records, invalid epoch/index combinations, and non-
contiguous prekey IDs. Secret buffers and temporary key material are
zeroizing, and `mmp-crypto` now passes both its standard and no-`std` library
checks. These methods are not persistence by themselves and are not exposed
through FFI.

The proposed generic persistence seam on the separate `codex/e5-s4-conformance`
branch was reviewed and deliberately not merged. It did not make generation
rollback rejection, identity/conversation-bound AAD, nonce uniqueness,
crash-atomic multi-state commit, trusted TTL enforcement, or complete receipt/
DAG/pending/prekey state mandatory. The resulting design gate is recorded in
`docs/KEYSTORE_SNAPSHOT_DESIGN.md`: Rust must seal one complete typed aggregate
under a fresh-nonce AEAD, the native provider may wrap only a DEK/opaque handle,
and missing or rollback-unsafe keystore state must fail closed without a
cleartext fallback.

Commit `c6685c0` adds the next isolated primitive, `mmp-crypto::snapshot`: a
fresh-injected-nonce ChaCha20-Poly1305 envelope with context-hash AAD, exact
framing, a 256 KiB plaintext cap, and trusted-clock TTL/future-timestamp
checks. It returns zeroizing plaintext and is still only a provider payload
primitive. Durable monotonic generation, DEK wrapping, complete Node aggregate
serialization, and crash-atomic recovery are intentionally not claimed.

Commit `c2edc90` adds a bounded, canonical Rust-internal Node secret-state
aggregate. It covers pairwise conversation ratchet/DAG state, the derived
receipt key, established and first-contact markers, pending async material,
BundleID delivery tracking, and the current prekey secret batch. Its tests
cover byte-stable round-trip plus malformed-input atomicity and busy-restore
rejection. This is an encoding seam only: it is not written to `MMPSNAP2`, not
passed to UniFFI/React Native, and not yet wrapped by a platform keystore.
Group sender-key ratchets, durable generation, and crash-atomic provider commit
remain explicit restart-continuity gates. The prekey aggregate now includes
provider-local generation/expiry metadata and idempotent OTK-consumption
tombstones; enforcing expiry/rotation still requires the trusted-clock and
native-provider work below.

The group non-secret restore path now carries an explicit re-gossip marker:
`GroupState::decode` creates a fresh local sender key and the first subsequent
group send schedules that key to every restored member before emitting group
ciphertext. A failed distribution leaves the marker set and fails the send;
this is an availability/re-gossip policy, not persistence of sender-key
secrets.

### 2026-08-08 secret-continuity and verification update

The prekey provider-local codec now has a version-2 state form containing the
private SPK/OTK material, lifecycle generation, local expiry metadata, and
sorted idempotent OTK-consumption tombstones. Successful authenticated prekey
first contact records the tombstone without changing the frozen signed batch
wire grammar. The pairwise aggregate consumes this state form, so a sealed
generation does not silently forget local OTK use. Expiry enforcement and
rotation remain trusted-clock/provider policy gates.

The Rust-internal `SnapshotVault` contract now couples fresh-DEK sealing to a
single provider commit candidate and validates identity-bound context,
conversation-ID derivation, fixed 72-hour freshness, and a generation floor on
restore. The reference provider test proves generation CAS and round-trip open;
it is not a durable native backend and must not be described as restart
continuity in product documentation.

The follow-up reference-provider tests (`9e3a408`) exercise a durable
high-water floor, stale/equal/forked generation rejection, unavailable-provider
behavior, and record preservation on invalid commit. `772a59c` adds the
high-level save/restore seam that obtains the generation floor and trusted
clock from the provider instead of accepting caller wall time. The provider
implementation remains test-only; native key wrapping and crash-safe durable
storage are still required.

The group restore path explicitly marks its regenerated local sender key as
requiring re-gossip. The first post-restore group send schedules distributions
before emitting ciphertext, and a failed distribution leaves the marker set.

Verification on this Windows host:

```text
cargo test --workspace --all-targets -- --test-threads=1   # pass; 4 simulator/deployment cases intentionally ignored
cargo clippy --workspace --all-targets -- -D warnings      # pass
cargo run -p layer-check                                  # pass
cargo run -p mmp-cli -- demo                              # pass; receipt observed
```

The Linux target check was attempted again with
`cargo check -p mmp-tal-peripheral --target x86_64-unknown-linux-gnu` and
stopped in `libdbus-sys`: this Windows host has no Linux pkg-config/sysroot.
This is an environment gate, not Linux/BlueZ evidence. Execute the two-PC
procedure in `docs/MANUAL_INTEROP_TESTING.md` on Linux before claiming physical
interoperability. Android/iOS remain artifact/API gates until consuming apps
and physical-device runs exist.

Commit `9357a49` adds the Rust-internal `SnapshotVault` seam. Its candidate
record binds pairwise/prekey coverage, identity, node ID, and sorted
conversation/peer context; `save_core` couples fresh-DEK sealing to a provider
commit and enforces the next-generation rule before the provider call. Restore
validates the fixed 72-hour TTL, context hash, local conversation IDs, and a
generation floor. This is an adapter contract plus reference tests, not a
durable keystore implementation: platform wrapping, trusted clock/high-water
storage, crash recovery, OTK lifecycle metadata, and native Android/iOS gates
remain open.

Commit `1dc5486` adds a CI boundary check over generated Kotlin/Swift
bindings. It rejects Rust-internal vault, secret-state, ratchet-key, skipped-
key, and raw-DEK identifiers, with deterministic Python unit tests. This does
not prove native provider behavior or React Native runtime behavior; it proves
that the generated UniFFI surface cannot accidentally expose the internal
snapshot types.

Commit `f6c4c68` wires the validated vault load into a Rust-internal Node
restore method. It rejects sessions, queued output, existing pairwise/group
secret state, or pending markers before provider access, then performs one
aggregate swap; the regression covers successful restart restoration and
busy-state rejection. This still stops before native keystore I/O.

## Highest-priority remaining work

1. Run the manual BLE procedure on two Linux PCs. The current Windows host has
   a working Bluetooth adapter but no usable BlueZ peripheral path, so this
   hardware gate cannot be executed here. Mobile work currently stops
   at building and exercising the FFI artifacts until a real Android/iOS host
   application is supplied.
2. Keep the candidate group-control implementation quarantined until a
   normative control schema/domain decision resolves the findings in
   `docs/GROUP_CONTROL_SECURITY_AUDIT.md`.
3. Add independent cryptographic/payload vectors for the remaining opaque HS,
   class-1, and class-2 interiors only after their normative grammar and key
   context are available.
4. Reconcile stale status text in the dirty handbook/API/mobile integration
   files (strict 4096-byte cap and secret-ratchet restart claims) with the shipped
   implementation. Keep that cleanup selective; do not stage the unrelated
   revocation and integration edits already present in those files.
5. Complete the Linux/BlueZ compile and hardware gate for the new peripheral
   stream-token and ingress implementation. The Windows host cannot build
   this backend because the cross-target `libdbus-sys` pkg-config/sysroot is
   absent. Exercise data-lane overflow and control-ledger saturation on Linux,
   then measure whether the 256-entry-per-direction retired-endpoint cap is
   appropriate under reconnect stress; replacements fail closed at that cap.
6. Execute the real two-Linux-PC BlueZ procedure in
   `docs/MANUAL_INTEROP_TESTING.md`. The current Windows host cannot close this
   hardware gate. Mobile remains API/artifact-only until native Android/iOS
   consuming radio shells exist.

The transport-close continuation is now implemented in the Rust host seams:
`Node::poll_close()` reports denied-link, session-closed, and outbox-overflow
actions; `LinkDown` purges stale per-link outbox frames; the UniFFI object
exposes the same poll; the central CLI calls `BleCentral::hangup`; and the
Linux peripheral exposes `PeripheralRole::close_link`. The bounded action queue
has a saturation counter (`close_actions_dropped`) and is documented in
`docs/TRANSPORT_CLOSE_FEEDBACK.md`. Native Android/iOS callers still need to
wire that method to their platform disconnect APIs and feed the resulting
`link_down` callback.

The canonical AEAD associated-data continuation is implemented without a
wire-spec edit: `Bundle::canonical_aead_ad()` binds the header prefix through
`payload_len` with lifetime zeroed, and class-1/2 callers use it for seal/open.
The exact boundary and residual vector work are recorded in
`docs/AEAD_AD_CONFORMANCE.md`. The encrypted `ID_PROOF` contradiction and
copy-budget wording are now recorded in `docs/ID_PROOF_ERRATUM.md` and
`docs/COPY_BUDGET_SECURITY_INVARIANT.md`; they are no longer open reasons to
reinterpret the Rust state machine.

## Transport-hardening continuation notes

The public API intentionally remains `events() -> tokio::sync::mpsc::Receiver<TalEvent>`.
Do not replace it with the internal sender type: the dispatcher is the
compatibility boundary. Lifecycle code must use only the synchronous
`EventSender` methods, and must not await while holding `event_gate`, the
per-link write gate, or the link table lock.

The central ingress stores `(LinkId, generation)` on every link-data record.
The peripheral ingress stores `(LinkId, generation, side)` where side 0 is the
BlueZ reader and side 1 is the writer. On replacement, stream state is updated,
stale data is purged, and `LinkDown` precedes the new `LinkUp`. On data-lane
overflow the affected link is retired and its reserved `LinkDown` is attempted;
on control-ledger saturation the implementation fails closed and never leaves
the link live without an admitted lifecycle transition. Beacon events are
best-effort low-priority data.

Known limitations that must not be hidden by a future “optimization”:

- The public dispatcher can wait behind a stalled consumer, but it owns no
  lifecycle lock; the bounded ingress remains live and memory-bounded.
- The normal control ledger has 64 records and the emergency `LinkDown`
  tombstone lane has at most 64 entries (128 internal lifecycle records in the
  worst case). If both are full, the live link is still retired and stale data
  is purged, but the close event may be lost; this remains an overload/fail-
  closed condition to measure during Linux stress testing and may need an
  operational fatal signal.
- BlueZ retired endpoint vectors are retained until logical link close to avoid
  re-admitting delayed callbacks. They are bounded at 256 identities per
  direction; further replacements fail closed rather than evicting history.
  Linux reconnect testing should measure whether that operational limit needs
  tuning, without weakening the no-eviction invariant.
- The Windows host cannot compile the Linux backend: `libdbus-sys` requires a
  Linux pkg-config/sysroot. A successful Windows build is not Linux evidence.
- The sealed-box fallback intentionally refuses to replace an existing
  conversation root. This prevents forged or stale async bundles from
  invalidating ratchet/receipt state, but a sender that lost local ratchet
  state needs the future live-session recovery path; availability under
  restart is not yet a production claim.
- `cargo fmt` is unavailable because rustfmt is absent from the pinned toolchain;
  use a Rustfmt-equipped Linux/macOS environment before release.

Recommended verification sequence for the next model:

```text
cargo test -p mmp-tal-btleplug --all-targets -- --test-threads=1
cargo test -p mmp-tal-peripheral --all-targets -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets -- --test-threads=1
cargo run -p layer-check
cargo check -p mmp-tal-peripheral --target x86_64-unknown-linux-gnu
```

Treat the final command's current `libdbus-sys`/pkg-config failure as an
environment gate unless it reaches Rust backend compilation. Then run the
two-Linux-PC procedure and attach its logs, adapter models, negotiated MTU,
RSSI, reconnect behavior, and receipt evidence to the phase review.

## Worktree hygiene

At handoff, these unrelated files are intentionally dirty or untracked and
must not be swept into a conformance commit:

- `docs/API_REFERENCE.md`, `docs/IMPLEMENTATION_HANDBOOK.md`;
- `implementation/mesh-milk/MOBILE_INTEGRATION.md` and
  `REACT_NATIVE_INTEGRATION.md`;
- `implementation/mesh-milk/crates/mmp-types/src/domain.rs`, `src/lib.rs`,
  `src/tests.rs`, and `src/group_control.rs`;
- `.tmp-*-audit-20260801/` directories;
- `docs/E5-S4_CONFORMANCE_DESIGN.md` and
  `docs/E5-S4_IMPLEMENTATION_PLAN.md` (planning/audit material).

Stage files explicitly by path. Do not use a blanket `git add .`.

## 2026-08-08 vault restore regression update

The Rust-internal vault seam now has a focused lifecycle regression in
`mmp-node::vault`: after a first authenticated contact, the established
conversation is sealed, restored into a fresh node, and paired with the
ordinary non-secret identity-claim bootstrap. The restored conversation stays
at epoch `(0, 0)` until the next live contact; that contact advances both
directions to `(1, 1)`. This proves the persisted `established_convs` marker is
consumed by the existing teardown ratchet policy and does not cause a premature
bump during restore.

The test is state-machine evidence only. The reference provider still stores a
raw DEK in a test fixture, and no Android Keystore, iOS Keychain, Linux secure
storage, crash-atomic native record, or mobile background run has been
implemented. Those remain explicit exit gates rather than claims hidden behind
the Rust adapter contract.

The next delegated security pass closed two additional gaps:

- `ab8d349` rejects authenticated secret-state snapshots whose
  `pending_first_contact` marker has no matching, same-conversation, nonterminal
  sent bundle. This prevents a restored `ConversationPending` deadlock with no
  possible receipt-clearing path.
- `1f9b9d4` applies the API-surface and secret-surface checks to the Android AAR
  and iOS XCFramework packaging scripts, not only to the standalone binding
  job. Nine Python checks and both shell syntax checks pass.

The current focused evidence is 56 `mmp-node` tests, 35 `mmp-crypto` tests,
and 9 mobile packaging/binding checks. Native provider, crash-journal, and
hardware gates remain unchanged.

Verification after this regression:

```text
cargo test --workspace --all-targets -- --test-threads=1   # pass
cargo clippy --workspace --all-targets -- -D warnings      # pass
cargo check -p mmp-crypto --no-default-features          # pass
cargo clippy -p mmp-crypto --no-default-features --lib -- -D warnings  # pass
cargo run -p layer-check                                  # pass
cargo run -p mmp-cli -- demo                              # pass; receipt observed
```

## 2026-08-08 final verification pass

After `ab8d349` and `1f9b9d4`, the complete workspace test suite was rerun
serially and passed. The follow-up checks also passed:

```text
cargo clippy --workspace --all-targets -- -D warnings  # pass
cargo check -p mmp-crypto --no-default-features      # pass
cargo run -p layer-check                              # pass
cargo run -p mmp-cli -- demo                          # pass; delivery receipt observed
```

At the time of this verification block, the Linux BlueZ target was still
environment-gated on the Windows host because `libdbus-sys` could not find a
Linux pkg-config/sysroot. The later WSL continuation below supersedes that
build limitation. Android/iOS native keystore, crash-atomic persistence,
background-radio, and physical-device tests remain open exit gates.

## 2026-08-08 Linux transport and recovery continuation

WSL Ubuntu supplied a real Linux build environment. The transport follow-up
closed stale lifecycle races without changing the wire protocol:

- `4ba26ef` fences stale LinkUp/LinkDown generations in both BLE ingress
  queues;
- `2f9a245` prevents a delayed peripheral retirement tombstone from purging a
  reused stable LinkId;
- `56821a1` restores per-LinkId high-water tracking with a bounded 4096-entry
  ledger and fail-closed admission rather than unsafe eviction;
- `d68638a` groups the Linux link-up request arguments and clears the
  Linux-only Clippy `too_many_arguments` gate.

The bounded ledger is an ingress resource policy, not a wire or routing
parameter. It rejects new identities after exhaustion; it never evicts stale
generation history. Existing LinkIds remain serviceable. The peripheral
pre-announcement path also refuses unknown streams after ledger exhaustion.

Linux evidence, run with a temporary target directory under WSL to avoid NTFS
incremental-build locking, is now:

```text
cargo check -p mmp-tal-peripheral --target x86_64-unknown-linux-gnu  # pass
cargo test --workspace --all-targets -- --test-threads=1              # pass
cargo clippy -p mmp-tal-btleplug -p mmp-tal-peripheral --all-targets -- -D warnings  # pass
btleplug library tests: 20 passed
peripheral library tests: 34 passed
```

The complete WSL workspace run also passed the 58 `mmp-node` tests, vector,
crypto, simulator, FFI, store, and adversarial suites. Windows rerun of the
complete workspace also passed after the same commits. These are build and
simulation results only; no physical Bluetooth adapter or two-PC radio result
has been obtained yet.

The journaled vault model in `fbd4b85` remains test-only. It covers write,
fsync, replace, floor-update, reopen recovery, corrupted-current fallback,
and rollback-safe floor behavior. It is evidence for the native adapter
contract, not an Android Keystore, iOS Keychain, or production filesystem
implementation.

## 2026-08-08 allocator overflow hardening

The final transport audit found one additional fail-open boundary: both
generation allocators previously wrapped `u64::MAX` back to `1`. `b826774`
now fails closed instead. The central backend returns
`BleError::GenerationExhausted`; the peripheral lifecycle returns
`AttachReject::GenerationExhausted` without replacing an existing endpoint.
Regression tests prove the counter is not mutated on overflow and a live
replacement remains current.

Post-fix evidence:

```text
Windows btleplug generation/lifecycle tests: 5 passed
Windows peripheral lifecycle tests: 13 passed
Windows workspace Clippy (-D warnings): pass
WSL generation/lifecycle tests and Linux Clippy: pass
```

This is an implementation safety invariant only; generation values and wire
format remain unchanged.

## 2026-08-08 vault TOCTOU fencing

`83551b6` closes a restore-time race in `mmp-node::vault`. Restoration now
loads the opaque record before reading the durable generation floor, then
rejects the record if the observed floor is newer. The deterministic
`restore_fences_record_when_floor_advances_after_load` fixture models another
writer advancing the floor between those provider calls and proves that a
stale secret snapshot is not accepted.

Current Windows evidence:

```text
cargo test -p mmp-node --lib -- --test-threads=1       # 59 passed
cargo clippy -p mmp-node --lib --tests -- -D warnings  # pass (agent evidence)
```

This is still not an atomic native-provider transaction. Android Keystore,
iOS Keychain, DPAPI/secure-storage integration, crash-atomic filesystem
replacement, and physical restart tests remain open. A production provider
must implement the equivalent load-plus-floor check atomically or retry on a
changed floor; the Rust ordering fix alone cannot manufacture that guarantee
across independent platform calls.

## 2026-08-08 central per-chunk lifecycle fencing

`e8b4e11` adds a generation check before every ATT chunk in
`mmp-tal-btleplug::BleCentral::send`. If a reconnect or hangup retires the
link while a multi-chunk write is in progress, subsequent chunks are stopped
instead of being sent through the replacement generation. The regression
`chunk_send_guard_stops_after_replacement_generation` makes this race
deterministic. No wire field, bundle format, or specification document was
changed.

Current Windows evidence:

```text
cargo test -p mmp-tal-btleplug --lib -- --test-threads=1  # 22 passed
cargo clippy --workspace --all-targets -- -D warnings      # pass
```

The guard is intentionally fail-closed at the transport boundary. It does
not claim that a physical adapter can never lose a packet, nor does it replace
the Linux/BlueZ and Android/iOS radio tests.

## 2026-08-08 mobile surface audit

The mobile surface was checked without changing the user's dirty integration
guides. Windows artifact evidence is:

```text
cargo build -p mmp-ffi --release                         # pass
UniFFI Kotlin + Swift generation with --features cli      # pass
check_mobile_binding.py (Kotlin and Swift)                # pass
check_mobile_api_surface.py                               # pass
check_mobile_secret_surface.py                            # pass
python -m unittest discover -s scripts/tests -p 'test_*.py'  # 9 passed
```

These checks prove Rust/UniFFI packaging and secret-surface constraints only;
they are not native Android/iOS compiler, Keystore/Keychain, background-radio,
or physical-device evidence. On this Windows host the CLI exits gracefully
when no usable adapter/backend is present (`connect` and `listen` both return
status 2); this is not a successful radio test.

The existing dirty mobile documents contain known examples that should be
corrected in a separately approved documentation pass: missing `--features
cli` in the binding command, an obsolete iOS `.dylib` path instead of the
script-produced static library, stale constructor and `linkData` signatures,
and event/API tables that omit newer friendship, announcement, and file
events. They also link to the absent `other/HANDOFF_MOBILE.md`. Treat those
guides as user-owned edits for now; do not stage them in a conformance commit.

## 2026-08-08 current verification snapshot

After `83551b6` and `e8b4e11`, the current Windows checks pass:

```text
cargo test -p mmp-node --lib -- --test-threads=1               # 59 passed
cargo test -p mmp-tal-btleplug --lib -- --test-threads=1      # 22 passed
cargo test -p mmp-tal-peripheral --lib -- --test-threads=1    # 36 passed
cargo test -p mmp-ffi --lib -- --test-threads=1                 # 16 passed
cargo clippy --workspace --all-targets -- -D warnings         # pass
cargo check -p mmp-crypto --no-default-features               # pass
cargo run -p layer-check                                      # pass
cargo run -p mmp-cli -- demo                                  # pass; delivery receipt observed
python -m unittest discover -s scripts/tests -p 'test_*.py'  # 9 passed
```

No specification file is modified by these fixes. The remaining release
gates are native secure-storage and crash-atomic providers, Linux BlueZ
adapter execution with two physical PCs, and real Android/iOS device and
background-radio measurements.

## 2026-08-08 non-secret snapshot crash recovery

`60d88fa` closes the remove-before-rename recovery window in
`mmp-store::SnapshotStore`. `persist` already fsyncs the temporary image; if
the primary path is absent after a crash, `open` now considers the `.tmp`
candidate only after validating complete framing, every record checksum, and
known-record semantics. An existing primary always wins, even if its normal
loader skips an individual corrupt record. Group payloads remain intentionally
opaque at this layer and are strictly decoded by `mmp-node` before adoption.
Unknown framed record kinds remain forward-compatible.

Evidence:

```text
cargo test -p mmp-store --lib -- --test-threads=1          # 11 passed
cargo clippy -p mmp-store --lib --tests -- -D warnings     # pass
```

This is non-secret snapshot recovery only. It does not add a monotonic
generation floor to `SnapshotStore` or claim rollback protection for a stale
temporary image after deliberate primary loss; the native secret vault's
generation/floor contract remains separate.

## 2026-08-08 RBSR pending-range correctness hardening

`4f3803a` fixes a latent session-availability bug in `mmp-core::rbsr`.
Previously, the reconciler tracked only an integer `open` count. When a large
range was partitioned, the original parent could remain counted after all
terminal lists had completed, so `SessionEngine::maybe_advance_domain` could
never move to the next domain. A forged reply could also decrement an
unrelated outstanding request.

The reconciler now tracks canonical `(lo, hi, count, fingerprint)` descriptors,
resolves exact echoes and bound-correlated ID lists, retires a containing
parent when a valid partition arrives, and derives `open` from the pending set.
The wire format and domain values are unchanged. Tests include the previously
failing 800-versus-800 divergent-set convergence and an unmatched-echo
regression.

Evidence:

```text
cargo test -p mmp-core --lib -- --test-threads=1           # 50 passed
cargo clippy -p mmp-core --lib --tests -- -D warnings      # pass
```

## 2026-08-08 peripheral quarantine hardening

`e19548d` closes a stale-callback fail-open at the Linux peripheral lifecycle
cap. When the retired-endpoint ledger is exhausted, the old path deleted the
`LinkId` record; a delayed BlueZ callback could then re-admit an old endpoint as
fresh. The new path keeps a quarantined record, clears active stream tokens,
emits the retirement event, and rejects later callbacks for that `LinkId` until
process shutdown. This is deliberately fail-closed and bounded; it changes no
wire value.

Evidence:

```text
cargo test -p mmp-tal-peripheral --lib -- --test-threads=1  # 37 passed
cargo clippy -p mmp-tal-peripheral --lib --tests -- -D warnings  # pass
```

The Linux target check still stops before Rust backend compilation on this
Windows host because `libdbus-sys` cannot find a Linux pkg-config/sysroot.
The latest WSL Ubuntu run now supplies that sysroot and passes:

```text
CARGO_TARGET_DIR=/tmp/mmp-target-final cargo check -p mmp-tal-peripheral --target x86_64-unknown-linux-gnu  # pass
CARGO_TARGET_DIR=/tmp/mmp-target-final cargo test -p mmp-tal-peripheral --lib -- --test-threads=1           # 37 passed
CARGO_TARGET_DIR=/tmp/mmp-target-final cargo clippy -p mmp-tal-peripheral --all-targets -- -D warnings      # pass
CARGO_TARGET_DIR=/tmp/mmp-target-final-full cargo test --workspace --all-targets -- --test-threads=1       # pass; Linux workspace
```

This is still build/test evidence only; a physical two-PC BlueZ run remains
required.

## 2026-08-08 remaining audit findings

The mobile surface audit found no safe code change that can substitute for a
native Android/iOS consumer. Existing dirty mobile/API guides contain stale
examples (UniFFI `--features cli`, target-specific iOS static-library paths,
constructor and `linkData` signatures, omitted friendship/announcement/file
events, persistence wording, an unsafe JS seed/clock sketch, and missing JNA
runtime documentation). They were intentionally not staged because they are
user-owned edits. A separate documentation review should correct them and
label the TypeScript snippets as abbreviated where appropriate.

The audit also confirmed that `mmp-cli::now_ms()` currently uses Unix wall-clock
milliseconds while persisted bundle deadlines and `Node::tick` otherwise use
monotonic values. Replacing it with process-relative `Instant` without a
restart/timebase design would make persisted TTL behavior nondeterministic.
This is a time-model design gate, not a safe one-line patch; do not claim the
CLI as a strict monotonic-clock implementation until that design has a
restart regression test and explicit persisted-clock policy.

## 2026-08-08 reconciliation-domain admission hardening

`8c0c989` makes the session FSM reject unknown or misflagged RBSR domain
values. Before this guard, an authenticated peer could inject an arbitrary
domain byte, replace the active reconciler, and consume reconciliation work
without a negotiated HELLO capability. The implementation now accepts only
D0--D3, allows the reply bit only on `RECON_RANGE`, and closes with the normal
unsupported-protocol path for unknown domains or reply-flagged `RECON_INIT` /
`RECON_IDS` frames. This is a local parser/state-machine policy; no wire
format or specification document changed.

Evidence:

```text
cargo test -p mmp-core --lib -- --test-threads=1       # 51 passed
cargo clippy -p mmp-core --lib --tests -- -D warnings  # pass
```

## 2026-08-08 reconciliation sequencing and CLI clock follow-up

The follow-up audit found that domain-value validation alone did not fence a
valid but out-of-order `RECON_INIT`. The session engine now enforces the
negotiated role and D0--D3 order: only the initiator may open a domain, a
responder cannot replace an unresolved domain with a later one, and a
reply-flagged or cross-domain frame is rejected. `RECON_RANGE` and
`RECON_IDS` additionally require an active matching domain; malformed ranges
(`lo > hi`) and ID lists containing values outside their declared bounds are
rejected before reconciliation state advances.

The final-domain DAG-repair WANT poll is latched once per session. This closes
an authenticated replay that repeatedly supplied tiny/empty reconciliation
frames to re-trigger the expensive final poll. The latch is local session
state and does not change the wire format.

The initial audit recorded a residual in which a malicious peer could send a
valid nested range and retire a pending parent before the full partition was
proven. That wording describes the pre-`ee1078d` implementation; the bounded
partition/terminal tracker below addresses this specific premature-advance
path. It remains an availability/withholding limitation rather than a claim
of complete Byzantine delivery correctness until the high-volume session
stress evidence is added.

The CLI now primes a freshly restored node with its current command-clock
reading before creating a bundle or entering the first drive tick. This fixes
the concrete pre-first-tick expiry regression, but `mmp-cli::now_ms()` remains
Unix wall time. Production FFI/mobile hosts must still supply a monotonic
clock with an explicit boot/restart policy; the bare persisted `u64`
deadlines cannot safely be reinterpreted across reboot by swapping in
`Instant`.

Evidence:

```text
cargo test --workspace --all-targets -- --test-threads=1  # pass (Windows)
cargo test -p mmp-core --lib -- --test-threads=1         # 53 passed
cargo clippy -p mmp-core --lib --tests -- -D warnings    # pass
cargo test -p mmp-cli --all-targets -- --test-threads=1  # 27 passed
```

Linux/WSL follow-up on the same working tree also passed 53 core tests,
core/crypto clippy, and 27 CLI tests plus CLI clippy. No physical Bluetooth
adapter was available in this validation environment.

## 2026-08-08 asymmetric RBSR and FFI clock contract

The RBSR audit found a concrete release-build failure path: a peer could
advertise a small/empty range while the local node held hundreds of IDs. The
old terminal-list condition used the peer's claimed count and could emit a
`RECON_IDS` payload larger than `MAX_FRAME_PAYLOAD`. `ff9d6aa` now selects a
terminal list only when the local ID set itself is within `RBSR_T_LIST`; larger
local sets are split into bounded ranges. Direct reconciliation now covers a
500-versus-empty case and verifies the emitted payload remains wire-safe.

The FFI surface now states and tests the required host call order: construct,
call `tick()` with a same-boot monotonic value, then invoke bundle-producing
APIs. The regression checks persisted `stored_at`, the full message TTL, and
retention on the first subsequent tick. This intentionally does not invent a
cross-reboot clock policy or read wall time in the FFI constructor.

The remaining RBSR evidence gap is now narrower: the interval-coverage and
terminal-child tracker is implemented, but a high-volume session-level
500-versus-empty pump is still required before claiming complete Byzantine
availability. The wire-preserving implementation and its focused tests are
recorded in the next section.

Evidence:

```text
cargo test -p mmp-core --lib -- --test-threads=1  # 54 passed
cargo clippy -p mmp-core --all-targets -- -D warnings  # pass
cargo test -p mmp-ffi --lib -- --test-threads=1   # 17 passed
cargo clippy -p mmp-ffi --all-targets -- -D warnings  # pass
```

The subsequent Windows full-workspace matrix also passed, including all
ignored-test exclusions, followed by workspace clippy with `-D warnings`.

## 2026-08-08 bounded RBSR partition tracker

`ee1078d` implements the previously scheduled wire-preserving RBSR state
redesign. Each pending range now keeps a bounded inclusive interval ledger and
an `await_terminal` flag. A nested response records coverage but cannot retire
the parent until the complete span is covered. When a local terminal
`RECON_IDS` response is emitted for an outstanding obligation, a child pending
state remains until the peer answers with the corresponding list or split
ranges. Interval-ledger overflow leaves the obligation open and fails closed.

The original premature-domain-advance exploit is covered by both direct RBSR
and session tests. The frozen wire fields and frame encodings are unchanged.
The high-volume SessionEngine pump with a full 500-versus-empty store now also
passes across many frames, keeping both sessions established and reaching the
final domain. This is still not a proof against every colluding/withholding
strategy, but the previously identified premature-parent and asymmetric
terminal-response paths are covered.

Evidence:

```text
cargo test -p mmp-core --lib -- --test-threads=1  # 56 passed
cargo clippy -p mmp-core --all-targets -- -D warnings  # pass
cargo test --workspace --all-targets -- --test-threads=1  # pass (Windows)
wsl: cargo test -p mmp-core --lib -- --test-threads=1  # 56 passed
wsl: cargo test -p mmp-ffi --lib -- --test-threads=1   # 17 passed
```

The Linux clippy pass initially exposed a toolchain-version lint in the
existing ratchet destructor (`for (_, value) in map.iter_mut()`). It was
rewritten as `map.values_mut()`, preserving zeroization behavior and changing
no protocol or persistence bytes. Windows crypto tests/clippy and the WSL
crypto/core clippy checks now pass. Rustfmt remains unavailable in the pinned
Windows toolchain (`rustfmt` component not installed); `git diff --check` is
clean.

## 2026-08-08 CLI clock seam and terminal-close flush

The standalone CLI's wall-clock dependency is now isolated behind a private
`ClockSource` seam. `WallClock` preserves the existing Unix-millisecond
snapshot behavior, while deterministic tests can inject raw samples (including
rollback) without silently introducing clamping, reboot inference, or an
`Instant`-based persistence policy. The FFI/mobile same-boot monotonic contract
remains unchanged and is still the required native-host boundary.

The transport lifecycle audit also found that `Node::pump_link` removed the
`ERROR`/`CLOSE` frames emitted immediately before `SessionEvent::Closed`, and
that `poll_close()` could ask the host to tear down the physical link before
those frames were drained. The implementation now discards stale data for the
closed link, retains terminal control frames, and defers the host close action
until the `CLOSE` envelope has been polled. Regressions cover malformed input
(`ERROR` then `CLOSE`), timer-triggered `CLOSE`, and unrelated-link outbox
isolation. No wire fields or specification files changed.

Fresh evidence after these changes:

```text
cargo test -p mmp-cli --bin mmp-cli -- --test-threads=1  # 28 passed
cargo test -p mmp-node --lib -- --test-threads=1          # 61 passed
cargo clippy -p mmp-node -p mmp-cli --all-targets -- -D warnings  # pass
cargo test --workspace --all-targets -- --test-threads=1        # pass (Windows)
cargo clippy --workspace --all-targets --all-features -- -D warnings  # pass
```

The physical Linux/BlueZ exchange and native Android/iOS radio/keystore
validation remain external gates: this Windows host has no usable WSL/Linux
BlueZ environment, Bluetooth adapter path, Android consuming shell, or Xcode
toolchain. The scoped implementation changes are committed separately from
the user's pre-existing dirty files.

## 2026-08-08 vault, group-policy, and Android artifact gates

`60153df` ties ratchet-secret restore freshness to the normative message
lifetime (`LIFETIME_MESSAGE_MS`, 72 hours) and rejects an expired provider
record atomically, leaving the target secret state unchanged. This is still a
provider contract: native Android Keystore, iOS Keychain, crash-atomic
replacement, and physical restart testing remain open.

`594129e` makes the immutable-v1 group administrator check symmetric. A
non-administrator can no longer call `group_invite` to mutate its local
membership OR-set and create a same-GID fork; the operation fails closed with
the existing `UnknownPeer` error. No wire field or specification text changed.
The separate uncommitted `mmp-types::group_control` candidate remains
quarantined: its unauthenticated/replayable control path is not suitable for
v1 and must not be staged without a new reviewed protocol version.

`6827e69` adds a native-only Android artifact verifier to both the AAR build
script and CI. It requires the manifest and arm64-v8a, armeabi-v7a, and
x86_64 `libmmp_ffi.so` members, rejects `classes.jar`, absolute/traversal or
duplicate ZIP paths, and checks that the Kotlin artifact contains only
non-empty source material passing the existing API/secret-surface scanners.
`efcd211` canonicalizes harmless `.` and repeated-slash ZIP spellings before
duplicate detection. The verifier has seven focused synthetic tests; the
complete script suite has 16 passing tests. A real NDK/AAR build remains a CI
or Android-host gate because this Windows machine has no usable Android NDK
shell or consuming application.

Current focused evidence:

```text
cargo test -p mmp-node --lib -- --test-threads=1                 # 63 passed
cargo clippy -p mmp-node --all-targets -- -D warnings             # pass
python -m unittest discover -s implementation/mesh-milk/scripts/tests -p 'test_*.py'  # 16 passed
git diff --name-only -- specification                                    # empty
```

All user-owned dirty documentation, integration guides, `mmp-types` edits,
temporary audit directories, and Python cache directories remain unstaged.
The remaining release gates are the physical two-PC Linux/BlueZ exchange,
native Android/iOS radio/background measurements, and platform secure-storage
restart/crash tests.

## 2026-08-09 authority clock readiness and iOS artifact gate

`3357278` closes a receiver-local authority admission fail-open. `NodeCore`
now represents the host wall clock as an explicit readiness value rather than
using Unix zero as an implicit timestamp. Before `set_wall_clock` is called,
attached or gossiped AuthCerts cannot elevate priority and cannot render an
announcement official; the announcement may still be relayed and surfaced at
NORMAL priority. Once initialized, backward samples are ignored, so an
expired certificate cannot be revalidated by an NTP/user-clock rollback.
Explicit `Some(0)` remains valid for deterministic tests. This is local policy
only: persisted reboot-time trust and a native trusted-clock provider remain
platform responsibilities.

The regression suite covers an unclocked future-dated certificate and an
expiry-then-rollback sequence. The fix changes no wire bytes and does not
modify the frozen specification.

`a175124` adds the iOS counterpart to the Android packaging gate. The
XCFramework verifier checks `Info.plist`, required `ios-arm64` and
`ios-arm64-simulator` slices, safe relative library/header paths, non-empty
static libraries, header/module-map presence, and non-empty UTF-8 Swift source.
The build script, CI job, and manual interop procedure invoke the verifier.
Five synthetic iOS cases and the complete mobile-script suite (21 tests) pass;
macOS/Xcode execution, Swift compilation, and device radio behavior remain
external gates. Follow-up `341d767` rejects duplicate `LibraryIdentifier`
entries instead of silently allowing plist ambiguity; the iOS verifier now
has six focused cases.

Fresh evidence:

```text
cargo test --workspace --all-targets -- --test-threads=1       # pass
cargo clippy --workspace --all-targets --all-features -- -D warnings  # pass
python -m unittest discover -s implementation/mesh-milk/scripts/tests -p 'test_*.py'  # 22 passed
git diff --name-only -- specification                         # empty
```

## 2026-08-08 fuzz, Android packaging, and reconciliation hardening

`9c9dbb0` closes a stale-artifact admission gap in the Android packaging
boundary. The AAR builder removes an existing regular output archive before
rebuilding and fails closed on non-file collisions. The verifier now allows
only the native-only manifest, expected ABI directories, and required
`libmmp_ffi.so` members; arbitrary payloads, `classes.jar`, directory-valued
manifests, traversal paths, and duplicate ZIP members are rejected. Eight
focused Android artifact tests pass. This remains artifact validation, not an
Android NDK build or a consuming-app/device result.

`f0a0e6d` adds a deterministic fuzz-corpus recipe. It derives 43 managed seeds
from the committed conformance vectors (positive, negative, malformed
handshake, beacon, frame, and bundle domains), preserves developer-owned
corpus members, and provides `check`, `write`, and `verify` operations. CI now
validates the recipe before building fuzz targets and verifies/runs every
managed seed. The full Python script suite contains 27 passing tests. The
recipe is reproducible and bounded; it is not a substitute for long-running
coverage-guided fuzzing or independent wire vectors.

`304609e` bounds hostile RBSR reconciliation state. Pending range descriptors,
listed ranges, and missing-ID material now have explicit caps; exhaustion is
observable and causes the session to close with a protocol error instead of
silently dropping state or growing without bound. Regression coverage includes
pending, terminal/missing, and session-closure paths. The full `mmp-core`
library suite has 59 passing tests, and workspace clippy is clean. The caps are
receiver-local resource limits; they do not create Byzantine-resistant global
copy-budget conservation and do not alter the frozen wire specification.

Verification on this Windows host after these commits:

```text
cargo test --workspace --all-targets -- --test-threads=1              # pass; 4 simulator/deployment cases intentionally ignored
cargo clippy --workspace --all-targets --all-features -- -D warnings  # pass
python -m unittest discover -s implementation/mesh-milk/scripts/tests -p 'test_*.py'  # 27 passed
python -m py_compile implementation/mesh-milk/scripts/verify_android_artifacts.py implementation/mesh-milk/scripts/verify_ios_artifacts.py implementation/mesh-milk/fuzz/seed_corpus.py  # pass
python implementation/mesh-milk/fuzz/seed_corpus.py check             # 43 seeds validated
cargo run -p layer-check                                             # pass
cargo run -p mmp-cli -- demo                                          # receipt observed; message Delivered
git diff --name-only -- specification                                # empty
```

The next implementation priorities remain physical Linux/BlueZ validation,
native Android/iOS radio and background measurements, and platform secure
keystore/crash-restart tests. The candidate `group_control` module remains
quarantined pending a normative control-schema decision. RBSR priority
partitioning, independent opaque-payload vectors, and any MMP/Φ research
mechanisms remain deliberately deferred until their normative grammar,
security model, and measurements exist. Do not stage the unrelated dirty
handbook/API/mobile integration files or temporary audit/cache directories.

## 2026-08-08 unknown-suite application gates

The security follow-up identified a cross-suite confusion boundary: generic
`RevocationList::decode` must remain permissive so an unknown claim body can be
stored and forwarded opaquely, but the v1 authority store must not interpret a
future suite as a v1 revocation. Commit `592745a` adds a `suite == 0x01` gate
at `AuthorityStore::merge_revocation_list`; an unknown suite is ignored before
any sticky revocation state is mutated. The same fail-closed rule is applied to
class-5 announcement rendering: an unknown suite remains relay/storage-only
and cannot produce an application event. This is wire-compatible and does not
change the frozen specification.

Regression evidence:

```text
cargo test -p mmp-core --lib -- --test-threads=1  # 60 passed
cargo test -p mmp-node --lib -- --test-threads=1  # 66 passed
cargo clippy -p mmp-core -p mmp-node --all-targets --all-features -- -D warnings  # pass
```

This is a future-suite interoperability/availability hardening, not a claim
that v1 issuer signatures can be forged: the outer claim signature and pinned
issuer/scope checks remain required. Future suites still need an explicit
decoder and application policy before they can be acted upon.

## 2026-08-09 continuation: parser, platform, and admission-state hardening

`a9ea2a1` closes an artifact-boundary gap in the iOS verifier. Structural
XCFramework checks previously passed copied Swift bindings without re-running
the API/secret-surface scanner that the build path used. The verifier now
re-runs that scanner after the UTF-8/non-empty checks; missing generated methods
and forbidden secret-state identifiers are rejected. The full Python suite has
29 passing tests, including eight iOS cases. This is still an artifact gate,
not macOS/Xcode compilation or device-radio evidence.

`b39a114` hardens the public encoded-BundleID helper. `id_of_encoded` now
returns `Result<Id16, WireError>` and rejects truncated or oversized input
before slicing or allocating; the bundle fuzz target exercises this path in
addition to canonical decoding. The repository has no callers that require a
migration, but external API consumers must handle the new `Result`. Focused
`mmp-types` all-target tests, Clippy, and fuzz-target compilation passed. The
wire format and BundleID derivation are unchanged for valid inputs.

`886ecd4` closes a Sybil-driven memory-growth path. `OriginBuckets` and
`AuthorityBuckets` now fail closed for new entries after 4096 live keys, while
existing entries continue to operate. `Node::tick` compacts idle maps after
`LIFETIME_MESSAGE_MS` (72 hours); this is conservative because both token
budgets refill fully much sooner. Cap, compaction, and node-tick regressions
pass. The change is receiver-local resource protection and does not alter the
wire contract or the copy-budget security invariant.

Focused evidence for this continuation:

```text
cargo test -p mmp-types --all-targets -- --test-threads=1              # pass
cargo test -p mmp-router -p mmp-node --lib -- --test-threads=1        # pass: 10 + 69
cargo clippy -p mmp-types -p mmp-router -p mmp-node --all-targets --all-features -- -D warnings  # pass
python -m unittest discover -s implementation/mesh-milk/scripts/tests -p 'test_*.py'  # 29 passed
git diff --name-only -- specification                                      # empty
```

After these continuation commits, the complete workspace matrix also passed:
`cargo test --workspace --all-targets -- --test-threads=1` and
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.
The Windows toolchain does not include the `cargo-fmt` component, so a
formatter check remains an environment limitation rather than a claimed pass.

The remaining gates are unchanged: Linux/BlueZ two-PC interoperability,
native Android/iOS runtime and background-radio measurements, and platform
secure-keystore crash/restart tests. Keep `group_control` quarantined and do
not introduce MMP/Φ research mechanisms into v1 until trace-driven, physical,
and adversarial measurements justify them. The Windows host lacks the Linux
BlueZ/pkg-config environment needed to close the physical gate.

## 2026-08-09 continuation: storage, vault, and packaging follow-up

`f83c975` hardens the primary snapshot loader against hostile record lengths.
Length-prefix, record-end, checksum-end, and slice boundaries now use checked
arithmetic, matching the already-safe complete-temp validator. A truncated
`u32::MAX` record fixture is rejected without panic. `mmp-store` has 11 unit
tests plus one integration regression passing. A separate design item remains:
`SnapshotStore::load` still reads the whole local file before record parsing;
introducing a storage-file byte cap requires an explicit policy decision and is
not silently inferred here.

`7d08611` closes a vault restore TOCTOU race. After decrypting and validating a
record, `restore_from_vault` re-reads the provider's durable generation floor
and rejects a record superseded during the expensive open step. The
`FloorAdvancingAfterClockVault` fixture exercises this race. The node vault
suite has 70 passing tests and its focused Clippy gate is clean. A fully
linearizable multi-process restore still requires a provider transaction
contract; this patch closes the concrete post-initial-floor window.

`8528916` makes iOS packaging reruns deterministic. The build script removes
the previous XCFramework and Swift-bindings directory before copying new
sources, and fails closed when the bindings path collides with a non-directory.
Static ordering/collision coverage and Bash syntax checks were added; the full
Python artifact suite has 31 passing tests. This remains packaging validation,
not Xcode compilation or CoreBluetooth evidence.

These changes preserve the frozen wire contract. The remaining external gates
are still Linux/BlueZ two-PC exchange, native Android/iOS runtime and energy
measurements, and platform keystore crash/restart testing.

## 2026-08-09 continuation: CI and X25519 hardening

`e357078` makes mobile artifact publication fail closed. The Kotlin/Swift
binding, Android AAR, and iOS XCFramework upload steps now use
`if-no-files-found: error`; a YAML regression verifies exact artifact names,
repository-relative provenance paths, and the error policy. `03a5b7b` also
serializes the root host workspace test command with
`-- --test-threads=1`, matching the documented Windows-safe procedure and
avoiding collisions in the shared temporary snapshot-store fixtures. The
Python CI/artifact suite reached 33 passing tests.

`9e227f3` adds RFC 7748 all-zero shared-secret rejection to the X25519 prekey
sender/recipient paths and sealed-box open path. Without this check, a
low-order public point can produce a predictable derived key and collapse the
confidentiality of first-contact ratchet state. Regression coverage exercises
low-order signed-prekey, ephemeral/static, and sealed-open inputs. The
`mmp-crypto` library suite has 36 passing tests and its Clippy gate is clean;
the change preserves valid wire behavior and does not modify the frozen
specification.

The storage-file-size issue remains deliberately unresolved: `SnapshotStore`
is tier-agnostic and bundles, claims, groups, indexes, authority state, and
blocklists have different quotas. A future tier-aware `SnapshotLimits` or
streaming parser is required before imposing a safe file cap. Group control
remains quarantined pending its normative schema decision.

## 2026-08-09 continuation: embedded allocation, Android CI, and groups

`228e518` hardens the lower `mmp-core` resource boundary. `Bloom::with_capacity`
now uses saturating arithmetic and clamps requested bits to the existing
production maximum before allocating. A `usize::MAX` request is therefore
bounded rather than overflowing or attempting an attacker-sized allocation.
The persistence-decoder integration suite has five passing cases for these
limits, and `mmp-core --no-default-features` still builds. The wire format is
unchanged.

`2a02cdc` fixes the Android cross-target CI environment. The `android-check`
job now provisions NDK r26d, installs `cargo-ndk`, exports `ANDROID_NDK_HOME`,
and runs `cargo ndk -t arm64-v8a check --workspace`; this addresses the
previous missing `aarch64-linux-android-clang` failure. YAML regression tests
cover the setup, environment, and command.

`ab91e1c` bounds TOFU group admission. A valid epoch-1 sender-key distribution
could previously create one `GroupState` per fresh random group ID, and
restoration could insert unlimited blobs. New network groups and restored
groups are capped at 256 live entries while updates to existing IDs remain
allowed. The node suite has 71 passing tests, including a 257th-group
regression; this is local resource policy and does not modify the frozen wire
contract.

The group-cap fix is intentionally scoped: it bounds fresh group identities,
but an authenticated member can still submit multiple valid future sender-key
epochs for an already-admitted group. A later hardening pass should define a
per-group epoch-retention/window policy, prove that it does not reject valid
reordering or recovery traffic, and add restore/ingest tests before claiming
that all group-state growth is bounded. Do not add an arbitrary epoch limit
without first reconciling it with the epoch-rotation and offline-delivery
rules in the frozen specification.

## 2026-08-09 epoch-retention audit: no safe v1 local eviction rule

The follow-up audit confirms that the residual is not safely fixable with an
implementation-only epoch cap or TTL:

* `SenderKeyDist` contains only `gid`, `epoch`, `chain_seed`, and `sig_pub`;
  it carries no issuance time, expiry, retention watermark, or authenticated
  membership proof. `GroupState::learn_remote` therefore has no evidence from
  which it can prove that one `(member, epoch)` is no longer needed.
* The receiver currently accepts every authenticated member distribution with
  `dist.epoch >= gs.epoch` and retains each unique pair in both `remotes` and
  `dists`. `RATCHET_WINDOW` bounds message-index reordering *inside one epoch*;
  it does not bound the number of epochs.
* A valid offline path can miss an arbitrary number of membership rotations,
  then receive a later epoch distribution followed by a delayed bundle from an
  older epoch. Any fixed `N`-epoch window can reject that path. A receive-time
  TTL is also insufficient: the frozen bundle lifetime is mutable and is
  deliberately normalized out of the signed identity/AEAD form, so a node
  cannot recover the bundle's original creation time from the group payload or
  the distribution (specification/08-crypto-spec.md §6.3–§6.4 and
  specification/03-protocol-spec.md §4.3–§4.5).

The per-origin admission bucket limits the attack rate but not the retained
state lifetime. Thus a malicious authenticated member can eventually occupy
all `u16` epoch slots for one admitted group without violating the v1 wire
grammar. The correct next step is a normative retention signal (for example an
authenticated epoch anchor with validity/watermark metadata) or a local store
index that tracks all unexpired group bundles referencing each key. Neither is
available in the frozen v1 contract. Until one is specified and tested against
partition/reordering scenarios, silently evicting or rejecting future epochs
would be an availability regression; leave this item open rather than add an
arbitrary cap.

## 2026-08-09 final local verification

The post-hardening local gate passed:

* `cargo test --workspace --all-targets -- --test-threads=1`;
* `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
* 34 repository Python regression tests, Python byte-compilation, and 43
  deterministic fuzz-seed checks;
* `cargo run -p layer-check`, `cargo run -p mmp-cli -- demo`, and
  `git diff --check`.

No staged or working-tree path under `specification/` changed. The remaining
release gates are external to this Windows host: two physical Linux/BlueZ
peers, native Android/iOS background-radio and battery measurements, secure
platform-keystore crash/restart tests, and a real CI run of the Android/Xcode
packaging jobs. The tier-aware snapshot-size design and the epoch-retention
policy above remain open; neither should be papered over by changing the wire
specification.

## 2026-08-09 continuation: bounded snapshots and clustered simulation

`8df61c7` adds an additive `SnapshotLimits` host-policy seam. Hosts that know
their storage budget can call `SnapshotStore::open_with_limits`; the loader
checks metadata before reading, reads at most one byte beyond the configured
complete-image limit, rejects oversized primary and recovery candidates, and
sets `load_rejected` so a rejected image cannot later be silently replaced by
an empty snapshot. Persist also removes an oversized temporary image before
rename. The default unbounded `open` behavior and the `MMPSNAP2` wire format
remain unchanged. The limit is for the whole image, not the bundle quota, and
must account for claims, groups, indexes, authority state, and blocklists.
Fourteen mmp-store tests and its Clippy gate pass.

`a749a14` adds `ClusteredMobility` and `Sim::run_clustered_meetings` for the
open clustered/SLAW-like simulation gap. The model uses balanced communities,
seeded intra-community contacts, and a periodic adjacent-community bridge;
the API rejects configurations that cannot produce valid same-community
contacts. Determinism and bridge cadence are unit-tested. This is explicitly
an experimental measurement model, not a claim of calibrated human mobility,
and it changes no routing or wire behavior. A future measurement pass should
compare this model against trace-derived contact distributions before using it
to revise performance claims.

## 2026-08-09 secret-state audit: group sender-key persistence seam remains open

The focused Conv/ratchet/prekey/vault audit found a persistence boundary that
must not be papered over in implementation code. `mmp-node::secret_state::RestoredState`
contains pairwise conversations, pending markers, delivery tracking, and the
prekey store, but it has no `GroupState` or `sent_dists` fields. Consequently,
`secret_state::encode` cannot carry group sender-key material, and
`Node::restore_secret_state` / `restore_secret_state_from_vault` refuse to run
once live `groups` or `sent_dists` exist. The ordinary non-secret snapshot path
restores only group membership/epoch metadata; `GroupState::decode` deliberately
generates a fresh local sender key and starts with empty remote distributions.

This is not a safe invitation to add chain seeds or `SendChain` snapshots to the
existing aggregate. The frozen crypto requirements treat sender-key chains as
secret ratchet state (specification/08-crypto-spec.md §6.2–§6.4), while the
Rust blueprint keeps key material behind the keystore boundary and requires an
atomic complete-generation restore (specification/08-crypto-spec.md §10;
specification/12-rust-blueprint.md §2 and §7). The current group persistence
codec explicitly documents the opposite safety property: membership/epoch
bookkeeping may be persisted, but local/remote chain seeds and signature
secrets must not be silently regenerated as if continuity had been restored
(`implementation/mesh-milk/crates/mmp-node/src/group.rs`, `GroupState::encode`).

The exact next safe seam is a normative and provider-backed group-secret
record: define which local/remote sender-key ratchet state, replay windows,
distribution cache, and generation/rollback context share one atomic vault
generation with pairwise secrets; then add authenticated vectors and crash/
restart tests for old-epoch delivery, post-restore re-gossip, and failed
commit rollback. Until that contract exists, leave groups outside the secret
aggregate and retain the current fail-closed restore behavior. No
implementation-only group-key cap, TTL, or deterministic regeneration should
be introduced: each can either lose valid delayed group traffic or reuse a
spent sender key.

## 2026-08-09 continuation: parser and loopback fail-closed checks

`bcd5288` closes three malformed-input allocation paths without changing any
wire grammar. `RECON_RANGE`, `RevocationList`, and `PrekeyBatch` now verify that
the remaining byte count can hold every fixed-width record before reserving a
vector. The regression corpus includes a zero-record `RECON_RANGE` count of
255; the precheck avoids approximately 12 KiB of speculative range storage in
that case, as well as the analogous revocation/prekey reservations. The
mmp-types suites and Clippy pass, and the deterministic corpus now validates
44 seeds.

`eecbd80` makes the `mmp-cli demo` a real fail-closed loopback check. It now
requires both Bob's `MessageReceived` event and Alice's `DeliveryState::Delivered`
state, prints explicit assertions, and exits non-zero if either signal is
missing. The manual interop document records that this is host-loopback
evidence only; it is not evidence of BLE, mobile background, or physical
radio behavior. The CLI test suite and Clippy pass.

The ignored clustered measurement was run explicitly with seed
`0x00C1_A57E_D00D`: 5,000 contacts over 4.21 simulated hours produced
`delivery_ratio=0.200`, p50 `6,146,595 ms`, and p90 `8,569,084 ms`. This is a
reproducible result for the toy clustered model, not a production performance
claim. Its low delivery ratio reinforces the need for trace-calibrated contact
parameters before using SLAW-like simulation to justify a protocol change.

## 2026-08-09 continuation: FFI storage quota and beacon-window hardening

The bounded snapshot policy is now reachable from the native mobile/desktop
boundary through the additive `MmpNode::new_with_store_limit` constructor.
Hosts pass a non-zero complete-image byte quota; the existing `MmpNode::new`
constructor remains unchanged and unbounded for compatibility. A focused FFI
regression constructs a one-byte-limited node, verifies that `flush()` fails
closed, and checks that `last_persist_error()` exposes the persistence failure
to the host. The quota covers framing and all auxiliary records, not only
bundles, and does not alter `MMPSNAP2` or any protocol field.

`fe0aee9` bounds the router's rolling resolved/ephemeral beacon-population
windows at `CONTACT_GRAPH_CAP` with oldest-first eviction. The regression feeds
5,000 distinct unauthenticated ephemeral IDs at one timestamp and verifies
that the estimate remains bounded. This is receiver-local resource protection;
it is not an authenticated beacon claim or a global spray-budget guarantee.

The Kotlin `MeshDrive` source module remains a platform-neutral integration
seam. Its source-level test must be run from the repository root and only
proves dependency injection, monotonic ordering, MTU-minus-three
fragmentation, back-pressure retention, and link/tier lifecycle rules. It does
not prove Kotlin compilation, Android background execution, GATT behavior, or
radio interoperability; those remain external gates.

## 2026-08-09 release binding verification

After the additive FFI constructor landed, the release library and both
generated mobile surfaces were exercised locally:

```text
cargo build -p mmp-ffi --release --features cli                         PASS
uniffi-bindgen generate ... --language kotlin                            PASS
uniffi-bindgen generate ... --language swift                             PASS
python scripts/check_mobile_binding.py kotlin <generated-dir>             PASS
python scripts/check_mobile_binding.py swift <generated-dir>              PASS
python scripts/check_mobile_api_surface.py <kotlin-dir> <swift-dir>       PASS
```

The generator could not auto-format because `ktlint` and `swift-format` are
not installed on this Windows host; this is a formatting-tool warning, not a
binding-generation failure. The checks prove generated API/secret-surface
coverage only. They do not prove Kotlin or Swift compilation, Android/iOS
background scheduling, keystore/keychain behavior, or physical BLE exchange.

`26938a0` synchronizes the clean mobile API-surface manifest and regression
test with the additive `eventQueueDropped` FFI method. A fresh release-library
generation after `09629bb` passes the Kotlin and Swift binding checks plus the
complete API-surface check; only the previously recorded missing formatter
tools remain.

## 2026-08-09 Windows shell-test boundary

The repository Python suite now reports 42 tests passing with one explicit
skip on this Windows host. `test_ios_build_script_is_valid_bash` invokes
`bash -n`; Windows' system `bash.exe` is the WSL launcher and, when WSL is not
installed, emits `wsl.exe --install` guidance instead of parsing the script.
`e3c5e57` skips only that unmistakable launcher response, while retaining a
strict failure for any actual Bash syntax error and retaining the Linux/macOS
syntax gate. This is an environment prerequisite, not evidence of an iOS
build or a protocol failure.

## 2026-08-09 continuation: bounded application callback FIFO

The `mmp-node` application callback queue (`Node::poll_event`) was previously
an unbounded `VecDeque<Event>`. A peer could therefore keep a valid session
busy while the host failed to poll, retaining an attacker-sized backlog of
ciphertext-free message, group, announcement, and file event payloads. The
queue now has a local capacity of 2,048 entries and is initialized with that
capacity. Every event producer uses one `NodeCore::enqueue_event` seam.

The saturation policy is deliberately explicit and deterministic: FIFO order
of already queued entries is preserved and the *newest* event is dropped when
the queue is full. No coalescing or silent eviction of older lifecycle events
is performed. `Node::event_queue_dropped` and the additive FFI accessor
`MmpNode::event_queue_dropped` expose a sticky loss counter. A non-zero value
is an application backpressure/observability failure: hosts MUST drain
`poll_event` promptly and should surface or recover from the loss. Required
user-visible events (for example `MessageReceived`) can be lost while the
queue is saturated; this is a documented local overload policy, not a claim
of lossless protocol delivery. The wire format, session grammar, and
specification remain unchanged.

The focused regression fills the queue through the public FFI tier callback and
asserts that saturation is observable. The `mmp-node` library suite passes 72
tests and the focused FFI saturation test passes. This bound protects process
memory but does not by itself provide transport-level backpressure or prove
mobile callback scheduling; a future lossless-event design would need an
explicit session admission/close policy and a reviewed API contract.

## 2026-08-09 continuation: bounded SessionEngine immediate output

The sans-IO `SessionEngine::out` FIFO was a second backlog boundary not
covered by the node outbox cap. A single transport callback can contain many
valid encrypted envelopes; receive-credit replenishment is emitted while that
callback is being parsed, so a core-only host that does not poll until return
could otherwise retain one output per response. `MAX_OUT_FRAMES` now bounds
the immediate queue at 256 entries, preallocates no larger queue, and applies
to both encrypted frames and the Noise handshake envelopes through one
`queue_raw` seam. Deferred data remains bounded by the existing 256-frame
`pending_out` cap.

Data output reserves two slots for an `ERROR` + `CLOSE` response. If a data
frame would consume that reserve, the engine emits the error/close pair and
terminates. If a control/handshake path reaches the hard cap, the engine
clears immediate and deferred queues and publishes one `Closed` event
(`output_overflow`); it does not recurse trying to enqueue another terminal
pair. This is a local fail-closed resource policy, not a wire-format change;
the host must react to `Closed` and tear down the physical link. No queue entry
can exceed the 256-frame bound.

The regression constructs one bounded input containing 1,600 independently
encrypted `SUMMARY` envelopes. Before this guard, the receiver's queued CREDIT
responses could grow past the cap during one `on_wire` call. Now the session
closes with `out.len() <= 256`. The full `mmp-core` matrix passes (62 unit + 5
decoder-limit integration tests) and package Clippy passes with `-D warnings`.

## 2026-08-09 continuation: authenticated-session map growth audit

This pass reviewed the remaining attacker-reachable or session-driven maps in
`mmp-node`, specifically identity learning, friendship state, proxy hints,
blob-serving counters, groups, and the reconciliation/blob indexes. The wire
format and frozen specification were not changed.

Two advisory/resource maps received local bounds:

* `proxy_hints` now retains at most 256 dependents per authenticated peer and
  at most 256 authenticated peers. A new dependent is ignored at the
  per-peer limit. When the peer table is full, one arbitrary existing peer is
  evicted before admitting a new one. These hints only suppress an optional
  spray copy (`peer_buffers_for`); they are never an admission or delivery
  requirement, so false positives/evictions can reduce efficiency but cannot
  make a bundle undeliverable.
* `blob_served` now retains counters for at most 256 distinct peers. An unseen
  peer after the cap is served no blob candidates and is not inserted. An
  empty or mismatched request also does not consume a peer slot; only a
  request that returns at least one candidate creates/updates its counter. An
  existing entry is removed on the next authenticated contact with that peer,
  preserving the per-contact reset rule. There is intentionally no arbitrary
  TTL or eviction yet: evicting a counter would let a peer reset its local
  serving allowance without a new authenticated contact. The residual policy
  is therefore fail-closed availability for unseen peers once the table is
  full; a future reviewed aging/eviction design may relax this.

The friendship maps do not currently justify an implementation-only cap:

* `pending_offers` is bounded by `MAX_PATRONS` (currently three) at the public
  `request_friendship` API, and `offered_since` is inserted only for those
  pending offers. `patrons` is also TTL-pruned (`FRIENDSHIP_TTL_MS`) and cannot
  exceed the same three-entry local budget.
* `friends` is created only when an authenticated peer sends a
  `FRIEND_OFFER` and that peer is already in the host-controlled
  `friend_allowlist`; it is TTL-pruned. Its buffered IDs are quota-accounted by
  the bundle store and therefore do not create an independent unbounded byte
  store. A hostile wire peer cannot add itself to the allowlist.
* `friend_allowlist` is a host policy set with no wire-side insertion path. It
  can grow if an application deliberately adds many identities, but that is
  outside the remote-adversary resource model and should be bounded by the
  host API/persistence policy rather than silently evicted by the protocol.

`known_identities` now has a safe implementation-only lifetime rule. The cache
is released when a link is removed, and an entry is retained while another
established link for the same peer remains. This bounds the cache by the
existing per-tier live-session admission limit rather than by an arbitrary
identity count or wall-clock TTL. FRIEND_OFFER/FRIEND_ACK callbacks still run
while their established `SessionEngine` is live, so interleaved or delayed
frames cannot lose the key merely because another peer was admitted. The
regressions `known_identity_cache_releases_after_contact_teardown` and
`known_identity_cache_survives_another_live_link` cover sequential contacts,
last-link cleanup, and the multi-link retention case. A host that never drives
`LinkDown` or `tick` can still retain an active session (and its identity), but
that is already bounded by the session admission cap and is an operational
drive-contract violation, not a process-lifetime map leak.

Group state is partially bounded but has a similar normative gap. The top-level
`groups` table is capped at `MAX_GROUPS = 256`, including network bootstrap.
However, each admitted group's `remotes` and `dists` maps are keyed by
`(member, epoch)` and currently accept every authenticated member's valid
sender-key distribution for a non-decreasing epoch. A Byzantine member can
therefore fill finite-but-large future epochs (up to the wire `u16` range),
while `sent_dists` also grows with local group/epoch sends. The frozen crypto
specification does not yet define an implementation-safe epoch retention or
administrative anchor rule that would permit dropping these records without
changing delayed-message/replay semantics. Leave these maps unchanged and
track the required normative decision before adding a cap.

Concrete next design (research/implementation gate, not a v1 wire change):
define an authenticated epoch-retention anchor that names `(gid, floor_epoch)`
and an issuance/validity context, and require the group authority/membership
rules to state when traffic below that floor is no longer valid. The receiver
may then retain all distributions needed by unexpired local bundles and the
anchored epoch floor, while pruning older unreferenced `(member, epoch)`
entries. `sent_dists` should be rebuilt from the same acknowledged epoch
watermark (or treated as a bounded retransmission cache); clearing it may cause
duplicate distribution bundles but must never reset a sender/receiver chain.
Until that anchor and its partition-merge rule exist, do not infer a floor from
receive time, the current `gs.epoch`, or a fixed number of epochs.

Required conformance tests before enabling such a policy:

1. A delayed but still-unexpired group bundle from an older epoch decrypts when
   its distribution is retained; no local eviction occurs merely because a
   newer epoch arrived.
2. After an authenticated floor is accepted and all local references below it
   have expired, pruning removes the old `remotes`/`dists` entries and a replay
   of the removed distribution cannot reset a receiver chain.
3. Partitioned anchors with conflicting floors merge conservatively (no false
   deletion), and a later valid anchor can advance the floor deterministically.
4. A Byzantine member cannot allocate unbounded future epochs before anchor
   validation; the receiver applies a measured admission budget and exposes a
   fail-closed diagnostic rather than silently changing replay semantics.
5. Restart/restore preserves the anchor, retained references, and sender-key
   generation atomically; failed persistence leaves the prior retention state
   intact.

The remaining indexes are indirectly bounded: `sync_view.entries` is rebuilt
from the bounded bundle store on each `ids()` call; `blob_index` and
`symbol_of` contain only indexed class-4 bundles and are pruned with store
expiry/eviction; verified file offers use the FIFO `FILE_OFFER_CACHE = 256`;
blob assemblies are host-created and swept by `BLOB_INTEREST_TTL_MS`; and
reconciliation ledgers already enforce their protocol limits. These paths did
not reveal a separate attacker-controlled lifetime accumulation in this pass.

Regression evidence for the new bounds is in
`proxy_summary_hints_are_bounded_per_peer` and
`blob_serve_counters_are_bounded_for_unseen_peers`. Identity lifetime evidence
is in `known_identity_cache_releases_after_contact_teardown`,
`known_identity_cache_survives_another_live_link`, and the engine-close
assertion in `session_slot_releases_when_engine_closes`. The focused
`mmp-node` library suite passes 76 tests, and `cargo clippy -p mmp-node
--all-targets --all-features -- -D warnings` passes.

## 2026-08-09 continuation: iOS slice architecture manifest gate

The host-only iOS artifact verifier now requires every expected XCFramework
slice emitted by `build-ios-xcframework.sh` to declare exactly `arm64` in
`SupportedArchitectures`. The build script targets `aarch64-apple-ios` and
`aarch64-apple-ios-sim`; accepting a plist that silently names only another
architecture could let a structurally valid but unusable artifact pass before
Xcode or a device ever sees it. The new regression mutates a synthetic device
slice to `x86_64` and confirms deterministic rejection. The complete Python
script suite passes 43 tests with one documented Windows Bash-launcher skip.
This is a manifest/artifact consistency check only; it does not claim Xcode
compilation, Swift compilation, simulator execution, or device/radio behavior.

## 2026-08-09 continuation: clustered mobility and BLE evidence gates

The simulator now includes `SocialClusteredMobility` as a seeded control for
paired routing experiments. It models explicit communities and per-mille bridge
transitions and reports realized bridge contacts. This is an experiment seam,
not a calibrated SLAW trace and not evidence of human mobility; calibration
inputs and a paired baseline remain required before any performance claim.

The BLE evidence helper `scripts/validate_ble_evidence.py` validates the shape
of a future `mmp-ble-evidence-v1` manifest: central/peripheral roles, clean
commit and toolchain metadata, host logs and markers, store paths, delivery,
receipt, reconnect, MTU/RSSI availability, and explicit gate flags. It is
path-safe and fail-closed for missing fields, but it does not authenticate
logs or assert that a physical two-host exchange succeeded.

## 2026-08-09 continuation: high-level vault unavailable guard

The Rust-internal vault acceptance matrix now has a regression at the
production-facing high-level seam: when the snapshot provider reports
`Unavailable`, `save_core_from_vault` fails before committing a record and
`Node::restore_secret_state_from_vault` returns the same error without mutating
the target aggregate. The focused `mmp-node` library suite passes 77 tests and
`cargo clippy -p mmp-node --all-targets --all-features -- -D warnings` passes.
This is provider-capability behavior only; it does not implement or claim
Android Keystore, iOS Keychain, native lock/invalidation, crash-injection, or
platform lifecycle behavior. Those remain external gates before a production
keystore provider can be enabled.

## 2026-08-09 continuation: group-state growth audit (no safe v1 cap)

The remaining group-state growth path was audited against the retention-anchor
gap above. `MAX_GROUPS = 256` bounds only the top-level `NodeCore::groups` map;
it is enforced for network bootstrap and snapshot restore. Within an admitted
group, `GroupState::remotes` and `GroupState::dists` retain one entry per
`(member, epoch)`, and `NodeCore::sent_dists` retains one entry per
`(gid, member, epoch)` after a successful distribution send. None of these
maps has an expiry or authenticated floor. Group snapshots intentionally omit
the sender-key maps, so this is a live-runtime admission issue rather than a
new persisted-secret surface.

No wire-neutral fail-closed cap is safe to add at this stage. A currently
admitted member can present a valid pairwise-authenticated distribution for a
future epoch, and the current protocol permits delayed traffic from older
epochs while partitions reconverge. Rejecting a fixed epoch delta, imposing an
arbitrary map-size limit, or pruning old entries would therefore change which
valid delayed messages can decrypt and could reset/lose replay-window state.
Adding only a diagnostic counter would not be a mitigation: it would observe
growth after the attacker had already consumed the unbounded allocation.

The required implementation gate remains normative: define an authenticated
`(gid, floor_epoch)` retention anchor with issuance/validity and a conservative
partition-merge rule; then add a measured future-epoch admission budget and
fail-closed diagnostic, prune only unreferenced entries below the accepted
floor, and bound `sent_dists` as a retransmission cache without resetting any
sender/receiver chain. Until that anchor exists, no code change was justified
by this audit and the existing behavior is intentionally left unchanged.
The focused group unit tests pass 2 tests, and
`cargo clippy -p mmp-node --all-targets --all-features -- -D warnings` passes.

## 2026-08-09 continuation: ratchet terminal-state and prekey composition audit

The focused crypto audit compared `mmp-crypto::ratchet` and `prekey` with
specification/08 §4.2 and §5.1–§5.3 and found no safe wire-format change. The
prekey construction remains intentionally serverless and bespoke: it derives
`CK₀` from `eph×SPK`, optional `eph×OTK`, `sender-static×SPK`, and both identity
public keys. The implementation now has a regression proving that changing the
sender identity, sender exchange key, or omitting the OTK changes the derived
root; the existing low-order-point and signed-batch tests remain green. This
does not replace the required external review of the non-standard X3DH
composition, especially its omission of the full X3DH sender-static×OTK term
and its replay/suppression semantics.

Live ratchet counters previously used unchecked `u32`/`u16` increments even
though provider-local snapshots already rejected terminal values. The
wire-neutral hardening adds fallible `try_next` and `try_bump_epoch` methods,
uses checked arithmetic, and keeps the existing `next`/`bump_epoch` methods as
compatibility wrappers. The final `u32::MAX` message index is still accepted
once, then the in-memory chain enters an explicit exhausted state; no counter
wrap or `(epoch, index)` reuse is possible. The wrappers fail closed with an
explicit panic at exhaustion; new providers should use the fallible methods to
surface `CryptoError::Ratchet` without panicking. Existing mmp-node/group
callers still use the compatibility wrappers because adding a new public node
error would be an API change; their normal output is byte-for-byte unchanged.
The terminal boundary tests verify final-index acceptance, no post-exhaustion
key generation, and no state mutation on fallible failure. Provider-local
snapshots continue to reject terminal states, so a provider must persist or
re-key before reaching the final counter. This is a local lifecycle residual,
not a claim that a peer can be forced to send 2³² messages; external review
should still define graceful application recovery and provider migration.

`b8d7dfc` also keeps exhaustion checks after the old-index/skipped-key branch:
if the final index arrives before an earlier message from the bounded reorder
window, that already-derived skipped key remains decryptable. The regression
drives `MAX-1`, then `MAX`, then releases the stored `MAX-2` key.

## 2026-08-09 continuation: raw secret-state identity binding

The Rust-internal aggregate restore seam now applies the same identity-to-
conversation binding already enforced by the provider-backed vault. Before
moving decoded ratchets, prekeys, or delivery state into a live `Node`,
`restore_secret_state` recomputes `conversation_id(local_identity, peer_key)`
for every decoded conversation and rejects any mismatched map key. The
restore remains atomic: a structurally valid but misbound aggregate leaves the
target node unchanged. This is a local consistency fence, not a keystore
implementation and not a new wire rule.

The regression mutates only the first encoded conversation key while retaining
valid aggregate framing and asserts `SecretStateError::Invalid` plus byte-for-
byte preservation of the pre-restore snapshot. The focused `mmp-node` suite
passes 87 tests and the provider-backed `open_record` path continues to retain
its independent context and identity checks. Native Android Keystore, iOS
Keychain, DPAPI, crash injection, and platform lifecycle evidence remain
external gates.

## 2026-08-09 continuation: CLI persistence failure is fail-closed

The reference `mmp-cli` drive loops now propagate the result of their exit-time
`SnapshotStore` persistence. Previously, a failed `--store` write was logged but
the successful-delivery and normal-listen paths could still return exit status
0, falsely implying restart state had been durably recorded. `finish` now
returns a boolean; successful delivery, timeout, normal shutdown, and both
central/peripheral loops return exit status 2 when persistence fails. Setup and
transport-overload exits still remain non-success while attempting best-effort
state persistence.

The regression uses a path whose parent does not exist and asserts that the
finish seam reports failure without creating the path. `mmp-cli` now passes 34
focused tests. This is host-process reliability evidence only; it does not
prove filesystem crash consistency, mobile storage semantics, or physical BLE
interoperability.

## 2026-08-09 continuation: bounded conversation, delivery history, and blob indexes

The node-state implementation now closes three runtime lifecycle gaps without
changing any frozen wire object. Receiver-side pairwise conversation state is
bounded at `MAX_CONVERSATION_STATE = 256`. The bound covers both the permanent
conversation map and the prior-contact marker table, and is checked before
first-contact X3DH/sealed work, one-time-prekey consumption, file coding, or
symbol minting. Existing conversations remain serviceable. New contacts and
outbound first-contact sends fail with the local `ResourceLimit` condition when
the state is saturated. Link-down handling also retires provisional state that
would otherwise race the marker-table cap.

The runtime delivery-history map now shares the 4096-entry limit used by the
secret-state snapshot. Only `Delivered` and `Expired` entries may be reclaimed;
`Queued` and `InMesh` records are never evicted to make room. If all entries are
active, the send is rejected before bundle creation or ratchet advancement.
This keeps secret-state snapshots admissible during long-lived operation while
preserving active delivery tracking.

Holder-side symbol indexes are now lifecycle-consistent: receipt purge removes
the reverse index, duplicate `(root, symbol)` replacement removes only the old
reverse owner, and stale reverse cleanup cannot erase a newer forward mapping.
Locally minted file symbols use the same indexing helper as received symbols.

Focused verification: 93 `mmp-node` tests and strict Clippy for `mmp-node` and
`mmp-ffi` pass. The post-change full gates are green: workspace tests and
Clippy, 124 Python tests (8 skipped), 46 deterministic fuzz seeds, the
embedded `thumbv7em-none-eabihf` closure, layer-check, release CLI delivery
demo, and six transactional-store tests. Native
keystore/Keychain/DPAPI providers, physical BLE exchange, Android/iOS build and
lifecycle evidence, and the group retention-anchor design remain open gates.

## 2026-08-09 continuation: terminal blob-assembly collection

`BlobAssembly` now records a monotonic terminal timestamp. `Wanted` and
`Fetching` entries still transition to `Abandoned` only after the existing
`BLOB_INTEREST_TTL_MS`; `Assembled` and `Abandoned` progress remains queryable
for one additional interest window. After that retention period, the sweep
removes only the requester-side assembly record. Verified partial symbols and
their holder-side `blob_index`/store entries are intentionally untouched, so
repair and serving semantics remain unchanged. A regression verifies that an
active assembly survives a terminal cleanup pass and that a partial holder
symbol remains indexed.

The group-state review remains a no-change result. `remotes`, `dists`, and
`sent_dists` cannot be arbitrarily capped while valid future-epoch
distributions and delayed older epochs may cross partitions. An authenticated
epoch floor/retention anchor, merge rule, tombstone persistence, and reference-
aware pruning are required before implementing that bound.

The mobile audit also identified a stale illustrative `Date.now()` tick in the
pre-existing dirty API reference. It was not modified in this cycle; the
authoritative FFI contract and committed manual procedure require same-boot
monotonic time and an initial tick before sends.

Focused verification is the new terminal-GC regression and strict `mmp-node`
Clippy. The post-change full gates are green: workspace tests and Clippy, 124
Python tests (8 skipped), 46 deterministic fuzz seeds, the embedded
`thumbv7em-none-eabihf` closure, layer-check, release CLI delivery demo, and
six transactional-store tests. Native platform and physical BLE gates remain
open.

## 2026-08-09 continuation: store, simulator, and mobile contract residuals

The production store audit found no safe local patch to commit. The default
`SnapshotStore::open()` path still reads an unbounded file and can retain all
valid claims/group blobs from an unbounded image. The explicit
`open_with_limits()` path rejects oversized images before retaining them, and
blocklist records are strictly versioned, sorted, and capped. A safe default
requires a tier-aware storage policy plus streaming/early-reject parsing; an
arbitrary cap would silently reject valid anchor snapshots or change recovery
semantics. This remains a host-integration gate.

The simulator audit found that adversary-session buffers, schedules, and
metrics are intentionally test-driver-owned and unbounded. Production `Node`
does not import those types, and adversarial/contact loops are iteration-bounded
in the test scenarios. No production memory guarantee relies on simulator
buffers, so no simulator change was made.

The mobile contract audit found stale pre-existing documentation for
`ConversationPending`, `pollClose`/queue-drop signals, monotonic `tick`, group
method return types, destructive `pollSend` back-pressure retention, and
`pollClose -> native close -> linkDown` ordering. Those user-owned dirty files
were not overwritten. The Rust FFI surface and committed manual procedure
remain the authoritative implementation contract; documentation correction and
native back-pressure/lifecycle tests are the next integration gate.

Focused evidence: `mmp-store` tests and strict Clippy pass; `mmp-sim` has 27
passing tests and 6 ignored research/HIL cases. Native Android/iOS builds,
real ATT back-pressure, and physical BLE exchange remain unverified.

## 2026-08-09 continuation: session, transport, and FFI boundary audit

The session-engine audit found no additional production patch. Remote-driven
reassembly, pending frames/handoffs, outbound frames, aggregate decode counts,
and RBSR ledgers are explicitly bounded; malformed input is rejected before
allocation, and exhaustion closes the session fail-closed. The lifecycle event
queue only emits the bounded established/closed transitions. Custom
`SessionHost` implementations must still keep their local callback results
quota-backed, as the production node does.

Both BLE transport audits also found no narrow safe patch. Central and
peripheral ingress use bounded control/data lanes, generation fences, stale
callback rejection, bounded disconnect feedback, and fail-closed tombstone
limits. Active-link registries and peer-list materialization are host-policy
surfaces where an arbitrary hard cap could reject valid simultaneous physical
links. Peripheral stream setup intentionally permits pre-announcement, so
blindly recording every unknown generation would introduce a stale-link bug;
an admission contract is required before changing it.

The FFI boundary now has a direct regression asserting that `ResourceLimit` and
`RatchetExhausted` map to `FfiError::Internal`. This preserves the current
versioned Kotlin/Swift surface; adding finer-grained variants requires an
explicit API decision. The pre-existing dirty API/mobile documents still need
reconciliation for persisted seen/receipt state, `ConversationPending`, close
feedback, monotonic tick examples, group return types, destructive-send
retention during ATT back-pressure, and close ordering. They remain untouched.

Verification: `cargo test -p mmp-core --all-targets -- --test-threads=1`
(79 passed), `cargo test -p mmp-tal-btleplug --all-targets` (23 passed),
`cargo test -p mmp-tal-peripheral --all-targets` (40 passed), and
`cargo test -p mmp-ffi --all-features -- --test-threads=1` (21 passed), with
strict Clippy green for the touched crates. Specification files and
AI-specific artifacts remain unchanged.

## 2026-08-09 continuation: priority-domain and residual-plane audit

The store audit found no safe default-cap change: `open_with_limits` already
rejects oversized images before retention, while `open` lacks tier context and
an arbitrary bound could reject valid anchor snapshots. The blob audit found
the existing per-contact symbol, peer-counter, offer, session, and output
caps; whole-file streaming and group-file application-type preservation need
cross-surface API decisions. The router audit confirmed that declared-priority
transfer ordering and effective-priority eviction are intentionally distinct;
RBSR/D3 priority ordering remains deferred until its normative partition rule
exists.

The audits did identify a local invalid-input bug. `Node::publish_announcement`
now rejects priorities above `prio::BULK` before signing or storing, returning
`SendError::InvalidPriority`; the FFI maps that caller error to `BadArg`. The
snapshot decoder applies the same bound to restored metadata, so a
tampered-but-checksummed image cannot inject an unrepresentable priority into
local scheduling. Tests cover the node admission, FFI mapping, and snapshot
decoder rejection.

Verification: the three focused regressions pass and strict Clippy passes for
`mmp-node`, `mmp-store`, and `mmp-ffi`. Frozen specification files and
pre-existing dirty documentation remain untouched.

## 2026-08-09 continuation: group epoch, vault, and blob verification

The ratchet-vault review confirms that the Rust seam is already fail-closed:
secret state is AEAD-sealed before provider access, context is checked before
DEK unwrap, trusted-clock TTL and generation floors are enforced before and
after open, and restore is atomic. Native keystore/Keychain/DPAPI adapters,
durable high-water storage, and consuming mobile shells remain external gates.

Group-state retention maps remain without an arbitrary cap because delayed
older epochs and authenticated future epochs require an authenticated floor and
merge rule. Two local canonicality/lifecycle gaps are now closed: group
snapshot decode rejects epoch zero and noncanonical admin bytes, and epoch
rotation fails closed at `u16::MAX` via `RatchetExhausted` without mutating
membership.

The blob receive path now verifies Merkle proofs even when no local assembly is
active. Invalid shape-valid symbols are removed from storage and both holder
indices before they can be advertised or served; only active fetches increment
the bad-symbol counter. Unsolicited and active invalid-symbol regressions are
included.

Verification: `mmp-node` 99 tests, `mmp-ffi` 22 tests, and the store suites
(16 unit, 3 bounded, 1 hostile, 6 transactional) pass; strict Clippy passes
for touched crates. Specification and unrelated dirty files remain untouched.

## 2026-08-09 continuation: snapshot secret-copy hygiene

The delegated snapshot/crypto audit found one narrow hygiene issue in the
Rust-internal restore decoder. A decoded 32-byte conversation receipt key was
temporarily held in a plain stack array while parsing the remaining components.
It is now held in `Zeroizing<[u8; 32]>` until ownership moves into the live
conversation object. This is wire-neutral and leaves key derivation, snapshot
format, and restore semantics unchanged; it removes a non-zeroizing transient
copy on rejected or malformed restores.

The same audit found no additional safe change: snapshot envelopes enforce
exact lengths, AEAD context binding, freshness and generation fences, while
secret-state components and prekey tails are bounded. The vault preflight found
no evidence-based envelope or wrapped-key cap that can be duplicated safely at
the generic provider seam; `snapshot::open` already rejects malformed envelope
sizes before decrypting. Native keystore/Keychain/DPAPI implementations must
still enforce provider-specific opaque-handle limits.

Focused verification: the three `mmp-node` secret-state tests pass after the
zeroization change, and existing snapshot/vault tests cover malformed lengths,
context binding, freshness, rollback, and atomic restore. No specification or
pre-existing dirty path was modified.

## 2026-08-09 continuation: persistence revalidation and beacon rotation

The next delegated security pass closed two checksum-only persistence gaps.
`mmp-store` now rejects noncanonical restored metadata before it influences
routing or eviction: class bytes above the reserved 4-bit domain, budgets above
`L_MAX`, noncanonical `own_origin`, reversed or overlong lifetimes, and invalid
metadata on persist. Classes `6..=15` remain accepted for forward-compatible
store-and-forward.

`NodeCore` now revalidates persisted bundle bytes before forwarding by checking
canonical decoding, content-derived ID, immutable metadata binding, and the
Ed25519 bundle signature. Invalid records are removed and never emitted.
Startup blob-index rebuild applies the same checks and verifies Merkle
membership for class-4 symbols before indexing them, preventing a locally
rewritten snapshot checksum from creating a servable invalid symbol. Tests
cover tampered handoff bytes and signed-but-Merkle-invalid startup state.

The Android-SDK-free Kotlin coordinator now has `refreshBeacon(nowMs)`, which
lets a long-lived host replace a rotated ephemeral advertisement without a
second protocol tick. It enforces monotonic time, suppresses unchanged payload
restarts, and copies byte arrays across the native adapter boundary. Kotlin
compilation and physical mobile exchange remain external gates.

No specification or pre-existing dirty path was modified.

## 2026-08-09 continuation: recovery-path and startup-lifecycle hardening

The persistence audit closed a filename collision in crash recovery. If the
primary snapshot path ended in `.tmp`, the historical `with_extension("tmp")`
candidate could equal the primary and a failed persist could truncate or remove
the only complete image. The store now selects a distinct `.part` sibling for
that case, including Windows case-insensitive `.TMP` aliases, while retaining
the normal `.tmp` recovery name for ordinary paths. Regressions cover bounded
failure preservation and the Windows alias.

The group audit found a concrete memory-growth path: `sent_dists` retained
historical `(group, peer, epoch)` markers indefinitely. Successful epoch
rotation now prunes stale markers for the rotated group while retaining the
current epoch; failed current-epoch sends remain retryable. Remote distribution
maps remain uncapped pending an authenticated epoch-floor and partition-merge
contract.

The mobile coordinator now rolls back links admitted synchronously during a
failed scanner startup or initial drain. It sends protocol `linkDown`, clears
coordinator ownership before native GATT close callbacks, closes each link, and
clears pending output/cache state. Source-level regression coverage passes;
Kotlin compilation and physical devices remain external gates.

No specification or pre-existing dirty path was modified.

## 2026-08-09 continuation: singleton rollback and monotonic host safety

The persistence loader now uses first-valid-wins semantics for duplicate
singleton records (`SeenIndex`, `ReceiptFilter`, authority, and blocklist), so
an appended valid stale record cannot override the selected state. This is
defense-in-depth rather than full anti-rollback; authenticated generation
floors remain a provider responsibility. Primary and missing-primary recovery
candidate read errors now fail closed and prevent `persist` from masking an
unreadable image; the ordinary missing-primary/missing-candidate create path is
unchanged.

The router ContactGraph now enforces `CONTACT_GRAPH_CAP` even when all entries
are verified. It evicts only an unverified stranger and drops a new advisory
tie when no safe eviction exists; encounter and duration counters saturate
instead of wrapping. The direct Rust/UniFFI node path also now ignores backward
`Node::tick` samples before mutating time, matching the existing MeshDrive
monotonic guard. Equal and forward samples remain accepted.

Focused regressions cover duplicate singleton records, unreadable primary and
recovery paths, all-trusted ContactGraph capacity, counter saturation, direct
Rust ticks, and concurrent UniFFI tick callbacks. No specification or
pre-existing dirty path was modified.

## 2026-08-09 continuation: credential provisioning and transfer scheduling

The platform gate audit found a real CLI credential hazard: `keygen` previously
used truncating file creation. Commit `4517df1` now uses exclusive creation,
Unix owner-only `0600` permissions, and `sync_all()` before reporting success;
existing identities cannot be silently replaced. The same audit corrected the
PC interoperability guide to use snapshot file paths rather than directories
(`92ad7b2`), with a two-case documentation regression. Windows still cannot
exercise the Linux/BlueZ peripheral or two-host radio gate.

Commit `7b0b35b` wires the already-defined `mmp-router::transfer_order` into
`SessionEngine` through a default-compatible `SessionHost::order_handoffs`
hook. `NodeCore` applies control/destination/declared-priority/size/lifetime
ordering to known WANT and blob requests, while unknown IDs remain last. A
session-level spy and NodeCore ordering regression prove the hook runs before
loading bytes. This remains wire-neutral and does not claim RBSR priority
partitioning.

The group-control audit also records an unresolved high-risk epoch boundary:
future sender-key distributions are accepted for current post-removal
convergence because the existing 0x02 message has no admin anchor. Rejecting
all future epochs breaks legitimate group delivery; accepting them without an
anchor is not Byzantine-resistant. The candidate control codec remains
quarantined pending a normative schema, replay state, aggregate limits, and
anchor semantics. No specification or pre-existing dirty path was modified.

## 2026-08-09 continuation: D3 transcript coverage and evidence binding

The RBSR audit added test-only coverage for every executable D3 transcript
branch (`e172223`). Terminal `RECON_IDS`, split `RECON_RANGE`, and matching
echo `RECON_RANGE` paths now assert that the base domain remains `3` and only
the existing reply bit varies. This strengthens the wire-preserving subset;
it does not implement the specification's undefined per-priority partition.

The same audit confirmed a normative blocker: `RECON_INIT.window` is described
as a D3/last-24-hour-window flag, but the frozen documents do not define its
value, peer classification, or filtering policy. `Reconciler` therefore keeps
emitting the current `window=0` behavior. Do not infer a 24-hour filter or add
a new wire value until an approved erratum supplies those semantics and
legacy-peer vectors.

The release preflight now binds an evidence manifest to the exact checked-out
revision (`a70d166`). It resolves abbreviated or full commit IDs and rejects a
package whose recorded commit differs from `HEAD`, while continuing to ignore
unrelated worktree changes and to enforce a clean frozen `specification/`
tree. Documentation and regression tests cover the new gate. This protects
evidence integrity but remains separate from physical BLE/mobile validation,
which is unavailable on the current Windows host.

## 2026-08-09 continuation: group restore/invite and RBSR terminal obligations

The group lifecycle audit found two local state bugs and fixed them in
`3bf47cb`. `Node::restore_group` now rejects a duplicate `GroupID` instead of
replacing live membership/epoch state and minting a fresh local sender key from
an appended or stale snapshot record. `group_invite` is idempotent for an
already-live member, preventing redundant OR-Set tags and unbounded local tag
growth; a removed member is still re-invited with a new tag. Remote sender-key
distribution retention remains intentionally uncapped pending an authenticated
epoch-floor/partition-merge contract.

The RBSR audit found a wire-neutral correctness gap and fixed it in `c88d780`:
when a responder has emitted `RECON_IDS`, its `await_terminal` obligation may
only be retired by the corresponding ID-list or split response. A forged
reply-flagged `RECON_RANGE` with the same descriptor can no longer shortcut the
obligation and advance reconciliation with an unresolved set difference. The
new regression covers this path; D3 `window` and priority-partition semantics
remain normative blockers.

The same commit contains the independently reviewed BLE evidence validator
hardening: command checks now parse shell tokens, reject echo-text lookalikes,
and accept documented direct, Cargo, quoted-Windows, and PowerShell forms. The
validator and release/manual suites pass. This validates evidence structure,
not physical radio execution; the Windows host still lacks a runnable
Linux/BlueZ and Android/iOS end-to-end environment.

## 2026-08-09 continuation: persistence, session clocks, and iOS artifact boundaries

The persistence audit found that duplicate valid `RECORD_BUNDLE` entries were
last-valid-wins. A stale appended record could therefore roll back stored
expiry, spray budget, or handoff metadata while retaining the same BundleID.
Commit `ff970c2` keeps the first complete record and adds a regression for the
rollback case; malformed first records still permit a later valid record.

The session audit found that the public sans-IO `SessionEngine::on_wire` seam
accepted a stale timestamp and rewound `last_activity`, causing premature
idle-session expiry for hosts that deliver callbacks out of order. Commit
`0f53844` preserves the maximum observed activity timestamp and covers the
stale-callback transcript without changing frames or timeout constants.

The monotonic-clock audit found unchecked `u64` deadline arithmetic in seen,
receipt, claims, and ingest state. At `u64::MAX` this could panic in debug or
wrap to an immediately-expired deadline in release. Commit `85cb63e` uses
saturating arithmetic, including bucket rotation, and adds four terminal-clock
regressions. Saturation is conservative and wire-neutral.

The mobile artifact audit found that the iOS verifier followed symlinked Swift
binding roots or descendants, allowing source outside the artifact boundary to
be scanned. Commit `d19e734` rejects both forms before reading source, matching
the Android artifact policy. Python and iOS verifier tests cover real and
deterministic symlink cases; physical Xcode/device validation remains external.

## 2026-08-09 continuation: embedded, codec, and simulator readiness

The embedded audit found unchecked refill multiplication in
`mmp-router::OriginBuckets`. A terminal monotonic-clock jump could panic in
debug or wrap the refill amount in release. Commit `6f15894` uses saturating
arithmetic and adds a regression while preserving ordinary-clock behavior.
The `thumbv7em-none-eabihf` debug/release checks for `mmp-router`,
`mmp-node-embassy`, and `mmp-tal-embedded` pass. The unbounded in-memory
`MemLink` remains a simulator/test seam and is not exposed by the embedded
orchestration crates.

The codec audit found that `Hello::decode` accepted arbitrary protocol version
bytes even though other v1 frame families reject unsupported versions. Commit
`02281a8` now requires `WIRE_VERSION` and returns `WireError::BadVersion` for
incompatible HELLO frames. The mmp-types and mmp-crypto suites pass; this is a
wire-conformance correction, not a new version-negotiation rule.

The simulator audit found that inverted `min_gap_ms..max_gap_ms` ranges were
silently converted into a fixed minimum-gap trace. Commit `5c80e1b` rejects
the invalid range (panic for the low-level random runner, `false`/`None` for
the clustered research runners) before mutating simulation state. The complete
simulator suite passes with 30 active tests and 6 intentionally ignored scale/
deployment tests. This improves experiment validity without affecting routing
or wire behavior.

## 2026-08-09 continuation: group ratchet, TAL queue, and fuzz-release gates

The group-crypto audit found that a malformed authenticated group frame could
consume a receiver sender-key index before AEAD verification completed. Commit
`4560b20` adds a non-mutating receive-chain derivation (`peek_key_for`) and
commits the index only after signature, associated-data, and ciphertext checks
succeed. The mmp-crypto suite passes 51 tests and the mmp-node suite passes 105
tests. Future sender-key epochs remain an explicit normative blocker: the
current distribution message has no authenticated membership/epoch anchor, so
accepting future epochs is not Byzantine-resistant and rejecting them would
break partitioned convergence. The quarantined control-schema work must not be
promoted without an approved wire contract.

The BLE/TAL lifecycle audit found that bounded event queues accepted raw
`LinkData` records larger than the maximum envelope they can ever deliver.
Commit `2551914` rejects records above `MAX_ENVELOPE_PAYLOAD + 3` (length
prefix plus type byte) before queue insertion, returning the existing
`DataEnqueueError::Full` fail-closed path. Symmetric btleplug and peripheral
tests cover the bound; all-target suites pass (24 and 41 tests respectively),
mobile TAL tests pass (3), and strict clippy passes. A Linux peripheral check
remains an environment gate because this Windows host has no `libdbus` sysroot;
this is not treated as a Rust defect. The unbounded `MemLink` remains confined
to simulator/test seams.

The fuzz/release audit found case-insensitive seed-name collisions that would
overwrite files on Windows while appearing distinct on Linux. Commit
`554b37f` makes build, write, and verify reject those collisions. The focused
fuzz suite passes 13 tests (one intentionally skipped), and the complete Python
script suite passes 134 tests (nine intentionally skipped). This is tooling
hardening only; it does not change protocol bytes.

## 2026-08-09 continuation: P0 conformance, lifecycle, and release gates

The session audit found that an inbound `BUDGET` frame was accepted whenever
the session was established, even if no immediately preceding `BUNDLE` existed.
Commit `067fb24` tracks the canonical ID of the last received bundle and rejects
orphan, mismatched, or intervening-frame budgets while preserving the ordered
same-stream `BUNDLE`→`BUDGET` pair. Terminal `ERROR`/`CLOSE` frames remain valid
termination paths. The new regression was observed failing before the fix and
passes afterward. A residual API boundary remains: `SessionHost::on_bundle`
returns no admission result, so the engine cannot distinguish a syntactically
valid but signature-rejected bundle from an admitted one; NodeCore safely ignores
budgets for absent records, but an explicit host-result contract is still needed
for complete conformance to the “unheld ID is ERROR” wording.

The mobile lifecycle audit found that a saturated Rust close-action queue could
leave physical links alive if the host treated the counter as diagnostic only.
Commit `0659a66` makes `MeshDrive` latch overload, stop admitting work, release
logical links, clear pending fragments, request native teardown, and reject late
callbacks. Tracked physical links are independently capped. Commit `0576ef2`
keeps the existing source contract, retries failed native close requests, and
preserves normal drain errors while allowing overload cleanup to continue.
History cleanup `967c41d` removes an accidental comment-only overlap. The
focused Kotlin source suite passes 17 tests and the full Python suite passes 140
tests (nine intentionally skipped); mmp-node passes 105 tests.

Release governance now has a machine-readable, hash-pinned status manifest in
`docs/SPEC_STATUS_MANIFEST.json`. Commit `23062ae` records all 18 top-level
specification documents, their declared status markers, and publication anchors;
`7dc9d97` requires this check before keyless signing; `5406dba` treats a dangling
historical `AUDIT.md` symlink as present rather than silently converting it to an
absent file. The checker passes from the repository root, and the focused
status/CI tests pass 18 tests. This is publication metadata only and does not
alter specification bytes.

## 2026-08-09 continuation: admission results, evidence fingerprints, and paired replay

The follow-up session audit completed the remaining host-admission refinement
in `71ff2ad`. `SessionHost::bundle_admitted` has a permissive default for
backward compatibility; NodeCore and conformance hosts report whether the
canonical bundle is actually held. A matching `BUNDLE` followed by `BUDGET`
for a locally dropped or duplicate bundle is consumed but ignored, preserving
friend-buffer retransmission and SeenIndex behavior. Orphan, mismatched, and
intervening budgets still close the session. This remains wire-neutral. The
remaining host contract is intentionally explicit: custom hosts that need strict
admission semantics must override the new method.

The interoperability audit added `156f095`, an evidence-only seed fingerprint
helper and runbook instructions. It hashes a decoded 32-byte keygen seed,
rejects malformed or symlinked files, and never prints seed material. The helper
standardizes evidence correlation for the future two-host run; it does not claim
that the Windows host has executed Linux/BlueZ or physical BLE validation.

The measurement audit added `0d534c2`, a bounded `ContactSchedule`/`ContactEvent`
replay seam in `mmp-sim`. A schedule preflights node count, seed, endpoint
validity, self-contact, and a 100,000-event cap before mutating a simulator. Its
configuration and full 16-byte trace IDs bind the seed, node count, undirected
contact pairs, gaps, and event order. Baseline and candidate simulators can now
consume the identical schedule without changing routing, copy budgets, or wire
behavior. The simulator suite passes 33 tests with 6 intentionally ignored and
strict clippy passes. EDR/EWS/contact-survival performance claims remain
unmeasured until this seam is driven by calibrated device traces.

## 2026-08-09 continuation: group-removal invariant, evidence format, and trace identity

The security/control-plane audit found that removing an unknown group member
still consumed an epoch and rotated the local sender key. Commit `4c1398f`
now rejects the non-effective removal before mutating membership, epoch, or
sender-key state; the regression passes in the `mmp-node` suite. The remaining
future-epoch sender-key authorization/state-growth risk is deliberately not
patched locally: a safe bound or equality rule would require an authenticated
epoch anchor and could break partitioned re-key convergence.

The interoperability evidence validator was tightened in `ce25f9e` to require
the canonical `sha256:<64 lowercase hex>` seed fingerprint emitted by the
evidence helper. This prevents ambiguous, hand-written, or truncated markers
from being accepted as proof. Focused validation tests pass; physical BLE
execution remains an external gate.

The research instrumentation audit found that EDR measurements used a 64-bit
trace identifier while `ContactSchedule` and `Sim` already produce a full
16-byte identity. Commit `e3f066e` carries `[u8; 16]` through
`RendezvousMeasurement`, paired validation, and comparison summaries, with a
regression that changes only the high byte and is rejected. The focused and
full simulator suites pass (33 passed, 6 intentionally ignored), and strict
Clippy is clean. This is a research-data integrity correction only; it adds no
wire field, scheduler policy, or performance claim.

## 2026-08-09 continuation: bounded blob advertisements, group errors, and EWS research

The blob-plane audit confirmed that origin-targeted `BLOB_REQ` scheduling is
not implementable without guessing semantics absent from the current frame:
after re-sharing, the bundle origin and manifest sharer can differ, while the
frame carries no target field. Instead, commit `0637a67` adds a local,
wire-neutral bound: at most four `BLOB_HAVE` advertisements are emitted per
authenticated contact, with a bounded per-peer cursor rotating active roots
across later contacts. The cursor resets at contact establishment. Node tests
cover the cap, rotation, and reset; the origin-targeted request remains
explicitly deferred pending a normative decision.

The same commit closes the public error-boundary mismatch introduced by the
unknown-member invariant. `Node::group_remove` now returns the existing
`UnknownPeer` error before any epoch work, and the FFI reports that error rather
than `Internal`; Node and FFI regressions cover both paths. Epoch exhaustion
continues to report `RatchetExhausted`.

The research track added `mmp-sim::ews`, committed as `49b9a41`. It is a
bounded, deterministic Evidence-Weighted Spray evaluator over caller-supplied
probabilities and costs. It validates finite inputs, duplicate labels, copy and
cost limits, records marginal gain/cost decisions, and explicitly states that
the result is a local planning heuristic rather than a global copy-budget
security primitive. Three focused tests and strict Clippy pass. It does not
change `mmp-router`, wire frames, admission, or default behavior.

## 2026-08-09 continuation: integrated verification record

After integrating `0637a67`, `49b9a41`, and this handoff update, the
orchestrator reran the affected gates from a shared worktree:

- `cargo test -p mmp-node -p mmp-ffi --all-targets -- --test-threads=1`:
  108 node tests and 25 FFI tests passed.
- `cargo test -p mmp-sim --all-targets -- --test-threads=1`: 36 passed and
  6 intentionally ignored, including the new three EWS tests.
- Strict Clippy with `-D warnings` passed for `mmp-node`, `mmp-ffi`, and
  `mmp-sim` with all targets/features.
- Python script suite: 144 passed, 10 expected skips.
- `check_spec_status.py --repo ../..`: passed.
- `git diff --check`: clean; no path under `specification/` changed.

The worktree still contains the previously identified user-owned dirty docs,
the quarantined `mmp-types` candidate files, and audit temporary directories;
none were staged by this cycle.
