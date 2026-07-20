# Аудит: безопасность, криптография, транспорт, wire-format, автоматы и Rust

Ниже фиксируется содержание исходных черновиков Phase 3–5, а не подтверждённая реализация. «⚠» сохраняет статус источника: константа либо утверждение ещё provisional/подлежит проверке.

## 07 — Security Specification

### Purpose

Документ задаёт модель локального активного противника, защищаемые активы и связь «атака → защита → остаточный риск» для MMP. Он консолидирует механизмы из `specification/03-protocol-spec.md`–`06-sync-spec.md`; статус — Phase 3 draft после аудита.

### Normative invariants

- Активы/цели `07 §2`: content — confidentiality/integrity/authenticity через E2E AEAD+signatures; identity keys — unforgeability (Ed25519, ключи не покидают устройство); presence — скрыта от strangers ephem_id; social graph — скрыт от strangers и раскрывается другу лишь по согласию; delivery — availability под атакой; battery/storage — bounded abuse через квоты, verify-on-sight и burst limits.
- Идентичность — ключ: `NodeID = BLAKE3(origin)[0..16]`; bundle подписан и проверяется до хранения (`03 §4.1.2`).
- Приложение MUST предложить QR/числовую проверку ключа и явно различать TOFU и verified; FRIEND_OFFER MUST раскрывать данные до согласия; блокирование MUST быть бесшумным (`07 §6.1–6.5`).
- SeenIndex ведётся с первого наблюдения class-max время, lifetime ограничивается; purge допустим только по точной аутентифицированной D0-квитанции, не по Bloom-hit (`03 §4.2–4.3`, `06 §6.3`).
- До доверенной проверки не хранить и не пересылать: inline `origin` позволяет drop при невалидной подписи; PADDING — лишь SHOULD (`07 §3 #10, #15`).

### Physical or distributed-systems model

Противник в своей физической зоне может принимать, записывать, внедрять, повторять и глушить радио, создавать неограниченно Sybil-ключей, быть relay-insider, входить в публичные группы и коррелировать несколько локальных наблюдений. Глобальное RF-наблюдение города, захват устройства/OS-malware, RF fingerprinting и долговременная статистика при почти всех контактах явно вне модели (`07 §1`; [A22]).

### Design choices and alternatives

- TOFU+Noise XX сочетается с внеканальной церемонией вместо недостижимой серверной аутентификации; spray, локальная utility и WoT-разнообразие не создают единственного маршрута (`07 §3 #2, #5–7`).
- Квоты, per-origin token bucket, лимит 4 KiB и pull-only blob symbols ограничивают storage/battery; PoW отклонён как враждебный IoT/батарее (`07 §3 #4, #8–9; §8`).
- Ротация ephem_id/slot salt, скрытие payload в AEAD и optional padding уменьшают локальную корреляцию; mixing/cover traffic и ephemeral destination ID оставлены для v2 privacy profiles/OQ-9 (`07 §3 #14–15; §5`).
- Компактная матрица «атака → защита → остаток»: replay/zombie → SeenIndex с first sight, lifetime clamp и app dedup → не более одного quota-capped store/offer цикла; Sybil → per-origin bucket/WoT+quota → `N_keys×base-rate`, fairness loss; eclipse/sinkhole/blackhole → spray diversity, local utility, receipts → полная eclipse остаётся partition, у relay теряются лишь врученные copies (`07 §3 #3–7, #17`).
- DoS storage/battery/cache → MAX_BUNDLE, eviction, burst/admission caps и verify-before-store → churn/degraded discovery, без key-unknown quarantine; blob pollution → signed BLAKE3 Merkle verification до store → источник заявляет отсутствующий residual; receipt forgery/suppression → conversation-key MAC+exact D0 purge → Bloom FP лишь deprioritizes offer; group infiltration → admin-signed membership+epoch re-key → прошлый traffic доступен бывшему участнику; beacon tracking → ephem_id+slot salt → RF fingerprint/continuous following out of scope; jamming → нет protocol defence, mobility может обойти область; WoT inference → paranoid uniform quotas → coarse co-present preference bit (`07 §3 #8–18`).

### Provisional or uncertain claims

