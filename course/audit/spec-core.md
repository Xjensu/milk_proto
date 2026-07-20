# Аудит ядра спецификации: 01–06

Этот файл отделяет описательные выводы и исследовательские рекомендации от нормативного текста. Метка «⚠ provisional» означает значение, которое исходный документ прямо оставляет для проверки, а не требование, уже подтверждённое измерениями.

## 01 — Executive Summary

### Purpose

Сводка представляет MMP/1 как офлайн mesh для телефонов, ПК и IoT, задаёт масштабные цели (полная функция до 100 узлов, usable около 1 000, graceful degradation при 10 000+) и карту всего комплекта. Это итоговое, описательное введение, а не самостоятельный RFC-нормативный источник (`specification/01-executive-summary.md §What mesh_milk is`).

### Normative invariants

Самостоятельных MUST/MUST NOT нет: нельзя превращать утверждения сводки в новые требования. Она указывает, что норму содержат документы 03–11; в частности, verify-before-store/forward и beacons-as-hints принадлежат их первоисточникам, а не этой сводке (`specification/01-executive-summary.md §The three facts that shaped everything`).

### Physical or distributed-systems model

Дальняя доставка — DTN store-carry-forward: устройства переносят данные физически, а радио даёт локальные hops; поэтому «100+ hops» не означает 100 одновременных BLE-переходов. Практическая связность ограничена BLE, мобильными ОС и короткими сессиями, отчего разделены beacon plane и bulk plane (`specification/01-executive-summary.md §The three facts that shaped everything`).

### Design choices and alternatives

Выбраны bounded Binary Spray-and-Wait с локальной utility вместо data-plane flooding и чужих routing claims; legacy beacon + GATT bulk вместо одного канала; Ed25519-идентичность, inline origin key, подписанные bundles и hints-only beacons. Альтернативы, от которых сводка предостерегает: flooding как data plane и security, добавленная после транспорта (`specification/01-executive-summary.md §The three facts that shaped everything`).

### Provisional or uncertain claims

- ~22–36 KiB сетевой стоимости для сообщения 1 KiB, ~1 MiB/day relay-load и ~1–2 min airtime — результаты модели doc 13, не самостоятельная гарантия (`specification/01-executive-summary.md §What the numbers say (doc 13)`).
- Радиооценка ~0.35%/day; с `k_sys ∈ [3,10]` — ~1–3.5%/day. Цель <5%/day названа правдоподобной, но непроверенной, OQ-13 (`specification/01-executive-summary.md §What the numbers say (doc 13)`).
- До v1.0 остаются ⚠ OQ-4 (симуляция routing constants), OQ-10 (внешний crypto review), OQ-12 (SIG UUID), OQ-13 (энергия); это blockers, а не закрытые факты (`specification/01-executive-summary.md §Status and the path to running code`).

### Dependencies and cross-references

Сводка направляет к `specification/02-research-report.md`, `specification/03-protocol-spec.md`, `specification/04-architecture.md`, `specification/05-routing-spec.md`, `specification/06-sync-spec.md`, а также 07–15. Идентификаторы исследовательских рисков: OQ-4, OQ-10, OQ-12, OQ-13; security closures отсылают к 07 §4, а форматы/состояния — к 10 и 11 (`specification/01-executive-summary.md §Architecture at a glance`; `§Status and the path to running code`).

### Teaching implications

**Предпосылки:** различать DTN и синхронную multi-hop сеть, ограничение канала и ОС, криптографическую аутентификацию и модель угроз. **Решения mesh_milk:** выбранная пара beacon/GATT, bounded spray, inline ключ origin, заданные tier-цели и перечень production blockers; их нельзя выдавать за общие свойства любых mesh-протоколов.

### Potential flaw / Alternative / Resolution

**Potential flaw:** сводные цифры могут быть прочитаны как доказанные полевые показатели. **Alternative:** обещать только измеренные показатели. **Resolution:** источник сам оставляет энергию и routing constants открытыми (OQ-13/OQ-4); в курсе помечать числа как модельные и вести к `specification/01-executive-summary.md §What the numbers say (doc 13)`.

## 02 — Scientific Research Report

### Purpose

