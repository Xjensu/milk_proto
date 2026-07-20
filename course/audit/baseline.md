# Базовая точка аудита курса

## Revision

Изолированная worktree `course-foundation-audit` зафиксирована на коммите
`a21d84bbe31db060264730d28eb978f69349bf20` (аудит: 2026-07-20). На момент
фиксации `git status --short` в этой worktree не вывел строк: baseline чистый
и состоит только из закоммиченной ревизии.

## Working-tree caveat

Этот baseline относится только к чистой изолированной worktree и к указанному
коммиту. В исходной checkout `C:\Users\Asus\Desktop\milk_proto` на дату
аудита были видны пользовательские изменённые и неотслеживаемые файлы. Они не
входят в baseline, не копировались в worktree и не редактировались в рамках
аудита; результаты курса нельзя истолковывать как проверку этого грязного
состояния.

## Rust toolchain

Буквальное содержимое `implementation/mesh-milk/rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
targets = ["thumbv7em-none-eabihf"]
```

## Specification inventory

Инвентаризация командой `rg --files specification | Sort-Object` обнаружила
18 основных документов: `01-executive-summary.md` …
`18-authority-announcements.md`. Также обнаружены три Markdown-файла
исследований: `research/bibliography.md`, `research/open-questions.md` и
`research/systems-comparison.md`.

## Implementation inventory

Команда `cargo metadata --manifest-path implementation/mesh-milk/Cargo.toml --no-deps --format-version 1`
зафиксировала workspace из 16 crate-пакетов: `mmp-cli`, `mmp-core`,
`mmp-crypto`, `mmp-ffi`, `mmp-node`, `mmp-node-embassy`, `mmp-router`,
`mmp-rs-code`, `mmp-sim`, `mmp-store`, `mmp-tal`, `mmp-tal-btleplug`,
`mmp-tal-embedded`, `mmp-tal-mobile`, `mmp-tal-peripheral`, `mmp-types`; и
инструмент `layer-check`. Инвентаризация `rg --files implementation/mesh-milk/crates implementation/mesh-milk/vectors | Sort-Object`
подтвердила исходники этих crate и `vectors/README.md`. На baseline отдельные
файлы test-векторов в `vectors/` отсутствуют; это факт инвентаризации, а не
вывод об их необходимости.

## Evidence layers

- Спецификация: документы `specification/01`–`18` задают нормативные и
  проектные требования.
- Исследования: три файла `specification/research/` содержат библиографию,
  сравнение систем и открытые вопросы.
- Реализация: исходный код и манифесты crate в `implementation/mesh-milk/`
  показывают фактическое состояние reference implementation.
- Тесты и векторы: тестовые исходники crate и каталог `vectors/` являются
  исполнимыми либо документированными проверочными материалами; их покрытие
  не предполагается без отдельной проверки.

## Audit method

Аудит сначала читает спецификацию и исследовательские материалы, затем
классифицирует каждый crate по ответственности и сверяет требования с кодом,
тестами и векторами. Каждое требование получает запись в traceability; при
отсутствии реализации оно помечается как unmapped. Расхождения между слоями
переносятся без исправления в `discrepancies.md` с указанием источника.

## Completion rule

Завершение аудита означает: прочитаны все 18 спецификаций и исследовательские
файлы; классифицированы все crate; все критичные для курса требования
сопоставлены либо помечены unmapped; все конфликты перенесены в
`discrepancies.md`.
