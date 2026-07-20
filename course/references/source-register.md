# Course source register

This register imports the sources used by the performance, failure, and extension audit. Confidence and limitations are transcribed or qualified from `specification/research/bibliography.md`; no source was fetched for this register.

## SRC-0001 — BLE power-consumption measurement
- URL: https://files01.core.ac.uk/download/pdf/80990492.pdf
- Source class: Thesis
- Accessed: 2026-06-10
- Supports: [B12]; controller-current proxy for doc 13 energy arithmetic.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [B12] as a thesis with M confidence.
- Limitations: Controller measurement is used as a phone proxy; whole-phone overhead is unmeasured.

## SRC-0002 — BLE neighbor-discovery energy model
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC6891668/
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B13]; doc 13 energy arithmetic.
- Confidence: High
- Evidence basis: Project bibliography classifies [B13] as a peer-reviewed paper with H confidence.
- Limitations: No whole-phone multiplier is recorded in project evidence.

## SRC-0003 — Precise BLE energy model
- URL: https://arxiv.org/pdf/1403.2919
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B14]; doc 13 energy arithmetic.
- Confidence: High
- Evidence basis: Project bibliography classifies [B14] as a paper with H confidence.
- Limitations: Model does not validate current phone OS wakeup cost.

## SRC-0004 — DTN routing performance comparison
- URL: https://www.academia.edu/9001708/Performance_Comparison_of_Epidemic_PRoPHET_Spray_and_Wait_Binary_Spray_and_Wait_and_PRoPHETv2
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [C1]; spray-versus-epidemic latency/cost framing.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [C1] as Paper, M.
- Limitations: Project notes BLE-specific contact validation remains OQ-4.

## SRC-0005 — Analytical Spray and Wait study
- URL: https://www.researchgate.net/publication/316513321_ANALYTICAL_STUDY_OF_SPRAY_AND_WAIT_ROUTING_PROTOCOL_IN_DELAY_TOLERANT_NETWORK
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [C3]; spray analysis in doc 13.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [C3] as Paper, M.
- Limitations: Project does not treat it as a BLE-stack measurement.

## SRC-0006 — Message ferrying approach
- URL: https://www.sigmobile.org/mobihoc/2004/presentations/p187-zhao.pdf
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H1]; conductor-ferry model in doc 17.
- Confidence: High
- Evidence basis: Project bibliography classifies [H1] as Paper, H.
- Limitations: No train-profile pass band is supplied by this source.

## SRC-0007 — Ferry-based linear sensor networks
- URL: https://cis.temple.edu/~wu/research/publications/Publication_files/2013globecom-Imad.pdf
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H2]; linear topology and `t=2R/v` contact model.
- Confidence: High
- Evidence basis: Project bibliography classifies [H2] as Paper, H.
- Limitations: Not a BLE train deployment measurement.

## SRC-0008 — BLE RSSI in train carriages
- URL: https://arxiv.org/abs/2006.06822
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H3]; non-monotonic RSSI caveat.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [H3] as Paper, M.
- Limitations: Supports a caveat, not a residual-contact-time estimator.

## SRC-0009 — Rolling-stock RF attenuation
- URL: https://assets.publishing.service.gov.uk/media/618d299cd3bf7f055b29332f/mobile-connectivity-in-rolling-stock-radio-frequency-attenuation-characteristics.pdf
- Source class: Government measurement report
- Accessed: 2026-07-02
- Supports: [H4]; per-carriage-cell rationale.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [H4] as a government measurement report, M.
- Limitations: Does not provide MMP's needed direct 2.4 GHz gangway value.

## SRC-0010 — Data MULEs architecture
- URL: https://labs.ece.uw.edu/funlab/Publications/2003/data_mules.pdf
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H5]; data-mule precedent.
- Confidence: Low
- Evidence basis: Bibliography records H existence but unverified content; this register does not treat existence alone as substantive support.
- Limitations: PDF content was not verified in the project.

## SRC-0011 — VANET contact-duration study
- URL: https://arxiv.org/pdf/1502.07282
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H6]; independent `t_c=2R/|Δv|` form.
- Confidence: High
- Evidence basis: Project bibliography classifies [H6] as Paper, H.
- Limitations: Vehicular model is not BLE connection setup evidence.

## SRC-0012 — Affinity mobility model
- URL: https://arxiv.org/pdf/1205.1867
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H7]; qualitative contact-duration context.
- Confidence: Low
- Evidence basis: Project bibliography records formula extraction failure and L confidence.
- Limitations: Qualitative confirmation only.

## SRC-0013 — Wireless inter-carriage link
- URL: https://www.westermo.com/us/industries/train-networks/wireless-solutions/wireless-inter-carriage-link
- Source class: Vendor documentation
- Accessed: 2026-07-02
- Supports: [H8]; industry band-choice evidence.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [H8] as vendor documentation, M.
- Limitations: Product documentation, not a direct MMP propagation measurement.

## SRC-0014 — Railway bearing monitoring case study
- URL: https://lprs.co.uk/knowledge-centre/solutions-and-case-studies/automotive-transportation/real-time-railway-bearing-monitoring.html
- Source class: Vendor case study
- Accessed: 2026-07-02
- Supports: [H9]; rail sensor deployment context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [H9] as vendor case study, M.
- Limitations: Uses proprietary RF, not BLE.