- T_alarm=24 h ⚠ и Bloom false-positive около 1% на пару — параметры, не абсолютные гарантии (`07 §6.3; §7 #6`).
- TOFU при первом активном контакте MITM-able; adversary может подавить свежие prekey claims и опустить first-message FS до sealed-box baseline (`07 §3 #2; §7 #1–2`).
- Сохраняются локальная видимость контакта/грубого объёма, полная eclipse-партиция, Sybil fairness loss и WoT quota side-channel. Корневой операторский ключ — единственный открыто неограниченный риск внутри scope до re-pin (`07 §7 #3–10`).
- Именно stolen operator ROOT key остаётся **unbounded within its scope until re-pin**; остальные перечисленные matrix/register residuals описаны как bounded (включая conductor key: budget×remaining validity), кроме явно документированных физики/вне-scope ограничений (`07 §3 #16, #19, #22; §7`).
- Утверждение «нет privacy regression» от inline public key относится к уже псевдонимной, а не анонимной модели; relay всё равно видит origin pubkey, destination, размер, lifetime и class (`07 §5 note`).

### Dependencies and cross-references

Криптографические механизмы — `08 §1–8`; квоты/бюджеты — `04–05`; parser/fuzz дисциплина — `12 §7`; privacy profiles — `specification/research/open-questions.md` OQ-9; crypto review — OQ-10. Bridgefy/bitchat failure closure перечислено в `07 §4`; файл-злоупотребления и authority cases ссылаются на `16 §9` и `18 §5–7`.

### Teaching implications

Базовые предпосылки: различать confidentiality, integrity, availability, pseudonymity и anonymity; понимать Sybil, eclipse, replay и физические пределы jammer. Выбор mesh_milk: «защита» всегда сопровождается остаточным риском; проверяемый ключ — не то же, что TOFU, а Bloom — подсказка, не право удалить данные.

### Potential flaw / Alternative / Resolution

- **Security Auditor — Potential flaw:** активный противник ожидаем именно при первом контакте. **Alternative:** сервер/PKI. **Resolution:** serverless TOFU не устраняет окно; обязательны видимый статус и QR/numeric ceremony (`07 §8`).
- **Skeptical Professor — Potential flaw:** parser claim не проверяем без дисциплины реализации. **Alternative:** довериться описанию. **Resolution:** fixed-frame/no-proportional-allocation и fuzz pre-auth parser в `12 §7`.
- **Distributed Systems Reviewer — Potential flaw:** receipt starvation даёт ложные alarm в sparse mesh. **Alternative:** не сигнализировать. **Resolution:** SHOULD-уведомление лишь когда контакты есть; формулировка «may be isolated» (`07 §8`).

## 08 — Cryptography Specification

### Purpose

`specification/08-crypto-spec.md` определяет suite 0x01, ключевую иерархию, pairwise/group криптографию, receipt/blob integrity и честно перечисляет отсутствующие свойства. Примитивы стандартные; часть их композиций проектная и не заявлена как независимо доказанная.

### Normative invariants

- v1 suite: Ed25519, X25519, IETF ChaCha20-Poly1305, BLAKE3 вне Noise; handshake ровно `Noise_XX_25519_ChaChaPoly_BLAKE2s` (`08 §1`).
- После XX обе стороны посылают ID_PROOF над `mmp1/chanbind || 0x01 || h || initiator_pubkey || responder_pubkey`; проверка MUST быть canonical-S `verify_strict`, а всякая Ed25519-подпись MUST использовать доменный тег (`08 §3.2; §10`, `10 §3.1`).
- Chain key после advance стирается; phones/PC SHALL хранить chain state и prekey secrets только keystore-wrapped, CK₀ MUST NOT persist, persisted ratchet-state record старше `RATCHET_PERSIST_TTL_MS=LIFETIME_MESSAGE_MS` MUST discard, а первая живая сессия после restore MUST bump epoch (`08 §5.3`). Core/`no_std` persistence не делает.
- Membership change ведёт к epoch++; удалённый член не получает новых sender keys; символ проверяется Merkle path до store (`08 §6.3; §8`).

### Physical or distributed-systems model

Это DTN: при первом сообщении peer может быть часами/хопами далеко, поэтому baseline — sealed box на exchange key. One-time prekey нельзя глобально «потратить» без сервера; DTN reorder требует skipped-key window, а PCS наступает только при живой встрече/interactive traffic, не непрерывно (`08 §4–5`).

### Design choices and alternatives

- XX вместо IK скрывает static keys от passive observer и работает с незнакомцем; resumption PSK MAY пропустить handshake до 15 min ⚠ и одноразовый (`08 §3.1–3.3`).
- Sealed box гарантирует Bob-only readability/confidentiality, но не FS; SHOULD prekey/X3DH-adaptation добавляет FS первого сообщения без серверной гарантии one-time use (`08 §4`).
- Symmetric ratchet + opportunistic contact DH приняты вместо требования interactive Double Ratchet; sender keys+epochs выбраны вместо MLS, поскольку partition создаёт concurrent commits без арбитра (`08 §5–6`).
- Suite 0x02 резервирует X25519+ML-KEM-768 hybrid, но точная Noise-композиция отложена до зрелого стандарта (OQ-11); это reservation, не готовый crypto proof (`08 §9`).

