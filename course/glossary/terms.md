# Словарь курса

Порядок стабилен и концептуален: сначала границы протокола и носителя, затем DTN и состояние, затем безопасность и бюджеты. Английское слово в заголовке — термин спецификации; русский текст — рабочее объяснение курса.

## Аутентифицированное шифрование с ассоциированными данными (`AEAD`)
- Кратко: Шифрование с аутентификацией связанных данных.
- Точно: AEAD одновременно скрывает plaintext и отвергает изменённые ciphertext или associated data.
- Не путать с: Подписью: подпись публично проверяема, AEAD обычно требует секретного ключа.
- В mesh_milk: Набор шифров включает ChaCha20-Poly1305; контекст и домен должны быть привязаны к данным ([08 §1](../../specification/08-crypto-spec.md#1-cipher-suite-0x01-mandatory-v1)).

## Противник (`adversary`)
- Кратко: Сторона, пытающаяся нарушить свойства системы.
- Точно: Модель определяет возможности противника, а не предполагаемые мотивы или конкретную реализацию атаки.
- Не путать с: Ошибкой сети: случайная потеря пакета не обязательно активная атака.
- В mesh_milk: Рассматривается локальный активный противник, поэтому входы проверяются до хранения и пересылки ([07 §1](../../specification/07-security-spec.md)).

## Эфирное время (`airtime`)
- Кратко: Время, в течение которого радиоканал занят передачей.
- Точно: Это ресурс канала, зависящий от длины, PHY и повторов, а не только число полезных байтов.
- Не путать с: Пропускной способностью приложения или временем CPU.
- В mesh_milk: Airtime ограничивает relay-нагрузку и отдельно учитывается для beacon и bulk plane ([13 §2](../../specification/13-performance-model.md#2-contact-capacity)).

## Аутентификация (`authentication`)
- Кратко: Проверка того, кем является предъявитель ключа или учётных данных.
- Точно: Она связывает доказательство с криптографической идентичностью, но не выдаёт разрешения.
- Не путать с: Авторизацией или отображаемым никнеймом.
- В mesh_milk: Ed25519-подписи и handshake аутентифицируют ключевой материал ([08 §2](../../specification/08-crypto-spec.md#2-key-hierarchy)).

## Авторизация (`authorization`)
- Кратко: Решение, разрешено ли уже аутентифицированному субъекту действие.
- Точно: Политика проверяет область, полномочие и ограничения после установления личности.
- Не путать с: Аутентификацией: действительный ключ сам по себе не даёт повышенный приоритет.
- В mesh_milk: Authority certificates ограничивают scope и capability объявлений ([18 §2](../../specification/18-authority-announcements.md#2-authority-certificates)).

## Доступность (`availability`)
- Кратко: Возможность получить сервис или доставку при допустимых условиях.
- Точно: Это свойство с явными ограничениями ресурсов, отказов и модели противника.
- Не путать с: Конфиденциальностью или гарантией немедленной доставки.
- В mesh_milk: DTN принимает минуты задержки и использует bounded replication вместо обещания непрерывной связи ([03 §1](../../specification/03-protocol-spec.md)).

## Маяк (`beacon`)
- Кратко: Короткое broadcast-объявление для обнаружения и подсказок.
- Точно: Beacon несёт строго ограниченную схему и не заменяет аутентифицированный bundle.
- Не путать с: Пакетом bulk-передачи или доказательством доставки.
- В mesh_milk: Legacy beacon — hint plane; production layout и UUID имеют открытый блокер DISC-001 ([09 §§2–3](../../specification/09-transport-ble.md#2-service-identity)).

## Фильтр Блума (`Bloom filter`)
- Кратко: Вероятностное компактное представление множества.
- Точно: Он допускает ложные совпадения, но при корректном построении не должен давать ложных отрицаний.
- Не путать с: Точным журналом receipt или доказательством наличия.
- В mesh_milk: Receipt filter служит только hint и не может сам удалить bundle ([06 §4](../../specification/06-sync-spec.md)).

## Бандл (`bundle`)
- Кратко: DTN-единица хранения, переноса и проверки.
- Точно: Bundle имеет каноническую форму, идентификатор, lifetime и подписанный origin-контекст.
- Не путать с: OSI packet, BLE frame или произвольным payload.
- В mesh_milk: Bundle проверяется канонически и криптографически до store/forward ([03 §4.1](../../specification/03-protocol-spec.md#41-canonical-encoding)).

## Каноническое кодирование (`canonical encoding`)
- Кратко: Единственное допустимое байтовое представление значения.
- Точно: Декодер отвергает альтернативные, хвостовые, неупорядоченные и недопустимые формы.
- Не путать с: Любой успешной сериализацией или pretty-print форматом.
- В mesh_milk: Каноничность фиксирует bytes для BundleID и подписей ([10 §6](../../specification/10-message-format.md#6-canonical-encoding-and-parser-rules-normative)).

## Причинный порядок (`causal order`)
- Кратко: Порядок, сохраняющий зависимость события от ранее наблюдённых событий.
- Точно: Если A могло повлиять на B, реплика не применяет B как независимое от A.
- Не путать с: Глобальным временем, FIFO всех узлов или total order.
- В mesh_milk: Group-события связываются parent hashes в DAG, а не общими часами ([06 §5](../../specification/06-sync-spec.md)).

## Конфиденциальность (`confidentiality`)
- Кратко: Недоступность содержания неуполномоченному наблюдателю.
- Точно: Свойство относится к данным и ключам, но не скрывает автоматически метаданные или наличие контакта.
- Не путать с: Целостностью, анонимностью или авторизацией.
- В mesh_milk: Sealed/direct payload и групповые ключи задают защиту содержания ([08 §§4,6](../../specification/08-crypto-spec.md#4-asynchronous-dtn-message-keys-first-contact-between-identities)).

## Контакт (`contact`)
- Кратко: Ограниченное по времени окно связи двух узлов.
- Точно: Контакт имеет начало, окончание, доступный канал и ограниченный budget обмена.
- Не путать с: Encounter как фактом сближения или долговременным маршрутом.
- В mesh_milk: Session FSM использует контакт для handshake, summary, reconciliation и handoff ([11 §3](../../specification/11-state-machines.md#3-session-fsm-one-per-peer-connection-consumes-doc-10-frames)).

## Бюджет копий (`copy budget`)
- Кратко: Ограниченное число разрешённых реплик bundle.
- Точно: Каждая передача расходует или делит budget по правилам, чтобы ограничить распространение.
- Не путать с: TTL, quota или числом фактически принятых копий.
- В mesh_milk: Mutable per-hop budget receive-clamp'ится до L_MAX ([03 §4.5](../../specification/03-protocol-spec.md#45-mutable-per-hop-metadata)).

## Сходящийся тип данных / наблюдаемый remove-set (`CRDT / OR-Set`)
- Кратко: CRDT — структура со сходимостью реплик; OR-Set — наблюдаемый remove-set.
- Точно: OR-Set различает добавления по тегам и удаления только наблюдённых добавлений.
- Не путать с: LWW register или произвольной merge-функцией.
- В mesh_milk: OR-Set выбран для группового состояния, а не для всех данных ([06 §5](../../specification/06-sync-spec.md)).

## Ориентированный ациклический граф (`DAG`)
- Кратко: Ориентированный ациклический граф зависимостей.
- Точно: Ребро parent указывает на предшествующее событие; отсутствие циклов позволяет вычислять зависимые frontier.
- Не путать с: Деревом: у вершины DAG может быть несколько родителей.
- В mesh_milk: Group causal state использует Merkle-аутентифицируемый DAG ([06 §5](../../specification/06-sync-spec.md)).

## Рабочий цикл (`duty cycle`)
- Кратко: Доля времени, когда радио или компонент активно работает.
- Точно: Его считают за заданное окно и связывают с энергией, scan/advertise политикой и режимом узла.
- Не путать с: Airtime одной передачи или latency.
- В mesh_milk: Duty-cycle допущения входят в телефонную энергетическую модель ([13 §1](../../specification/13-performance-model.md#1-phone-energy-budget-5day-target)).

## Сеть с задержками и разрывами (`DTN`)
- Кратко: Delay/Disruption-Tolerant Networking для связности с разрывами.
- Точно: Данные хранятся у промежуточных узлов и переносятся между редкими контактами.
- Не путать с: Непрерывной IP-сетью с end-to-end path в момент отправки.
- В mesh_milk: DTN store-carry-forward — основной режим, поэтому минуты задержки допустимы ([03 §1](../../specification/03-protocol-spec.md)).

## Встреча (`encounter`)
- Кратко: Наблюдаемое физическое или радиосближение узлов.
- Точно: Encounter может не дать пригодного соединения из-за политики ОС, роли или времени.
- Не путать с: Успешным session contact.
- В mesh_milk: BLE discovery превращает encounter в попытку контакта, а не в обещание передачи ([09 §4](../../specification/09-transport-ble.md#4-discovery-dial-decision)).

## Эпоха (`epoch`)
- Кратко: Версия ключевого или группового состояния.
- Точно: Переход epoch меняет допустимый ключевой материал и границы применения сообщений.
- Не путать с: Unix timestamp или TTL bundle.
- В mesh_milk: Sender keys вращаются при изменении группы и отделяются по epoch ([08 §6](../../specification/08-crypto-spec.md#6-group-messaging-sender-keys-epochs)).

## Прямая секретность (`forward secrecy`)
- Кратко: Компрометация текущего ключа не раскрывает прошлые защищённые сообщения.
- Точно: Свойство требует эволюции/стирания секретов и корректного ограничения истории ключей.
- Не путать с: Посткомпромиссной безопасностью или простым шифрованием долгим ключом.
- В mesh_milk: Noise/ratchet выбор должен оцениваться вместе с persistence и recovery ([08 §§4–5](../../specification/08-crypto-spec.md#4-asynchronous-dtn-message-keys-first-contact-between-identities)).

## Кадр (`frame`)
- Кратко: Структурированная единица обмена в session/wire протоколе.
- Точно: Frame имеет type и bounded payload, который парсится по каноническим правилам.
- Не путать с: Bundle: frame переносит управляющие и data-plane сообщения, bundle живёт в DTN store.
- В mesh_milk: Handshake, summary, want и transfer упорядочены session FSM ([10 §4](../../specification/10-message-format.md)).

## Профиль обобщённых атрибутов BLE (`GATT`)
- Кратко: BLE Generic Attribute Profile для структурированного обмена через connection.
- Точно: Это BLE application-layer модель service/characteristic операций, не DTN routing protocol.
- Не путать с: Advertising beacon или самим payload форматом MMP.
- В mesh_milk: GATT bulk plane переносит session frames после discovery ([09 §5](../../specification/09-transport-ble.md#5-bulk-plane)).

## Идентичность (`identity`)
- Кратко: Криптографически проверяемый субъект.
- Точно: Идентичность определяется ключом, а не nickname, устройством или сетевым адресом.
- Не путать с: NodeID-сокращением, petname или account в облаке.
- В mesh_milk: Ed25519 public key — псевдонимная identity; NodeID — её производное ([03 §3](../../specification/03-protocol-spec.md#3-identity-normative-summary-full-treatment-in-doc-08)).

## Целостность (`integrity`)
- Кратко: Обнаружимость несанкционированного изменения данных.
- Точно: Получатель проверяет аутентификатор над точным контекстом и отклоняет несоответствие.
- Не путать с: Доступностью, конфиденциальностью или защитой от replay.
- В mesh_milk: Подписи, AEAD и Merkle proofs защищают разные контексты целостности ([08 §§1,3,8](../../specification/08-crypto-spec.md#1-cipher-suite-0x01-mandatory-v1)).

## Интерфейс (`interface`)
- Кратко: Явная граница между компонентами с контрактом входов и выходов.
- Точно: Интерфейс задаёт ответственность и допустимые данные, не обязательно сетевой wire format.
- Не путать с: Пользовательским UI, BLE service или Rust trait без контекста ответственности.
- В mesh_milk: TAL и storage seams отделяют platform adaptation от protocol core ([12 §2](../../specification/12-rust-blueprint.md#2-core-trait-surface-the-two-seams-platform-ports)).

## Инвариант (`invariant`)
- Кратко: Свойство, которое должно сохраняться во всех допустимых переходах.
- Точно: Инвариант проверяется на границах состояния, а не предполагается по обычному пути выполнения.
- Не путать с: Примером поведения, метрикой или необязательной оптимизацией.
- В mesh_milk: Verify-before-store/forward и bounded lifetime — проверяемые инварианты ([03 §4](../../specification/03-protocol-spec.md)).

## Энергетический баланс линии (`link budget`)
- Кратко: Баланс мощности передачи, потерь и чувствительности приёма.
- Точно: Он выражает запас физического канала при заданных предположениях и не гарантирует session goodput.
- Не путать с: Airtime budget, энергетическим бюджетом или количеством копий.
- В mesh_milk: Физические допущения должны быть отделены от DTN delivery claim ([13 §2](../../specification/13-performance-model.md#2-contact-capacity)).

## Аутентифицированный хешами граф (`Merkle-DAG`)
- Кратко: DAG, чьи узлы или связи аутентифицируются hash-значениями.
- Точно: Hash связывает содержимое и зависимости, позволяя проверять целостность и искать отсутствующие вершины.
- Не путать с: Merkle tree с единственным деревообразным путём.
- В mesh_milk: Parent-hash DAG используется для причинного группового состояния ([06 §5](../../specification/06-sync-spec.md)).

## Максимальный размер передаваемого блока (`MTU`)
- Кратко: Наибольший размер единицы данных на данной границе передачи.
- Точно: MTU относится к конкретному layer/adapter и требует fragmentation или bounded frame design выше него.
- Не путать с: Максимальным размером bundle или BLE advertising payload.
- В mesh_milk: ATT chunking — обязанность mobile transport adapter, а не изменение wire grammar ([09 §5](../../specification/09-transport-ble.md#5-bulk-plane)).

## Пакет (`packet`)
- Кратко: Единица данных, определённая конкретным сетевым или канальным протоколом.
- Точно: Смысл packet определяется его layer и не переносится автоматически между OSI, BLE и DTN.
- Не путать с: MMP bundle: bundle не следует называть OSI packet.
- В mesh_milk: Курс называет wire units frames, DTN units bundles, а BLE units — transport fragments ([10 §3](../../specification/10-message-format.md#3-bundle-layout-normative-bit-level-expands-doc-03-41)).

## Полезная нагрузка (`payload`)
- Кратко: Содержательная часть контейнера после управляющих полей.
- Точно: Payload имеет собственный type, bounds и криптографический domain context.
- Не путать с: Полным bundle, frame header или transport fragment.
- В mesh_milk: Direct, group и receipt payload кодируются раздельно ([10 §3](../../specification/10-message-format.md#3-bundle-layout-normative-bit-level-expands-doc-03-41)).

## Протокол (`protocol`)
- Кратко: Набор правил формата, переходов и проверок между независимыми участниками.
- Точно: Протокол задаёт наблюдаемое совместимое поведение, включая ошибки и ограничения.
- Не путать с: Одной библиотекой, service API или implementation-only форматом.
- В mesh_milk: Норма распределена по спецификациям 03–11; локальный MMPSNAP2 не является wire protocol ([12 §1](../../specification/12-rust-blueprint.md#1-workspace-layout)).

## Квота (`quota`)
- Кратко: Явный лимит ресурса для субъекта, класса или хранилища.
- Точно: Quota задаёт измеряемое ограничение и policy реакции при превышении.
- Не путать с: TTL: quota ограничивает объём, TTL — возраст.
- В mesh_milk: Storage и origin quotas ограничивают relay без бесконечного роста ([05 §8](../../specification/05-routing-spec.md#8-fairness-quotas-anti-abuse)).

## Храповик ключей (`ratchet`)
- Кратко: Механизм одностороннего продвижения ключевого состояния.
- Точно: После шага новый ключ выводится из состояния, а старый материал должен быть ограничен или уничтожен по политике.
- Не путать с: Простым счётчиком epoch или handshake без последующих сообщений.
- В mesh_milk: Ratchet поддерживает out-of-order окно в direct-message crypto path ([08 §5](../../specification/08-crypto-spec.md#5-conversation-ratchet-pairwise)).

## Квитанция (`receipt`)
- Кратко: Криптографически проверяемое свидетельство доставки или обработки.
- Точно: Receipt несёт ограниченную семантику и не равен вероятностной подсказке фильтра.
- Не путать с: Bloom filter hit или подтверждением радиокадра.
- В mesh_milk: Exact receipt допускает purge, тогда как filter — только advisory hint ([06 §4](../../specification/06-sync-spec.md)).

## Согласование множеств (`reconciliation`)
- Кратко: Обмен, выявляющий различия реплик и передающий недостающие элементы.
- Точно: Он должен переносить только доказуемо нужные данные при bounded памяти и контакте.
- Не путать с: Полной пересылкой всех bundles или causal merge сам по себе.
- В mesh_milk: RBSR согласует bounded bundle summaries в session ([06 §3](../../specification/06-sync-spec.md)).

## Маршрутизация (`routing`)
- Кратко: Выбор того, какие данные и копии переносить через контакт.
- Точно: В DTN routing использует локальные наблюдения, budgets и policy без обязательного end-to-end path.
- Не путать с: Forwarding — конкретной передачей уже выбранной копии.
- В mesh_milk: Выбран bounded Binary Spray-and-Wait с локальной utility ([05 §2](../../specification/05-routing-spec.md)).

## Сервис (`service`)
- Кратко: Набор возможностей, предоставляемых через определённую границу.
- Точно: Service описывает назначение и контракт, но не обязательно конкретный протокол wire bytes.
- Не путать с: BLE GATT service UUID, Rust crate или облачной службой.
- В mesh_milk: BLE service — транспортная точка доступа; protocol semantics остаются выше TAL ([09 §5](../../specification/09-transport-ble.md#5-bulk-plane)).

## Машина состояний (`state machine`)
- Кратко: Модель допустимых состояний и переходов системы.
- Точно: Каждый переход имеет вход, guard, эффект и timeout/error обработку.
- Не путать с: Списком функций или диаграммой happy path.
- В mesh_milk: Отдельные FSM задают lifecycle, session, bundle и другие границы ([11 §1](../../specification/11-state-machines.md#1-node-lifecycle-fsm)).

## Хранение-и-пересылка (`store-and-forward`)
- Кратко: Сначала сохранить данные, затем передать при будущем контакте.
- Точно: Промежуточный узел владеет bounded store и применяет expiry, dedup и policy до следующего hop.
- Не путать с: Сквозным streaming-соединением или broadcast flooding.
- В mesh_milk: Carrier уменьшает lifetime, а expired bundle очищается ([03 §4.3](../../specification/03-protocol-spec.md#43-lifetime-age-based-no-global-clock)).

## Модель угроз (`threat model`)
- Кратко: Явное описание активов, противника, границ и целей защиты.
- Точно: Он ограничивает обещания системы и задаёт проверяемые attack scenarios.
- Не путать с: Перечнем криптоалгоритмов или общим списком угроз.
- В mesh_milk: Локальный активный adversary и pseudonymous identities — исходные требования ([07 §1](../../specification/07-security-spec.md)).

## Слой адаптации транспорта (`transport adaptation layer`)
- Кратко: Слой, переводящий platform/radio события в нейтральные события протокола.
- Точно: TAL изолирует MTU, connection lifecycle и API ОС, не меняя core message semantics.
- Не путать с: Routing layer, GATT service или application UI.
- В mesh_milk: TAL event seam отделяет core от BLE/mobile/embedded adapters ([12 §2](../../specification/12-rust-blueprint.md#2-core-trait-surface-the-two-seams-platform-ports)).

## Время жизни (`TTL`)
- Кратко: Время жизни объекта до обязательного истечения.
- Точно: TTL измеряет возраст согласно правилам протокола и становится нулём независимо от удобства relay.
- Не путать с: Quota, session timeout или криптографическим epoch.
- В mesh_milk: Lifetime age-based, receive-clamped и уменьшается carriers без global clock ([03 §4.3](../../specification/03-protocol-spec.md#43-lifetime-age-based-no-global-clock)).

## Пересылка (`forwarding`)
- Кратко: Фактическая передача выбранных данных следующему peer.
- Точно: Она применяется после admission, verification, routing и quota решений.
- Не путать с: Routing как выбором политики или replication без реального контакта.
- В mesh_milk: Невалидный bundle MUST быть отброшен до forwarding ([03 §4.1](../../specification/03-protocol-spec.md#41-canonical-encoding)).