## SRC-0015 — SMRTag railcar mesh
- URL: https://www.inetlp.com/smrtag-system
- Source class: Vendor documentation
- Accessed: 2026-07-02
- Supports: [H10]; railcar mesh context.
- Confidence: Medium
- Evidence basis: Bibliography gives M confidence and marks BLE attribution L.
- Limitations: BLE attribution is secondary/unverified.

## SRC-0016 — 2 GHz inter-car propagation
- URL: https://ieeexplore.ieee.org/document/7026607/
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H11]; OQ-14 provenance.
- Confidence: Low
- Evidence basis: Existence confirmed in project bibliography; content inaccessible.
- Limitations: Paywalled and not content-verified.

## SRC-0017 — High-speed-train propagation model
- URL: https://www.sciencedirect.com/science/article/abs/pii/S0263224117304001
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [H12]; OQ-14 provenance.
- Confidence: Low
- Evidence basis: Existence confirmed in project bibliography; fetch returned 403.
- Limitations: Content not verified in project.

## SRC-0018 — Drive-thru Internet
- URL: https://api.semanticscholar.org/graph/v1/paper/DOI:10.1109/INFCOM.2004.1354509 (IEEE: https://ieeexplore.ieee.org/document/1354509)
- Source class: Paper
- Accessed: 2026-07-03
- Supports: [H13]; R16 prior-art qualification.
- Confidence: Medium
- Evidence basis: Bibliography records H existence and M content from secondary literature.
- Limitations: IEEE full text was paywalled.

## SRC-0019 — BLE neighbour discovery evaluation
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC5375785/
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [I1]; dense-advertiser context.
- Confidence: High
- Evidence basis: Project bibliography classifies [I1] as Paper, H.
- Limitations: Does not validate the 10k-node MMP stadium result.

## SRC-0020 — BLE interference awareness
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC8037550/
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [I2]; interference context.
- Confidence: High
- Evidence basis: Project bibliography classifies [I2] as Paper, H.
- Limitations: Not a phone-mesh field trial.

## SRC-0021 — Bluetooth Channel Sounding
- URL: https://www.bluetooth.com/learn-about-bluetooth/feature-enhancements/channel-sounding/
- Source class: SIG documentation
- Accessed: 2026-07-02
- Supports: [I3]; OQ-15 feature/update-rate gap.
- Confidence: High
- Evidence basis: Project bibliography classifies [I3] as SIG documentation, H.
- Limitations: Update rate is not stated.

## SRC-0022 — Arrival/departure predictor
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC5134511/
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [I4]; contact-window scheduling prior art.
- Confidence: High
- Evidence basis: Project bibliography classifies [I4] as Paper, H.
- Limitations: Temporal prediction, not in-session BLE ranging.

## SRC-0023 — PAwR explanation
- URL: https://novelbits.io/periodic-advertising-with-responses-pawr/
- Source class: Blog
- Accessed: 2026-07-02
- Supports: [I5]; future dense-BLE context.
- Confidence: Low
- Evidence basis: Project bibliography classifies [I5] as Blog, L.
- Limitations: Engineering blog; no project validation.

## SRC-0024 — AOSP BLE advertising
- URL: https://source.android.com/docs/core/connect/bluetooth/ble_advertising
- Source class: Official documentation
- Accessed: 2026-07-02
- Supports: [I6]; Android capability context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [I6] as official documentation, M.
- Limitations: Hardware-gated capabilities do not establish fleet behavior.

## SRC-0025 — BLE throughput guide
- URL: https://interrupt.memfault.com/blog/ble-throughput-primer
- Source class: Blog
- Accessed: 2026-07-02
- Supports: [I7]; throughput planning context.
- Confidence: Low
- Evidence basis: Project bibliography labels [I7] Blog, L.
- Limitations: Not current phone-to-phone measurement.

## SRC-0026 — Mobile BLE throughput
- URL: https://punchthrough.com/maximizing-ble-throughput-on-ios-and-android/
- Source class: Blog
- Accessed: 2026-07-02
- Supports: [I8]; historical throughput context.
- Confidence: Low
- Evidence basis: Project bibliography labels [I8] Blog, L.
- Limitations: Page itself carries an outdated disclaimer.

## SRC-0027 — ITU indoor attenuation summary
- URL: https://en.wikipedia.org/wiki/ITU_model_for_indoor_attenuation
- Source class: Secondary source
- Accessed: 2026-07-02
- Supports: [I9]; campus floor-loss planning model.
- Confidence: Low
- Evidence basis: Project bibliography classifies [I9] as Secondary, L.
- Limitations: Not MMP-band-specific measurement.

## SRC-0028 — iOS extended advertising forum
- URL: https://devzone.nordicsemi.com/f/nordic-q-a/107037/ble5-extended-advertising-and-ios--any-updates
- Source class: Forum
- Accessed: 2026-07-02
- Supports: [I10]; iOS capability caveat.
- Confidence: Low
- Evidence basis: Project bibliography classifies [I10] as Forum, L.
- Limitations: Staff answer is not platform guarantee.

## SRC-0029 — Android RangingManager
- URL: https://developer.android.com/develop/connectivity/ranging
- Source class: Official documentation
- Accessed: 2026-07-02
- Supports: [I11]; OQ-15 Android API path.
- Confidence: High
- Evidence basis: Project bibliography classifies [I11] as official documentation, H.
- Limitations: Does not establish iPhone support or update rate.

## SRC-0030 — Pixel 10 Bluetooth 6 report
- URL: https://www.androidpolice.com/pixel-10-family-bluetooth-6-channel-sounding/
- Source class: Technology press
- Accessed: 2026-07-02
- Supports: [I12]; device-availability context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [I12] as tech press, M.
- Limitations: Not a protocol performance measurement.