### Provisional or uncertain claims

- M=16 prekeys, W=512 (Core W=32), 72 h, Merkle arity 4, ≤5 levels и ≤1 MiB auto-fetch — ⚠; exact sizing ждёт doc 13 (`08 §4.2; §5.1; §6.4; §8`).
- X3DH adaptation, channel binding и contact DH-ratchet — проектные/не-стандартные композиции, явно для внешнего review OQ-10. Replay-consume понижает first-message FS к signed-prekey level; suppression свежих claims может принудить sealed-box/no-FS baseline, но не ниже его confidentiality floor (`08 §3.2; §4.2; §5.2`).
- Persistence открывает snapshot future-chain-key до next meeting и ≤W уже derived skipped keys; keystore wrap не превращает это в доказательство отсутствия capture, а только запрещает clear fallback (`08 §5.3`).
- Преднамеренно нет deniability, PCS между встречами/внутри group epoch, anonymity за пределами pseudonymity+beacon unlinkability и metadata-free relay (`08 §11`).

### Dependencies and cross-references

Формы handshake/prekey/group — `10 §2–4`; session state — `11 §3`; storage/zeroize/crate choices — `12 §4, §7`; residual first-contact/prekey риск — `07 §3 #2, §7 #2`; group membership — `06 §8`; OQ-10/OQ-11 — `specification/research/open-questions.md`.

### Teaching implications

Общее: primitive, protocol composition и security property нельзя смешивать; FS и PCS разные, а signature даёт аутентичность, не deniability. Специфика mesh_milk: standard Noise остаётся в своей анализируемой конфигурации, однако sealed-box/prekey/ratchet glue требует отдельной оценки и имеет serverless degradation.

### Potential flaw / Alternative / Resolution

- **Security Auditor — Potential flaw:** опубликованные one-time prekeys можно replay-consume. **Alternative:** серверная атомарная выдача. **Resolution:** недостижимо в mesh; деградация к signed-prekey документирована, OQ-10 (`08 §12`).
- **Skeptical Professor — Potential flaw:** подпись session hash — custom composition. **Alternative:** не связывать identity с channel. **Resolution:** подписываются hash и обе identity keys с domain tag; всё равно external review OQ-10 (`08 §3.2; §12`).
- **Performance Engineer — Potential flaw:** окна 512 ключей дороги Core. **Alternative:** единый размер. **Resolution:** Core policy W=32, не wire-visible (`08 §12`).

## 09 — BLE Transport Specification

### Purpose

`specification/09-transport-ble.md` реализует TAL из `03 §7`: legacy advertising/scanning — beacon plane, GATT — conformance bulk plane, L2CAP CoC — optional fast path. Все константы ⚠ до модели `13`.

### Normative invariants

- Standard/Anchor чередуют Peripheral (advertise/accept) и Central (scan/dial); Core peripheral-mostly, iOS T1 leaf central-mostly (`09 §1`).
- Ровно 31 B legacy advertisement: Flags 3 B + Service Data header 4 B + beacon 24 B; без name/TX-power, scan response v1 empty ⚠ (`09 §3.1`). Это total advertising budget, тогда как MMP Service Data несёт только header+24 B, не все 31 B.
- Для iOS foreground таблица источника отдельно указывает лишь 28 B usable; документ не задаёт packing workaround, и из этой оговорки его выводить нельзя (`09 §6`).
- ephem_id каждые 15 min ⚠ MUST совпасть с RPA rotation; summary fragments MUST быть per-epoch re-salt. Fragment/Bloom — hint, не authority; 1k+ Bloom fragment FP-saturated (`09 §3.2–3.3`).
- GATT stream использует length-prefix, ATT payload `(MTU−3)`, buffer preallocated `MAX_BUNDLE=4096`; prefix >4096 — immediate ERROR+CLOSE, idle-stall discard (`09 §5.1`).

### Physical or distributed-systems model

Beacon — малый, вероятностный discovery; связь bulk — короткий BLE contact с ordered/reliable ATT. Advertising interval 2–10 s ⚠ получает ±10% randomization; novel beacon hit запускает continuous 30 s burst scan, rate-limited (`09 §3.2`). Dial score учитывает novelty/role/RSSI >−85 dBm ⚠/budget; при dial запрашиваются MTU≥247, PHY 2M и DLE, но сессия должна работать и на MTU=23 (`09 §4`). Anti-storm: при ≥3 ⚠ центральных подключениях/подключениях к peripheral busy flag даёт random 1–8 s backoff.

### Design choices and alternatives

