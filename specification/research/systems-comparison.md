# Systems Comparison — Prior Art Survey (Track A)

Date: 2026-06-10. Confidence tags: **H**igh (peer-reviewed or primary spec), **M**edium (project docs / multiple secondary sources), **L**ow (single secondary source, needs verification). *Rev. B (2026-07-03): 2024–26 landscape and file-support addendum appended at the end (Track K citations → research/bibliography.md).*

## Summary Matrix

| System | Class | Transport | Routing | Identity | Offline/DTN | Key strength | Key weakness | Scale evidence |
|---|---|---|---|---|---|---|---|---|
| Briar/Bramble | Messenger | BT, WiFi, Tor, USB | Direct-contact sync only (no multi-hop relay between strangers) | Public key + out-of-band QR verify | Yes (BSP store-and-forward to contacts) | Mature threat model; BTP forward secrecy via key rotation (H) | No stranger relaying → no true mesh; contact-only topology limits reach (H) | Small groups only (M) |
| Bridgefy | Messenger | BLE | Flooding | Phone-derived | Partial | Mass-market UX | **Broken twice** (CT-RSA'21, USENIX Sec'22): tracking, impersonation, network-wide DoS via one crafted message; Signal adoption didn't fix metadata (H) | Collapsed under analysis (H) |
| bitchat (2025) | Messenger | BLE | Flooding + cache | Key-based, weak binding | Partial (cache) | Demonstrated BLE mesh demand | Cache poisoning + replay; optional signatures covering only payload; identity from packet fields; static session keys (M; Trail of Bits + independent reports) | Unknown (L) |
| Meshtastic | LoRa mesh | LoRa | Managed flooding, hop limit ≤7 (default 3) | Node ID | Limited store | Simple, robust at small scale | Delivery collapses at scale: <10% delivery at 1000+ nodes; hop limit is a delivery ceiling; broadcast storms in dense areas (M) | Good <100, fails ~1000 (M) |
| Serval/Rhizome | Mesh platform | WiFi adhoc | Epidemic DTN (bundle-based) | SID (public key), DNA name resolution | Yes (every node stores bundles) | Pioneer of phone DTN bundles; prioritized store | Epidemic = unbounded replication cost; WiFi adhoc no longer accessible on modern phones; project dormant (M) | Disaster-scale pilots only (M) |
| Reticulum | Network stack | Any (LoRa/serial/TCP/WiFi) | Next-hop crypto "paths"; Transport nodes announce-based; no global topology knowledge | 512-bit EC keyset (X25519/Ed25519); address = hash of pubkey | Yes (link-layer agnostic, works at 5 bps) | Closest existing design to our goals: crypto-native addressing, transport agnosticism, works on tiny links (M) | Python reference impl (not `no_std`); announce propagation cost at scale unquantified in literature; no formal security analysis published (M) | No published large-scale eval (L) |
| Yggdrasil | Overlay routing | IP overlay | Spanning-tree + DHT distance labels | Key = address | No (assumes connectivity) | Compact routing state | Root-node sensitivity; convergence delays under churn; no metadata resistance; assumes mostly-connected graph — wrong model for sparse BLE meshes (M) | Internet-overlay scale, not DTN (M) |
| cjdns | Overlay routing | IP overlay | DHT → later supernodes (partial centralization) | Key = IPv6 addr | No | Crypto addressing pioneer | DHT lookup proved a poor fit for nearest-peer discovery → retreated to supernodes (M) | Stalled (M) |
| Bluetooth Mesh (SIG) | IoT mesh std | BLE adv bearer | Managed flooding, TTL ≤127 | Provisioned keys (central provisioner) | No (msgs not stored) | Standardized, hardware support; friendship feature for LPNs | Broadcast storms in dense nets (Rondón et al.); provisioning is centralized — wrong trust model; tiny payloads (11-byte access PDU segments); relays must be mains-powered in practice (H) | Reliability degrades with density (H, simulations) |
| DTN BPv7 (RFC 9171) | DTN std | Any (CLA abstraction) | External (routing not in spec) | EID URIs | Yes — the canonical store-and-forward design | Bundle/block format, lifetimes, custody concepts proven in space (ION on ISS 2012–2019) | Custody transfer removed from BPv7 core (timer estimation hard); EIDs/CBOR overhead heavy for 31-byte BLE adv; no native crypto identity (H) | Space + sparse nets (H) |
| IPFS/libp2p | Content P2P | TCP/QUIC | Kademlia DHT + gossipsub | PeerID = key hash | No | Modular transports; gossipsub episub research; content addressing | DHT random-walks and provider records are battery-hostile on mobile; assumes always-on connectivity; heavy handshakes (M) | Internet-scale but not DTN (M) |
| SSB | Social P2P | TCP/LAN | Gossip along social graph | Key = feed ID | Yes (append-only log replication) | Single-writer append-only logs make anti-entropy trivial (just seq numbers); social replication = natural spam/Sybil resistance (H, Kermarrec et al.) | Log immutability → unbounded growth, no deletion; full-feed replication wasteful; onboarding hard (H) | Thousands of users (H) |
| Matrix | Federated chat | HTTPS | Server federation | Server-scoped IDs | No (server dependent) | Room-DAG eventual consistency model is instructive | Metadata exposed to every homeserver in room; P2P Matrix still research (ReP2P, CoNEXT'24 wksp) (M) | Federation scale (M) |
| Nostr | Relay-based social | WSS to relays | None (client multi-homing) | secp256k1 keypair | No | Identity = key, simplest possible; censorship resistance via relay choice | Not P2P at all — relays are infrastructure; no relay federation; reply threads need shared relay (M) | Relay-bound (M) |
| Automerge/Yjs | CRDT libs | n/a | n/a | n/a | Yes (by design) | Mature operation-based CRDTs, efficient binary encodings (Yjs/Automerge 2) | General rich-text CRDTs are overkill + RAM-heavy for IoT chat logs; covered further in Track D | n/a |

## Lessons Inherited / Mistakes Avoided

**Inherit:**
1. **Reticulum**: identity = hash(pubkey) as address; transport-agnostic interface layer; design for 5 bps floors. (Closest philosophical ancestor.)
2. **SSB**: single-writer append-only logs → anti-entropy reduces to (feed_id, seq) vectors; social-graph-weighted replication doubles as Sybil/spam defense — directly matches our web-of-trust moderation requirement.
3. **DTN BPv7**: bundle lifetime (TTL) + block structure; the *concepts* of custody without the timer machinery.
4. **Bluetooth Mesh**: friendship concept (low-power node + friend node buffering) maps perfectly onto IoT-sensor + phone pairing; managed-flooding TTL discipline.
5. **Briar**: transport-keyed forward secrecy (rotating session keys per transport); out-of-band verification UX.
6. **Serval/Rhizome**: bundle prioritization under storage quota.

**Avoid:**
1. **Bridgefy's sins**: identity not bound to crypto keys; unauthenticated headers; parsing untrusted input without hardening; "we added Signal" ≠ secure protocol (metadata, DoS remain). Entire packet must be authenticated, not just payload (also bitchat's flaw).
2. **Meshtastic/BT-Mesh flooding at scale**: pure/managed flooding has a delivery cliff in dense or large networks. Flooding acceptable only as a *bounded, small-scale or control-plane* mechanism.
3. **cjdns DHT**: DHT lookups for peer/route discovery don't work on sparse, churning, battery-bound networks.
4. **SSB unbounded logs**: must have TTL/expiry + deletion from day one.
5. **Bluetooth Mesh centralized provisioning**: trust must be establishable peer-to-peer.
6. **Yggdrasil root sensitivity**: no global coordinator/root nodes; degrade gracefully instead.

