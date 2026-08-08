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