- GATT TX=central→server Write Without Response, RX=server→central Notify; имена намеренно от central perspective. Service/TX/RX — implementation-pinned 128-bit **GATT interop identifiers**, которые все реализации должны одинаково использовать, а не локальные UUID (`09 §5.1`).
- Точные pinned bulk UUID, которые второе implementation MUST match для interop: Service `6d6d7031-0000-4d4d-5031-6d6573686d6c`; TX (central/client writes) `6d6d7031-0001-4d4d-5031-6d6573686d6c`; RX (central receives Notify) `6d6d7031-0002-4d4d-5031-6d6573686d6c`. От них отделён только **advertised 16-bit Service Data UUID**: он нужен из SIG range для targeted iOS background scan, пока development использует `mmp1` 128-bit namespace ⚠, и именно его production assignment — OQ-12 release blocker (`09 §2; §5.1`).
- Credits N=8 ⚠ на HELLO/CREDIT предотвращают flood медленного stack; CoC MAY мигрировать для TRANSFER/BLOB при обеих поддержках, иначе transparent fallback к GATT (`09 §5.2–5.4`).
- iOS background — connectable peripheral/overflow, не полноценный advertiser; relays SHOULD scan overflow areas, но это reverse-engineered OQ-3, не требование conformance (`09 §6; §8`).

### Provisional or uncertain claims

- MTU 247+, PHY 2M и DLE запрашиваются при dial, а не являются ⚠-параметрами или performance guarantee: handshake продолжается независимо от negotiated values, включая MTU=23 floor (`09 §4`). `09` реально помечает ⚠ development `mmp1` namespace, empty scan response v1, advertising 2–10 s, baseline scan 30 ms/1.28 s (≈2.3% duty), 15-min slot/±1 acceptance, RSSI >−85 dBm, `≥3` centrals и busy bit, credit N=8, connection interval 15–30 ms/50 ms и 4-s supervision, airtime cap 60 s, duty ≤0.5%/≤2.5% и idle-renegotiation >2 s; их подтверждение ждёт doc 13 (`09 §2–8`).
- В самом `09` есть неразрешённое противоречие development advertising: заявленная нормативная layout требует Service Data **16-bit** AD header (AD type 0x16, 4 B) и ровно 24-B beacon в 31-B budget, тогда как development упоминает 128-bit `mmp1` namespace, не задавая ни advertising AD type/encoding, ни пересчитанный byte budget. Поэтому development advertising framing не определён и не реализуем из одного `09` до разрешения OQ-12/16-bit assignment; это не затрагивает pinned bulk GATT UUID (`09 §2–3.1`).
- Burst scan допустим лишь foreground/Android foreground service; iOS background и Android без FG деградируют до opportunistic-on-open (`09 §3.2`).
- Throughput CoC «modest» против tuned GATT и iOS overflow behaviour — зависимости [B18], [B1], [B11], а не гарантии (`09 §5.4; §8`).

### Dependencies and cross-references

Beacon payload — `03 §5.1`, `10 §5`; session frames — `10 §2`; Noise — `08 §3`; reassembly/FSM idle — `10 §6.4`, `11 §3, §7`; budget/tier — `04–05`; energy — `13 §1.2`; OQ-12 — `specification/research/open-questions.md`. Источники транспортных утверждений обозначены [B1], [B2], [B4], [B9]–[B11], [B18], [C-B2], [A3].

### Teaching implications

Предпосылки: advertising и GATT — разные каналы; MTU не равен ATT payload и total advertisement не равен Service Data. Для mesh_milk: beacon — неполная подсказка, вся значимая передача идёт через защищённый session stream; platform role — честная деградация, а не обещание одинакового background поведения.

### Potential flaw / Alternative / Resolution

- **Security Auditor — Potential flaw:** ephem_id без RPA rotation связывается controller address. **Alternative:** только смена ephem_id. **Resolution:** совместная MUST-ротация; при uncontrollable RPA обнаружить persistence и честно сообщить linkability (`09 §8`).
- **Performance Engineer — Potential flaw:** credits добавляют RTT stalls. **Alternative:** полагаться на ATT. **Resolution:** 8×244 B ≈2 KiB in flight, CREDIT piggyback; проверка в doc 13 (`09 §8`).
- **Skeptical Professor — Potential flaw:** overflow scanning ненадёжно. **Alternative:** сделать обязательным. **Resolution:** только SHOULD/OQ-3, базовая система работает деградированно (`09 §8`).

## 10 — Message Format Specification

### Purpose

`specification/10-message-format.md` — единственный нормативный источник байтов на wire: little-endian, byte-aligned, без implicit padding и с единственным canonical encoding. Он описывает frame envelope, bundle/payload/beacon layouts, parsing и size accounting.