## Notable adjacent work
- "Strong Anonymity for Mesh Messaging" (arXiv 2207.04145) — explores the anonymity-overhead tradeoff we explicitly de-scoped (nation-state tier).
- ReP2P Matrix (CoNEXT-24 workshop) — decentralized relays for Matrix; confirms federation→P2P retrofit is hard.
- Ember (arXiv 2603.16735) — E2EE messaging over IPv6 mesh (Yggdrasil-class), assumes connected overlay.

## Extension-phase addendum (rev. B, 2026-07-03) — 2024–26 landscape & file support

Supports doc 16's landscape claim (§1.3): a signed, two-tier (eager-preview / lazy-payload), content-addressed file manifest over a DTN blob plane is presently unoccupied ground. Citations [K*] in `research/bibliography.md` Track K.

| System (2024–26 state) | Class | File support | Async/DTN file delivery | Preview tier | Integrity/attribution binding | Evidence |
|---|---|---|---|---|---|---|
| bitchat v1.5.3 (2026-06) | BLE mesh messenger | **None** — text+LZ4 only, no file/image/attachment transfer | n/a | n/a | n/a | [K14] H (directly fetched README) |
| Reticulum LXMF / Sideband | DTN messaging over Reticulum | Whole-file inline attachments (`FIELD_FILE_ATTACHMENTS`) | Yes (LXMF propagation nodes) | **No** — full file or nothing | Message-level crypto; no content-addressed per-file manifest | [K15] M |
| Meshtastic 2.x (→2.7.x, 2026-03) | LoRa mesh | **None native**; 3rd-party ZModem direct transfer only | No (direct link only for the 3rd-party path) | n/a | n/a | [K16] M |
| AirDrop / Quick Share | Synchronous local transfer | Full files, mature UX | **No — synchronous only**, both parties present | Live preview in offer UI | Contact-hash gating; offer parsing is a documented pre-auth vuln surface (6 vulns incl. plist recursion) | [K8][K9][K10] H |
| Off-grid civilian emergency comms (Zürich/St. Gallen, 2025) | LoRa+BLE emergency mesh | **None** — messaging only (92% PDR at 1.2 km urban) | Messages yes, files no | n/a | n/a | [K11] M |
| Dual-radio BLE+LoRa hierarchical mesh (2026) | Emergency mesh architecture | **None** — architecture paper, no file model | n/a | n/a | n/a | [K12] M (abstract-level) |
| **MMP/1 + doc 16 (this project)** | BLE DTN mesh | Photos/documents ≤8 MiB, albums ≤16 | **Yes** — store-carry-forward blob plane, partial caches servable | **Yes** — separate ≤32 KiB thumbnail root, eager ≤1 MiB auto-fetch class | Signed `FileManifest` binds name↔thumb↔file to a non-repudiable sharer key; per-symbol Merkle verify-before-store | docs 16, 08 §8 |

Reading of the column that matters: every deployed system either has no file transfer at all, requires synchronous co-presence, or attaches whole files with no preview tier — none combines *asynchronous DTN delivery* with a *signed eager-preview/lazy-payload split*. The two historical systems that pioneered eager-metadata/lazy-payload over opportunistic contacts (Haggle [K1]–[K3], PodNet [K4]) left both tiers unsigned.
