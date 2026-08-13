# Bibliography

All sources cited anywhere in this project. Accessed 2026-06-10 unless noted. Confidence: H = peer-reviewed/primary spec, M = project docs/official, L = secondary.

## Track A — Existing systems

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| A1 | Albrecht, Blasco, Jensen, Mareková. "Mesh Messaging in Large-scale Protests: Breaking Bridgefy." CT-RSA 2021 | https://eprint.iacr.org/2021/214.pdf | Paper | H |
| A2 | Albrecht et al. "Breaking Bridgefy, again: Adopting libsignal is not enough." USENIX Security 2022 | https://www.usenix.org/conference/usenixsecurity22/presentation/albrecht | Paper | H |
| A3 | Rondón, Mahmood, Grimaldi, Gidlund. "Understanding the Performance of Bluetooth Mesh: Reliability, Delay, and Scalability Analysis." IEEE IoT Journal 2020 | https://arxiv.org/pdf/1910.03345 | Paper | H |
| A4 | "Demystifying the Performance of Bluetooth Mesh" | https://arxiv.org/pdf/2106.04230 | Paper | H |
| A5 | Baumgärtner et al. / Serval: "The Serval Mesh: A platform for resilient communications in disaster & crisis" (Gardner-Stephen et al.) | https://www.researchgate.net/publication/260165109_The_serval_mesh_A_platform_for_resilient_communications_in_disaster_crisis | Paper | H |
| A6 | Serval Rhizome technical docs | https://developer.servalproject.org/dokuwiki/doku.php?id=content:tech:rhizome | Project docs | M |
| A7 | RFC 9171 — Bundle Protocol Version 7 | https://www.rfc-editor.org/rfc/rfc9171.html | RFC | H |
| A8 | "Custody Transfer and Compressed Status Reporting for Bundle Protocol Version 7" | https://arxiv.org/html/2507.17403v1 | Paper | H |
| A9 | Kermarrec, Lavoie, Tschudin. "Gossiping with Append-Only Logs in Secure-Scuttlebutt." DICG 2020 | https://dicg2020.github.io/papers/kermarrec.pdf | Paper | H |
| A10 | Reticulum Manual (Mark Qvist), v1.3.4 | https://reticulum.network/manual/understanding.html | Project docs | M |
| A11 | Briar — Bramble protocol suite docs / project | https://briarproject.org/ | Project docs | M |
| A12 | Song, Y. "Cryptography in the Wild: Briar." ETH Zürich semester project report, 2023 | https://ethz.ch/content/dam/ethz/special-interest/infk/inst-infsec/appliedcrypto/education/theses/report_YuanmingSong.pdf | Report | M |
| A13 | Meshtastic mesh broadcast algorithm docs | https://meshtastic.org/docs/overview/mesh-algo/ | Project docs | M |
| A14 | Meshtastic blog: "Why Meshtastic Uses Managed Flood Routing" | https://meshtastic.org/blog/why-meshtastic-uses-managed-flood-routing/ | Project docs | M |
| A15 | Trail of Bits: "Building secure messaging is hard: A nuanced take on the Bitchat security debate" (2025-07-18) | https://blog.trailofbits.com/2025/07/18/building-secure-messaging-is-hard-a-nuanced-take-on-the-bitchat-security-debate/ | Vendor analysis | M |
| A16 | "BitChat cache poisoning and replay in Bluetooth mesh" (Barghest) | https://barghest.asia/blog/bitchat-cache-poisoning/ | Analysis | L→M (corroborated by A15) |
| A17 | bitchat protocol security report, GitHub issue #376 | https://github.com/permissionlesstech/bitchat/issues/376 | Issue report | M |
| A18 | Yggdrasil Network — about & addressing design notes | https://yggdrasil-network.github.io/about.html | Project docs | M |
| A19 | libp2p gossipsub v1.0 spec | https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md | Spec | M |
| A20 | "An Empirical Analysis of the Nostr Social Network" | https://arxiv.org/html/2402.05709v2 | Paper | H |
| A21 | ReP2P Matrix: Decentralized Relays (ACM CoNEXT 2024 workshop) | https://dl.acm.org/doi/abs/10.1145/3694809.3700741 | Paper | H |
| A22 | "Strong Anonymity for Mesh Messaging" | https://arxiv.org/pdf/2207.04145 | Paper | H |
| A23 | "BLE Mesh Networks: Survey of Communication and Security Protocols" (Sensors, 2020) | https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7349184/ | Survey | H |
| A24 | "The Bluetooth Mesh Standard: An Overview and Experimental Evaluation" (Sensors, 2018) | https://pmc.ncbi.nlm.nih.gov/articles/PMC6111614/ | Paper | H |

