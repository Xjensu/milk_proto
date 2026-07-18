import { useEffect, useState } from "react";
import { BookOpen, CalendarPlus, ChevronLeft, ChevronRight, GraduationCap, Library, Plus, Search, Trash2 } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type AdminCollection, type JsonApiResource } from "../../lib/api";
import type { SemesterSummary } from "./domain";
import { formatSemester } from "./formatters";
import { resourceLabel } from "./utils";
import { EntityPicker, type EntityOption } from "./workspace/EntityPicker";
import { Inspector } from "./workspace/Inspector";
import { SemesterRail } from "./workspace/SemesterRail";
import { useWorkspace } from "./workspace/WorkspaceContext";

type CatalogTab = "specialities" | "groups" | "reservations" | "reference";
const tabs: Array<{ id: CatalogTab; label: string }> = [
  { id: "specialities", label: "Specialities" },
  { id: "groups", label: "Student groups" },
  { id: "reservations", label: "Reservations" },
  { id: "reference", label: "Reference data" },
];

// Shared create/edit state for the generic per-resource Inspector forms below.
type RecordMode = { type: "create" } | { type: "edit"; item: JsonApiResource } | null;

export function AdminResourcePage({ canWrite }: { canWrite: boolean }) {
  const [tab, setTab] = useState<CatalogTab>("specialities");
  return <div className="admin-section catalog-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Administration</span><h1>University catalog</h1></div><p>Browse and create academic entities through readable, scoped workflows.</p></div>
    <div className="admin-resource-tabs catalog-tabs" role="tablist" aria-label="Catalog sections">{tabs.map((item) => <button type="button" role="tab" aria-selected={tab === item.id} className={tab === item.id ? "active" : ""} key={item.id} onClick={() => setTab(item.id)}>{item.label}</button>)}</div>
    {tab === "specialities" ? <Specialities canWrite={canWrite} /> : null}
    {tab === "groups" ? <StudentGroups canWrite={canWrite} /> : null}
    {tab === "reservations" ? <Reservations canWrite={canWrite} /> : null}
    {tab === "reference" ? <ReferenceData canWrite={canWrite} /> : null}
  </div>;
}

function useCatalogContext() {
  const { state, dispatch } = useWorkspace();
  const [faculties, setFaculties] = useState<EntityOption[]>([]);
  const [semesters, setSemesters] = useState<SemesterSummary[]>([]);
  const [facultyQuery, setFacultyQuery] = useState("");
  useEffect(() => { let active = true; Promise.allSettled([api.listAdminCollection("faculties"), api.listAdminCollection("semesters")]).then(([f, s]) => { if (!active) return; if (f.status === "fulfilled") setFaculties(f.value.data.map((item) => option(item, "Faculty"))); if (s.status === "fulfilled") setSemesters(s.value.data.map(toSemester).filter(isSemester)); }); return () => { active = false; }; }, []);
  return { state, dispatch, faculties, semesters, facultyQuery, setFacultyQuery };
}

function FacultyControl({ context }: { context: ReturnType<typeof useCatalogContext> }) {
  return <EntityPicker label="Faculty" query={context.facultyQuery} onQueryChange={context.setFacultyQuery} options={filtered(context.faculties, context.facultyQuery)} selectedId={context.state.facultyId} onSelect={(item) => { context.dispatch({ type: "selectFaculty", facultyId: item.id }); context.setFacultyQuery(item.label); }} />;
}

