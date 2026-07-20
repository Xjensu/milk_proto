# Компетенции курса

Каждая компетенция проверяется наблюдаемым действием и артефактом, а не декларацией знания. Идентификаторы стабильны внутри доменов: PHY — physical/energy; NET — networking/wire; DTN — disruption/routing; SYN — synchronization/state; SEC — security/crypto; RST — Rust implementation; SIM — modeling/simulation/measurement; DES — requirements/architecture/tradeoffs/review.

## Шкала выпускного уровня

1 — объяснить на конкретном примере; 2 — рассчитать и проверить единицы/допущения; 3 — реализовать ограниченный артефакт; 4 — сломать его негативным случаем или adversarial test; 5 — защитить решение, альтернативы и остаточный риск перед review.

## Assessment gates

A — wire format + contact simulator; B — mini_milk end-to-end; C — mini_milk adversarial and scale review; D — capstone design review before implementation; E — reproducible capstone implementation and defense.

## PHY-01 — Рассчитывает ограничение радиоканала и airtime
- Действие: вычисляет link budget, airtime и duty-cycle для заданного beacon/bulk сценария, помечая каждое допущение.
- Доказательство: таблица единиц и расчёт с чувствительным анализом длины, скорости и повторов.
- Уровень выпуска: 2 — рассчитать и проверить единицы/допущения.
- Предпосылки: нет.
- Traceability: BLE-003, SIM-001.

## PHY-02 — Проектирует энергетический эксперимент вместо неподтверждённой цифры
- Действие: составляет воспроизводимый план измерения whole-phone энергии и сравнивает диапазоны множителя системы.
- Доказательство: протокол измерения с устройствами, нагрузкой, выборкой, порогом и правилом остановки.
- Уровень выпуска: 5 — защитить метод, альтернативы и остаточный риск.
- Предпосылки: PHY-01, DES-01.
- Traceability: SIM-002, SIM-003.

## PHY-03 — Ограничивает физические утверждения данными контакта
- Действие: отделяет link-budget расчёт от утверждений о GATT goodput, доступности ОС и delivery.
- Доказательство: review-заметка, отклоняющая недоказанный переход от PHY к product claim.
- Уровень выпуска: 4 — сломать некорректный вывод контрпримером.
- Предпосылки: PHY-01, DES-01.
- Traceability: BLE-005, SIM-001.

## NET-01 — Реализует и атакует канонический wire parser
- Действие: реализует bounded decoder frame/bundle и добавляет случаи trailing bytes, unknown-critical и over-bound input.
- Доказательство: mini_milk codec, negative test matrix и byte-level expected results.
- Уровень выпуска: 4 — сломать parser некорректным входом.
- Предпосылки: RST-01.
- Traceability: WIR-001, WIR-002, WIR-004, BND-001.

## NET-02 — Защищает FSM контакта и сессии
- Действие: строит таблицу состояний handshake, summary, reconciliation, transfer, timeout и protocol error; проверяет запрещённые переходы.
- Доказательство: переходная таблица, тесты timeout/close и трасса одного успешного контакта.
- Уровень выпуска: 4 — сломать FSM недопустимой последовательностью.
- Предпосылки: NET-01, RST-03.
- Traceability: FSM-001, FSM-003, FSM-004, FSM-006.

## NET-03 — Проектирует TAL/GATT границу без ложной platform-конформности
- Действие: задаёт адаптер событий link-up/data/link-down, MTU chunking и BLE discovery/bulk роли; фиксирует непроверенные платформы.
- Доказательство: contract test или mock trace TAL и перечень platform evidence, необходимого до claims.
- Уровень выпуска: 5 — защитить границу, альтернативы и ограничения.
- Предпосылки: NET-02, PHY-03, RST-03.
- Traceability: TAL-001, BLE-001, BLE-003, BLE-004, BLE-006.

## DTN-01 — Защищает bounded routing policy
- Действие: сравнивает Binary Spray-and-Wait с flooding и обосновывает split copy budget по локальным наблюдениям.
- Доказательство: мини-симуляция или расчёт передачи с budget ledger и defense memo.
- Уровень выпуска: 5 — защитить выбор, альтернативы и остаточный риск.
- Предпосылки: PHY-01, DES-01.
- Traceability: RTE-001, RTE-002, RTE-003.