## SRC-0031 — Nordic Channel Sounding test
- URL: https://www.nordicsemi.com/Nordic-news/2025/10/Nordics-Android-app-and-nRF54L-Series-evaluate-Bluetooth-Channel-Sounding-using-the-Google-Pixel-10
- Source class: Vendor blog
- Accessed: 2026-07-02
- Supports: [I13]; ±1 m to 20 m measurement.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [I13] as vendor blog, M.
- Limitations: Pixel-to-nRF reference, not general phone fleet behavior.

## SRC-0032 — STM32WBA BLE throughput forum
- URL: https://community.st.com/t5/stm32-mcus-wireless/stm32wba-ble-data-throughput-example-throughput-optimization/td-p/827102
- Source class: Forum
- Accessed: 2026-07-02
- Supports: [I14]; 250–680 kbps phone-to-MCU planning range.
- Confidence: Low
- Evidence basis: Project bibliography classifies [I14] as Forum, L.
- Limitations: Phone-to-MCU, not phone-to-phone; OQ-2 remains open.

## SRC-0033 — LTE alert spoofing
- URL: https://dl.acm.org/doi/10.1145/3307334.3326082 (mirror: https://ericw.us/trow/lte-alerts.pdf)
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [J1]; authenticated-broadcast threat model.
- Confidence: High
- Evidence basis: Project bibliography classifies [J1] as Paper, H.
- Limitations: Cellular alert system, not DTN mesh.

## SRC-0034 — Hawaii false-alert report
- URL: https://docs.fcc.gov/public/attachments/DOC-350119A1.pdf
- Source class: Regulatory report
- Accessed: 2026-07-02
- Supports: [J2]; dual-control/process failure lesson.
- Confidence: High
- Evidence basis: Project bibliography classifies [J2] as regulatory report, H.
- Limitations: Does not provide a cryptographic authority mechanism.

## SRC-0035 — 5G alert spoofing testbed
- URL: https://arxiv.org/html/2604.24404v1
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [J3]; disconnected-DTN limitation.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [J3] as Preprint, M.
- Limitations: Preprint; fresh verification required.

## SRC-0036 — 5G warning-system analysis
- URL: https://dl.acm.org/doi/fullHtml/10.1145/3564625.3568000
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [J4]; unauthenticated PWS context.
- Confidence: Medium
- Evidence basis: Bibliography records M confidence but unverified fetch.
- Limitations: 403 on project fetch; content derived from search summary.

## SRC-0037 — TESLA
- URL: https://datatracker.ietf.org/doc/html/rfc4082
- Source class: RFC
- Accessed: 2026-07-02
- Supports: [J5]; bounded-clock requirement.
- Confidence: High
- Evidence basis: Project bibliography classifies [J5] as RFC, H.
- Limitations: Structural mismatch with multi-hop DTN delay.

## SRC-0038 — TESLA-for-5G
- URL: https://arxiv.org/pdf/2606.26528
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [J6]; live-cellular comparison.
- Confidence: Low
- Evidence basis: Project bibliography records L→M confidence.
- Limitations: Targets live cellular rather than DTN.

## SRC-0039 — Privacy Pass rate-limit tokens
- URL: https://datatracker.ietf.org/doc/html/draft-ietf-privacypass-rate-limit-tokens-06
- Source class: IETF draft
- Accessed: 2026-07-02
- Supports: [J7]; online-budget-token contrast.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [J7] as IETF draft, M.
- Limitations: Requires online trusted attester.

## SRC-0040 — Bundle Protocol v7
- URL: https://www.rfc-editor.org/rfc/rfc9171.html
- Source class: RFC
- Accessed: 2026-07-02
- Supports: [J8]; priority comparison.
- Confidence: High
- Evidence basis: Project bibliography classifies [J8] as RFC, H.
- Limitations: Does not provide sender-authenticated priority admission; duplicate URL also appears in bibliography entry A7.

## SRC-0041 — ACPC revocation
- URL: https://www.sciencedirect.com/science/article/pii/S1570870518304761 (eprint: https://eprint.iacr.org/2018/324)
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [J9]; activation-code revocation comparison.
- Confidence: High
- Evidence basis: Project bibliography classifies [J9] as Paper, H.
- Limitations: Assumes periodic infrastructure connectivity.

## SRC-0042 — Macaroons
- URL: https://research.google/pubs/macaroons-cookies-with-contextual-caveats-for-decentralized-authorization-in-the-cloud/
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [J10]; offline-delegation comparison.
- Confidence: High
- Evidence basis: Project bibliography classifies [J10] as Paper, H.
- Limitations: No native rate-budget primitive in project analysis.

## SRC-0043 — Biscuit documentation
- URL: https://www.biscuitsec.org/
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [J11]; delegation comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [J11] as project docs, M.
- Limitations: No built-in rate budget in project analysis.

## SRC-0044 — UCAN specification
- URL: https://ucan.xyz/specification/
- Source class: Community specification
- Accessed: 2026-07-02
- Supports: [J12]; capability-chain comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [J12] as community spec, M.
- Limitations: No first-class rate budget in project analysis.

## SRC-0045 — Vouchsafe
- URL: https://arxiv.org/pdf/2601.02254
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [J13]; offline identity/delegation comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [J13] as Preprint, M.
- Limitations: Generic constraints; no rate-budget bound.