Это Phase-1 literature review: он сопоставляет системы, BLE/OS-ограничения, DTN routing, sync и coding и формулирует направления для последующих спецификаций. Отчёт не является нормативным документом; его F/C/R-формулировки — evidence-backed findings/recommendations с тегами confidence (`specification/02-research-report.md §1.1 Findings`; `§6. Synthesis`).

### Normative invariants

Самостоятельных RFC MUST/MUST NOT нет. Жёстко сформулированные design consequences (например, «no DHT in core», legacy 31-byte floor, no flooding data plane) являются рекомендациями/входами для документов 03–06, не нормой самого отчёта (`specification/02-research-report.md §1.1 Findings`; `§2.2 Design consequences`; `§6. Synthesis`).

### Physical or distributed-systems model

Legacy advertising имеет 31 B payload; плановый GATT-поток ~50–150 kbps/connection (M); iOS background асимметричен, Android background зависит от OS policy. DTN предполагает непредсказуемые контакты, ограниченные буферы и перенос устройствами; CRDT sync должен терпеть дубликаты/потери/переупорядочивание (`specification/02-research-report.md §2.1 Quantified constraints table`; `§3.1 Candidate algorithms and published evidence`; `§4.1 Evidence`).

### Design choices and alternatives

R-C1 рекомендует Binary Spray-and-Wait + локально наблюдаемую utility и receipt summaries; отклоняет geographic routing, DHT и data flooding. R-D1 использует RBSR для messages, LWW single-writer claims, OR-Set groups и parent-hash DAG; отклоняет general CRDT, Merkle-tree primary sync и global vectors. R-E1 выбирает source-level RS GF(256), signed Merkle root и откладывает RLNC/recoding; RaptorQ оставлен до проверки IPR (`specification/02-research-report.md §3.2 Recommendation R-C1`; `§4.2 Recommendation R-D1`; `§5.3 Recommendation R-E1`).

### Provisional or uncertain claims

- Extended adv на Android и iOS background↔background имеют M-confidence; 50–150 kbps — консервативный M planning number (`specification/02-research-report.md §2.1 Quantified constraints table`).
- Hybrid routing composition имеет M; нужна BLE-parameterized simulation, OQ-4. Ожидаемые RBSR ≤3–4 rounds требуют BLE validation, OQ-5 (`specification/02-research-report.md §3.3 Persona review (Track C)`; `§4.3 Persona review (Track D)`).
- RaptorQ legal status указан L; benchmark RS на nRF52 и корреляция symbol populations остаются OQ-6/OQ-7 и M (`specification/02-research-report.md §5.1 Evidence`; `§5.4 Persona review (Track E)`).

### Dependencies and cross-references

Исследовательские идентификаторы: F-A1–F-A8, C-B1–C-B5, R-C1, F-D1–F-D4, R-D1, F-E1–F-E4, R-E1, OQ-1–OQ-7. Библиографические группы: [A1]–[A24], [B1]–[B17], [C1]–[C7], [D1]–[D12], [E1]–[E7]; их реестром назван `specification/research/bibliography.md` (`specification/02-research-report.md §1`; `§7. Open Questions`).

### Teaching implications

**Предпосылки:** radio airtime, мобильные ограничения, epidemic vs bounded replication, CRDT виды, set reconciliation, Merkle authentication и confidence/evidence hierarchy. **Решения mesh_milk:** конкретные 31 B, GATT 50–150 kbps, выбранные R-C1/R-D1/R-E1 и iOS/Android tiering — синтез источников, требующий валидации, а не универсальный рецепт.

### Potential flaw / Alternative / Resolution

**Potential flaw:** данные routing/sync получены преимущественно из симуляторов или Internet-relay среды, а не BLE mesh. **Alternative:** выбрать flooding либо Merkle walk по привычности. **Resolution:** отчёт предпочитает bounded spray/RBSR как гипотезу композиции M и явно требует OQ-4/OQ-5; до проверки не объявлять превосходство доказанным (`specification/02-research-report.md §3.3 Persona review (Track C)`; `§4.3 Persona review (Track D)`).

## 03 — Protocol Specification (Normative Core)

### Purpose

Нормативное ядро MMP/1 задаёт модель, identity, bundle, planes, conformance, TAL и v1 constants; RFC 2119 слова имеют обязательную силу. Документ — Phase-2 draft, revised 2026-06-11; все `⚠` constants ждут doc 13 (`specification/03-protocol-spec.md §1. Scope and Model`; `§9. Protocol constants (v1, all ⚠ pending doc 13)`).