## DTN-02 — Реализует bounded store-and-forward
- Действие: применяет verify-before-store, TTL aging, dedup, quota и copy-budget clamp к очереди bundles.
- Доказательство: mini_milk store с тестами expiry, duplicate resurrection и quota eviction.
- Уровень выпуска: 4 — сломать store negative cases.
- Предпосылки: NET-01, RST-02.
- Traceability: BND-002, BND-003, BND-004, RTE-004, STO-001, STO-002.

## DTN-03 — Проверяет receipt и fairness policy
- Действие: различает exact receipt и probabilistic hint, затем оценивает origin quota при конкуренции.
- Доказательство: таблица purge decisions и тест, где Bloom hit не удаляет bundle.
- Уровень выпуска: 4 — сломать ошибочное purge-правило.
- Предпосылки: DTN-02, SYN-01.
- Traceability: RTE-005, RTE-006, STO-004.

## SYN-01 — Реализует bounded reconciliation
- Действие: вычисляет различие двух bounded summary и передаёт только запрошенные bundles.
- Доказательство: mini_milk RBSR trace для disjoint и overlapping sets с лимитами памяти.
- Уровень выпуска: 3 — реализовать ограниченный артефакт.
- Предпосылки: NET-01, DTN-02.
- Traceability: SYN-001, SYN-004, SYN-007.

## SYN-02 — Разрешает причинные зависимости в Merkle-DAG
- Действие: добавляет события с parent hashes, выявляет missing parents и сохраняет causal order при merge.
- Доказательство: DAG fixture с concurrent writers и проверкой frontier/зависимостей.
- Уровень выпуска: 4 — сломать merge отсутствующим или циклическим parent.
- Предпосылки: SYN-01, RST-02.
- Traceability: SYN-002, SYN-003.

## SYN-03 — Выбирает CRDT по семантике состояния
- Действие: сопоставляет LWW claim, OR-Set group и DAG event с их конфликтами, затем реализует один merge.
- Доказательство: decision table и OR-Set add/remove test с concurrent additions.
- Уровень выпуска: 4 — сломать неверно выбранную merge-семантику.
- Предпосылки: SYN-02.
- Traceability: SYN-005, SYN-006, FSM-005.

## SEC-01 — Формализует threat model, non-goals и инварианты
- Действие: извлекает assets, local active adversary, trust boundaries, non-goals и verify-before-store invariant в attack matrix.
- Доказательство: requirements/threat/non-goals sheet с трассировкой каждого control к threat.
- Уровень выпуска: 5 — защитить границы модели и остаточный риск.
- Предпосылки: нет.
- Traceability: SEC-001, SEC-002, SEC-004, BND-002.

## SEC-02 — Проверяет криптографические контексты и identity
- Действие: проверяет Ed25519 identity, domain-separated signature, AEAD associated data и authenticated handshake на фиксированных input.
- Доказательство: тестовый набор с позитивным и cross-domain negative case.
- Уровень выпуска: 4 — сломать неверный domain/context.
- Предпосылки: SEC-01, RST-01.
- Traceability: ID-001, ID-002, CRY-001, CRY-002, CRY-003.

## SEC-03 — Защищает rotation и остаточный crypto risk
- Действие: моделирует ratchet, sender-key epoch rotation и forward-secrecy последствия компрометации, включая непроверенные persistence/platform claims.
- Доказательство: compromise timeline и capstone security review с residual-risk register.
- Уровень выпуска: 5 — защитить меры и нерешённые риски.
- Предпосылки: SEC-02, SYN-02.
- Traceability: CRY-004, CRY-005, CRY-008, AUT-005.

## RST-01 — Применяет Rust типы, ownership и Result к протокольной границе
- Действие: моделирует идентификаторы, bounded input и failure path типами; объясняет ownership заимствований в decoder.
- Доказательство: mini_milk crate с typed API и tests для ошибок без panic.
- Уровень выпуска: 3 — реализовать ограниченный артефакт.
- Предпосылки: нет.
- Traceability: IMP-001, WIR-001, WIR-004.