function Specialities({ canWrite }: { canWrite: boolean }) {
  const context = useCatalogContext();
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [mode, setMode] = useState<RecordMode>(null);
  const [loading, setLoading] = useState(false);
  useEffect(() => { if (!context.state.facultyId) { setItems([]); return; } setLoading(true); api.listFacultySpecialities(context.state.facultyId).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [context.state.facultyId]);
  return <><Panel className="catalog-context-card"><FacultyControl context={context} />{canWrite ? <Button className="button-primary" disabled={!context.state.facultyId} onClick={() => setMode({ type: "create" })}><Plus size={16} /> Create speciality</Button> : null}</Panel>
    <Panel className="catalog-list-panel"><CatalogHeading icon={<BookOpen />} title="Specialities" count={items.length} />{!context.state.facultyId ? <Empty text="Choose a faculty to see its specialities." /> : loading ? <Empty text="Loading specialities…" /> : <div className="catalog-card-grid">{items.map((item) => <button type="button" className="catalog-card catalog-card-button" key={item.id} onClick={() => setMode({ type: "edit", item })}><strong>{resourceLabel(item, "Speciality")}</strong><span>{String(item.attributes?.duration_of_study ?? "—")} years of study</span></button>)}</div>}</Panel>
    <SpecialityInspector mode={mode} canWrite={canWrite} facultyId={context.state.facultyId} onClose={() => setMode(null)}
      onSaved={(item) => { setItems((current) => mode?.type === "edit" ? current.map((row) => row.id === item.id ? item : row) : [...current, item]); setMode(null); }}
      onDeleted={(id) => { setItems((current) => current.filter((row) => row.id !== id)); setMode(null); }} />
  </>;
}

function SpecialityInspector({ mode, canWrite, facultyId, onClose, onSaved, onDeleted }: { mode: RecordMode; canWrite: boolean; facultyId: string | null; onClose: () => void; onSaved: (item: JsonApiResource) => void; onDeleted: (id: string) => void }) {
  const [name, setName] = useState(""); const [years, setYears] = useState("4"); const [error, setError] = useState(""); const [saving, setSaving] = useState(false); const [deleting, setDeleting] = useState(false);
  useEffect(() => {
    if (mode?.type === "edit") { setName(String(mode.item.attributes?.name ?? "")); setYears(String(mode.item.attributes?.duration_of_study ?? "4")); }
    else { setName(""); setYears("4"); }
    setError("");
  }, [mode]);
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (mode?.type !== "edit" && !facultyId) return;
    setSaving(true); setError("");
    try {
      const response = mode?.type === "edit"
        ? await api.updateSpeciality(mode.item.id, { name, duration_of_study: Number(years) })
        : await api.createSpeciality({ faculty_id: Number(facultyId), name, duration_of_study: Number(years) });
      onSaved(response.data);
    } catch { setError("The speciality could not be saved."); }
    finally { setSaving(false); }
  }
  async function remove() {
    if (mode?.type !== "edit") return;
    if (!window.confirm("Delete this speciality?")) return;
    setDeleting(true);
    try { await api.deleteSpeciality(mode.item.id); onDeleted(mode.item.id); }
    catch { setError("The speciality could not be deleted."); }
    finally { setDeleting(false); }
  }
  return <Inspector title={mode?.type === "edit" ? "Edit speciality" : "Create speciality"} open={Boolean(mode)} onClose={onClose}><form className="inspector-form" onSubmit={submit}>
    <label><span>Name</span><input name="name" required value={name} onChange={(e) => setName(e.target.value)} /></label>
    <label><span>Duration of study</span><input name="duration" required type="number" min="1" max="10" value={years} onChange={(e) => setYears(e.target.value)} /></label>
    {error ? <div className="form-error" role="alert">{error}</div> : null}
    {canWrite ? <Button className="button-primary" type="submit" disabled={saving}>{saving ? "Saving…" : mode?.type === "edit" ? "Save changes" : "Create speciality"}</Button> : null}
    {canWrite && mode?.type === "edit" ? <Button type="button" className="danger-button" disabled={deleting} onClick={remove}><Trash2 size={15} /> {deleting ? "Deleting…" : "Delete"}</Button> : null}
  </form></Inspector>;
}

