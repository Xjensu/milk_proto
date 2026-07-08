# 03 — Protocol Specification (Normative Core)

**mesh_milk Protocol, version 1 (MMP/1).** Status: Phase 2 draft; **revised 2026-06-11 post-audit** (see `AUDIT.md`, `CHANGELOG.md`). The key words MUST, MUST NOT, SHOULD, SHOULD NOT, MAY are to be interpreted as in RFC 2119. Constants marked ⚠ are provisional until validated in doc 13. Design rationale traces to doc 02 findings (cited as [F-A2], [R-C1], …).

> **Audit revision summary (this doc):** §3.2/§4.1 carry the origin **identity public key inline** so verify-before-forward is unconditional (AUDIT F2); §4.3/§4.2 add the **SeenIndex-from-first-sight** anti-resurrection rule and name the per-class dedup backstop (AUDIT F1); §4.5 corrects the mutable-per-hop argument and adds a **copy-budget receive clamp** (AUDIT F3); §4.1/§5.1 reference the global **signing-domain separation** scheme now normative in doc 10 §3.1 (AUDIT F4) and the per-epoch **beacon-summary re-salt** (AUDIT F8a).

---

## 1. Scope and Model

MMP/1 is a delay-tolerant, store-carry-forward messaging protocol over short-range radio (BLE in v1). It provides: pseudonymous cryptographic identity, signed profile claims (nicknames, avatars), end-to-end encrypted direct and group messaging, multi-source blob transfer, and opportunistic multi-hop dissemination without any infrastructure.

**Network model:** nodes encounter each other unpredictably; a *contact* is a time-bounded bidirectional communication opportunity. There is no global clock, no global topology knowledge, and no always-reachable node. End-to-end paths exist over space *and time* [C-B3].

**Trust model:** every peer and every link is assumed potentially hostile (local active adversary). No unauthenticated field may influence node behavior [F-A2].

## 2. Terminology

- **Node** — a device running MMP. **Origin** — the node that created a bundle. **Carrier** — a node currently storing a bundle. **Recipient** — the destination identity (node key or group).
- **Bundle** — the atomic unit of transport: an immutable, signed, TTL-bounded datagram.
- **Beacon** — a ≤24-byte broadcast advertisement (beacon plane).
- **Sync session** — an authenticated connection-oriented exchange during a contact (bulk plane).
- **Claim** — a signed, versioned statement by an identity about itself (nickname, avatar root, exchange key).
- **Blob** — content-addressed binary object > 1 bundle payload, transferred as erasure-coded **symbols**.
- **Conversation DAG** — the causal partial order of messages in one conversation, encoded by parent references.

## 3. Identity (normative summary; full treatment in doc 08)

3.1. An identity **IS** an Ed25519 signing keypair. There is no registration authority [F-A4 lineage; Zooko-triangle position: secure + decentralized, not globally unique names].

3.2. `NodeID = BLAKE3-256(identity_pubkey)[0..16]` (128 bits) is a *derived identifier*, never an authenticator. To make **verify-before-forward unconditional** (AUDIT F2), every bundle carries its origin's **full 32-byte Ed25519 identity public key inline** in the `origin` field (§4.1), not the 16-byte NodeID; any relay re-derives `NodeID = BLAKE3-256(origin)[0..16]` and verifies the signature with no dependency on having previously received the origin's claim. This closes the prior race where a stranger-originated bundle could arrive before the origin's key claim and be stranded in quarantine. Claims (§3.3, doc 06 §4) remain how peers learn *other* identity data (exchange keys, prekeys, nicknames) and how trust/verification decisions (TOFU, doc 07 §6.1) are made — distinct from signature *validity*, which is now always checkable on sight.

3.3. Each identity publishes a static **exchange key** (X25519) as a Claim signed by its identity key. Exchange keys MAY rotate; claims carry a monotonic `seq` (LWW semantics, [R-D1]). Identity keys never rotate — key loss/compromise means identity replacement (consistent with the threat tier; see doc 07).

3.4. Nicknames are Claims, not registrations: collisions are permitted; UIs MUST display disambiguators (petname assignments and/or NodeID art). Rationale: doc 02 §1, avoiding first-claim-wins consensus that an asynchronous mesh cannot provide.

## 4. Bundle

### 4.1 Canonical encoding

All multi-byte integers are little-endian. The bundle is encoded as a fixed header followed by TLV extensions and payload. The **canonical bytes** (everything except `signature`) are the input to both the bundle ID and the signature.