### Normative invariants

- `Frame { len:u16 (длина именно payload, ≤4096), typ:u8, payload }`; typ: HS1/2/3=0x01, ID_PROOF=0x02, CREDIT=0x03, SUMMARY=0x10, RECON_INIT/RANGE/IDS=0x11–0x13, WANT=0x14, BUNDLE=0x20, BUDGET=0x21, BLOB_REQ/HAVE=0x30/0x31, FRIEND_OFFER/ACK/PROXY_SUMMARY=0x40–0x42, ERROR=0x7E, CLOSE=0x7F. HS1–3/ID_PROOF — единственные pre-channel parsers, всё остальное AEAD; unknown type → ERROR(unsupported)+CLOSE (`10 §2`).
- Bundle: `ver_class, flags, origin:key32, dest:id16, lifetime:u40, priority, ext_len, TLV, payload_len, payload, sig64`; fixed overhead `124+ext`. `BundleID` и signature используют canonical bytes с zeroed `lifetime_ms`; origin MUST быть проверен как источник NodeID (`10 §3`).
- HELLO внутри Noise payload: `ver, suites, tier, flags(coc_avail/proxy_carrier…), credits, psm`; ID_PROOF=`identity_pubkey(key32) || sig64`; SUMMARY, RBSR, BUDGET и BLOB_REQ/HAVE имеют только layout `10 §2.3–2.6`. `BUDGET` без непосредственно предшествующего held BUNDLE — ERROR, grant MUST clamp к 32 (`10 §2`).
- Payload layouts: class-1 `(conv,epoch,index,body)` либо async sealed/prekey form; class-2 `(gid,epoch, member/index,chain_index,body,member_sig)`; class-0 `(delivered,mac16)`; class-3 `(owner,field,seq,body,sig)`; class-4 raw RS symbol с SYMBOL_INFO TLV (`10 §4`).
- Все Ed25519 signed_input — `dom_tag || object_bytes`; wrong tag MUST reject. Parsers MUST reject trailing bytes, overlong/exceeding length, unknown critical/non-zero reserved, duplicate или unordered TLV; pre-auth — fixed-size/streaming без пропорционального heap allocation; noncanonical S MUST reject (`10 §3.1; §6`).
- Frame reassembly ≤4096 и stall discard; каждый parser MUST fuzz; vectors MUST включать cross-tag и noncanonical-S negative cases (`10 §6`).

### Physical or distributed-systems model

Wire разделяет короткий BLE session stream и переносимый signed bundle. `lifetime_ms` нормализуется в ноль при вычислении BundleID и signing preimage; live lifetime исключён из обоих digest scopes, так как carrier может его легально менять, а DTN dedup остаётся стабильным (`10 §3`). Relay видит header, но AEAD direct-message binds canonical header as associated data (`10 §3–4`).

### Design choices and alternatives

- Inline 32 B origin избран вместо 16 B NodeID: unconditional verify-before-forward дороже на 16 B; unsigned compact receipts/NodeID-only отклонены (`10 §3; §8`).
- Claim, friend, prekey, blobroot, chanbind и rev.B object используют раздельные prefix-free tags; hand-written codec вместо permissive serde фиксирует одно представление (`10 §3.1`, `12 §4`).
- BLOB symbols идут class-4 BUNDLE с SYMBOL_INFO, а не отдельным непроверяемым transport path (`10 §2.6; §4.5`).

### Provisional or uncertain claims

- Size estimates: direct ~356 B, receipt ~166 B, nickname claim ~226 B, RBSR 16-split ~600 B, SUMMARY Core ~700 B/Standard ~20 KiB; 50–150 kbps даёт 30–90 KiB за 5 s и +16 B оценивается как ~1.05 MiB/day — model/sanity estimates, опираются на [C-B1] и `13`, не гарантия (`10 §7`).
- Beacon layout ровно 24 B MMP payload: ver_role 1, ephem_id 8, epoch/busy 1, frag 2, fragment 12; полный 31 B advertising budget задан только `09 §3.1` (`10 §5`).
- `member NodeID?` group payload допускает sealed member index, а file/authority rows помечены rev.B registry additions; semantics принадлежат `16/18` (`10 §4.2; §3.1`).

### Dependencies and cross-references

Bundle semantics — `03 §4`; RBSR — `06 §3`; crypto — `08`; GATT framing — `09 §5`; state consumers — `11`; vectors/fuzz — `12 §7`; sizes/energy — `13`. Research/design IDs: AUDIT F1–F4, F9–F10 и [C-B1] указаны самим документом.

### Teaching implications