## SRC-0046 — NC3 bounded delegation
- URL: https://www.researchgate.net/publication/396843429
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [J14]; bounded-authority comparison.
- Confidence: Low
- Evidence basis: Bibliography records abstract-level only and L confidence.
- Limitations: Fetch returned 403; content unverified.

## SRC-0047 — k-times anonymous authentication
- URL: https://api.semanticscholar.org/graph/v1/paper/DOI:10.1007/978-3-540-30539-2_22 (Springer: https://link.springer.com/chapter/10.1007/978-3-540-30539-2_22)
- Source class: Paper
- Accessed: 2026-07-03
- Supports: [J15]; cryptographic-budget alternative.
- Confidence: Medium
- Evidence basis: Bibliography records H existence and M content due to paywall.
- Limitations: Full text paywalled in project research.

## SRC-0048 — Untraceable electronic cash
- URL: https://chaum.com/wp-content/uploads/2021/12/Untraceable_Electronic_Cash.pdf (Springer: https://link.springer.com/chapter/10.1007/0-387-34799-2_25)
- Source class: Paper
- Accessed: 2026-07-03
- Supports: [J16]; offline double-spend alternative.
- Confidence: Medium
- Evidence basis: Bibliography records H existence and M content from scanned PDF.
- Limitations: PDF scan was not machine-extracted.

## SRC-0049 — Haggle architecture
- URL: https://www.researchgate.net/publication/234781274_Haggle_A_Data-centric_Network_Architecture_for_Mobile_Devices
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K1]; eager-metadata/lazy-payload precedent.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K1] as Paper, M.
- Limitations: Venue/year not independently verified in project.

## SRC-0050 — Haggle implementation
- URL: https://github.com/dmonakhov/haggle
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [K2]; Haggle object-model detail.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K2] as project docs, M.
- Limitations: Does not establish signed manifest semantics.

## SRC-0051 — Haggle project page
- URL: https://icawww1.epfl.ch/haggle/
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [K3]; interest/delegation context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K3] as project docs, M.
- Limitations: Project documentation, not a current deployed-system evaluation.

## SRC-0052 — PodNet
- URL: https://dl.acm.org/doi/abs/10.1145/1755743.1755779
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K4]; opportunistic metadata/payload precedent.
- Confidence: Medium
- Evidence basis: Bibliography records M confidence with unverified fetch.
- Limitations: DOI page returned 403 in project research.

## SRC-0053 — Fountain code over DTN
- URL: https://ntrs.nasa.gov/api/citations/20230007643/downloads/IEEE_Access_CSU_HDTN_2023_submission.pdf
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K5]; DTN coding precedent.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K5] as Paper, M.
- Limitations: Does not validate MMP's RS configuration.

## SRC-0054 — Distributed fountain network code
- URL: https://dl.acm.org/doi/pdf/10.1145/2482967.2482978
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K6]; intermittent multi-source swarming.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K6] as Paper, M.
- Limitations: Vehicular setting, not MMP wire format.

## SRC-0055 — Cooperative VANET download
- URL: https://www.mdpi.com/1424-8220/16/10/1685
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K7]; fountain-code downloading precedent.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K7] as Paper, M.
- Limitations: Does not establish end-user thumbnail semantics.

## SRC-0056 — AWDL protocol analysis
- URL: https://arxiv.org/abs/1808.03156
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K8]; local-transfer landscape.
- Confidence: High
- Evidence basis: Project bibliography classifies [K8] as Paper, H.
- Limitations: AWDL is synchronous/local, not DTN file delivery.

## SRC-0057 — AWDL/AirDrop security analysis
- URL: https://www.usenix.org/conference/usenixsecurity19/presentation/stute
- Source class: Paper
- Accessed: 2026-07-02
- Supports: [K9]; contact-hash and attack-surface context.
- Confidence: High
- Evidence basis: Project bibliography classifies [K9] as Paper, H.
- Limitations: Does not specify MMP parser rules.

## SRC-0058 — AirDrop and Quick Share research
- URL: https://arxiv.org/abs/2606.26967
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [K10]; manifest parser hardening rationale.
- Confidence: High
- Evidence basis: Project bibliography classifies [K10] as Preprint, H.
- Limitations: Preprint; fresh verification required.

## SRC-0059 — Off-grid emergency communication report
- URL: https://www.helpnetsecurity.com/2025/10/08/off-grid-emergency-communication/ (paper: https://arxiv.org/pdf/2509.22568)
- Source class: Preprint and press
- Accessed: 2026-07-02
- Supports: [K11]; mesh landscape comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K11] as preprint+press, M.
- Limitations: Messaging-only system; linked paper URL is separately recorded in bibliography.

## SRC-0060 — Dual-radio BLE-LoRa mesh
- URL: https://arxiv.org/abs/2604.15532
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [K12]; current landscape comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K12] as Preprint, M.
- Limitations: Abstract-level read only.

## SRC-0061 — ResQMesh writeup
- URL: https://hackernoon.com/when-the-internet-dies-your-phone-can-still-be-smart-building-ai-powered-disaster-communication
- Source class: Press and OSS writeup
- Accessed: 2026-07-02
- Supports: [K13]; bitchat-fork landscape context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K13] as press/OSS writeup, M.
- Limitations: No file transport; not a protocol evaluation.

## SRC-0062 — bitchat README
- URL: https://github.com/permissionlesstech/bitchat
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [K14]; no-file-transfer landscape claim.
- Confidence: High
- Evidence basis: Bibliography records directly fetched project documentation, H.
- Limitations: Version-specific project documentation.