### Normative invariants

- Identity — Ed25519 keypair; `NodeID=BLAKE3-256(pubkey)[0..16]` (128 bits) только derived ID; nickname collisions допустимы, UI MUST show disambiguator (`specification/03-protocol-spec.md §3.1`–`§3.4`).
- Bundle: LE canonical bytes; origin — inline 32-byte public key; `MAX_BUNDLE=4096 B` ⚠, большее MUST use blob. Signature MUST verify before store/forward; invalid, inconsistent или non-canonical bundle MUST be dropped, never quarantined/forwarded (`specification/03-protocol-spec.md §4.1`–`§4.1.2`).
- SeenIndex MUST reject already seen IDs и жить class-maximum lifetime from first sight; carrier MUST decrement lifetime and delete at 0; receiver MUST clamp lifetime. Unknown critical TLV MUST NOT process/forward (`specification/03-protocol-spec.md §4.2`–`§4.4`).
- Copy grant MUST clamp to `L_MAX=32` ⚠; stable NodeID MUST NOT appear in beacon cleartext; every summary input MUST be re-salted per epoch; beacon MUST be hints only; sessions MUST be resumable/idempotent (`specification/03-protocol-spec.md §4.5`; `§5.1`–`§5.2`).
- TAL MUST provide stated beacon/dial/listen contract, reliable ordered bidirectional stream, quality hints and MUST NOT reorder frames (`specification/03-protocol-spec.md §7. Transport Adaptation Layer contract`).

### Physical or distributed-systems model

Контакты непредсказуемы, global clock/topology/always-reachable node отсутствуют; hostile local peer/link — базовая trust model. Beacon plane имеет ≤24 B ⚠ и неаутентифицирован, bulk plane — authenticated GATT stream; lifetime age-based по monotonic clock. MMP-Core может omit blob, membership CRDT и utility routing, но Standard/Anchor расширяют relay/storage (`specification/03-protocol-spec.md §1. Scope and Model`; `§5`; `§6`).

### Design choices and alternatives

Inline key выбран вместо key-unknown quarantine; lifetime исключён из signature/BundleID, но ограничен clamp+SeenIndex; copy budget вынесен в MAC’d session control и ограничен clamp+origin quota. Выбраны 31-byte legacy BLE service-data beacon, rotating pseudonym/salted summary, RBSR phase order SUMMARY→RECONCILE и TAL abstraction; alternative signed spray certificates отвергнуты из-за topology/certificate-size (`specification/03-protocol-spec.md §3.2`; `§4.2`–`§4.5`; `§5`; `§11. Persona review checkpoints`).

### Provisional or uncertain claims

⚠ values: `MAX_BUNDLE 4096 B`; transient ReasmBuf ≤16 KiB; lifetime control 6 h, direct/group 72 h, claim 7 d, symbol 72 h; quotas Standard/Core/Anchor 128 MiB/32 KiB/2 GiB; beacon interval 2–10 s; scan duty ≤2.5%; `L_MAX=32`; phone ephem_id hash lookup ≤4096 contacts (`specification/03-protocol-spec.md §4.1.1`; `§4.1.2`; `§4.3`; `§9`; `§11`). Все значения в §9 имеют статус «pending doc 13», а не окончательно измеренную производительность.

### Dependencies and cross-references

Идентификаторы/основания: [F-A2], [F-A4], [F-A5], [R-D1], [C-B2], [C-B4], AUDIT F1/F2/F3/F4/F8a. Точные продолжения: `specification/04-architecture.md §2, §4, §5`; `specification/05-routing-spec.md §2.3, §8`; `specification/06-sync-spec.md §4, §6`; docs 07–11 для угроз, crypto, BLE, wire и states (`specification/03-protocol-spec.md §12. Cross-references`).

### Teaching implications

**Предпосылки:** canonical encoding, hash-derived IDs, signature verification, dedup/TTL, AEAD sessions, TLV criticality и abstraction boundary. **Решения mesh_milk:** inline-origin key, lifetime-zeroed BundleID, first-sight SeenIndex, five session phases и три conformance classes — это конкретные MMP invariants, которые должны прослеживаться в реализации.

### Potential flaw / Alternative / Resolution