function StudentGroups({ canWrite }: { canWrite: boolean }) {
  const context = useCatalogContext();
  const [items, setItems] = useState<JsonApiResource[]>([]); const [mode, setMode] = useState<RecordMode>(null); const [loading, setLoading] = useState(false);
  const refresh = () => { if (context.state.facultyId && context.state.semesterId) api.listFacultyGroups(context.state.facultyId, context.state.semesterId).then((response) => setItems(response.data)); };
  useEffect(() => { if (!context.state.facultyId || !context.state.semesterId) { setItems([]); return; } setLoading(true); api.listFacultyGroups(context.state.facultyId, context.state.semesterId).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [context.state.facultyId, context.state.semesterId]);
  return <><Panel className="catalog-guided-context"><FacultyControl context={context} /><div><label className="catalog-control-label">Semester</label><SemesterRail semesters={context.semesters} selectedId={context.state.semesterId} onSelect={(semester) => context.dispatch({ type: "selectSemester", semesterId: semester.id })} /></div>{canWrite ? <Button className="button-primary" disabled={!context.state.facultyId} onClick={() => setMode({ type: "create" })}><Plus size={16} /> Create student group</Button> : null}</Panel>
    <Panel className="catalog-list-panel"><CatalogHeading icon={<GraduationCap />} title="Student groups" count={items.length} />{!context.state.facultyId || !context.state.semesterId ? <Empty text="Choose a faculty and semester to see active groups." /> : loading ? <Empty text="Loading student groups…" /> : <div className="catalog-card-grid">{items.map((item) => <button type="button" className="catalog-card catalog-card-button" key={item.id} onClick={() => setMode({ type: "edit", item })}><strong>{groupLabel(item)}</strong><span>{String(item.attributes?.students_now ?? "—")} students</span><small>Admitted {year(item.attributes?.year_of_admission)}</small></button>)}</div>}</Panel>
    <GroupInspector mode={mode} canWrite={canWrite} facultyId={context.state.facultyId} onClose={() => setMode(null)} onSaved={() => { setMode(null); refresh(); }} onDeleted={() => { setMode(null); refresh(); }} />
  </>;
}

function GroupInspector({ mode, canWrite, facultyId, onClose, onSaved, onDeleted }: { mode: RecordMode; canWrite: boolean; facultyId: string | null; onClose: () => void; onSaved: () => void; onDeleted: () => void }) {
  const [specialities, setSpecialities] = useState<EntityOption[]>([]); const [speciality, setSpeciality] = useState<EntityOption | null>(null); const [query, setQuery] = useState(""); const [admissionYear, setAdmissionYear] = useState(String(new Date().getFullYear())); const [students, setStudents] = useState("1"); const [duration, setDuration] = useState("8"); const [error, setError] = useState(""); const [saving, setSaving] = useState(false); const [deleting, setDeleting] = useState(false);
  const open = Boolean(mode);
  useEffect(() => { if (!open || !facultyId) return; api.listFacultySpecialities(facultyId).then((response) => setSpecialities(response.data.map((item) => ({ ...option(item, "Speciality"), description: `${String(item.attributes?.duration_of_study ?? "4")} years` })))); }, [open, facultyId]);
  useEffect(() => {
    if (mode?.type === "edit") {
      const a = mode.item.attributes ?? {};
      const specialityId = a.speciality_id != null ? String(a.speciality_id) : "";
      setSpeciality(specialityId ? { id: specialityId, label: String(a.speciality_name ?? "Speciality") } : null);
      setQuery(specialityId ? String(a.speciality_name ?? "") : "");
      setAdmissionYear(typeof a.year_of_admission === "string" ? a.year_of_admission.slice(0, 4) : String(new Date().getFullYear()));
    } else {
      setSpeciality(null); setQuery(""); setAdmissionYear(String(new Date().getFullYear())); setStudents("1"); setDuration("8");
    }
    setError("");
  }, [mode]);
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!speciality) return;
    setSaving(true); setError("");
    try {
      if (mode?.type === "edit") await api.updateStudentGroup(mode.item.id, { speciality_id: Number(speciality.id), year_of_admission: `${admissionYear}-09-01` });
      else await api.createStudentGroup({ speciality_id: Number(speciality.id), year_of_admission: `${admissionYear}-09-01` }, { students_now: Number(students), semester_duration: Number(duration) });
      onSaved();
    } catch { setError("The student group could not be saved."); }
    finally { setSaving(false); }
  }
  async function remove() {
    if (mode?.type !== "edit") return;
    if (!window.confirm("Delete this student group?")) return;
    setDeleting(true);
    try { await api.deleteStudentGroup(mode.item.id); onDeleted(); }
    catch { setError("The student group could not be deleted."); }
    finally { setDeleting(false); }
  }
  return <Inspector title={mode?.type === "edit" ? "Edit student group" : "Create student group"} open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}>
    <EntityPicker label="Speciality" query={query} onQueryChange={setQuery} options={filtered(specialities, query)} selectedId={speciality?.id ?? null} onSelect={(item) => { setSpeciality(item); setQuery(item.label); if (mode?.type !== "edit") { const years = Number(item.description?.split(" ")[0]) || 4; setDuration(String(years * 2)); } }} />
    <label><span>Admission year</span><input name="admission-year" required type="number" min="2000" max="2100" value={admissionYear} onChange={(e) => setAdmissionYear(e.target.value)} /></label>
    {mode?.type !== "edit" ? <><label><span>Students</span><input name="students" required type="number" min="1" value={students} onChange={(e) => setStudents(e.target.value)} /></label><label><span>Generated semesters</span><input name="semester-duration" readOnly value={duration} /></label><div className="inspector-note">The number of semester records is derived from the speciality duration and starts in the admission academic year.</div></> : null}
    {error ? <div className="form-error" role="alert">{error}</div> : null}
    {canWrite ? <Button className="button-primary" type="submit" disabled={!speciality || saving}>{saving ? "Saving…" : mode?.type === "edit" ? "Save changes" : "Create and generate semesters"}</Button> : null}
    {canWrite && mode?.type === "edit" ? <Button type="button" className="danger-button" disabled={deleting} onClick={remove}><Trash2 size={15} /> {deleting ? "Deleting…" : "Delete"}</Button> : null}
  </form></Inspector>;
}

