# MMP — Product Feature Map: v2 Directions & Hidden Powers of v1

Two lists with different rules. **Part I** proposes protocol-level features for MMP/2 that materially improve user experience (each traced to the research agenda in `docs/research/open-questions.md`, so engineering and research stay honest with each other). **Part II** is the more interesting list: non-obvious features that need **zero wire changes** — they fall out of properties the v1 protocol already guarantees, and several of them are "wow"-grade demos.

---

## Part I — v2 protocol features (wire/protocol work required)

### 1. Warp contacts: Wi-Fi burst transport (R5)
When two phones meet and both support Wi-Fi Aware / Wi-Fi Direct, escalate the BLE session to a ~250× faster side-channel for the blob plane. UX effect: a shared photo album that would trickle over BLE for minutes lands in seconds, with a visible "warp" moment ("near Alice — album synced in 4 s"). The TAL seam (doc 03 §7) takes a second transport unchanged; the v2 work is negotiation (a HELLO capability bit) and the energy policy of *when* a 250× contact is worth the Wi-Fi power burn. This single feature raises the honest file-size ceiling from 8 MiB to hundreds.

### 2. Tap-to-verify with UWB distance bounding (R6)
Replace the QR verification ceremony with "hold your phones together": UWB ranging proves the peer is physically *here* (relay attacks are excluded by physics), then exchanges identity claims automatically. Verification — the protocol's weakest UX moment — becomes its most delightful. Falls back to QR everywhere UWB is absent.

### 3. Zero-cost presence via extended advertising (R7)
BLE 5 extended advertisements carry 254 B — enough for a full receipt-filter summary instead of a 12-byte fragment. Between capable pairs, the 20 KiB SUMMARY phase drops to zero, which converts three-second corridor contacts (today: handshake + summary, no payload) into contacts that actually deliver messages. Purely additive; legacy 31-byte beacons remain the floor.