## Track B — BLE physical & OS constraints

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| B1 | Memfault Interrupt: "A Practical Guide to BLE Throughput" | https://interrupt.memfault.com/blog/ble-throughput-primer | Eng. analysis | M |
| B2 | Spörk, Boano et al. "Performance and Trade-offs of the new PHY Modes of BLE 5" (EWSN/related) | https://www.carloalbertoboano.com/documents/spoerk19ble5phy.pdf | Paper | H |
| B3 | Rutronik whitepaper: "Determination of practical extremes of Bluetooth Low Energy" | https://www.rutronik.com/fileadmin/Rutronik/Downloads/printmedia/products/06_wireless/bluetooth5.pdf | Whitepaper | M |
| B4 | Apple: Core Bluetooth Background Processing for iOS Apps | https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html | Vendor docs | H |
| B5 | AOSP: Bluetooth Low Energy advertising | https://source.android.com/docs/core/connect/bluetooth/ble_advertising | Vendor docs | H |
| B6 | Nordic: "Bluetooth 5 Advertising Extensions" | https://blog.nordicsemi.com/getconnected/bluetooth-5-advertising-extensions | Vendor docs | M |
| B7 | Novel Bits: "Bluetooth 5 Speed: PHY, DLE, MTU" | https://novelbits.io/bluetooth-5-speed-maximum-throughput/ | Eng. analysis | M |
| B8 | freeCodeCamp: "How Does Extended Bluetooth Advertising Work in AOSP?" | https://www.freecodecamp.org/news/how-does-extended-bluetooth-advertising-work-in-aosp/ | Eng. analysis | L→M |
| B9 | "Android BLE Background Scanning Limits & Production Fixes" (devsflow) | https://www.devsflow.ca/blog/ble-android-lessons.html | Eng. analysis | M (corroborated B10) |
| B10 | Punch Through: "Troubleshooting Android BLE Scan Errors" | https://punchthrough.com/android-ble-scan-errors/ | Eng. analysis | M |
| B11 | David G. Young: "Hacking The Overflow Area" + ios-overflow-area repo | https://davidgyoungtech.com/2020/05/07/hacking-the-overflow-area | Reverse eng. | M |
| B12 | Shrestha: "Measurement of power consumption of BLE" | https://files01.core.ac.uk/download/pdf/80990492.pdf | Thesis | M |
| B13 | "Energy Modeling of Neighbor Discovery in Bluetooth Low Energy Networks" (Sensors) | https://pmc.ncbi.nlm.nih.gov/articles/PMC6891668/ | Paper | H |
| B14 | "Precise Energy Modeling for the Bluetooth Low Energy Protocol" | https://arxiv.org/pdf/1403.2919 | Paper | H |
| B15 | "The Bluetooth Mesh Standard: An Overview and Experimental Evaluation" — PER degradation with redundant relays | https://pmc.ncbi.nlm.nih.gov/articles/PMC6111614/ | Paper | H |
| B16 | "Multi-Hop Real-Time Communications Over BLE Industrial Wireless Mesh Networks" | https://www.researchgate.net/publication/325024654_Multi-Hop_Real-Time_Communications_Over_Bluetooth_Low_Energy_Industrial_Wireless_Mesh_Networks | Paper | H |
| B17 | "A Comprehensive Evaluation of Bluetooth Low Energy Mesh" (ICPADS'24) | https://dang.fan/publication/icpads24-ble-mesh-evaluation/icpads24-ble-mesh-evaluation.pdf | Paper | H |

## Track C — DTN / opportunistic routing

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| C1 | "Performance Comparison of Epidemic, PRoPHET, Spray and Wait, Binary Spray and Wait, and PRoPHETv2" | https://www.academia.edu/9001708/Performance_Comparison_of_Epidemic_PRoPHET_Spray_and_Wait_Binary_Spray_and_Wait_and_PRoPHETv2 | Paper | M |
| C2 | "A Survey of Delay Tolerant Networks Routing Protocols" | https://arxiv.org/pdf/1210.0965 | Survey | H |
| C3 | "Analytical Study of Spray and Wait Routing Protocol in DTN" (incl. Spyropoulos et al. SIGCOMM WDTN'05 lineage) | https://www.researchgate.net/publication/316513321_ANALYTICAL_STUDY_OF_SPRAY_AND_WAIT_ROUTING_PROTOCOL_IN_DELAY_TOLERANT_NETWORK | Paper | M |
| C4 | "Enhanced Prophet Routing Protocol in Delay Tolerant Networks" | https://www.researchgate.net/publication/325463483_Enhanced_Prophet_Routing_Protocol_in_Delay_Tolerant_Networks | Paper | M |
| C5 | Hui, Crowcroft, Yoneki. "BUBBLE Rap: Social-based Forwarding in Delay Tolerant Networks" (MobiHoc'08 / IEEE TMC) | https://www.cl.cam.ac.uk/~jac22/camhor/bubble_tmc-sub.pdf | Paper | H |
| C6 | "Beyond Traditional DTN Routing: Social Networks for Opportunistic Communication" | https://arxiv.org/pdf/1110.2480 | Survey | H |
| C7 | "Recent progress in routing protocols of mobile opportunistic networks: A clear taxonomy, analysis and evaluation" (JNCA 2016) | https://www.sciencedirect.com/science/article/abs/pii/S1084804516000084 | Survey | H |
| C8 | "Survey on Opportunistic Routing for Delay/Disruption Tolerant Networks" | https://www.researchgate.net/publication/320347333_Survey_on_Opportunistic_Routing_for_DelayDisruption_Tolerant_Networks | Survey | M |
| C9 | RFC 6693 — Probabilistic Routing Protocol for Intermittently Connected Networks (PRoPHET) | https://www.rfc-editor.org/rfc/rfc6693.html | RFC | H |

## Track D — Sync & consistency

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| D1 | "Approaches to Conflict-free Replicated Data Types" (ACM Computing Surveys) | https://dl.acm.org/doi/10.1145/3695249 | Survey | H |
| D2 | Almeida, Shoker, Baquero. "Efficient State-based CRDTs by Delta-Mutation" | https://arxiv.org/pdf/1410.2803 | Paper | H |
| D3 | Almeida, Shoker, Baquero. "Delta State Replicated Data Types" (JPDC) | https://arxiv.org/pdf/1603.01529 | Paper | H |
| D4 | "Delta-State-Based Synchronization of CRDTs in Opportunistic Networks" | https://hal.science/hal-03405138/document | Paper | H |
| D5 | BIP 330: Transaction announcements reconciliation (Erlay) | https://github.com/bitcoin/bips/blob/master/bip-0330.mediawiki | Spec | H |
| D6 | Naumenko et al. "Bandwidth-Efficient Transaction Relay for Bitcoin" (Erlay) | https://arxiv.org/pdf/1905.10518 | Paper | H |
| D7 | "Range-Based Set Reconciliation" (Log Periodic / Aljoscha Meyer's work) | https://logperiodic.com/rbsr.html | Analysis+paper lineage | M→H |
| D8 | Negentropy wire protocol + reference implementations | https://github.com/hoytech/negentropy | Spec/impl | M |
| D9 | "Practical Rateless Set Reconciliation" (ACM SIGCOMM 2024) | https://dl.acm.org/doi/10.1145/3651890.3672219 | Paper | H |
| D10 | Auvolat, Taïani. "Merkle Search Trees: Efficient State-Based CRDTs in Open Networks" | https://www.semanticscholar.org/paper/441e9662f8532e4dbc798a30c1af85627c623373 | Paper | H |
| D11 | Sanjuán, Pöyhtäri, Teixeira, Psaras. "Merkle-CRDTs: Merkle-DAGs meet CRDTs" | https://arxiv.org/pdf/2004.00107 | Paper | H |
| D12 | Baquero (HASlab): "Version Vectors are not Vector Clocks" | https://haslab.wordpress.com/2011/07/08/version-vectors-are-not-vector-clocks/ | Expert note | M |

## Track E — Network coding / erasure coding

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| E1 | "Improving the transport performance in delay tolerant networks by random linear network coding and global acknowledgments" (Computer Networks) | https://www.sciencedirect.com/science/article/abs/pii/S1570870513001418 | Paper | H |
| E2 | "Characterization of Random Linear Network Coding with Application to Broadcast Optimization in Intermittently Connected Networks" | https://arxiv.org/pdf/1104.3466 | Paper | H |
| E3 | "NTC: Towards Efficient Network Coding in Delay Tolerant Networks" | https://www.researchgate.net/publication/224251698_NTC_Towards_Efficient_Network_Coding_in_Delay_Tolerant_Networks | Paper | M |
| E4 | RFC 6330 — RaptorQ Forward Error Correction Scheme | https://www.rfc-editor.org/rfc/rfc6330.html | RFC | H |
| E5 | RFC Editor IPR info for RFC 6330 (Qualcomm statement) | https://www.rfc-editor.org/info/rfc6330 | IPR record | M |
| E6 | OpenRQ — open-source RFC 6330 implementation | https://openrq-team.github.io/openrq/ | Impl | M |
| E7 | Shokrollahi, Luby. "Raptor Codes" (Foundations and Trends in Communications and Information Theory) | https://www.nowpublishers.com/article/DownloadSummary/CIT-060 | Monograph | H |

## Track F — Cryptography (Phase 3)

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| F1 | Perrin, T. "The Noise Protocol Framework" (rev. spec) | https://noiseprotocol.org/noise.html | Spec | H |
| F2 | Marlinspike, Perrin. "The X3DH Key Agreement Protocol" (Signal) | https://signal.org/docs/specifications/x3dh/ | Spec | H |
| F3 | Kobeissi, Nicolas, Bhargavan. "Noise Explorer: Fully Automated Modeling and Verification for Arbitrary Noise Protocols" | https://eprint.iacr.org/2018/766.pdf | Paper | H |
| F4 | libsodium documentation: Sealed boxes (crypto_box_seal) | https://libsodium.gitbook.io/doc/public-key_cryptography/sealed_boxes | Docs | M |
| F5 | Balbás, Collins, Gajland. "Analysis and Improvements of the Sender Keys Protocol for Group Messaging" | https://arxiv.org/pdf/2301.07045 | Paper | H |
| F6 | RFC 9420 — The Messaging Layer Security (MLS) Protocol (verified: ordering nuance re-checked by fetch 2026-06-10) | https://www.rfc-editor.org/rfc/rfc9420.html | RFC | H |
| F7 | libp2p Noise spec (identity-key channel binding precedent) | https://github.com/libp2p/specs/blob/master/noise/README.md | Spec | M |

## Track B addendum (Phase 4)

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| B18 | Nordic DevZone: "Confusion: To use L2CAP vs GATT for max throughput with iOS" (incl. iOS 11 / Android 10 CoC API availability; marginal throughput delta vs tuned GATT) | https://devzone.nordicsemi.com/f/nordic-q-a/61028/confusion-to-use-l2cap-vs-gatt-for-max-throughput-with-ios | Vendor forum (engineer-answered) | M |

## Track G — Audit (post-audit revision, accessed 2026-06-11)

Sources fetched during the formal architectural audit (`AUDIT.md`) to verify facts that audit findings turned on; cited by the 2026-06-11 revision (`CHANGELOG.md`).

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| G1 | RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA). §5.1.7: verifier MUST reject `S ≥ L`; Ed25519 is non-malleable only with this canonical-S check (basis for AUDIT F9 / `verify_strict` mandate). | https://www.rfc-editor.org/rfc/rfc8032.html | RFC | H |
| G2 | openssl/openssl issue #7693 — real-world Ed25519 malleability from a missing S-range check (corroborating evidence that the §5.1.7 check is omitted by some implementations; also tinyssh #101, PuTTY `eddsa-overlarge-s`). | https://github.com/openssl/openssl/issues/7693 | Issue report | L→M |
| G3 | Silicon Labs: "Advertising Data Basics" — legacy advertising = 31 B; each AD structure costs 1 B length + 1 B type; Service Data 16-bit adds a 2 B UUID → 7 B total overhead for Flags(3)+SD-header(4) → 24 B usable payload (confirms doc 09 §3.1 beacon budget). | https://docs.silabs.com/bluetooth/latest/bluetooth-fundamentals-advertising-scanning/advertising-data-basics | Vendor docs | M |

_(Tracks H–K added in the extension-phase (rev. B) research, accessed 2026-07-02.)_

## Track H — Vehicular/train DTN & linear topologies (extension phase, accessed 2026-07-02)

Sources on vehicular/train message-ferrying, linear-topology DTN, and inter-carriage RF propagation, fetched to support the train-mesh profile (doc 17) and OQ-14.

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| H1 | Zhao, Ammar, Zegura. "A Message Ferrying Approach for Data Delivery in Sparse Mobile Ad Hoc Networks." ACM MobiHoc 2004 — canonical message-ferrying (FIMF/NIMF), non-random mobility exploited for store-carry-forward | https://www.sigmobile.org/mobihoc/2004/presentations/p187-zhao.pdf | Paper | H |
| H2 | Jawhar, Ammar, Zhang, Wu, Mohamed. "Ferry-Based Linear Wireless Sensor Networks." IEEE GLOBECOM 2013 — closest linear-topology DTN prior art; railroad/subway named; closed-form delay/contact/buffer formulas (t_fp = 2Rc/s) | https://cis.temple.edu/~wu/research/publications/Publication_files/2013globecom-Imad.pdf | Paper | H |
| H3 | Leith, Farrell. "Coronavirus Contact Tracing: Evaluating The Potential Of Using Bluetooth Received Signal Strength For Proximity Detection." arXiv:2006.06822, 2020 (later ACM CCR) — measured BLE RSSI inside train carriages: RSSI need NOT decrease with distance (metal-body reflections) | https://arxiv.org/abs/2006.06822 | Paper | M |
| H4 | Arup Alliance for UK DfT (Star Two TTWO 0036). "Mobile Connectivity in Rolling Stock — RF Attenuation Characteristics." UK Gov report, 2021 — vehicle penetration loss 3–28 dB across 5 GB carriage types incl. 2395 MHz; crowding body loss 0→~60 dB; also cites Deniau 2010 TGV: inter-car WiFi bridged at 5 GHz (802.11a), not 2.4 GHz | https://assets.publishing.service.gov.uk/media/618d299cd3bf7f055b29332f/mobile-connectivity-in-rolling-stock-radio-frequency-attenuation-characteristics.pdf | Gov measurement report | M |
| H5 | Shah, Roy, Jain, Brunette. "Data MULEs: Modeling a Three-Tier Architecture for Sparse Sensor Networks." 2003 (Ad Hoc Networks 1(2–3)) (content unverified — PDF fetched but text extraction interrupted) — foundational data-mule architecture | https://labs.ece.uw.edu/funlab/Publications/2003/data_mules.pdf | Paper | H existence / unverified content |
| H6 | Magdy, Ezzat et al. "Empirical Study of Traffic Velocity Distribution and its Effect on VANETs Connectivity." IJWMN 2015 — eq. (5): t_c = 2R/|Δv| — independent second source for contact-window formula (chain to Nagel, IEEE IV 2010) | https://arxiv.org/pdf/1502.07282 | Paper | H |
| H7 | Batabyal, Bhaumik. "Improving Network Performance with Affinity based Mobility Model in Opportunistic Network." IJWMN 2012 (formula glyphs lost in PDF extraction; qualitative confirmation only) — contact duration determined by RF range and node velocity | https://arxiv.org/pdf/1205.1867 | Paper | L |
| H8 | Westermo. "Wireless Inter-Carriage Link for Train Coupling" (vendor product page) — commercial inter-carriage bridges use 5.8/6.0 GHz directional links — industry avoids 2.4 GHz across carriage boundary | https://www.westermo.com/us/industries/train-networks/wireless-solutions/wireless-inter-carriage-link | Vendor docs | M |
| H9 | Perpetuum Ltd / LPRS case study: vibration-energy-harvesting wireless bearing monitoring (Southeastern/ScotRail fleets) — real rail wireless-sensor deployment; proprietary eRA400TRS RF, NOT BLE | https://lprs.co.uk/knowledge-centre/solutions-and-case-studies/automotive-transportation/real-time-railway-bearing-monitoring.html | Vendor case study | M |
| H10 | Industrial Networks "SMRTag/SMRTMesh" railcar sensor mesh + PR Newswire 2023-10-02 pilot (1,320 railcars, Houston) (BLE attribution is secondary/unverified) — mesh sensor network on freight railcars; battery anchors | https://www.inetlp.com/smrtag-system | Vendor docs | M (BLE claim L) |
| H11 | IEEE. "Propagation characteristic estimations of 2 GHz inter-car wireless links in high-speed train cars in a railway tunnel." 2014 (existence confirmed via search; paywalled, content inaccessible) — inter-car propagation literature exists; retrieve via library access for OQ-14 | https://ieeexplore.ieee.org/document/7026607/ | Paper | L |
| H12 | "Propagation modeling for outdoor-to-indoor and indoor-to-indoor wireless links in high-speed train." Measurement, 2017 (existence confirmed; 403 on fetch) — car-to-car measurements at 2.4/2.6/5.7 GHz exist; retrieve for OQ-14 | https://www.sciencedirect.com/science/article/abs/pii/S0263224117304001 | Paper | L |
| H13 | Ott, Kutscher. "Drive-thru Internet: IEEE 802.11b for 'automobile' users." IEEE INFOCOM 2004 (accessed 2026-07-03; metadata verified via Semantic Scholar API — DOI 10.1109/INFCOM.2004.1354509; IEEE full text paywalled) — canonical contact-window phase model (entry/production/exit zones across a roadside AP pass); closest prior art for contact-window-aware session planning (doc 15 R16, doc 17 §11, COUNCIL-REVIEW N1 disposition) | https://api.semanticscholar.org/graph/v1/paper/DOI:10.1109/INFCOM.2004.1354509 (IEEE: https://ieeexplore.ieee.org/document/1354509) | Paper | H existence / M content (phase model from secondary literature) |

## Track I — Dense-environment BLE + BLE 5.x/6.0 (extension phase, accessed 2026-07-02)

Sources on BLE performance in dense environments and BLE 5.x/6.0 features (extended advertising, PAwR, Channel Sounding), fetched to support dense-crowd scaling models (doc 13) and OQ-15.

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| I1 | Perez-Diaz de Cerio et al. "Analytical and Experimental Performance Evaluation of BLE Neighbor Discovery Process Including Non-Idealities of Real Chipsets." Sensors 2017 — non-detection probability at 200 advertisers: 12.96% (Type-1 chipsets) vs 2.52% (Type-2), 5 s window | https://pmc.ncbi.nlm.nih.gov/articles/PMC5375785/ | Paper | H |
| I2 | Pang, T'Jonck, Claeys, Pissoort, Hallez, Boydens. "BLE Interference Awareness Scheme and Improved Channel Selection Algorithm for Connection Robustness." Sensors 2021 — ~16.7% PLR under WiFi co-channel interference; <1–2.8% with interference-aware channel selection | https://pmc.ncbi.nlm.nih.gov/articles/PMC8037550/ | Paper | H |
| I3 | Bluetooth SIG. "Bluetooth Channel Sounding" feature page — Core 6.0; PBR+RTT; cm-level goal, ±20 cm early impls, ≤150 m; update rate NOT stated | https://www.bluetooth.com/learn-about-bluetooth/feature-enhancements/channel-sounding/ | SIG docs | H |
| I4 | Pozza, Georgoulas, Moessner, Nati, Gluhak, Krco. "An Arrival and Departure Time Predictor for Scheduling Communication in Opportunistic IoT." Sensors 2016 — closest N1 prior art: LSTD-learned arrival/departure prediction for discovery scheduling (temporal patterns, not radio-signal, not in-session) | https://pmc.ncbi.nlm.nih.gov/articles/PMC5134511/ | Paper | H |
| I5 | Afaneh. "PAwR Explained: Bidirectional Bluetooth LE Advertising with Responses." Novel Bits, 2023 — PAwR: 1–128 subevents, ≤255 response slots, 7.5–318.75 ms; >10k-node claim | https://novelbits.io/periodic-advertising-with-responses-pawr/ | Blog | L |
| I6 | Google AOSP docs. "Bluetooth Low Energy advertising" — Android 8.0+ extended advertising, hardware-gated; capability-check APIs | https://source.android.com/docs/core/connect/bluetooth/ble_advertising | Official docs | M |
| I7 | Coleman. "A Practical Guide to BLE Throughput." Memfault Interrupt 2019 (= B1, re-verified 2026-07-02 for Track I) — iPhone iOS10+: ≈98 kbps GATT (MTU 185, 15 ms CI, 1M PHY); ATT ceiling 226–301 kbps | https://interrupt.memfault.com/blog/ble-throughput-primer | Blog | L |
| I8 | Punch Through. "Maximizing BLE Throughput on iOS and Android." 2016 (page carries an "outdated" disclaimer) — BLE 4.x era: iPhone 6 ≈21 kbps; Nexus ≈128 kbps | https://punchthrough.com/maximizing-ble-throughput-on-ios-and-android/ | Blog | L |
| I9 | Wikipedia. "ITU model for indoor attenuation" (secondary for ITU-R P.1238) — 1.8–2.0 GHz: power-loss coefficient N=30 office/28 residential; floor loss 15+4(n−1) dB office | https://en.wikipedia.org/wiki/ITU_model_for_indoor_attenuation | Secondary | L |
| I10 | Nordic DevZone. "BLE5 extended advertising and iOS — any updates?" (~2021–22, Nordic-staff answered) — iOS has no app-level API to transmit extended advertisements | https://devzone.nordicsemi.com/f/nordic-q-a/107037/ble5-extended-advertising-and-ios--any-updates | Forum | L |
| I11 | Android Developers. "Range between devices" (RangingManager) — Android 16 Ranging module: unified API over UWB, BT Channel Sounding, WiFi NAN-RTT, BLE RSSI; initiator/responder roles | https://developer.android.com/develop/connectivity/ranging | Official docs | H |
| I12 | Android Police. "The Pixel 10 brings Bluetooth 6 and its improvements to the masses." 2025-08-21 — Pixel 10 first US phone with BT6 Channel Sounding | https://www.androidpolice.com/pixel-10-family-bluetooth-6-channel-sounding/ | Tech press | M |
| I13 | Nordic Semiconductor news. "Nordic's Android app and nRF54L Series evaluate Bluetooth Channel Sounding using the Google Pixel 10." 2025-10-28 — measured ±1 m accuracy to 20 m via Android CS API vs nRF54L15; CS back-ported to Pixel 9 (Android 16 QPR2) | https://www.nordicsemi.com/Nordic-news/2025/10/Nordics-Android-app-and-nRF54L-Series-evaluate-Bluetooth-Channel-Sounding-using-the-Google-Pixel-10 | Vendor blog | M |
| I14 | ST Community forum. "STM32WBA BLE Data Throughput example: throughput optimization." 2025 — 2M PHY + DLE, MTU 244–247: phone↔MCU 250–680 kbps (iOS ~30 ms CI vs Android ~7.5 ms); MCU↔MCU ≈950 kbps | https://community.st.com/t5/stm32-mcus-wireless/stm32wba-ble-data-throughput-example-throughput-optimization/td-p/827102 | Forum | L |

| I15 | Bluetooth SIG. "Bluetooth Core 6.0 feature overview" — Channel Sounding, Decision-Based Advertising Filtering, and Monitoring Advertisers | https://www.bluetooth.com/core-specification-6-feature-overview/ | SIG docs | H |

## Track J — Authenticated broadcast, authority & offline revocation (extension phase, accessed 2026-07-02)

Sources on authenticated broadcast, emergency-alert spoofing, offline-capable delegation/revocation, and rate-limiting tokens, fetched to support the operator-authority design (doc 18) and OQ-16/OQ-17.

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| J1 | "This Is Your President Speaking: Spoofing Alerts in 4G LTE Networks." ACM WiSec 2019 — SDR fake eNodeB delivered spoofed Presidential Alert to ~90% of phones in 50k-seat stadium sim; zero device-side crypto verification | https://dl.acm.org/doi/10.1145/3307334.3326082 (mirror: https://ericw.us/trow/lte-alerts.pdf) | Paper | H |
| J2 | FCC PSHSB. "Report: Hawaii Emergency Management Agency, Jan 13 2018 False Missile Alert." 2018 — root cause: single-operator Execute, no dual control, no drill/live distinction | https://docs.fcc.gov/public/attachments/DOC-350119A1.pdf | Regulatory report | H |
| J3 | Hasna, Chendeb, El Falou. "From Spoofing to Trust: Emergency Alerts Spoofing Testbed and Cross-Cell Verification." arXiv:2604.24404, 2026 — open 5G PWS spoofing testbed; cross-cell verification defense does NOT generalize to disconnected/DTN | https://arxiv.org/html/2604.24404v1 | Preprint | M |
| J4 | "You Have Been Warned: Abusing 5G's Warning and Emergency Systems." ACSAC 2022 (403 on fetch — content from search summary, unverified) — 3GPP PWS deliberately allows unauthenticated alert delivery | https://dl.acm.org/doi/fullHtml/10.1145/3564625.3568000 | Paper | M (unverified fetch) |
| J5 | Perrig, Song, Canetti, Tygar, Briscoe. RFC 4082 — TESLA — delayed-key-disclosure broadcast auth requires bounded receiver clock sync — structurally incompatible with multi-hop DTN delays | https://datatracker.ietf.org/doc/html/rfc4082 | RFC | H |
| J6 | "TESLA-for-5G: Broadcast Authentication for 5G Networks Using TESLA." arXiv:2606.26528, 2026 — TESLA adaptation targets live cellular, not delay-tolerant mesh | https://arxiv.org/pdf/2606.26528 | Preprint | L→M |
| J7 | IETF draft-ietf-privacypass-rate-limit-tokens-06 — closest rate-budget-token prior art; enforcement is ONLINE at a trusted Attester — opposite of DTN needs | https://datatracker.ietf.org/doc/html/draft-ietf-privacypass-rate-limit-tokens-06 | IETF draft | M |
| J8 | RFC 9171 — Bundle Protocol Version 7 (priority analysis; = A7, re-examined for Track J) — BPv7 priority classes have no sender authentication and no abuse resistance | https://www.rfc-editor.org/rfc/rfc9171.html | RFC | H |
| J9 | Simplicio, Cominetti, Kupwade Patil, Ricardini, Silva. "ACPC: Efficient Revocation of Pseudonym Certificates Using Activation Codes." Ad Hoc Networks 90, 2019 — V2X revocation via periodically-broadcast activation codes withheld from revoked vehicles — no-CRL-lookup revocation pattern | https://www.sciencedirect.com/science/article/pii/S1570870518304761 (eprint: https://eprint.iacr.org/2018/324) | Paper | H |
| J10 | Birgisson et al. "Macaroons: Cookies with Contextual Caveats for Decentralized Authorization in the Cloud." NDSS 2014 — chained-HMAC attenuation-only offline delegation; no native budget-counter primitive | https://research.google/pubs/macaroons-cookies-with-contextual-caveats-for-decentralized-authorization-in-the-cloud/ | Paper | H |
| J11 | Eclipse Biscuit project spec/docs — public-key block-chained tokens, offline attenuation-only delegation; no built-in rate-budget primitive | https://www.biscuitsec.org/ | Project docs | M |
| J12 | UCAN Working Group specification — offline-verifiable, time-bounded, attenuable capability chains for local-first/P2P; no first-class rate budget | https://ucan.xyz/specification/ | Community spec | M |
| J13 | Kuri. "Vouchsafe: A Zero-Infrastructure Capability Graph Model for Offline Identity and Trust." arXiv:2601.02254, 2026 — CA/CRL-free delegation graphs with expiry-based revocation; constraints generic, no rate-budget bound | https://arxiv.org/pdf/2601.02254 | Preprint | M |
| J14 | "NC3 When Leaders Vanish: Bounded Pre-Delegation with Conflict-Aware Legitimacy Beacons..." ResearchGate preprint ~2025-10 (403 on fetch — abstract-level only, unverified) — offline-verifiable bounded emergency authority via time+quorum, not rate budget | https://www.researchgate.net/publication/396843429 | Preprint | L |
| J15 | Teranishi, Furukawa, Sako. "k-Times Anonymous Authentication (Extended Abstract)." ASIACRYPT 2004, LNCS 3329, pp. 308–322 (accessed 2026-07-03; metadata verified via Semantic Scholar API — DOI 10.1007/978-3-540-30539-2_22; Springer full text paywalled) — cryptographically-enforced at-most-k authentications per epoch, offline traceability of over-users — the "prevention, not accounting" primitive doc 18 §5.4(a) names and deliberately does NOT adopt | https://api.semanticscholar.org/graph/v1/paper/DOI:10.1007/978-3-540-30539-2_22 (Springer: https://link.springer.com/chapter/10.1007/978-3-540-30539-2_22) | Paper | H |
| J16 | Chaum, Fiat, Naor. "Untraceable Electronic Cash." CRYPTO '88, LNCS 403, pp. 319–327 (accessed 2026-07-03; PDF fetched from chaum.com — scanned, text not machine-extracted; metadata cross-checked against Springer DOI 10.1007/0-387-34799-2_25 and ACM index) — offline e-cash where double-spending a coin cryptographically reveals the spender's identity — second named instance of offline budget-violation detection in doc 18 §5.4(a) | https://chaum.com/wp-content/uploads/2021/12/Untraceable_Electronic_Cash.pdf (Springer: https://link.springer.com/chapter/10.1007/0-387-34799-2_25) | Paper | H existence / M content (scan) |

## Track K — DTN file transfer + 2024–26 mesh landscape (extension phase, accessed 2026-07-02)

Sources on DTN file-transfer precedents, fountain-coding swarming, and the 2024–26 mesh/local-transfer landscape (AWDL, bitchat, LXMF, Meshtastic), fetched to support the file-sharing manifest design (doc 16) and OQ-18.

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| K1 | Su, Scott, Hui, Crowcroft et al. "Haggle: A Data-centric Network Architecture for Mobile Devices." ~2007–09 (venue/year from search index, not independently verified) — attribute-tagged metadata objects + interest-based fetch — manifest-model ancestor | https://www.researchgate.net/publication/234781274_Haggle_A_Data-centric_Network_Architecture_for_Mobile_Devices | Paper | M |
| K2 | Haggle reference implementation (doc/protocol.txt) — primary-source detail of Haggle data-object/metadata model | https://github.com/dmonakhov/haggle | Project docs | M |
| K3 | EPFL ICA Haggle project page — interest subscription + content delegation for altruistic forwarding | https://icawww1.epfl.ch/haggle/ | Project docs | M |
| K4 | PodNet (Legendre, Ott et al.). "Extending the reach of online social networks to opportunistic networks with PodNet." ACM MobiOpp 2010 (DOI page 403 — attribution from search index) — channel metadata pub/sub + opportunistic episode download — closest eager-metadata/lazy-payload prior art | https://dl.acm.org/doi/abs/10.1145/1755743.1755779 | Paper | M (unverified fetch) |
| K5 | NASA/CSU. "Application of Fountain Code to High-rate Delay Tolerant Networking." 2023 — modern app-layer fountain coding over DTN bundles | https://ntrs.nasa.gov/api/citations/20230007643/downloads/IEEE_Access_CSU_HDTN_2023_submission.pdf | Paper | M |
| K6 | Li, Jose, Wu. "Distributed-Fountain Network Code (DFNC) for Content Delivery in Vehicular Networks." ACM VANET Workshop 2013 — multi-source fountain-coded swarming over intermittent vehicular contacts | https://dl.acm.org/doi/pdf/10.1145/2482967.2482978 | Paper | M |
| K7 | "A Cooperative Downloading Method for VANET Using Distributed Fountain Code." Sensors 16(10):1685, 2016 — peer-reviewed cooperative fountain-code downloading | https://www.mdpi.com/1424-8220/16/10/1685 | Paper | M |
| K8 | Stute, Kreitschmann, Hollick. "One Billion Apples' Secret Sauce: Recipe for the AWDL Ad hoc Protocol." ACM MobiCom 2018 — reverse-engineered AWDL — closest deployed-at-scale local-transfer analog | https://arxiv.org/abs/1808.03156 | Paper | H |
| K9 | Stute et al. "A Billion Open Interfaces for Eve and Mallory: MitM, DoS, and Tracking Attacks on AWDL." USENIX Security 2019 — AirDrop contact-hash authorization + its leak/attack surface | https://www.usenix.org/conference/usenixsecurity19/presentation/stute | Paper | H |
| K10 | CISPA. "Protocol Prying: Systematic Vulnerability Research in AirDrop and Quick Share." arXiv:2606.26967, 2026-06-25 — 6 pre-auth vulns in AirDrop/QuickShare file-transfer paths (plist recursion, encryption bypass) — manifest-parser attack-surface warning | https://arxiv.org/abs/2606.26967 | Preprint | H |
| K11 | Help Net Security on "Off-grid Civilian Emergency Communication" (Zürich/St. Gallen), arXiv 2509.22568 — LoRa+BLE emergency system, 92% PDR at 1.2 km urban; messaging-only, no file model | https://www.helpnetsecurity.com/2025/10/08/off-grid-emergency-communication/ (paper: https://arxiv.org/pdf/2509.22568) | Preprint+press | M |
| K12 | "Dual-Radio BLE-LoRa Hierarchical Mesh for Infrastructure-Free Emergency Communication." arXiv:2604.15532, 2026 (abstract-level read) — fresh hybrid BLE+LoRa architecture, matrix-worthy | https://arxiv.org/abs/2604.15532 | Preprint | M |
| K13 | HackerNoon. "ResQMesh AI Platform" (bitchat fork), 2025/26 — bitchat fork adds on-device ML triage; no file transport | https://hackernoon.com/when-the-internet-dies-your-phone-can-still-be-smart-building-ai-powered-disaster-communication | Press/OSS writeup | M |
| K14 | permissionlesstech/bitchat GitHub README (v1.5.3, 2026-06) — NO file/image/attachment transfer as of v1.5.3; text+LZ4, TTL≤7, Noise | https://github.com/permissionlesstech/bitchat | Project docs | H (directly fetched) |
| K15 | markqvist/LXMF + Sideband READMEs (Fields spec itself not fetched) — FIELD_FILE_ATTACHMENTS = whole-file inline attach; no two-tier preview/full split, no content-addressed manifest | https://github.com/markqvist/LXMF | Project docs | M |
| K16 | meshtastic/firmware releases + AkitaEngineering/Meshtastic-Zmodem — stock 2.x (→2.7.x, 2026-03) has NO native file transfer; only 3rd-party ZModem direct transfer | https://github.com/meshtastic/firmware | Project docs | M |
| K17 | Huang, Wang, Zhao, Xu, Wang. "Seeing is (Not) Believing: Practical Phishing Attacks Targeting Social Media Sharing Cards." arXiv:2405.10758, 2024 — link-preview/content mismatch as documented attack class ("sharing card forgery") — thumbnail-mismatch analog | https://arxiv.org/abs/2405.10758 | Preprint | H |
| K18 | JPCERT/CC. "Malware with a Fake Thumbnail Preview." 2015 — fake-thumbnail malware technique — supports mismatch threat class | https://blogs.jpcert.or.jp/en/2015/04/malware-with-a-fake-thumbnail-preview.html | Vendor blog | M |

## Track L - nRF52840 embedded board profile (accessed 2026-08-13)

Authoritative sources fetched for `docs/E7_NRF52840_DK_PROFILE.md`. These
entries establish hardware and HAL facts only; they do not select an MMP flash
layout, runtime, or production configuration.

| # | Source | URL | Type | Conf |
|---|---|---|---|---|
| L1 | Nordic Semiconductor, nRF52840 Product Specification (latest HTML rendition): Cortex-M4F at 64 MHz, 1 MB flash, 256 kB RAM, and peripheral/security feature list | https://docs.nordicsemi.com/r/bundle/ps_nrf52840/page/keyfeatures_html5.html | Product specification | H |
| L2 | Nordic Semiconductor, nRF52840 DK product page: PCA10056-class development board capabilities, on-board SEGGER J-Link, external QSPI memory, and current-measurement pins | https://www.nordicsemi.com/Products/Development-hardware/nrf52840-dk | Product page | H |
| L3 | Nordic Semiconductor, nRF52840 DK Hardware - Kit content: explicit PCA10056 board identification and hardware-file scope | https://docs.nordicsemi.com/r/bundle/ug_nrf52840_dk/page/ug/dk/kit_content.html | Hardware guide | H |
| L4 | Embassy, `embassy-nrf` 0.11.0 nRF52840 documentation: nRF52840 flavor, chip-selection feature, blocking/async HAL, and feature notes | https://docs.embassy.dev/embassy-nrf/0.11.0/nrf52840/index.html | Official Rust documentation | H |

_(End of bibliography; Track G added in the 2026-06-11 post-audit revision; Tracks H–K added in the 2026-07-02 extension-phase (rev. B) research; H13/J15/J16 addendum entries added 2026-07-03 for prior art previously cited inline by name in docs 17/18; Track L added 2026-08-13 for the embedded board-profile proposal.)_