## SRC-0063 — LXMF README
- URL: https://github.com/markqvist/LXMF
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [K15]; whole-file attachment comparison.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K15] as project docs, M.
- Limitations: Fields specification itself was not fetched.

## SRC-0064 — Meshtastic firmware
- URL: https://github.com/meshtastic/firmware
- Source class: Project documentation
- Accessed: 2026-07-02
- Supports: [K16]; no-native-file-transfer landscape claim.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K16] as project docs, M.
- Limitations: Version-specific; third-party ZModem is outside stock support.

## SRC-0065 — Sharing-card phishing
- URL: https://arxiv.org/abs/2405.10758
- Source class: Preprint
- Accessed: 2026-07-02
- Supports: [K17]; thumbnail-mismatch threat class.
- Confidence: High
- Evidence basis: Project bibliography classifies [K17] as Preprint, H.
- Limitations: Analogous preview attack, not a file-manifest proof.

## SRC-0066 — Fake thumbnail malware
- URL: https://blogs.jpcert.or.jp/en/2015/04/malware-with-a-fake-thumbnail-preview.html
- Source class: Vendor blog
- Accessed: 2026-07-02
- Supports: [K18]; thumbnail-mismatch threat class.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [K18] as vendor blog, M.
- Limitations: Historical example; no protocol-level mitigation implied.

## SRC-0067 — Breaking Bridgefy
- URL: https://eprint.iacr.org/2021/214.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A1]; systems-comparison security lessons.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0068 — Breaking Bridgefy again
- URL: https://www.usenix.org/conference/usenixsecurity22/presentation/albrecht
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A2]; systems-comparison security lessons.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0069 — Bluetooth Mesh performance
- URL: https://arxiv.org/pdf/1910.03345
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A3]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0070 — Bluetooth Mesh demystified
- URL: https://arxiv.org/pdf/2106.04230
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A4]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0071 — Serval Mesh
- URL: https://www.researchgate.net/publication/260165109_The_serval_mesh_A_platform_for_resilient_communications_in_disaster_crisis
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A5]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0072 — Serval Rhizome docs
- URL: https://developer.servalproject.org/dokuwiki/doku.php?id=content:tech:rhizome
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A6]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: Project documentation, not an independent evaluation.

## SRC-0073 — Bundle Protocol v7 primary
- URL: https://www.rfc-editor.org/rfc/rfc9171.html
- Source class: RFC
- Accessed: 2026-06-10
- Supports: [A7]; DTN concepts.
- Confidence: High
- Evidence basis: Project bibliography: RFC, H.
- Limitations: No native sender-authenticated priority admission; duplicate URL also appears in bibliography entry J8.

## SRC-0074 — BPv7 custody transfer
- URL: https://arxiv.org/html/2507.17403v1
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A8]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0075 — Secure Scuttlebutt logs
- URL: https://dicg2020.github.io/papers/kermarrec.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A9]; anti-entropy comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0076 — Reticulum manual
- URL: https://reticulum.network/manual/understanding.html
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A10]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: No published large-scale evaluation recorded.

## SRC-0077 — Briar project
- URL: https://briarproject.org/
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A11]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: Project documentation, not independent evaluation.

## SRC-0078 — Cryptography in Briar
- URL: https://ethz.ch/content/dam/ethz/special-interest/infk/inst-infsec/appliedcrypto/education/theses/report_YuanmingSong.pdf
- Source class: Report
- Accessed: 2026-06-10
- Supports: [A12]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Report, M.
- Limitations: Semester project report.

## SRC-0079 — Meshtastic routing docs
- URL: https://meshtastic.org/docs/overview/mesh-algo/
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A13]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: Project documentation.

## SRC-0080 — Meshtastic managed flooding
- URL: https://meshtastic.org/blog/why-meshtastic-uses-managed-flood-routing/
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A14]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: Project documentation.

## SRC-0081 — Bitchat security analysis
- URL: https://blog.trailofbits.com/2025/07/18/building-secure-messaging-is-hard-a-nuanced-take-on-the-bitchat-security-debate/
- Source class: Vendor analysis
- Accessed: 2026-06-10
- Supports: [A15]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Vendor analysis, M.
- Limitations: Analysis rather than primary protocol specification.

## SRC-0082 — Bitchat cache-poisoning analysis
- URL: https://barghest.asia/blog/bitchat-cache-poisoning/
- Source class: Analysis
- Accessed: 2026-06-10
- Supports: [A16]; systems comparison.
- Confidence: Medium
- Evidence basis: Bibliography records L→M after corroboration by [A15].
- Limitations: Secondary analysis.

## SRC-0083 — Bitchat security issue
- URL: https://github.com/permissionlesstech/bitchat/issues/376
- Source class: Issue report
- Accessed: 2026-06-10
- Supports: [A17]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Issue report, M.
- Limitations: Issue report, not a peer-reviewed result.

## SRC-0084 — Yggdrasil design notes
- URL: https://yggdrasil-network.github.io/about.html
- Source class: Project documentation
- Accessed: 2026-06-10
- Supports: [A18]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Project docs, M.
- Limitations: Project documentation.

## SRC-0085 — Gossipsub v1 specification
- URL: https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md
- Source class: Specification
- Accessed: 2026-06-10
- Supports: [A19]; systems comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Spec, M.
- Limitations: Internet-connected context.

## SRC-0086 — Nostr network analysis
- URL: https://arxiv.org/html/2402.05709v2
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A20]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Relay-based system, not DTN mesh.