function Reservations({ canWrite }: { canWrite: boolean }) {
  const { state, dispatch } = useWorkspace(); const [rooms, setRooms] = useState<EntityOption[]>([]); const [room, setRoom] = useState<EntityOption | null>(null); const [query, setQuery] = useState(""); const [items, setItems] = useState<JsonApiResource[]>([]); const [creating, setCreating] = useState(false); const [loading, setLoading] = useState(false);
  useEffect(() => { api.listAdminCollection("rooms", "?page=1&per_page=100").then((response) => setRooms(response.data.map((item) => option(item, "Room")))); }, []);
  useEffect(() => { if (!room || !state.date) { setItems([]); return; } setLoading(true); api.listRoomReservations({ roomId: room.id, date: state.date }).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [room, state.date]);
  return <><Panel className="catalog-context-card reservation-context"><EntityPicker label="Room" query={query} onQueryChange={setQuery} options={filtered(rooms, query)} selectedId={room?.id ?? null} onSelect={(item) => { setRoom(item); setQuery(item.label); }} /><label className="date-field"><span>Date</span><input name="reservation-date" type="date" value={state.date} onChange={(e) => dispatch({ type: "selectDate", date: e.target.value })} /></label>{canWrite ? <Button className="button-primary" disabled={!room || !state.date} onClick={() => setCreating(true)}><CalendarPlus size={16} /> Create reservation</Button> : null}</Panel>
    <Panel className="catalog-list-panel"><CatalogHeading icon={<Library />} title="Room reservations" count={items.length} />{!room ? <Empty text="Search for a room to see its reservations." /> : loading ? <Empty text="Loading reservations…" /> : <div className="catalog-card-grid">{items.map((item) => <article className="catalog-card" key={item.id}><strong>{String(item.attributes?.purpose ?? "Room reservation")}</strong><span>{state.date}</span><small>{String(item.attributes?.status ?? "Active")}</small>{canWrite ? <button className="catalog-delete" type="button" aria-label="Delete reservation" onClick={() => api.deleteRoomReservation(item.id).then(() => setItems((current) => current.filter((row) => row.id !== item.id)))}><Trash2 size={15} /></button> : null}</article>)}</div>}</Panel>
    <ReservationInspector open={creating} room={room} date={state.date} onClose={() => setCreating(false)} onCreated={(item) => { setItems((current) => [...current, item]); setCreating(false); }} />
  </>;
}

function ReservationInspector({ open, room, date, onClose, onCreated }: { open: boolean; room: EntityOption | null; date: string; onClose: () => void; onCreated: (item: JsonApiResource) => void }) {
  const [teachers, setTeachers] = useState<EntityOption[]>([]); const [teacher, setTeacher] = useState<EntityOption | null>(null); const [teacherQuery, setTeacherQuery] = useState(""); const [slots, setSlots] = useState<JsonApiResource[]>([]); const [slot, setSlot] = useState(""); const [purpose, setPurpose] = useState(""); const [error, setError] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) api.listAdminCollection("time_slots").then((response) => setSlots(response.data)); }, [open]);
  useEffect(() => { if (teacherQuery.trim().length < 2) { setTeachers([]); return; } const timer = window.setTimeout(() => api.searchTeachers(teacherQuery).then((response) => setTeachers(response.data.map((item) => option(item, "Teacher")))), 250); return () => window.clearTimeout(timer); }, [teacherQuery]);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!room || !teacher || !slot) return; setSaving(true); setError(""); try { const response = await api.createRoomReservation({ room_id: Number(room.id), teacher_id: Number(teacher.id), reserved_date: date, time_slot_id: Number(slot), purpose: purpose || undefined }); onCreated(response.data); } catch { setError("The room is already occupied or the reservation could not be created."); } finally { setSaving(false); } }
  return <Inspector title="Create room reservation" open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}>
    <div className="inspector-note"><strong>{room?.label}</strong><span>{date}</span></div>
    <EntityPicker label="Teacher" query={teacherQuery} onQueryChange={setTeacherQuery} options={teachers} selectedId={teacher?.id ?? null} onSelect={(item) => { setTeacher(item); setTeacherQuery(item.label); }} />
    <label><span>Time slot</span><select name="time-slot" required value={slot} onChange={(e) => setSlot(e.target.value)}><option value="">Choose a time slot</option>{slots.map((item) => <option value={item.id} key={item.id}>{slotLabel(item)}</option>)}</select></label>
    <label><span>Purpose</span><textarea name="purpose" value={purpose} onChange={(e) => setPurpose(e.target.value)} placeholder="Optional reservation purpose" /></label>
    <div className="inspector-note">Reservations cannot be edited once created — delete this one and create a new reservation instead.</div>
    {error ? <div className="form-error" role="alert">{error}</div> : null}
    <Button className="button-primary" type="submit" disabled={!teacher || !slot || saving}>{saving ? "Creating…" : "Create reservation"}</Button>
  </form></Inspector>;
}