**Potential flaw:** mutable lifetime/copy budget допускают resurrection либо amplification. **Alternative:** подписывать оба поля или хранить budget в origin certificate. **Resolution:** исходник выбирает receive clamps, first-sight SeenIndex и per-origin quotas; residual после dedup horizon ограничен, но ⚠ constants ещё требуют doc 13 (`specification/03-protocol-spec.md §4.2`; `§4.5`; `§11. Persona review checkpoints`).

## 04 — Architecture Document

### Purpose

Архитектура дополняет wire-норму 03 структурой компонентов, stores, device tiers, friendship и power/scheduling; она должна 1:1 отображаться на crate plan doc 12 (`specification/04-architecture.md §1. Component architecture`).

### Normative invariants

BundleStore eviction: expired → receipt-confirmed → over-quota; node MUST NOT evict own undelivered original before expiry. Tier claim в beacon — лишь hint, behavior измеряется. Friendship создаётся signed exchange; dependent MAY hold до 3 friends; friend не даёт delivery receipt. Архитектура также требует only monotonic clocks for aging (`specification/04-architecture.md §2. Data stores`; `§3`; `§4`; `§7. Failure containment`).

### Physical or distributed-systems model

Слои изолируют radio HAL/TAL от Bundle/Router/Sync; контакт — finite energy opportunity. T0 sensor polls friend, T1 leaf не relay, T2 full duty, T3 anchor continuous; tiers динамичны по battery/charger/OS. iOS background остаётся T1, Android T2 требует FG service; scheduler оптимизирует information value per joule (`specification/04-architecture.md §1`; `§3`; `§5`; `§6`).

### Design choices and alternatives

Выбраны narrow `Stream`/`BeaconPort` traits и decentralized Friend/LPN вместо provisioner/custody transfer. Базовый scan ≤2.5% с foreground/Android-FG 30-s burst; при горячем `k_sys` первым режется burst. На 1k+ beacon summaries считаются FP-saturated, поэтому dial опирается на novelty/RSSI/tier, а exact state — in-session RBSR/ReceiptFilter (`specification/04-architecture.md §1`; `§4`; `§5`; `§6`).

### Provisional or uncertain claims

⚠ bounds: BundleStore 128 MiB/32 KiB, SeenIndex Core ring 4 KiB, ReceiptFilter 64 KiB/1 KiB, ContactGraph ≤4096/≤16 peers, ReasmBuf ≤16/4 KiB; sensor poll e.g. every 5 min, friendship 24 h, beacon 2–10 s, scan ≤2.5%, `T_redial=60 s`, T2 sessions=2, burst ≤6/min (`specification/04-architecture.md §2`; `§4`–`§6`; `§8`). FG-service survival — OEM-policy deployment risk; source требует field validation, а не обещает T2 availability (`specification/04-architecture.md §5. Power management`).

### Dependencies and cross-references

Идентификаторы: [F-A6], [F-A5], [C-B2], [C-B4], AUDIT F2/F7/F8b/F10/F13. Зависимости: `specification/03-protocol-spec.md §4.3, §5.2`; `specification/05-routing-spec.md §8`; `specification/06-sync-spec.md`; `specification/11-state-machines.md`; `specification/12-rust-blueprint.md`; model — `specification/13-performance-model.md` (`specification/04-architecture.md §9. Cross-references`).

### Teaching implications

**Предпосылки:** layered architecture, durable/volatile state, cache vs custody, scheduler/energy trade-offs, observability of peer behavior. **Решения mesh_milk:** T0–T3 roles, exact eviction order, friend proxy, narrow traits и platform-specific FG gates — не общий «IoT mesh» шаблон.

### Potential flaw / Alternative / Resolution

**Potential flaw:** beacon spam может вызвать 30-s continuous scans и разрядить relay. **Alternative:** никогда не делать burst scan. **Resolution:** источник rate-limits/gates burst и переключается на low duty в hostile neighborhood; эффективность и `k_sys` остаются проверяемой предпосылкой, не закрытым результатом (`specification/04-architecture.md §5`; `§8. Persona review checkpoints`).

## 05 — Routing Specification

### Purpose

Нормативный routing layer реализует R-C1: per-contact выбор offer/transfer/eviction, Binary Spray-and-Wait with Focus, local utility и receipt back-propagation; data flooding, DHT и geographic routing исключены. Все ⚠ constants ждут doc 13/OQ-4 (`specification/05-routing-spec.md §1. Overview`; `§2`).