## SRC-0087 — ReP2P Matrix
- URL: https://dl.acm.org/doi/abs/10.1145/3694809.3700741
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A21]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Workshop context noted by bibliography title.

## SRC-0088 — Strong anonymity for mesh
- URL: https://arxiv.org/pdf/2207.04145
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A22]; privacy research gap.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Anonymity tier is de-scoped in this project.

## SRC-0089 — BLE mesh survey
- URL: https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7349184/
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [A23]; systems comparison.
- Confidence: High
- Evidence basis: Project bibliography: Survey, H.
- Limitations: None recorded in project bibliography.

## SRC-0090 — Bluetooth Mesh overview
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC6111614/
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [A24]; Bluetooth Mesh comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Not a DTN phone-mesh evaluation; duplicate URL also appears in bibliography entry B15.

## SRC-0091 — BLE throughput primer
- URL: https://interrupt.memfault.com/blog/ble-throughput-primer
- Source class: Engineering analysis
- Accessed: 2026-06-10
- Supports: [B1]; BLE throughput planning context.
- Confidence: Medium
- Evidence basis: Project bibliography classifies [B1] as engineering analysis, M.
- Limitations: Not a current phone-to-phone matrix; duplicate URL also appears in bibliography entry I7, which has its own 2026-07-02 Low-confidence record.

## SRC-0092 — BLE 5 PHY trade-offs
- URL: https://www.carloalbertoboano.com/documents/spoerk19ble5phy.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B2]; PHY planning.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0093 — Practical BLE extremes
- URL: https://www.rutronik.com/fileadmin/Rutronik/Downloads/printmedia/products/06_wireless/bluetooth5.pdf
- Source class: Whitepaper
- Accessed: 2026-06-10
- Supports: [B3]; BLE constraints.
- Confidence: Medium
- Evidence basis: Project bibliography: Whitepaper, M.
- Limitations: Vendor whitepaper.

## SRC-0094 — iOS background processing
- URL: https://developer.apple.com/library/archive/documentation/NetworkingInternetWeb/Conceptual/CoreBluetooth_concepts/CoreBluetoothBackgroundProcessingForIOSApps/PerformingTasksWhileYourAppIsInTheBackground.html
- Source class: Vendor documentation
- Accessed: 2026-06-10
- Supports: [B4]; iOS-background constraint.
- Confidence: High
- Evidence basis: Project bibliography: Vendor docs, H.
- Limitations: Current behavior still requires OQ-3 verification.

## SRC-0095 — AOSP BLE advertising primary
- URL: https://source.android.com/docs/core/connect/bluetooth/ble_advertising
- Source class: Vendor documentation
- Accessed: 2026-06-10
- Supports: [B5]; Android advertising context.
- Confidence: High
- Evidence basis: Project bibliography classifies [B5] as vendor documentation, H.
- Limitations: Capability availability is hardware-dependent; duplicate URL also appears in bibliography entry I6.

## SRC-0096 — BLE advertising extensions
- URL: https://blog.nordicsemi.com/getconnected/bluetooth-5-advertising-extensions
- Source class: Vendor documentation
- Accessed: 2026-06-10
- Supports: [B6]; extended-advertising context.
- Confidence: Medium
- Evidence basis: Project bibliography: Vendor docs, M.
- Limitations: Does not establish platform availability.

## SRC-0097 — BLE 5 speed guide
- URL: https://novelbits.io/bluetooth-5-speed-maximum-throughput/
- Source class: Engineering analysis
- Accessed: 2026-06-10
- Supports: [B7]; throughput context.
- Confidence: Medium
- Evidence basis: Project bibliography: Engineering analysis, M.
- Limitations: Not a mixed-fleet field measurement.

## SRC-0098 — AOSP extended advertising article
- URL: https://www.freecodecamp.org/news/how-does-extended-bluetooth-advertising-work-in-aosp/
- Source class: Engineering analysis
- Accessed: 2026-06-10
- Supports: [B8]; advertising context.
- Confidence: Low
- Evidence basis: Bibliography records L→M.
- Limitations: Secondary explanation.

## SRC-0099 — Android scanning limits
- URL: https://www.devsflow.ca/blog/ble-android-lessons.html
- Source class: Engineering analysis
- Accessed: 2026-06-10
- Supports: [B9]; Android background constraints.
- Confidence: Medium
- Evidence basis: Bibliography records M, corroborated by [B10].
- Limitations: Engineering analysis.

## SRC-0100 — Android scan errors
- URL: https://punchthrough.com/android-ble-scan-errors/
- Source class: Engineering analysis
- Accessed: 2026-06-10
- Supports: [B10]; Android scan constraints.
- Confidence: Medium
- Evidence basis: Project bibliography: Engineering analysis, M.
- Limitations: Not a controlled platform study.

## SRC-0101 — iOS overflow area
- URL: https://davidgyoungtech.com/2020/05/07/hacking-the-overflow-area
- Source class: Reverse engineering
- Accessed: 2026-06-10
- Supports: [B11]; OQ-3 provenance.
- Confidence: Medium
- Evidence basis: Project bibliography: Reverse engineering, M.
- Limitations: Circa-2020 behavior; current status unverified.

## SRC-0102 — BLE industrial mesh
- URL: https://www.researchgate.net/publication/325024654_Multi-Hop_Real-Time_Communications_Over_Bluetooth_Low_Energy_Industrial_Wireless_Mesh_Networks
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B16]; BLE mesh comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Industrial setting.