// ---- Reference data: generic field-config driven CRUD for the plain-attribute
// resources (kafedras get their own faculty-first flow further below). ----

type FieldKind = "text" | "number" | "textarea";
type FieldConfig = { key: string; label: string; kind: FieldKind; required?: boolean; placeholder?: string };
type ReferenceResourceConfig = {
  id: AdminCollection;
  label: string;
  singular: string;
  bodyKey: string;
  fields: FieldConfig[];
  search?: "teachers" | "disciplines" | "client";
  labelFn?: (item: JsonApiResource) => string;
};

const referenceResources: ReferenceResourceConfig[] = [
  { id: "teachers", label: "Teachers", singular: "Teacher", bodyKey: "teacher", search: "teachers", fields: [
    { key: "last_name", label: "Last name", kind: "text", required: true },
    { key: "first_name", label: "First name", kind: "text", required: true },
    { key: "middle_name", label: "Middle name", kind: "text" },
    { key: "post_id", label: "Post (numeric id)", kind: "number", placeholder: "Optional" },
    { key: "academic_degree_id", label: "Academic degree (numeric id)", kind: "number", placeholder: "Optional" },
    { key: "academic_title_id", label: "Academic title (numeric id)", kind: "number", placeholder: "Optional" },
  ] },
  { id: "rooms", label: "Rooms", singular: "Room", bodyKey: "room", search: "client", fields: [
    { key: "name", label: "Name / number", kind: "text", required: true },
    { key: "capacity", label: "Capacity", kind: "number", placeholder: "Optional" },
  ] },
  { id: "subjects", label: "Disciplines", singular: "Discipline", bodyKey: "subject", search: "disciplines", fields: [
    { key: "name", label: "Name", kind: "text", required: true },
  ] },
  { id: "activity_types", label: "Activity types", singular: "Activity type", bodyKey: "activity_type", search: "client", fields: [
    { key: "name", label: "Name", kind: "text", required: true },
  ] },
  { id: "time_slots", label: "Time slots", singular: "Time slot", bodyKey: "time_slot", search: "client", fields: [
    { key: "call_number", label: "Call number", kind: "number", required: true },
    { key: "start_time", label: "Start time", kind: "text", required: true, placeholder: "09:00" },
    { key: "end_time", label: "End time", kind: "text", required: true, placeholder: "10:30" },
  ], labelFn: slotLabel },
  { id: "week_types", label: "Week types", singular: "Week type", bodyKey: "week_type", search: "client", fields: [
    { key: "name", label: "Name", kind: "text", required: true },
  ] },
];

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

const PER_PAGE = 25;