```
Bundle {
  version      : u4        // wire version = 1
  class        : u4        // 0=control 1=direct 2=group 3=claim 4=symbol
  flags        : u8        // bit0: has_parents, bit1: wants_receipt, …
  origin       : [u8; 32]  // origin's Ed25519 identity PUBLIC KEY (inline, AUDIT F2)
  dest         : [u8; 16]  // NodeID, GroupID, or well-known scope ID
  lifetime_ms  : u40       // REMAINING lifetime (age-based, §4.3)
  priority     : u8        // 0 (highest) … 3
  ext_len      : u16
  extensions   : TLV*      // §4.4
  payload_len  : u16
  payload      : bytes     // AEAD ciphertext for class 1,2; plaintext for 0,3,4 (still signed)
  signature    : [u8; 64]  // Ed25519 by origin, domain-separated (doc 10 §3.1), over lifetime-zeroed canonical bytes
}
BundleID = BLAKE3-256(canonical bytes, lifetime zeroed)[0..16]
```

The `origin` field is the full 32-byte public key (AUDIT F2); `NodeID = BLAKE3-256(origin)[0..16]` is derived wherever a 16-byte identifier is needed (dedup indices, ContactGraph keys, `dest` of replies). The signature input is **domain-separated** per the global scheme in doc 10 §3.1 (context tag `mmp1/bundle\x01`, AUDIT F4) and computed over the **lifetime-zeroed** canonical form (§4.5; byte-exact rule in doc 10 §3). `BundleID` is the content address over the same lifetime-zeroed canonical bytes (no domain tag) — so a relay that resets only `lifetime_ms` produces a bundle with an **identical BundleID**, which is what makes dedup resurrection-resistant (§4.2).

4.1.1. Maximum bundle size: **4096 bytes** ⚠ (fits one GATT exchange burst at realistic MTUs; larger content MUST use the blob mechanism).

4.1.2. A node MUST verify `signature` (domain-separated, doc 10 §3.1) against the **inline `origin` public key** (§3.2, §4.1) **before storing or forwarding** a bundle, and MUST check `NodeID = BLAKE3-256(origin)[0..16]` is consistent with any NodeID-keyed routing/index decision. Because the key is carried inline (AUDIT F2), verification is **unconditional** — it never depends on having previously received the origin's claim, so the prior key-unknown quarantine path is eliminated for the normal case. A bundle whose signature fails, whose `origin`→NodeID derivation is inconsistent, or which is non-canonical (doc 10 §6) is **dropped, never quarantined, never forwarded**. The only residual quarantine use is a small transient hold (≤16 KiB ⚠, lowest eviction priority, volatile) for partially-reassembled frames (doc 09 §5.1, doc 10 §7); nothing is ever *forwarded* out of it. This strictly tightens the bitchat cache-poisoning closure [F-A2]: there is no longer any "store-before-key" window at all.

### 4.2 Replay and duplicate suppression (resurrection-resistant)

Nodes MUST maintain a seen-ID structure (SeenIndex) and MUST NOT re-process or re-store a BundleID already seen. **SeenIndex entries persist for the bundle's *class-maximum* lifetime measured from the moment of FIRST sight, not from the bundle's current `lifetime_ms`** (AUDIT F1). This is the anti-resurrection rule: because `lifetime_ms` is excluded from both the signature and the BundleID (§4.1, §4.5), a malicious carrier can reset a captured bundle's remaining lifetime to the class maximum and re-inject it — but the BundleID is unchanged, so SeenIndex catches the resurrection for a full class-max window after any honest node last saw it.

Two backstops bound the residual (a bundle resurrected *after* its BundleID has aged out of every honest SeenIndex):
1. **Per-class application dedup.** Every class carries an application-layer identity that a recipient deduplicates independently of routing: class 1 direct `(conv, ratchet_epoch, msg_index)` (doc 08 §5, doc 10 §4.1); class 2 group `(gid, epoch, member, chain_index)` (doc 10 §4.2); class 0 receipt `(delivered_id, mac)` — idempotent re-application (doc 10 §4.3); class 3 claim `(owner, field, seq)` LWW (doc 06 §4.2); class 4 symbol `(root, block, sym)` (doc 10 §4.5). A resurrected bundle therefore never produces a duplicate user-visible event — only authentic, already-delivered content the recipient discards.
2. **Quota.** The only resurrection cost is one bounded relay store/offer cycle, capped by the per-origin token bucket and store quota (doc 05 §8, doc 04 §2).

(Receipt filters in doc 05/06 extend offer-suppression on top of SeenIndex.)