## RST-02 — Реализует traits, serialization и storage contract
- Действие: определяет минимальный trait для store/host и сериализует только канонический protocol state.
- Доказательство: trait contract, in-memory implementation и snapshot-corruption test.
- Уровень выпуска: 3 — реализовать ограниченный артефакт.
- Предпосылки: RST-01.
- Traceability: STO-001, STO-003, STO-006, IMP-002.

## RST-03 — Оркестрирует async событийный adapter без блокировки core
- Действие: связывает monotonic tick, TAL events, bounded channel и explicit error propagation в mock adapter.
- Доказательство: async/event trace с link lifecycle и structured diagnostics.
- Уровень выпуска: 3 — реализовать ограниченный артефакт.
- Предпосылки: RST-01, NET-01.
- Traceability: TAL-001, FSM-001, IMP-002.

## RST-04 — Строит portability evidence для vectors, fuzz и no_std
- Действие: подготавливает golden vectors, independent decoder check, parser fuzz corpus/run и cross-target build matrix.
- Доказательство: versioned vectors, fuzz report с allocation bounds и CI matrix для desktop/mobile/embedded targets.
- Уровень выпуска: 5 — защитить interoperability и robustness evidence.
- Предпосылки: RST-01, RST-02, NET-01.
- Traceability: WIR-005, WIR-006, IMP-003, IMP-004, IMP-005, IMP-006.

## SIM-01 — Строит детерминированный DTN experiment
- Действие: фиксирует seed, topology/contact schedule, workload и outcome metrics, затем воспроизводит запуск.
- Доказательство: сценарий simulator с seed, assertions для delivery/receipt/quota и result table.
- Уровень выпуска: 3 — реализовать ограниченный артефакт.
- Предпосылки: DTN-01, SYN-01, RST-03.
- Traceability: SIM-001, RTE-001.

## SIM-02 — Опровергает невалидные выводы модели
- Действие: изменяет допущение контакта, loss, storage или energy и показывает, какая метрика перестаёт поддерживать claim.
- Доказательство: sensitivity report с baseline, variant, confidence и явно исключёнными факторами.
- Уровень выпуска: 4 — сломать вывод модели контрсценарием.
- Предпосылки: SIM-01, PHY-01.
- Traceability: SIM-001, FIL-004, DEP-003.

## SIM-03 — Защищает план field validation
- Действие: проектирует измерение battery, long-running fuzz и platform/OEM drills для release gate.
- Доказательство: preregistered plan с sample, instrumentation, pass bands, failure handling и воспроизводимым raw-data schema.
- Уровень выпуска: 5 — защитить validation plan и границы обобщения.
- Предпосылки: PHY-02, SIM-02, SEC-01.
- Traceability: SIM-002, SIM-003.

## DES-01 — Превращает требования в проверяемые границы
- Действие: формулирует scale tiers, latency, retention, battery, threat model и non-goals как testable acceptance statements.
- Доказательство: requirements sheet с owner, metric, evidence и unresolved question для каждого ограничения.
- Уровень выпуска: 2 — рассчитать и проверить единицы/допущения.
- Предпосылки: нет.
- Traceability: SEC-001, SIM-001, SIM-003.

## DES-02 — Защищает архитектурный выбор и альтернативы
- Действие: сравнивает beacon/GATT, bounded routing, reconciliation и crypto seams с отклонёнными альтернативами по constraints.
- Доказательство: architecture decision record с rationale, tradeoffs, evidence confidence и adversarial review block.
- Уровень выпуска: 5 — защитить архитектуру перед review.
- Предпосылки: DES-01, PHY-03, DTN-01, SYN-01, SEC-01.
- Traceability: PLN-001, TAL-001, RTE-001, SYN-001, CRY-001.

## DES-03 — Проводит evidence review без завышения конформности
- Действие: классифицирует claim как implemented, partial, specified-only или conflict и связывает его с audit evidence/discrepancy.
- Доказательство: capstone review memo с trace links, confidence, potential flaw/alternative/resolution и residual-risk register.
- Уровень выпуска: 5 — защитить выводы и неизвестности.
- Предпосылки: DES-02, RST-04, SIM-03.
- Traceability: BLE-001, WIR-005, WIR-006, EVD-001.