### Normative invariants

- Carried copy имеет `budget ≥1`; receiver of grant MUST clamp to `L_MAX=32` ⚠. Focus-copy передаётся и удаляется только при `U_peer > U_self + θ_focus`; direct delivery always wins (`specification/05-routing-spec.md §2`).
- `N̂` MUST NOT exchange; eligible/inbound admission требует authenticated session, verify signature, quota/rate checks и tier policy (`specification/05-routing-spec.md §3`; `§5`).
- Beacon summary MUST NOT be relied on for suppression/purge at scale; exact D0 receipt, не Bloom hit, purge’ит stored bundle (`specification/05-routing-spec.md §7.2`–`§7.2.1`).
- Group IDs — random 128-bit; blocked IDs drop at admission; origin SHOULD cap in-flight bundles per destination at 64 ⚠ (`specification/05-routing-spec.md §8`; `§9`).

### Physical or distributed-systems model

Маршрутизация не вычисляет global path: она работает на коротких непредсказуемых contacts и local state. Spray распределяет budget, Focus держит одиночную копию до destination/better carrier, receipts идут обратно; utility разрешена лишь из direct observation/explicitly trusted local data, иначе U=0/pure spray (`specification/05-routing-spec.md §1`; `§4`).

### Design choices and alternatives

Origin budget — log-scaled Binary Spray, PRoPHET-style direct predictability без transitivity и social tie вместо third-party claims; transfer order small/urgent first. Использованы ReceiptFilter Bloom как offer hint и exact receipt purge. Отклонены flooding, transitive PRoPHET, global route claims, DHT и geographic coordinates (`specification/05-routing-spec.md §2`–`§7`; `§11. Security considerations`).

### Provisional or uncertain claims

⚠: `L=clamp(4,2·ceil(log2(N̂+1)),32)`; sensor `L=8`; collision discount 0.75/24 h/EWMA α=0.25/day; `σ=0.5`, `P_init=0.5`, `γ=0.98/h`, P table ≤256, ContactGraph ≤4096, `θ_spray=0.3`, `θ_focus=0.15`; default contact 8 s; receipt `L=8`; per-origin 32 KiB/h + 128 KiB burst, WoT ×4; group `L=clamp(8,member_count_estimate,48)`. Таблица 22/28/36 KiB при N=100/1k/10k — preliminary и delivery зависит от mobility, OQ-4 (`specification/05-routing-spec.md §2`–`§10`).

### Dependencies and cross-references

Идентификаторы: R-C1, [C1], [C2], [C3], [C5], OQ-4/OQ-5, AUDIT F2/F3/F8b/F11. Wire budget message — `specification/10-message-format.md`; phase order — `specification/03-protocol-spec.md §5.2`; filters — `specification/06-sync-spec.md §6`; quota stores/tier — `specification/04-architecture.md §2, §3`; simulation — doc 13 (`specification/05-routing-spec.md §13. Cross-references`).

### Teaching implications

**Предпосылки:** replication budgets, probabilistic forwarding, local observations versus untrusted advertisements, rate limiting, Bloom false positives. **Решения mesh_milk:** exact formulas/clamps, no-transitivity, utility bounds, receipt policy и group per-scope budget; формулы нельзя преподавать как validated optimum.

### Potential flaw / Alternative / Resolution

**Potential flaw:** per-scope group budget может остаться в одном географическом кластере, а utility без transitivity слаба вдали. **Alternative:** вновь добавить transitive PRoPHET или flood groups. **Resolution:** source сохраняет spray floor, предлагает member re-spray и отмечает partitioned-group scenarios для OQ-4; гарантия delivery не заявлена (`specification/05-routing-spec.md §4`; `§9`; `§12. Persona review checkpoints`).

## 06 — Synchronization Specification

### Purpose

Нормативная sync layer реализует R-D1 внутри authenticated session: RBSR для bundles, LWW для claims, causal DAG, ReceiptFilter и group OR-Set. Это Phase-3 draft, revised 2026-06-11; `⚠` constants pending doc 13 (`specification/06-sync-spec.md §1. What is synchronized`; `§3`; `§6`).

### Normative invariants

