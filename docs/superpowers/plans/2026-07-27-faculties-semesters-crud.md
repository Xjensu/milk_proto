# Faculties & Semesters CRUD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let admins create, edit, and delete Faculties and Semesters from the Reference Data tab, matching the CRUD experience every other reference resource (Teachers, Rooms, Disciplines, ...) already has.

**Architecture:** The backend (`apps/api_geteway`) already exposes full CRUD for both (`resources :faculties`/`resources :semesters` in `config/routes.rb`, with `create`/`update`/`destroy` actions and contracts). The frontend already has a generic, field-config-driven CRUD machine (`ReferenceResourcePanel` + `RecordInspector` in `AdminResourcePage.tsx`) used by Teachers/Rooms/Disciplines/Activity types/Time slots/Week types. Faculties and Semesters are the only two reference resources still on the old read-only `staticItems` fallback path. This plan moves them onto the generic machine — no new components, no new API methods (the already-present `api.createResource`/`updateResource`/`deleteResource` cover both once they're `AdminCollection` members, which they already are).

**Tech Stack:** React 19 + TypeScript, Vite, vitest (SSR `renderToStaticMarkup` smoke tests — this codebase has no jsdom/`@testing-library` installed, so tests assert on rendered markup strings only, not interaction).

## Global Constraints

- Codebase has no custom hooks — all data access goes through the single `api` object in `src/lib/api.ts`. Do not introduce a hook for this.
- Every admin resource CRUD panel in this file follows the `ReferenceResourceConfig` + `ReferenceResourcePanel` + `RecordInspector` pattern. Do not create a bespoke Faculty/Semester form component — extend the existing config array.
- New UI text in this file should be written in Russian (the sidebar shell and several existing labels already are; the surrounding English labels in this specific file are pre-existing debt out of scope for this phase — do not do a wholesale i18n pass here).
- Verification bar for this codebase (confirmed via project history across all prior admin-panel phases): `tsc -b` clean (`npm run lint`) + `vitest run` (`npm run test`) green + manual browser check against the running stack. There is no jsdom interaction-test setup to extend — do not add one speculatively for this phase.

---

### Task 1: Add `min`/`max` support to the generic field config

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminResourcePage.tsx:185-186` (`FieldKind`/`FieldConfig` types)
- Modify: `apps/frontend/src/features/admin/AdminResourcePage.tsx:343-348` (`RecordInspector` field rendering)

**Interfaces:**
- Produces: `FieldConfig` gains optional `min?: number` and `max?: number`, applied to the rendered `<input type="number">` when present. Needed because `semesters.number` is constrained to `1|2` by the backend contract (`contracts/semesters/create_contract.rb`), and start/end year should be sanity-bounded the same way `StudentGroups`' admission-year field already is (`min="2000" max="2100"` at line 148).

- [ ] **Step 1: Extend `FieldConfig`**

In `AdminResourcePage.tsx`, change:
```ts
type FieldConfig = { key: string; label: string; kind: FieldKind; required?: boolean; placeholder?: string };
```
to:
```ts
type FieldConfig = { key: string; label: string; kind: FieldKind; required?: boolean; placeholder?: string; min?: number; max?: number };
```

- [ ] **Step 2: Pass `min`/`max` through to the rendered input**

In `RecordInspector`'s field-rendering map, change:
```tsx
: <input type={field.kind === "number" ? "number" : "text"} value={values[field.key] ?? ""} required={field.required} placeholder={field.placeholder} onChange={(e) => setValues((current) => ({ ...current, [field.key]: e.target.value }))} />}
```
to:
```tsx
: <input type={field.kind === "number" ? "number" : "text"} value={values[field.key] ?? ""} required={field.required} placeholder={field.placeholder} min={field.min} max={field.max} onChange={(e) => setValues((current) => ({ ...current, [field.key]: e.target.value }))} />}
```

- [ ] **Step 3: Type-check**

Run: `cd apps/frontend && npm run lint`
Expected: no new errors (this is a pure additive optional-prop change).

---

### Task 2: Register Faculties and Semesters as generic reference resources

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminResourcePage.tsx:197-224` (`referenceResources` array)
- Modify: `apps/frontend/src/features/admin/AdminResourcePage.tsx:226-246` (`ReferenceData` component)