function ReferenceResourcePanel({ config, canWrite }: { config: ReferenceResourceConfig; canWrite: boolean }) {
  const [page, setPage] = useState(1);
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [loading, setLoading] = useState(false);
  const [rawQuery, setRawQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [mode, setMode] = useState<RecordMode>(null);

  useEffect(() => { setPage(1); setRawQuery(""); setDebouncedQuery(""); }, [config.id]);
  useEffect(() => { const timer = window.setTimeout(() => setDebouncedQuery(rawQuery), 250); return () => window.clearTimeout(timer); }, [rawQuery]);

  const trimmedQuery = debouncedQuery.trim();
  const searching = (config.search === "teachers" || config.search === "disciplines") && trimmedQuery.length >= 3;

  function load() {
    setLoading(true);
    const request = config.search === "teachers" && trimmedQuery.length >= 3 ? api.searchTeachers(trimmedQuery)
      : config.search === "disciplines" && trimmedQuery.length >= 3 ? api.searchDisciplines(trimmedQuery)
      : api.listAdminCollection(config.id, `?page=${page}&per_page=${PER_PAGE}`);
    request.then((response) => setItems(response.data)).catch(() => setItems([])).finally(() => setLoading(false));
  }
  useEffect(load, [config.id, page, trimmedQuery]);

  const displayed = config.search === "client" && rawQuery.trim()
    ? items.filter((item) => labelFor(config, item).toLowerCase().includes(rawQuery.trim().toLowerCase()))
    : items;

  return <>
    <Panel className="reference-toolbar">
      {config.search ? <div className="entity-picker reference-search"><label>Search</label><div className="entity-picker-input"><Search aria-hidden="true" size={17} /><input value={rawQuery} onChange={(e) => setRawQuery(e.target.value)} placeholder={config.search === "client" ? `Filter ${config.label.toLowerCase()}` : `Search ${config.label.toLowerCase()} (min. 3 characters)`} /></div></div> : <span />}
      {canWrite ? <Button className="button-primary" onClick={() => setMode({ type: "create" })}><Plus size={16} /> Create {config.singular.toLowerCase()}</Button> : null}
    </Panel>
    <Panel className="catalog-list-panel">
      <CatalogHeading icon={<Library />} title={config.label} count={displayed.length} />
      {loading ? <Empty text={`Loading ${config.label.toLowerCase()}…`} /> : !displayed.length ? <Empty text={`No ${config.label.toLowerCase()} found.`} /> : <div className="catalog-card-grid">{displayed.map((item) => <button type="button" className="catalog-card catalog-card-button" key={item.id} onClick={() => setMode({ type: "edit", item })}><strong>{labelFor(config, item)}</strong></button>)}</div>}
      {!searching ? <div className="pagination-controls"><button type="button" disabled={page <= 1} onClick={() => setPage((current) => Math.max(1, current - 1))}><ChevronLeft size={15} /> Prev</button><span>Page {page}</span><button type="button" disabled={items.length < PER_PAGE} onClick={() => setPage((current) => current + 1)}>Next <ChevronRight size={15} /></button></div> : null}
    </Panel>
    <RecordInspector config={config} mode={mode} canWrite={canWrite} onClose={() => setMode(null)} onSaved={() => { setMode(null); load(); }} onDeleted={() => { setMode(null); load(); }}>
      {config.id === "teachers" && mode?.type === "edit" ? <TeacherDepartments teacherId={mode.item.id} canWrite={canWrite} /> : null}
    </RecordInspector>
  </>;
}

function RecordInspector({ config, mode, canWrite, extraAttributes, onClose, onSaved, onDeleted, children }: {
  config: ReferenceResourceConfig;
  mode: RecordMode;
  canWrite: boolean;
  extraAttributes?: Record<string, unknown>;
  onClose: () => void;
  onSaved: (item: JsonApiResource) => void;
  onDeleted: (id: string) => void;
  children?: React.ReactNode;
}) {
  const [values, setValues] = useState<Record<string, string>>({});
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    const next: Record<string, string> = {};
    for (const field of config.fields) next[field.key] = mode?.type === "edit" ? String(mode.item.attributes?.[field.key] ?? "") : "";
    setValues(next);
    setError("");
  }, [config, mode]);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setSaving(true); setError("");
    const attributes: Record<string, unknown> = { ...extraAttributes };
    for (const field of config.fields) {
      const raw = values[field.key] ?? "";
      if (raw !== "") attributes[field.key] = field.kind === "number" ? Number(raw) : raw;
    }
    try {
      const response = mode?.type === "edit"
        ? await api.updateResource(config.id, config.bodyKey, mode.item.id, attributes)
        : await api.createResource(config.id, config.bodyKey, attributes);
      onSaved(response.data);
    } catch { setError(`The ${config.singular.toLowerCase()} could not be saved.`); }
    finally { setSaving(false); }
  }

  async function remove() {
    if (mode?.type !== "edit") return;
    if (!window.confirm(`Delete this ${config.singular.toLowerCase()}?`)) return;
    setDeleting(true);
    try { await api.deleteResource(config.id, mode.item.id); onDeleted(mode.item.id); }
    catch { setError(`The ${config.singular.toLowerCase()} could not be deleted.`); }
    finally { setDeleting(false); }
  }

  return <Inspector title={mode?.type === "edit" ? `Edit ${config.singular.toLowerCase()}` : `Create ${config.singular.toLowerCase()}`} open={Boolean(mode)} onClose={onClose}>
    <form className="inspector-form" onSubmit={submit}>
      {config.fields.map((field) => <label key={field.key}>
        <span>{field.label}</span>
        {field.kind === "textarea"
          ? <textarea value={values[field.key] ?? ""} required={field.required} placeholder={field.placeholder} onChange={(e) => setValues((current) => ({ ...current, [field.key]: e.target.value }))} />
          : <input type={field.kind === "number" ? "number" : "text"} value={values[field.key] ?? ""} required={field.required} placeholder={field.placeholder} onChange={(e) => setValues((current) => ({ ...current, [field.key]: e.target.value }))} />}
      </label>)}
      {error ? <div className="form-error" role="alert">{error}</div> : null}
      {canWrite ? <Button className="button-primary" type="submit" disabled={saving}>{saving ? "Saving…" : mode?.type === "edit" ? "Save changes" : `Create ${config.singular.toLowerCase()}`}</Button> : null}
      {canWrite && mode?.type === "edit" ? <Button type="button" className="danger-button" disabled={deleting} onClick={remove}><Trash2 size={15} /> {deleting ? "Deleting…" : "Delete"}</Button> : null}
    </form>
    {children}
  </Inspector>;
}