Нужно отделять «поле есть на wire» от «его можно доверять»: canonical parse/strict verify — часть security boundary. В mesh_milk size accounting включает 31 B total BLE advert отдельно от 24 B beacon и Service Data framing, а lifetime normalization — намеренная мутабельность carrier, не подпись живого времени.

### Potential flaw / Alternative / Resolution

- **Skeptical Professor — Potential flaw:** «outside signed region» lifetime двусмыслен. **Alternative:** подписывать live value. **Resolution:** zeroed-lifetime для ID и подписи задаёт байтовое правило (`10 §8`).
- **Security Auditor — Potential flaw:** re-headered ciphertext при пропущенной signature check. **Alternative:** только подпись. **Resolution:** AEAD AD связывает header и ciphertext как defence in depth (`10 §8`).
- **Performance Engineer — Potential flaw:** 124 B overhead для receipt ~166 B. **Alternative:** unsigned/NodeID-only receipt. **Resolution:** отклонено ради verify-on-sight; batch/piggyback где возможно (`10 §8`).

## 11 — State Machine Specification

### Purpose

`specification/11-state-machines.md` задаёт нормативные FSM node, tier, session, bundle, friendship и blob assembly. Все неперечисленные transitions — protocol error в designated error state; это не перечень автоматически «неreachable» состояний.

### Normative invariants

- Node: BOOT→INIT при keys loaded, INIT→FIRST_RUN без keys, INIT→IDLE scheduler tick; IDLE/DISCOVERING/ENGAGED, SUSPENDED при OS revoke radio с crash-consistent persistence. Beacon работает кроме SUSPENDED; ENGAGED до MAX_SESSIONS T2=2/T3=8/Core=1 ⚠ (`11 §1`).
- Source diagram задаёт также IDLE→DISCOVERING по duty timer, DISCOVERING→ENGAGED только по scan hit и успешному dial, ENGAGED→IDLE только когда закрыты все sessions; duty slot on/off не является новым node state (`11 §1`).
- Tier transitions: T2→T1 при <20%/FG lost/iOS bg; T1→T2 >25%+FG+opt-in; T2→T3-like charging+opt-in; T3→T2 unplugged; `any→T0` explicitly invalid, потому что T0 build profile. Изменение только при следующей beacon rotation (`11 §2`).
- Session: DIALING→HANDSHAKE→BINDING→ESTABLISHED→SUMMARY/RECONCILE/TRANSFER/BLOB/CLOSING→CLOSED; timeout/refuse → BACKOFF, bad MAC/timeout/bad proof → ABORT; invalid phase frame → ERROR+CLOSING; CLOSE означает stop sending. Inbound deny — silent disconnect before HS response (`11 §3`).
- Bundle хранится только после strict signature+NodeID; inbound budget clamp L_MAX; lifetime decrement/clamp; SeenIndex first-sight; exact D0-only purge. QUARANTINE explicitly отсутствует (`11 §4 I1–I5`).
- I4 отдельно: собственный originated bundle **никогда не EVICTED до EXPIRED** (`11 §4 I4`; eviction order — `04 §2`).
- Friendship: NONE→PENDING по OFFER sent/received; PENDING→ACTIVE по ACK+counter-signature, PENDING→NONE по timeout 60 s ⚠; ACTIVE→ACTIVE по renewal, ACTIVE→NONE по expiry 24 h ⚠/revoke (buffered bundles возвращаются normal spray pool). Blob: WANTED→FETCHING только по verified root; invalid root→REJECTED; FETCHING→BLOCK_DONE при k symbols/block→ASSEMBLED при всех blocks; symbol Merkle failure лишь drop+blacklist count++; interest expiry/cancel→ABANDONED (`11 §5–6`).

### Physical or distributed-systems model

Контакт краток и может обрываться: crash/disconnect эквивалентен CLOSING без flush, phases idempotent; failure после HANDSHAKE+BINDING budget ведёт в BACKOFF с per-peer exponential 1 s base ×2 до 5 min ⚠. D0…D3 serial per domain, BLOB лишь после D0–D3 либо explicit WANT; D3 имеет ≤40% session cap ⚠, BLOB may pre-empt после D0/D1. Sender/receiver дружбы переживают reboot как counter-signed claims; active friend буферизует dependent traffic в quota slice 25% ⚠ своего, а blob-interest истекает через 7 d ⚠/user cancel. Partial cache можно serve только после individual Merkle verification (`11 §3, §5–6, §8`).

### Design choices and alternatives

- Session grammar строгая, без tolerant parsing; CLOSING flushes CREDIT-owed frames и close, а unconfirmed persisted hand-off после crash считается complete: under-spray, never over-spray (`11 §3–4 I3`).
- Bundle lifecycle не object-per-bundle: slot/index state; transient transport reassembly — отдельный bounded buffer, не QUARANTINE и не forward state (`11 §4; §8`).
- Blob ABANDONED сохраняет verified partial symbols до expiry вместо удаления; это ускоряет swarm без снижения integrity (`11 §6`).