**Interfaces:**
- Consumes: `ReferenceResourceConfig` type (unchanged shape besides Task 1's addition), `api.createResource`/`updateResource`/`deleteResource`/`listAdminCollection` (unchanged, already accept `"faculties"|"semesters"` since they're part of `AdminCollection` in `api.ts:53`), `semesterResourceLabel` (existing local function at line 455, reused as `labelFn`).
- Produces: two new entries in `referenceResources`; `ReferenceData` no longer special-cases faculties/semesters.

- [ ] **Step 1: Add the two config entries**

At the top of the `referenceResources` array (before the `teachers` entry), add:
```ts
  { id: "faculties", label: "Факультеты", singular: "Факультет", bodyKey: "faculty", search: "client", fields: [
    { key: "name", label: "Название", kind: "text", required: true },
    { key: "short_name", label: "Краткое название", kind: "text", required: true },
  ] },
  { id: "semesters", label: "Семестры", singular: "Семестр", bodyKey: "semester", search: "client", fields: [
    { key: "start_year", label: "Год начала", kind: "number", required: true, min: 2000, max: 2100 },
    { key: "end_year", label: "Год окончания", kind: "number", required: true, min: 2000, max: 2100 },
    { key: "number", label: "Номер семестра (1 — осенний, 2 — весенний)", kind: "number", required: true, min: 1, max: 2 },
  ], labelFn: semesterResourceLabel },
```

- [ ] **Step 2: Simplify `ReferenceData` to drop the dead `staticItems` fallback path**

Replace the whole function body:
```tsx
function ReferenceData({ canWrite }: { canWrite: boolean }) {
  const [selected, setSelected] = useState<string>("faculties");
  const [staticItems, setStaticItems] = useState<JsonApiResource[]>([]);
  const activeConfig = referenceResources.find((resource) => resource.id === selected);
  useEffect(() => {
    if (selected !== "faculties" && selected !== "semesters") return;
    api.listAdminCollection(selected).then((response) => setStaticItems(response.data));
  }, [selected]);
  const tabButtons: Array<{ id: string; label: string }> = [
    { id: "faculties", label: "Faculties" },
    { id: "semesters", label: "Semesters" },
    { id: "kafedras", label: "Departments" },
    ...referenceResources.map((resource) => ({ id: resource.id, label: resource.label })),
  ];
  return <>
    <div className="reference-resource-grid">{tabButtons.map((item) => <button type="button" className={selected === item.id ? "active" : ""} key={item.id} onClick={() => setSelected(item.id)}>{item.label}</button>)}</div>
    {selected === "kafedras" ? <KafedrasPanel canWrite={canWrite} /> : activeConfig
      ? <ReferenceResourcePanel key={activeConfig.id} config={activeConfig} canWrite={canWrite} />
      : <Panel className="catalog-list-panel"><CatalogHeading icon={<Library />} title={selected === "faculties" ? "Faculties" : "Semesters"} count={staticItems.length} /><div className="catalog-card-grid">{staticItems.map((item) => <article className="catalog-card" key={item.id}><strong>{selected === "semesters" ? semesterResourceLabel(item) : resourceLabel(item, "Faculty")}</strong></article>)}</div></Panel>}
  </>;
}
```
with:
```tsx
function ReferenceData({ canWrite }: { canWrite: boolean }) {
  const [selected, setSelected] = useState<string>("faculties");
  const activeConfig = referenceResources.find((resource) => resource.id === selected);
  const tabButtons: Array<{ id: string; label: string }> = [
    { id: "kafedras", label: "Кафедры" },
    ...referenceResources.map((resource) => ({ id: resource.id, label: resource.label })),
  ];
  return <>
    <div className="reference-resource-grid">{tabButtons.map((item) => <button type="button" className={selected === item.id ? "active" : ""} key={item.id} onClick={() => setSelected(item.id)}>{item.label}</button>)}</div>
    {selected === "kafedras" ? <KafedrasPanel canWrite={canWrite} /> : activeConfig
      ? <ReferenceResourcePanel key={activeConfig.id} config={activeConfig} canWrite={canWrite} />
      : null}
  </>;
}
```

