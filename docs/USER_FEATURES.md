# MMP/1 — What the App Can Offer Users

A product-facing catalog of end-user features buildable on the protocol as it will exist at production, per the phased plan in `docs/ROADMAP_TO_PRODUCTION.md`. This document complements `implementation/mesh-milk/V2_FEATURES.md`: that document's framing is "what the protocol hides" (engineering-facing, organized by protocol mechanism); this one's framing is "what the user gets" (product-facing, organized by feature). Read V2_FEATURES.md first for the underlying mechanism detail — this document references its sections rather than repeating them.

Every feature below is honest about the UX constraint the protocol imposes. mesh_milk is a store-and-forward mesh with no Internet infrastructure (see `CLAUDE.md` fixed requirements); that buys enormous resilience and privacy properties, and it costs latency and eventual — not instant — consistency. The features that sell hardest are the ones that make that tradeoff feel like a feature instead of a bug.

---

## Summary table

| Feature | Powered by | Earliest phase |
|---|---|---|
| Offline 1:1 messaging with cryptographic delivery proof | Ed25519/X3DH-adapted sessions + ratchet, receipt MAC (V2_FEATURES §D) | Phase A |
| Groups with member removal that provably converges | Group OR-Set, epoch-anchor GC (audit finding F5), sender-keys | Phase A (basic groups); full provable-removal UX polish Phase C |
| QR contact verification + petnames | Identity claims, LWW claims merge, petname-model nicknames | Phase A |
| Store-and-forward "dead drop" / time-capsule messaging | TTL-by-physics store-and-forward (V2_FEATURES §E) | Phase A |
| Crossed-paths radar | Rotating ephemeral beacon IDs (V2_FEATURES §C) | Phase A |
| Message journey map | Diagnostics journey log (V2_FEATURES §B) | Phase B (needs real radio + multi-hop field data to be interesting) |
| Mesh vitality widget | Population estimate N̂ (V2_FEATURES §J) | Phase A |
| Relay karma dashboard | Diag counters, per-node relay accounting (V2_FEATURES §F) | Phase B |
| Panic wipe + duress profile | Seed-only identity, disposable store (V2_FEATURES §H) | Phase B (needs platform Keystore/Keychain integration) |
| Sneakernet / any-channel import-export | Self-verifying, self-deduplicating bundles (V2_FEATURES §A) | Phase A (protocol-ready today; needs a UI affordance) |
| File/photo sharing with Reed-Solomon resilience | Class-4 symbol bundles, GF(256) systematic RS (`mmp-rs-code`) | Phase C (blob plane wiring) |
| Broadcast/event channels | Group scopes, 16-byte `dest` (V2_FEATURES §I) | Phase A (mechanism); Phase B for a polished large-audience UX |
| Nickname/avatar gossip directory | Claims gossip (class 3, 7-day TTL) (V2_FEATURES §G) | Phase A |

---

## Feature descriptions

### Offline 1:1 messaging with cryptographic delivery proof
Send a message to someone whether or not either of you has a network connection right now; the mesh carries it. Delivery isn't a guess — the ✓✓ only appears once the true recipient's conversation key has minted a MAC that could not have come from anyone else (V2_FEATURES §D). **UX constraint:** delivery is minutes-tier, not instant, because it depends on store-and-forward physically bringing a copy to the recipient. The app must frame ✓✓ as "delivered, proven cryptographically" rather than "read," and must not imply real-time chat responsiveness.

### Groups with member removal that provably converges
Groups use an OR-Set membership CRDT with sender-keys encryption and epoch-anchor garbage collection (audit finding F5). Removing a member is a signed operation the group converges on, not a server-enforced ACL. **UX constraint:** removal "applies as the network reconverges" (doc 06 §8.6) — a removed member's already-issued sender key can decrypt messages sent before the removal claim reaches every honest member. The app must show a `MembershipState::PendingConvergence` state rather than implying instant server-grade eviction, and should surface epoch rotation events so users understand why an old message occasionally shows as undecryptable after a membership change.

### QR contact verification + petnames
Scan a QR code to verify a contact's Ed25519 identity key out-of-band, then assign them a petname (a local, mutable nickname — never a shared namespace, so there's no impersonation-by-nickname attack surface). **UX constraint:** verification is the protocol's most manual step today; Phase A ships QR only. The tap-to-verify UWB flow in V2_FEATURES Part I item 2 (R6) is a v2 improvement, not something to promise at launch.