### Provisional or uncertain claims

- ⚠: 2–10 s beacon, 30 ms/1.28 s scan, burst 30 s/≤6 min⁻¹, 15 min rotation, 5 s handshake, 10 s idle, 60 s cap, 60 s redial, 1 s/5 min backoff, 24 h friendship, T0 poll 5 min, reassembly stall=session idle, 6 h×12 receipt, 24 h claims, AuthCert priority refill per-cert admissions/hour, 15 min clock-skew и REVLIST_MAX=48; revocation republish не имеет отдельного timer, а идёт claim-gossip cadence (`11 §7`).
- Terminal bundle states explicitly: DROPPED, PURGED, EXPIRED, EVICTED, RELEASED, DELIVERED. Error paths explicitly ABORT (session), DROPPED/REJECTED (bundle/blob) и ERROR+CLOSING; BLOB ABANDONED — конечное/stop-fetch, но partial serve MAY. `any→T0` — source-defined invalid transition; остальные не перечисленные transitions — errors, без выдумывания unreachable states (`11 §2–6`).
- SUSPENDED resume target <50 ms/<1 J — provisional measurement target; silent-deny «constant-time-ish» — утверждение проверки, не формальное constant-time proof (`11 §8`).

### Dependencies and cross-references

Frames — `10`; node/tier rules — `04`; domain ordering/receipts — `03`, `05`, `06`; crypto/merkle — `08`; Rust typestate/invariant tests — `12 §3, §7`; timers — `13`; failure walks — `14`; class-5 gates — `18 §5–6`.

### Teaching implications

Общее: FSM обязан обозначать guard, timeout, error и terminal state; «не нарисовано» не значит unreachable. Выбор mesh_milk: direct DTN requires idempotency and persisted hand-off semantics; security guards (canonical signature, exact receipt) стоят до store/offer, а platform suspension — нормальный lifecycle, не exceptional success path.

### Potential flaw / Alternative / Resolution

- **Distributed Systems Reviewer — Potential flaw:** D3 reconciliation голодает BLOB. **Alternative:** свободно смешать все domains. **Resolution:** D3 sub-budget и WANT BLOB pre-emption после D0/D1 (`11 §8`).
- **Skeptical Professor — Potential flaw:** partial blob cache нарушает verify-before-store. **Alternative:** не serve partial. **Resolution:** каждый symbol Merkle-verified до store/serve; QUARANTINE больше нет (`11 §8`).
- **Battery Specialist — Potential flaw:** Doze suspend/resume thrashes persistence. **Alternative:** не persist. **Resolution:** incremental persistence, target измерить Phase 5; пока provisional (`11 §8`).

## 12 — Rust Implementation Blueprint

### Purpose

`specification/12-rust-blueprint.md` нормативно задаёт форму reference workspace, seams, mobile strategy, test mandates и build order; version pinning происходит лишь при implementation start. Это blueprint, не заявление о conformance или статусе кода.

### Normative invariants

- Layer rule: `mmp-types ← mmp-crypto ← mmp-core ← mmp-router` — `#![no_std]`, alloc optional; ниже `mmp-node` нельзя tokio, std time или TAL impl; CI должен применять cargo-deny и layer test (`12 §1`).
- Radio seam ровно BeaconPort/ContactPort/FrameLink, platform ports MonotonicClock/CryptoRng/KeyVault/BundleStore; FrameLink — reliable ordered Frame plus LinkQuality (`12 §2`).
- Session typestate ограничивает frames фазой; ERROR/CLOSE consume self. Bundle lifecycle — `SlotState` в storage index, не per-bundle object; transition tables должны покрывать I1–I4 (`12 §3`).
- `verify_strict` only, hand-written wire codec; vectors, fuzz allocation proof, property/FSM/concurrency tests и CI matrix/size gate обязательны для reference impl (`12 §4, §7`).
- Тестовый объём не ограничен happy path: golden bytes independent decoder и отрицательные domain-tag/canonical-S/lifetime-reset/budget-clamp cases; fuzz beacon, HS1, ID_PROOF, every post-auth frame, bundle/TLV/RBSR; proptest canonical/RBSR/LWW/OR-Set/ratchet; exhaustive I1–I4, Loom/equivalent, MIRI `mmp-types`, шесть target builds, cargo-deny и size gate (`12 §7`).
- Reference impl MUST иметь отдельный structured trace под build flag `diag`: off by default, never on Core и независим от user event stream. Он записывает journey `(BundleID, created/received/verified/stored/offered/handed_off/receipt_seen/purged/expired/evicted/dropped, peer?, monotonic_ts)`, contact `(ephem_id→nodeid?, dial/accept, handshake_ms, domains, bytes, close_reason)` и counters quarantine/clamp/drop; line-delimited feed для `mmp-cli inspect`/`mmp-sim`. Это **не** меняет protocol surface (`12 §7.7`).