- Domains D0→D4 идут в priority order; bundles с remaining lifetime <10 min ⚠ excluded from offers (`specification/06-sync-spec.md §2. Reconciliation domains`).
- Claims use domain-separated `mmp1/claim`; unknown claim field MUST be stored/re-offered if outer signature verifies but MUST NOT be interpreted. Higher seq wins; equal seq/different body chooses lexicographically smaller `BLAKE3(body)` and surfaces anomaly (`specification/06-sync-spec.md §4.1`; `§4.2`).
- Message SHOULD carry ≤4 PARENTS; display order MUST be DAG topological, then height/BundleID; wall clocks cannot order messages (`specification/06-sync-spec.md §5`).
- Receipt Bloom MUST NOT delete; exact authenticated D0 receipt is the only purge authority (`specification/06-sync-spec.md §6.3`). OR-Set operations are only group-encrypted; anchor-acknowledged GC MAY compact reflected tags/tombstones; apps SHOULD disclose partitioned-removal latency (`specification/06-sync-spec.md §8.2`; `§8.5`; `§8.6`).

### Physical or distributed-systems model

Sync assumes interrupted contacts, no global clock and expiring replicated sets. RBSR is stateless between sessions and idempotent; a malicious peer can waste bounded airtime or withhold data but cannot corrupt verified store. Causal DAG tolerates expired/missing parents as permanent visual gaps; OR-Set removal propagates only on reconvergence (`specification/06-sync-spec.md §3.3`–`§3.5`; `§5`; `§8.6`).

### Design choices and alternatives

RBSR/Negentropy over BundleIDs chosen вместо per-peer cursors and primary Merkle walk; claim LWW uses sequence not clocks; causal parent DAG replaces vectors/sequences; Bloom is suppress-only, exact receipt purges; epoch anchors bound OR-Set history. Alternative skip-list/Merkle store digests rejected for v1 because state/round trips, while global vectors rejected for state explosion (`specification/06-sync-spec.md §3`; `§4`; `§5.4`; `§6`; `§8.5`; `§9. Persona review checkpoints`).

### Provisional or uncertain claims

⚠: expired-offer cutoff 10 min; `FP` 64-bit, `T_list=24`, `B_f=16`, expected ≤3 rounds for n≤100k; group-interest hashes salted per-session; claim digest shared contacts ≤64; ReceiptFilter 12×6 h, k=7, 9.6 bits/element, Standard 8192 receipts/bucket/118 KiB, Core ~600 B; proxy horizon 24 h and stale >12 h; immutable admins v1. RBSR depth/FP and filter cost ожидают doc 13/OQ-5; admin rotation — OQ-8 (`specification/06-sync-spec.md §2`; `§3.1`–`§3.2`; `§4.3`; `§6`; `§7`; `§8.4`; `§10`).

### Dependencies and cross-references

Идентификаторы: R-D1, [D1], [D2], [D7], [D8], F-D3/F-D4, OQ-1/OQ-5/OQ-8, AUDIT F4/F5/F6/F11. Frame layouts — `specification/10-message-format.md`; session order — `specification/03-protocol-spec.md §5.2`; friendship — `specification/04-architecture.md §4`; receipts/routing — `specification/05-routing-spec.md §7`; crypto/group semantics — doc 08 (`specification/06-sync-spec.md §10. Cross-references`).

### Teaching implications

**Предпосылки:** exact versus approximate reconciliation, stateless anti-entropy, LWW/OR-Set semantics, causal DAG, false-positive safety direction, eventual consistency under partitions. **Решения mesh_milk:** D0–D4 priority, 64-bit RBSR fingerprint, exact-receipt purge, display tie-breaker и epoch-anchor GC; фиксировать ⚠ параметры отдельно от общих CRDT принципов.

### Potential flaw / Alternative / Resolution

**Potential flaw:** stateless RBSR может тратить слишком много rounds на Anchor-scale inventory, а Bloom summary тяжёл для короткого Core contact. **Alternative:** persistent per-peer Merkle/skip-list state. **Resolution:** source partitions/window’ит domains и допускает Core mini-filters/deltas, но переносит depth/cost validation в doc 13/OQ-5; улучшение не следует считать доказанным (`specification/06-sync-spec.md §3.3`; `§6`; `§9. Persona review checkpoints`).