### 4.3 Lifetime (age-based, no global clock)

`lifetime_ms` is *remaining* lifetime. A carrier MUST decrement it by its holding duration (monotonic local clock) before handing the bundle to another node, and MUST delete bundles at 0. Origins set initial lifetime per class (defaults: control 6 h, direct/group 72 h, claim 7 d, symbol 72 h ⚠). On receive, a node MUST **clamp `lifetime_ms` to the class maximum** (zombie-lifetime defense, §11). This is BPv7's bundle-age solution to clockless DTN nodes [F-A5]. Because `lifetime_ms` is mutated per-hop it lies **outside** the signed and BundleID regions (zeroed-canonical form, doc 10 §3) — see §4.5 for why this is sound, and §4.2 for why excluding it from the BundleID is what makes dedup resist lifetime-reset resurrection rather than enabling it (AUDIT F1).

### 4.4 Extensions (TLV)

`type:u8, len:u16, value`. Type space: 0x00–0x7F *critical* (a node that does not understand a critical TLV MUST NOT process or forward the bundle), 0x80–0xFF *optional* (ignore if unknown) — BPv7-style criticality [F-A5]. Defined v1 TLVs:

| Type | Crit | Name | Content |
|---|---|---|---|
| 0x01 | yes | PARENTS | 1–4 × BundleID — causal references (conversation DAG) [R-D1] |
| 0x02 | yes | GROUP_EPOCH | group key epoch number (class 2) |
| 0x03 | yes | SYMBOL_INFO | blob root hash (16 B), block index, symbol index, k, n |
| 0x80 | no | PADDING | length padding (traffic-shape hint) |