## SRC-0103 — BLE mesh evaluation
- URL: https://dang.fan/publication/icpads24-ble-mesh-evaluation/icpads24-ble-mesh-evaluation.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B17]; BLE mesh comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0104 — L2CAP versus GATT
- URL: https://devzone.nordicsemi.com/f/nordic-q-a/61028/confusion-to-use-l2cap-vs-gatt-for-max-throughput-with-ios
- Source class: Vendor forum
- Accessed: 2026-06-10
- Supports: [B18]; transport-selection context.
- Confidence: Medium
- Evidence basis: Project bibliography: engineer-answered vendor forum, M.
- Limitations: Forum source; platform APIs may change.

## SRC-0105 — DTN routing survey
- URL: https://arxiv.org/pdf/1210.0965
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [C2]; routing context.
- Confidence: High
- Evidence basis: Project bibliography: Survey, H.
- Limitations: Not BLE-specific.

## SRC-0106 — Enhanced PRoPHET
- URL: https://www.researchgate.net/publication/325463483_Enhanced_Prophet_Routing_Protocol_in_Delay_Tolerant_Networks
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [C4]; routing comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Paper, M.
- Limitations: None recorded in project bibliography.

## SRC-0107 — BUBBLE Rap
- URL: https://www.cl.cam.ac.uk/~jac22/camhor/bubble_tmc-sub.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [C5]; social-routing comparison.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Social routing assumptions differ from MMP.

## SRC-0108 — Opportunistic communication survey
- URL: https://arxiv.org/pdf/1110.2480
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [C6]; routing comparison.
- Confidence: High
- Evidence basis: Project bibliography: Survey, H.
- Limitations: Not BLE-stack validation.

## SRC-0109 — Opportunistic routing taxonomy
- URL: https://www.sciencedirect.com/science/article/abs/pii/S1084804516000084
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [C7]; routing comparison.
- Confidence: High
- Evidence basis: Project bibliography: Survey, H.
- Limitations: None recorded in project bibliography.

## SRC-0110 — DTN opportunistic-routing survey
- URL: https://www.researchgate.net/publication/320347333_Survey_on_Opportunistic_Routing_for_DelayDisruption_Tolerant_Networks
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [C8]; routing comparison.
- Confidence: Medium
- Evidence basis: Project bibliography: Survey, M.
- Limitations: None recorded in project bibliography.

## SRC-0111 — CRDT approaches
- URL: https://dl.acm.org/doi/10.1145/3695249
- Source class: Survey
- Accessed: 2026-06-10
- Supports: [D1]; consistency teaching.
- Confidence: High
- Evidence basis: Project bibliography: Survey, H.
- Limitations: None recorded in project bibliography.

## SRC-0112 — Delta-mutation CRDTs
- URL: https://arxiv.org/pdf/1410.2803
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D2]; sync design context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0113 — Delta-state CRDTs
- URL: https://arxiv.org/pdf/1603.01529
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D3]; sync design context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0114 — Opportunistic CRDT synchronization
- URL: https://hal.science/hal-03405138/document
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D4]; sync design context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0115 — Erlay BIP
- URL: https://github.com/bitcoin/bips/blob/master/bip-0330.mediawiki
- Source class: Specification
- Accessed: 2026-06-10
- Supports: [D5]; reconciliation context.
- Confidence: High
- Evidence basis: Project bibliography: Spec, H.
- Limitations: Bitcoin relay context.

## SRC-0116 — Erlay paper
- URL: https://arxiv.org/pdf/1905.10518
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D6]; reconciliation context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Internet relay context.

## SRC-0117 — Range-based reconciliation
- URL: https://logperiodic.com/rbsr.html
- Source class: Analysis and paper lineage
- Accessed: 2026-06-10
- Supports: [D7]; RBSR design context.
- Confidence: High
- Evidence basis: Bibliography records M→H confidence.
- Limitations: Not a BLE contact measurement.

## SRC-0118 — Negentropy protocol
- URL: https://github.com/hoytech/negentropy
- Source class: Specification and implementation
- Accessed: 2026-06-10
- Supports: [D8]; RBSR context.
- Confidence: Medium
- Evidence basis: Project bibliography: Spec/impl, M.
- Limitations: Internet-relay evidence; OQ-5 remains open.

## SRC-0119 — Rateless set reconciliation
- URL: https://dl.acm.org/doi/10.1145/3651890.3672219
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D9]; reconciliation alternatives.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0120 — Merkle search trees
- URL: https://www.semanticscholar.org/paper/441e9662f8532e4dbc798a30c1af85627c623373
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D10]; Merkle-CRDT context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: Bibliography URL is an index page.

## SRC-0121 — Merkle-CRDTs
- URL: https://arxiv.org/pdf/2004.00107
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [D11]; Merkle-DAG context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0122 — Version vectors note
- URL: https://haslab.wordpress.com/2011/07/08/version-vectors-are-not-vector-clocks/
- Source class: Expert note
- Accessed: 2026-06-10
- Supports: [D12]; causal-ordering teaching.
- Confidence: Medium
- Evidence basis: Project bibliography: Expert note, M.
- Limitations: Expert note, not a formal standard.