### Physical or distributed-systems model

Одно core должно обслуживать PC, phone и embedded: TAL скрывает радио, KeyVault — keystore/HSM, ordered `iter_domain` поддерживает RBSR (redb range на std, sorted flash log+RAM index на no_std). Мобильная платформа владеет BLE и wakeups; Rust не sleep-ит сам, поэтому Doze/OS alarm напрямую влияет на scheduling (`12 §2, §6`).

### Design choices and alternatives

- Запланированы `mmp-types`, `mmp-crypto`, `mmp-core`, `mmp-router`, `mmp-store`, `mmp-tal`, `mmp-tal-btleplug`, `mmp-tal-mobile`, `mmp-tal-embedded`, `mmp-node`, `mmp-node-embassy`, `mmp-rs-code`, `mmp-ffi`, `mmp-sim`, `mmp-cli` и `vectors/` (`12 §1`). Наблюдаемые имена в `implementation/mesh-milk/crates/` совпадают с этими именами и дополнительно содержат `mmp-tal-peripheral`; это только mapping названий, без вывода о реализации/соответствии.
- Крипто-кандидаты: dalek/x25519-dalek, chacha20poly1305, blake3, snow, zeroize; `ring` отвергнут из-за no_std, hand-rolled XX — лишь fallback при size gate, RS own code против std feature alternative (`12 §4`).
- UniFFI callbacks оставляют Kotlin/Swift владельцами BLE; KeyVault не пропускает secret через FFI, public API предоставляет events, conversation/group/blob operations (`12 §5–6`).
- Build order источника: (1) types+vectors+fuzz, (2) crypto+property tests, (3) core/session/sync against in-memory TAL+router, (4) sim и de-⚠ doc-13 gate, (5) redb store+CLI/btleplug contact, (6) FFI+Android FG/iOS skeletons, (7) embedded+nRF52/OQ-7/size, (8) OQ-10 external crypto review before v1.0 (`12 §10`).

### Provisional or uncertain claims

- Crate versions ещё не pinned; `snow` replacement и OQ-7 RS bench зависят от size/bench; 256 KiB flash и 48 KiB static RAM Core — ⚠ gates, не достигнутые результаты (`12 §1, §4, §7, §9`).
- Rev-B ownership `AuthCert`/announcement описана как backlog `docs/AGILE_BACKLOG.md` E5-S5, still open (`12 §1`).
- Simulator должен de-⚠ constants 13; synthetic contact 50–150 kbps, mobility/adversary outputs — план в `12 §8`, не верифицированная performance claim. External crypto review OQ-10 — prerequisite перед v1.0 tag (`12 §10`).

### Dependencies and cross-references

Wire truth — `10`; crypto — `08`; TAL/roles — `03 §7`, `09`; FSM — `11`; UX events — `07 §6`; RBSR — `06 §3`; constants/failure tests — `13–14`; OQ-4/OQ-5/OQ-6/OQ-7/OQ-10 — `specification/research/open-questions.md`/указанные разделы. Реальные workspace names проверены только через `implementation/mesh-milk/Cargo.toml` и `implementation/mesh-milk/crates/`.

### Teaching implications

Предпосылки: trait seam отделяет policy от platform I/O; typestate не заменяет runtime validation; golden vectors независимым decoder ловят ambiguity. Специфика mesh_milk: no_std layering, KeyVault boundary, platform-owned mobile BLE и simulator с real core — намеренные архитектурные решения, а не универсальные требования Rust.

### Potential flaw / Alternative / Resolution

- **Performance Engineer — Potential flaw:** snow+dalek+blake3+chacha может превысить Core budget. **Alternative:** принять размер заранее. **Resolution:** CI size gate с первого дня; snow — первый кандидат замены, OQ-7 (`12 §9`).
- **Security Auditor — Potential flaw:** FFI отдаёт key material. **Alternative:** экспортировать crypto API. **Resolution:** KeyVault boundary; FFI только public keys/ciphertexts/events, signing через vault (`12 §9`).
- **Distributed Systems Reviewer — Potential flaw:** simulator создаст cfg-only routing. **Alternative:** отдельная reimplementation. **Resolution:** `mmp-sim` внедряется только через public trait seams и использует real core/router (`12 §8–9`).