### 4. Groups that heal: DCGKA epoch agreement (R10)
v1 sender-keys concede post-compromise security within an epoch and make concurrent admin changes merge crudely. Mapping DCGKA-class group key agreement onto the existing conversation DAG + OR-Set gives v2 groups PCS and clean concurrent-admin semantics — invisible cryptography, but the user-visible payoffs are "remove takes effect faster and provably" and admin succession (the #1 group-management complaint v1 will generate; OQ-8).

### 5. Multi-device identity (R13)
One identity, phone + laptop + tablet: per-device subkeys certified by an identity root key, ratchet state synchronized through the owner's devices as a private mini-mesh. This is the single most-requested feature class in every messenger; in a mesh it's harder (no server to arbitrate) and therefore also a differentiator.

### 6. Island bridges (R14)
Optional, explicitly-marked gateway nodes that tunnel bundles between mesh islands over the Internet (a TCP TAL — technically trivial). UX: "your neighborhood mesh reaches the one across town." Gated on the privacy work (R8) because gateways concentrate metadata; v2 should ship it with sealed-origin bundles or not at all.

### 7. Deadline-aware contacts (R16)
Estimate residual contact time (RSSI slope now; BLE 6 Channel Sounding where available) and re-plan the sync phase order against the deadline — "we have ~4 s: skip relay inventory, deliver the user's message first." Users experience it as the mesh feeling *smart* in motion: walking past someone still delivers.

### 8. Self-certifying revocation (R20)
A gossiped "this key is retired" claim with LWW-tombstone semantics, generalizing doc 18 §6 to identity/exchange/admin keys. UX: a lost phone stops being a permanent ghost identity; the successor key inherits the petname graph via counter-signed succession claims.

**Priority recommendation for v2:** 1 → 3 → 7 first (they compound: faster contacts × cheaper summaries × deadline scheduling = the mesh feels 10× more alive with no new crypto risk), then 4/5 (crypto-review-gated), then 2, 8, 6.

---

## Part II — hidden powers of v1 (no wire changes — build these NOW)

These exploit guarantees the protocol already pays for. Most teams never notice they own them.

### A. Any-channel transport: the mesh works over *anything*
The deepest hidden feature. Every bundle is **self-verifying** (inline origin key, domain-separated signature, canonical bytes — doc 03 §4.1.2) and **self-deduplicating** (SeenIndex by content address). Ingest doesn't care where bytes came from. Therefore *any* byte channel is a valid MMP transport with zero protocol work:

- **QR-relay**: render a bundle as an animated QR sequence; another phone scans it. Two users behind a fully-jammed radio environment can pass a signed, encrypted message *through a window*.
- **Sneakernet**: "export mesh state to USB stick / file share" — a `.mmp` file of bundles that anyone can carry, import, and re-spray. A bus driver's phone becomes a scheduled ferry between neighborhoods without knowing or reading anything it carries.
- **Audio coupling, NFC taps, shared cloud folders** — all equally valid. Verify-before-store makes injection attacks pointless; the SeenIndex makes duplicates free.

Demo value: enormous. "We passed an encrypted group chat through a *piece of paper*" is a launch-day story.

### B. The message journey map ("where has my message been?")
The diag journey log (doc 12 §7.7) records created → stored → handed-off(peer, budget) → receipt-seen per bundle. Render a sent message's life as an animated constellation: copies splitting (binary spray!), hopping between anonymized carriers, one path lighting up when the receipt returns. No other messenger *can* show this — centralized ones have no journey. This converts the protocol's biggest UX liability (minutes-tier latency) into its most fascinating screen.

### C. Crossed-paths radar (presence without connection)
Beacons carry rotating `ephem_id`s only resolvable by peers who already know your identity key (doc 03 §5.1.1). So the app can show — with zero connections, zero payload exchange, pure passive scanning — "3 of your contacts are within radio range" and "you crossed paths with Dmitri today at ~14:20". Strangers see nothing (unlinkable pseudonyms); your contacts appear like a radar. This is privacy-preserving proximity sociality that BLE messengers ship whole products around, and v1 gets it for free from the ephem-id design.

### D. Cryptographic ✓✓ (receipts as proof, not vibes)
The delivery receipt is a MAC only the true recipient's conversation key can mint (doc 08 §7). Surface it as such: tap ✓✓ → "delivery proven cryptographically at 17:42, via 3 intermediate carriers". In contested environments (protests, disasters — the core use cases) *provable* delivery is a feature users will trust with real decisions. No wire change; just UI honesty about what the protocol already guarantees.

### E. Dead-drop messages & time capsules
Store-and-forward semantics mean you can message someone who is *not there and won't be for hours* — the mesh itself holds the message (72 h TTL, spray-distributed). Productize the mental model: "leave a message at the festival for whoever of my friends arrives" (group message to a scope), "time capsule: this expires in 72 h by physics, not policy". TTL-by-design is also an *ephemerality guarantee* — messages provably age out of the network everywhere, which is a privacy feature Snapchat can only simulate.

### F. Mesh karma (the relay dashboard)
Every node already counts bundles carried for others, airtime given/received, deliveries it enabled (diag counters + ContactGraph stats). Surface it: "your phone delivered 14 messages for strangers today; the mesh carried 6 for you." Reciprocity visibility is the cheapest known lever for keeping relays enabled (doc 15 R12 asks whether it suffices — instrumenting it in the app *is* the experiment), and it makes the invisible protocol feel like a living commons.

### G. Offline-first profile gossip = contact discovery that works in airplane country
Nickname/avatar claims spread as ordinary bundles (class 3, 7-day TTL) through the same spray fabric. Consequence: profile updates propagate through the neighborhood *without any two specific people meeting*. The UI can show "profiles that reached you through the mesh" — an organic, serverless social directory of your radio neighborhood, fully pseudonymous until someone chooses to verify.

### H. Panic ergonomics
Because identity = one 32-byte seed and the store is disposable-by-design (drop-and-redownload, the network is the backup — doc 04 §7): **panic wipe** (delete seed + store = the identity verifiably ceases to exist, nothing to seize), **duress profile** (second seed = a clean parallel identity, indistinguishable on-air thanks to pseudonymous beacons), and **paranoid mode** (uniform quotas flag, doc 05 §12 — removes the WoT timing side-channel). All three are configuration, not protocol.

### I. Group broadcast channels for events
`dest` is any 16-byte scope. A festival/conference can print a QR containing a GroupID + sender key: everyone who scans it hears the organizer's announcements *carried peer-to-peer through the crowd itself* — no infrastructure, no signup, and the crowd is the CDN (spray scales with density exactly where infrastructure collapses). This is the "10,000 people, zero bars, everyone gets the schedule change" demo.

### J. The mesh vitality widget
`N̂` (the population estimate), neighbor count, tier, and store occupancy are already computed for routing. A home-screen widget — "mesh around you: ~40 nodes, healthy" — turns an invisible protocol into ambient reassurance, and doubles as the growth loop: watching the number climb as friends install is the referral mechanic.

---

### The one-sentence pitch for Part II
v1 already paid — in signatures, content addressing, TTLs, and local-only routing state — for transport-agnostic bundles, provable delivery, passive friend radar, and honest ephemerality. **Shipping Part II is extracting interest on security costs already sunk.**