## SRC-0123 — RLNC in DTN
- URL: https://www.sciencedirect.com/science/article/abs/pii/S1570870513001418
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [E1]; coding alternatives.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0124 — RLNC characterization
- URL: https://arxiv.org/pdf/1104.3466
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [E2]; coding alternatives.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0125 — NTC network coding
- URL: https://www.researchgate.net/publication/224251698_NTC_Towards_Efficient_Network_Coding_in_Delay_Tolerant_Networks
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [E3]; coding alternatives.
- Confidence: Medium
- Evidence basis: Project bibliography: Paper, M.
- Limitations: None recorded in project bibliography.

## SRC-0126 — RaptorQ RFC
- URL: https://www.rfc-editor.org/rfc/rfc6330.html
- Source class: RFC
- Accessed: 2026-06-10
- Supports: [E4]; coding alternative and OQ-6.
- Confidence: High
- Evidence basis: Project bibliography: RFC, H.
- Limitations: IPR status remains unresolved.

## SRC-0127 — RaptorQ IPR record
- URL: https://www.rfc-editor.org/info/rfc6330
- Source class: IPR record
- Accessed: 2026-06-10
- Supports: [E5]; OQ-6 provenance.
- Confidence: Medium
- Evidence basis: Project bibliography: IPR record, M.
- Limitations: Proper IPR review remains required.

## SRC-0128 — OpenRQ
- URL: https://openrq-team.github.io/openrq/
- Source class: Implementation
- Accessed: 2026-06-10
- Supports: [E6]; coding implementation context.
- Confidence: Medium
- Evidence basis: Project bibliography: Impl, M.
- Limitations: Does not settle IPR or no_std suitability.

## SRC-0129 — Raptor codes monograph
- URL: https://www.nowpublishers.com/article/DownloadSummary/CIT-060
- Source class: Monograph
- Accessed: 2026-06-10
- Supports: [E7]; coding theory context.
- Confidence: High
- Evidence basis: Project bibliography: Monograph, H.
- Limitations: None recorded in project bibliography.

## SRC-0130 — Noise framework
- URL: https://noiseprotocol.org/noise.html
- Source class: Specification
- Accessed: 2026-06-10
- Supports: [F1]; cryptography baseline.
- Confidence: High
- Evidence basis: Project bibliography: Spec, H.
- Limitations: Does not validate MMP-specific composition.

## SRC-0131 — X3DH
- URL: https://signal.org/docs/specifications/x3dh/
- Source class: Specification
- Accessed: 2026-06-10
- Supports: [F2]; serverless-prekey comparison.
- Confidence: High
- Evidence basis: Project bibliography: Spec, H.
- Limitations: MMP adaptation remains OQ-10.

## SRC-0132 — Noise Explorer
- URL: https://eprint.iacr.org/2018/766.pdf
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [F3]; R3 formal-modeling path.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: No MMP model is supplied.

## SRC-0133 — Libsodium sealed boxes
- URL: https://libsodium.gitbook.io/doc/public-key_cryptography/sealed_boxes
- Source class: Documentation
- Accessed: 2026-06-10
- Supports: [F4]; crypto baseline.
- Confidence: Medium
- Evidence basis: Project bibliography: Docs, M.
- Limitations: Does not provide first-message forward secrecy.

## SRC-0134 — Sender Keys analysis
- URL: https://arxiv.org/pdf/2301.07045
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [F5]; group-crypto context.
- Confidence: High
- Evidence basis: Project bibliography: Paper, H.
- Limitations: None recorded in project bibliography.

## SRC-0135 — MLS RFC
- URL: https://www.rfc-editor.org/rfc/rfc9420.html
- Source class: RFC
- Accessed: 2026-06-10
- Supports: [F6]; group-design alternative.
- Confidence: High
- Evidence basis: Project bibliography: RFC, H.
- Limitations: Partition trade-off remains in project analysis.

## SRC-0136 — libp2p Noise specification
- URL: https://github.com/libp2p/specs/blob/master/noise/README.md
- Source class: Specification
- Accessed: 2026-06-10
- Supports: [F7]; channel-binding precedent.
- Confidence: Medium
- Evidence basis: Project bibliography: Spec, M.
- Limitations: MMP composition still requires external review.

## SRC-0137 — EdDSA RFC
- URL: https://www.rfc-editor.org/rfc/rfc8032.html
- Source class: RFC
- Accessed: 2026-06-11
- Supports: [G1]; canonical-S verification.
- Confidence: High
- Evidence basis: Project bibliography: RFC, H.
- Limitations: None recorded in project bibliography.

## SRC-0138 — OpenSSL Ed25519 issue
- URL: https://github.com/openssl/openssl/issues/7693
- Source class: Issue report
- Accessed: 2026-06-11
- Supports: [G2]; canonical-S implementation risk.
- Confidence: Medium
- Evidence basis: Bibliography records L→M corroborated evidence.
- Limitations: Issue report, not a formal proof.

## SRC-0139 — Advertising data basics
- URL: https://docs.silabs.com/bluetooth/latest/bluetooth-fundamentals-advertising-scanning/advertising-data-basics
- Source class: Vendor documentation
- Accessed: 2026-06-11
- Supports: [G3]; beacon-byte budget.
- Confidence: Medium
- Evidence basis: Project bibliography: Vendor docs, M.
- Limitations: Vendor documentation.

## SRC-0140 — Bluetooth Mesh relay degradation
- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC6111614/
- Source class: Paper
- Accessed: 2026-06-10
- Supports: [B15]; redundant-relay PER degradation.
- Confidence: High
- Evidence basis: Project bibliography classifies [B15] as Paper, H.
- Limitations: Same paper URL as bibliography entry A24; this separately preserves the B15 bibliography alias.