function labelFor(config: ReferenceResourceConfig, item: JsonApiResource) {
  return config.labelFn ? config.labelFn(item) : resourceLabel(item, config.singular);
}

const kafedraConfig: ReferenceResourceConfig = { id: "kafedras", label: "Departments", singular: "Department", bodyKey: "kafedra", fields: [{ key: "name", label: "Name", kind: "text", required: true }] };

function KafedrasPanel({ canWrite }: { canWrite: boolean }) {
  const [faculties, setFaculties] = useState<JsonApiResource[]>([]);
  const [facultyId, setFacultyId] = useState<string | null>(null);
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<RecordMode>(null);

  useEffect(() => { api.listAdminCollection("faculties").then((response) => setFaculties(response.data)); }, []);
  function refresh() { if (facultyId) { setLoading(true); api.listFacultyDepartments(facultyId).then((response) => setItems(response.data)).finally(() => setLoading(false)); } }
  useEffect(() => { if (!facultyId) { setItems([]); return; } setLoading(true); api.listFacultyDepartments(facultyId).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [facultyId]);

  return <>
    <Panel className="catalog-guided-context">
      <div><label className="catalog-control-label">Faculty</label><div className="reference-resource-grid">{faculties.map((item) => <button type="button" className={facultyId === item.id ? "active" : ""} key={item.id} onClick={() => setFacultyId(item.id)}>{resourceLabel(item, "Faculty")}</button>)}</div></div>
      {canWrite ? <Button className="button-primary" disabled={!facultyId} onClick={() => setMode({ type: "create" })}><Plus size={16} /> Create department</Button> : null}
    </Panel>
    <Panel className="catalog-list-panel">
      <CatalogHeading icon={<Library />} title="Departments" count={items.length} />
      {!facultyId ? <Empty text="Choose a faculty to see its departments." /> : loading ? <Empty text="Loading departments…" /> : <div className="catalog-card-grid">{items.map((item) => <button type="button" className="catalog-card catalog-card-button" key={item.id} onClick={() => setMode({ type: "edit", item })}><strong>{resourceLabel(item, "Department")}</strong></button>)}</div>}
    </Panel>
    <RecordInspector config={kafedraConfig} mode={mode} canWrite={canWrite} extraAttributes={facultyId ? { faculty_id: Number(facultyId) } : {}} onClose={() => setMode(null)} onSaved={() => { setMode(null); refresh(); }} onDeleted={() => { setMode(null); refresh(); }} />
  </>;
}

function TeacherDepartments({ teacherId, canWrite }: { teacherId: string; canWrite: boolean }) {
  const [links, setLinks] = useState<JsonApiResource[]>([]);
  const [kafedraNames, setKafedraNames] = useState<Map<string, string>>(new Map());
  const [loading, setLoading] = useState(true);
  const [faculties, setFaculties] = useState<JsonApiResource[]>([]);
  const [facultyId, setFacultyId] = useState("");
  const [departments, setDepartments] = useState<JsonApiResource[]>([]);
  const [departmentId, setDepartmentId] = useState("");
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState("");

  function load() {
    setLoading(true);
    Promise.allSettled([api.listAdminCollection("teacher_on_kafedras"), api.listAdminCollection("kafedras")]).then(([linksResult, kafedraResult]) => {
      if (linksResult.status === "fulfilled") setLinks(linksResult.value.data.filter((item) => String(item.attributes?.teacher_id ?? "") === teacherId));
      if (kafedraResult.status === "fulfilled") setKafedraNames(new Map(kafedraResult.value.data.map((item) => [item.id, resourceLabel(item, "Department")])));
    }).finally(() => setLoading(false));
  }
  useEffect(load, [teacherId]);
  useEffect(() => { api.listAdminCollection("faculties").then((response) => setFaculties(response.data)); }, []);
  useEffect(() => { if (!facultyId) { setDepartments([]); return; } api.listFacultyDepartments(facultyId).then((response) => setDepartments(response.data)); }, [facultyId]);

  async function addLink(event: React.FormEvent) {
    event.preventDefault();
    if (!departmentId) return;
    setAdding(true); setError("");
    try {
      await api.createResource("teacher_on_kafedras", "teacher_on_kafedra", { teacher_id: Number(teacherId), kafedra_id: Number(departmentId) });
      setDepartmentId("");
      load();
    } catch { setError("The department link could not be added."); }
    finally { setAdding(false); }
  }
  async function removeLink(id: string) {
    if (!window.confirm("Remove this department link?")) return;
    try { await api.deleteResource("teacher_on_kafedras", id); load(); }
    catch { setError("The department link could not be removed."); }
  }

  return <div className="teacher-departments">
    <h3>Departments</h3>
    {loading ? <p className="resource-empty">Loading departments…</p> : links.length
      ? <ul className="teacher-department-list">{links.map((link) => <li key={link.id}><span>{kafedraNames.get(String(link.attributes?.kafedra_id ?? "")) ?? "Department"}</span>{canWrite ? <button type="button" aria-label="Remove department link" onClick={() => removeLink(link.id)}><Trash2 size={14} /></button> : null}</li>)}</ul>
      : <p className="resource-empty">No department links yet.</p>}
    {canWrite ? <form className="teacher-department-add" onSubmit={addLink}>
      <select value={facultyId} onChange={(e) => { setFacultyId(e.target.value); setDepartmentId(""); }}>
        <option value="">Choose a faculty</option>
        {faculties.map((item) => <option value={item.id} key={item.id}>{resourceLabel(item, "Faculty")}</option>)}
      </select>
      <select value={departmentId} disabled={!facultyId} onChange={(e) => setDepartmentId(e.target.value)}>
        <option value="">Choose a department</option>
        {departments.map((item) => <option value={item.id} key={item.id}>{resourceLabel(item, "Department")}</option>)}
      </select>
      <Button className="button-primary" type="submit" disabled={!departmentId || adding}>{adding ? "Adding…" : "Add department"}</Button>
    </form> : null}
    {error ? <div className="form-error" role="alert">{error}</div> : null}
  </div>;
}

function CatalogHeading({ icon, title, count }: { icon: React.ReactNode; title: string; count: number }) { return <div className="panel-heading"><div className="section-inline-title">{icon}<div><span className="eyebrow">Current selection</span><h2>{title}</h2></div></div><span className="resource-count">{count}</span></div>; }
function Empty({ text }: { text: string }) { return <p className="resource-empty">{text}</p>; }
function option(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback) }; }
function filtered(items: EntityOption[], query: string) { const q = query.trim().toLowerCase(); return q ? items.filter((item) => item.label.toLowerCase().includes(q)).slice(0, 20) : items.slice(0, 20); }
function toSemester(item: JsonApiResource): SemesterSummary | null { const a = item.attributes ?? {}; const number = Number(a.number), startYear = Number(a.start_year), endYear = Number(a.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function groupLabel(item: JsonApiResource) { return `${String(item.attributes?.speciality_name ?? "Student group")}-${String(item.attributes?.course ?? "—")}`; }
function year(value: unknown) { return typeof value === "string" ? value.slice(0, 4) : "—"; }
function slotLabel(item: JsonApiResource) { const a = item.attributes ?? {}; return `${String(a.call_number ?? "Class")} · ${String(a.start_time ?? "")}-${String(a.end_time ?? "")}`; }
function semesterResourceLabel(item: JsonApiResource) { const value = toSemester(item); return value ? formatSemester(value) : "Semester"; }