### Store-and-forward "dead drop" / time-capsule messaging
Leave a message for someone who isn't there yet — "leave it at the festival for whoever of my friends arrives" — and the mesh holds it, spray-distributed across nearby nodes, until it's delivered or it expires (24–72 h TTL, per CLAUDE.md's fixed retention requirement). This is the same store-and-forward mechanism as 1:1 messaging, productized around absence rather than presence (V2_FEATURES §E). **UX constraint:** TTL expiry is physics, not a settings toggle — a message that ages out is gone, and the app should frame that honestly as an ephemerality guarantee ("this message provably could not still exist anywhere on the mesh") rather than apologize for it as a limitation.

### Crossed-paths radar (zero connections)
See, with zero connections and zero payload exchange, "3 of your contacts are within radio range right now" and "you crossed paths with Dmitri today at ~14:20" — purely from passive scanning of rotating ephemeral beacon IDs that only your contacts can resolve (V2_FEATURES §C). **UX constraint:** this is a real-time-feeling feature layered on a store-and-forward protocol — it works because beacons are the one plane that doesn't wait for a connection. It's also the feature most likely to surprise users if not explained: "your contacts appear like a radar" needs an explicit onboarding moment, because passive presence detection can otherwise read as unsettling rather than delightful.

### Message journey map
Watch a sent message's actual physical life — created, stored, handed off to carrier after carrier, receipt returning — as an animated constellation, powered by the diag journey log (V2_FEATURES §B). No centralized messenger can show this because centralized delivery has no journey to show. **UX constraint:** this feature only becomes visually interesting with real multi-hop radio contact, which needs Phase A's radio wiring at minimum and is genuinely compelling once Phase B's multi-device field data exists. It also doubles as the most honest possible explanation of why delivery took eleven minutes instead of one.

### Mesh vitality widget
A home-screen widget showing "mesh around you: ~40 nodes, healthy" — the population estimate N̂, neighbor count, and node tier, all numbers the router already computes for its own use (V2_FEATURES §J). **UX constraint:** N̂ is a local, probabilistic estimate, not a census — the widget should communicate "approximately" rather than a precise headcount, and should be honest that it reflects radio-range density, not total app userbase.

### Relay karma dashboard
"Your phone delivered 14 messages for strangers today; the mesh carried 6 for you." Every node already counts bundles carried for others and airtime given/received (V2_FEATURES §F). **UX constraint:** this is explicitly framed in V2_FEATURES as an open experiment ("doc 15 R12 asks whether reciprocity visibility suffices to keep relays enabled — instrumenting it in the app *is* the experiment"), not a proven retention mechanic. Ship it, but don't over-promise its effect on relay participation before there's field data.

### Panic wipe + duress profile
Because identity is a single 32-byte seed and the store is disposable-by-design (drop-and-redownload — doc 04 §7), a panic wipe (delete seed + store) makes the identity verifiably cease to exist with nothing left to seize, and a duress profile (a second seed = a fully separate identity, indistinguishable on-air) gives a user under coercion a clean fallback (V2_FEATURES §H). **UX constraint:** this feature's trustworthiness depends entirely on platform secret storage discipline — the seed must live in Android Keystore / iOS Keychain, never in app-readable storage, which is a Phase B integration requirement (`MOBILE_INTEGRATION.md` §1's "identity secrets at rest" seam), not just an app-layer toggle. Promising panic wipe before that integration is complete would be dishonest.

### Sneakernet / any-channel import-export
Export mesh state to a file, a USB stick, an animated QR sequence scanned by another phone, an NFC tap, a shared cloud folder — because every bundle is self-verifying (inline origin key, domain-separated signature, canonical bytes) and self-deduplicating (SeenIndex by content address), ingest genuinely does not care where the bytes came from (V2_FEATURES §A). This is arguably the single most novel end-user demo: "we passed an encrypted group chat through a piece of paper." **UX constraint:** none at the protocol level — this already works today in the sense that the guarantee is real. What's missing is a UI affordance (an "export/import bundle" screen), which can ship as soon as there's a store to export, i.e. Phase A.

### File/photo sharing with Reed-Solomon resilience
Share files with symbol-level erasure coding, so a photo can be reconstructed even if some carrier nodes never deliver their share — the manifest schema, `no_std` GF(256) systematic Reed-Solomon codec, class-4 symbol orchestration, bounded origin-targeted requests, and receive-side serving are implemented in the workspace (`mmp-rs-code`, `mmp-types`, `mmp-node`). The lossy-relay simulator regression demonstrates reconstruction and deduplication. **UX constraint:** this is implemented but still bounded: per-domain airtime accounting, streaming symbol production, non-origin partial-cache request policy, group file sharing, and automatic thumbnails remain open. The honest file-size ceiling is small (single-digit MiB) until the Wi-Fi warp-contact v2 feature (V2_FEATURES Part I item 1, R5) raises it.

### Broadcast/event channels
A festival or conference prints a QR containing a GroupID and sender key; everyone who scans it hears the organizer's announcements carried peer-to-peer through the crowd itself, no infrastructure or signup required — because `dest` is any 16-byte scope and spray scales with density exactly where infrastructure collapses (V2_FEATURES §I). **UX constraint:** the underlying mechanism (group scopes) is Phase-A-ready, but a polished "scan to join a broadcast channel" flow with appropriate read-only member semantics (attendees shouldn't reply-all into the organizer channel by default) is a UX design task, best targeted for Phase B alongside other mobile-alpha polish.

### Nickname/avatar gossip directory
Profile updates (nickname/avatar claims) spread as ordinary low-priority bundles through the same spray fabric that carries messages, so contact discovery keeps working without any two specific people meeting — an organic, serverless, fully pseudonymous-until-verified social directory of your radio neighborhood (V2_FEATURES §G). **UX constraint:** claims are single-writer LWW with deterministic tie-break; a genuine key-compromise scenario surfaces as `CompromiseWarning` (equal-sequence conflicting claims), which `MOBILE_INTEGRATION.md` marks as a **normative, non-dismissable banner** (doc 07 §6.4) — the app must not soften or hide this warning, even though it will occasionally fire on benign clock skew rather than actual compromise.

---

### On phasing
Everything above is mechanically ready or nearly ready at Phase A (protocol correctness already verified in `mmp-sim`/`mmp-node` today, per `docs/ROADMAP_TO_PRODUCTION.md` §1) — what Phase A actually gates is *real radio contact*, without which none of these features have anything to talk to. Phase B gates the features that need platform secret storage or sustained multi-device field behavior (panic wipe, duress profile, journey map, karma dashboard). Phase C gates production hardening of the file/blob plane and the remaining transfer policies; initial bounded file sharing is already implemented, but is not yet a production/platform gate. Nothing in this catalog requires the v2 protocol changes cataloged in V2_FEATURES.md Part I — those (Wi-Fi warp contacts, UWB tap-to-verify, DCGKA group healing, multi-device identity, island bridges, deadline-aware contacts, self-certifying revocation) are upgrades to features already listed here, not prerequisites for them.