Note: `"kafedras"` is moved to the front of `tabButtons` and its own label translated to `"Кафедры"` for consistency with the two new Russian entries sitting next to it — this is the one pre-existing label in this exact list I'm touching, since leaving `"Departments"` next to `"Факультеты"`/`"Семестры"` in the same button row would look broken, not just untranslated. No other pre-existing label in this file is touched.

- [ ] **Step 3: Remove now-dead code paths**

After Step 2, `resourceLabel` may still be used elsewhere in the file (it is — by `Specialities`, `KafedrasPanel`, `TeacherDepartments`) so keep the import. Confirm `staticItems`/its `useEffect` are fully gone (no leftover references) by re-reading the file section.

- [ ] **Step 4: Type-check**

Run: `cd apps/frontend && npm run lint`
Expected: no errors. This also catches if `JsonApiResource` import becomes unused (it won't — still used throughout the file).

---

### Task 3: Regression-test the default render, verify manually against the live stack

**Files:**
- Modify: `apps/frontend/src/features/admin/AdminResourcePage.test.tsx`

**Interfaces:**
- Consumes: `AdminResourcePage`, `WorkspaceProvider` (unchanged).

- [ ] **Step 1: Extend the existing smoke test to cover the reference-tab button labels are reachable from a static render of the reference tab's own subtree**

The existing tests only render `AdminResourcePage`'s default tab (`specialities`). Since there's no jsdom/click simulation in this suite, add a new `describe` block that renders `ReferenceData` indirectly isn't possible without exporting it — instead, assert the regression that matters at this layer: the top-level catalog tab strip still contains the four unchanged top-level tab labels, and the *English* leftover architecture (raw `staticItems` "Faculties"/"Semesters" fallback strings) is gone from the module. Add:

```tsx
  it("still exposes the four top-level catalog tabs after the reference-data refactor", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminResourcePage canWrite /></WorkspaceProvider>);
    expect(markup).toContain("Specialities");
    expect(markup).toContain("Student groups");
    expect(markup).toContain("Reservations");
    expect(markup).toContain("Reference data");
  });
```

This is intentionally light — it's a regression guard that the top-level tab bar still renders after editing `ReferenceData`'s internals, matching the shallow-smoke-test depth already established by this file's other two tests. Deeper coverage (actually reaching the Faculties/Semesters forms) requires manual verification in Step 3, since this suite has no way to simulate the tab click.

- [ ] **Step 2: Run the frontend test suite**

Run: `cd apps/frontend && npm run test`
Expected: all tests pass, including the new one.

- [ ] **Step 2.5 (found during manual verification, not foreseeable from the routes/contracts inventory alone): faculties require `short_name`**

Live-testing the create flow against the running stack surfaced a 422 from `POST /admin/faculties` with only `{name}` in the body — `contracts/faculties/create_contract.rb` requires both `name` and `short_name`. Added a second required text field (`short_name`, "Краткое название") to the faculty config in Task 2 Step 1 above. `update_contract.rb` makes both optional, so edits without changing the short name still work.

- [ ] **Step 3: Manual verification against the running stack**

With `docker-compose --env-file .env.local up -d --build` stack running (frontend on `:4173`, api_geteway on `:2300`):
1. Log in as an account with `canWrite` (super_admin or faculty_admin).
2. Navigate to Reference data → Факультеты. Confirm: list loads, "Create Факультет" opens a form with a "Название" field, submitting creates a faculty and it appears in the list, clicking a card opens edit mode pre-filled, saving persists, delete removes it.
3. Navigate to Семестры. Confirm the same create/edit/delete cycle, and that the number field only accepts 1 or 2 (browser-level `min`/`max` on the number input) and the created card's label renders as e.g. "1 semester 2026/2027" via `semesterResourceLabel`.
4. Confirm no console errors, and that both flows correctly hit `/admin/faculties` / `/admin/semesters` (check Network tab or api_geteway logs) rather than 404ing.

- [ ] **Step 4: Commit**

```bash
git add apps/frontend/src/features/admin/AdminResourcePage.tsx apps/frontend/src/features/admin/AdminResourcePage.test.tsx
git commit -m "feat(admin): add create/edit/delete for faculties and semesters"
```