**Rev. B registry additions (pointers only — defined and owned by the extension docs, no v1 format change):** optional-range TLV `0x81 AUTHCERT_ATTACH` (self-contained AuthCert on elevated-priority announcements, doc 18 §5.2.1; doubles as doc 18 §2.1's unknown-claim-field fallback slot); optional-range TLV `0x82 SCOPE_INFO` (scope-name hint on announcement bundles, doc 18 §3) — both ignore-if-unknown per the 0x80–0xFF rule above; claim fields `0x07 = AuthCert` (doc 18 §2.1) and `0x08 = REVOCATION_LIST` (issuer-owned whole-state revocation value, doc 18 §6) — the claim *field* registry itself lives in doc 06 §4.1; recorded here for registry discoverability.

### 4.5 Mutable per-hop metadata

Exactly two values are legitimately relay-mutable: `lifetime_ms` (§4.3) and the **spray copy budget** (doc 05), which travels in session control messages, *not* inside the bundle.

**Security argument (corrected, AUDIT F3).** The earlier framing — "neither field grants an adversary more than it already has" — is only true for bundles the adversary *originated*. For a bundle it merely *relays*, copy-budget inflation **does** grant new power: a malicious relay can hand an inflated budget to many *honest* downstream peers, who then spray on *their own* airtime — amplifying replication using other nodes' batteries. The correct bound is therefore **not** "the attacker spends its own airtime"; it is:

1. **Receive-side budget clamp (normative):** a node MUST clamp any granted copy budget to `L_max = 32` ⚠ on receipt (doc 05 §2.3), exactly mirroring the §4.3 lifetime clamp. A single grant can thus never exceed the protocol ceiling.
2. **Per-origin token buckets (normative):** every honest relay meters carried traffic by *origin key* (doc 05 §8), so total systemic amplification of any one origin's bundles is capped regardless of inflated budgets handed out anywhere — the attacker cannot make honest relays exceed the origin's quota.
3. `lifetime_ms` inflation is bounded by the receive clamp (§4.3) + SeenIndex resurrection resistance (§4.2).

Correctness against *third parties* still holds because both values are exchanged only inside the MAC'd session channel (§6.3); all other header bytes are signature-covered [F-A2].

## 5. Planes

### 5.1 Beacon plane

Carried in BLE legacy advertisements under the MMP 16-bit Service Data AD type (one well-known UUID, required for iOS targeted scans [C-B2]). Payload budget **≤24 bytes** ⚠:

```
Beacon {
  ver_role   : u8   // 4b version, 2b tier (doc 04), 2b battery class
  ephem_id   : [u8;8]   // rotating pseudonym, §5.1.1
  epoch      : u8       // summary rotation counter
  summary    : [u8;14]  // rotating fragment: Bloom of held BundleIDs,
                        // receipt-filter fragment, or claim-version digest
}
```

5.1.1. **Ephemeral IDs.** Beacons MUST NOT carry stable NodeIDs in the clear (tracking resistance). `ephem_id = BLAKE3(identity_pubkey ‖ coarse_time_slot ‖ beacon_salt)[0..8]`, resolvable only by peers that already know the identity key (they can recompute candidates for their contact list). Strangers learn presence-of-a-node, not identity. Linkage across slots is prevented for non-contacts. (Doc 07 details; coarse_time_slot tolerance ±1 slot.)

5.1.1.1. **Summary re-salting (normative, AUDIT F8a).** The 14-byte rotating `summary` is a slowly-changing function of node state (held-set Bloom, receipt-filter, claim digest) and would otherwise be a quasi-stable fingerprint that lets a continuous co-present scanner **link two `ephem_id`s across a rotation boundary** by matching fragments — defeating §5.1.1. Therefore every structure feeding a `summary` fragment MUST be re-salted per `ephem_id` epoch: fragment hashing mixes in `slot_salt = BLAKE3("mmp1 beacon-salt" ‖ coarse_time_slot ‖ beacon_salt)`, so fragment bytes are uncorrelated before and after a rotation. Re-salting is free for receivers (fragments are accumulated hints, doc 09 §3.3 — they re-learn after each rotation anyway).

5.1.2. Beacons are unauthenticated (31-byte physics) and therefore MUST be treated as *hints only*: they trigger connections; they never directly cause storage, deletion, or routing changes [F-A2 discipline].

### 5.2 Bulk plane

A sync session over a reliable byte stream (GATT in v1, TAL §7). Session establishment performs mutual authentication and key agreement (Noise-pattern handshake; doc 08), yielding an AEAD channel. All session control messages (RBSR rounds, copy-budget transfers, receipt-filter exchange, symbol requests) flow inside this channel.

Session phases (normative order):
1. **HELLO/handshake** → authenticated channel.
2. **SUMMARY** — exchange receipt filters + claim-version vectors (cheap, fixed-size first: maximizes value of very short contacts).
3. **RECONCILE** — RBSR over BundleID sets per priority class, highest priority first [R-D1].
4. **TRANSFER** — bundles in doc 05 §6 order; spray budget accounting.
5. **BLOB** — symbol requests/streams for wanted blob roots.
Sessions MUST be resumable: every phase is idempotent; a dropped contact loses at most the in-flight item.

## 6. Layering and conformance

```
Application services:  Messaging | Profile/Claims | Groups | Blob
Sync layer:            RBSR reconciliation, claim LWW, receipt filters
Bundle layer:          §4 (store, verify, lifetime, dedupe)
Routing layer:         doc 05 (spray+utility, queues, quotas)
Session layer:         handshake, AEAD channel, control msgs (doc 08)
TAL (transport adaptation): BLE beacon+GATT (doc 09); future: WiFi Direct, UWB
```

**Conformance classes** [F-A6, platform asymmetry C-B2/C-B4]:
- **MMP-Core** (sensors, `no_std`): bundle layer, session layer, RBSR for class 0/1/2 + own claims, beacon TX/RX, friendship-LPN role. MAY omit: blob plane, group membership CRDTs, utility routing (pure spray).
- **MMP-Standard** (phones): everything; relay duty subject to OS/battery policy.
- **MMP-Anchor** (PCs): Standard + elevated storage quota + always-on relay.

## 7. Transport Adaptation Layer contract

A TAL implementation MUST provide: `beacon_set(payload)`, `beacon_scan(cb)`, `dial(peer_hint) -> Stream`, `listen(cb)`, where `Stream` is reliable, ordered, bidirectional. It MUST report contact-quality hints (RSSI, estimated throughput) and MUST NOT reorder session frames. Everything above the TAL is radio-agnostic (future WiFi Direct/UWB slots in here).

## 8. Versioning and agility

- Wire version in every bundle and beacon (4 bits).
- Cipher suites: 1-byte registry pinned in the handshake (v1 = suite 0x01: X25519/Ed25519/ChaCha20-Poly1305/BLAKE3; 0x02 reserved for hybrid ML-KEM — doc 08).
- Unknown bundle classes: store-and-forward if signature verifies and TLVs are non-critical (forward-compat relaying), never process.
- **Rev. B class-registry addition (pointer only):** class `5` = public announcement, defined entirely in doc 18 §3 under exactly this unknown-class rule — a v1 (rev. A) node relays class-5 bundles blindly and correctly without modification. Classes 6–15 remain free.
- **Rev. B convention note (not a wire allocation):** class 1/2 message bodies MAY begin, *inside* the AEAD plaintext, with a one-byte application type tag (`0x00` text, `0x01` FileManifest — doc 16 §3.1, including the fail-safe legacy-body disambiguation rule). Relays never see this byte; it is recorded here only so readers of the class-1/2 payload description know the body-interior convention exists.

## 9. Protocol constants (v1, all ⚠ pending doc 13)

| Constant | Value | Note |
|---|---|---|
| MAX_BUNDLE | 4096 B | §4.1.1 |
| L_MAX (copy-budget clamp) | 32 | §4.5, doc 05 §2.3 (AUDIT F3) |
| LIFETIME_DIRECT / GROUP | 72 h | retention decision |
| LIFETIME_CONTROL | 6 h | receipts, acks |
| LIFETIME_CLAIM | 7 d | profile gossip |
| QUOTA_STANDARD | 128 MiB | phone default store |
| QUOTA_CORE | 32 KiB | sensor default |
| QUOTA_ANCHOR | 2 GiB | PC default |
| BEACON_INTERVAL | 2–10 s adaptive | doc 04 power mgmt |
| SCAN_DUTY | ≤2.5% | doc 04/13 |

## 10. Security invariants (normative checklist)

1. No unauthenticated input changes durable state (beacons are hints; nothing forwards before signature verification).
2. Verify-before-store, verify-before-forward, **unconditionally** — origin pubkey is inline (§3.2, §4.1.2, AUDIT F2); failed/inconsistent/non-canonical bundles are dropped, never forwarded.
3. All session control is AEAD-protected after mutual auth.
4. Stable identifiers never appear in cleartext on air (§5.1.1); rotating summary fragments are re-salted per epoch so they cannot bridge rotations (§5.1.1.1, AUDIT F8a).
5. Relay-mutable values are limited to lifetime + copy budget, **both clamped on receive** (§4.3, §4.5) and bounded by per-origin quotas (AUDIT F3).
6. Dedup is resurrection-resistant: SeenIndex persists from first sight for class-max lifetime, with per-class application dedup as backstop (§4.2, AUDIT F1).
7. Payload confidentiality is end-to-end; relays verify origin signatures without payload access.
8. Every Ed25519 signature in the system is domain-separated (doc 10 §3.1, AUDIT F4) — no cross-context signature reuse is representable.

## 11. Persona review checkpoints

- **Security Auditor — Potential flaw:** "Unsigned lifetime lets a malicious relay extend a bundle's life arbitrarily (zombie bundles), and resurrect captured bundles after the dedup horizon." *Resolution (strengthened post-audit, AUDIT F1):* receivers clamp `lifetime_ms ≤ class maximum` (§4.3); the BundleID excludes lifetime, so a reset bundle has an *identical* ID and is caught by SeenIndex, which now persists **from first sight for class-max** (§4.2) rather than from current lifetime. Past that horizon, per-class application dedup (§4.2 backstop) guarantees no duplicate user-visible event; the only residual is one quota-bounded relay store/offer cycle. The earlier "storage waste only on first sight" framing understated the resurrection window and is superseded. Accepted.
- **Distributed Systems Reviewer — Potential flaw (resolved by AUDIT F2):** "Quarantine re-introduced store-before-verify." *Resolution:* the key-unknown quarantine is **removed entirely** — the origin public key is now inline (§4.1), so every bundle is verified on sight and either stored or dropped. There is no store-before-verify window of any kind; the bitchat poisoning channel cannot exist. (Originally accepted with an explicit cap; the audit superseded it with the stronger no-quarantine design.)
- **Battery Specialist:** SUMMARY-before-RECONCILE ordering chosen so a 3-second contact still yields receipt-filter value even if reconciliation never starts. Accepted.
- **Skeptical Professor:** ephem_id resolvability by contacts is O(contacts) hash trials per beacon — fine on phones (≤4096 contacts ⚠), needs a cap on sensors (friend nodes resolve on their behalf, doc 04). Accepted; noted in doc 12 perf budget.
- **Inventive Scientist — Alternative considered:** putting copy budget inside the signed bundle with origin-delegated "spray certificates." Rejected: origin cannot foresee relay topology; certificate chains explode bundle size; cooperative-not-security framing (§4.5) is honest and cheaper.

## 12. Cross-references

Routing detail → doc 05. Sync semantics → doc 06. Threats/mitigations → doc 07. Handshake/AEAD/keys → doc 08. BLE mapping of beacon+GATT → doc 09. Exact bit layouts → doc 10. Node lifecycle → doc 11.
