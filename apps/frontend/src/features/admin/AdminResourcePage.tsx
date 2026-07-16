import { useEffect, useMemo, useState } from "react";
import { BookOpen, CalendarPlus, GraduationCap, Library, Plus, Trash2 } from "lucide-react";

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

export function AdminResourcePage({ canWrite }: { canWrite: boolean }) {
  const [tab, setTab] = useState<CatalogTab>("specialities");
  return <div className="admin-section catalog-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Administration</span><h1>University catalog</h1></div><p>Browse and create academic entities through readable, scoped workflows.</p></div>
    <div className="admin-resource-tabs catalog-tabs" role="tablist" aria-label="Catalog sections">{tabs.map((item) => <button type="button" role="tab" aria-selected={tab === item.id} className={tab === item.id ? "active" : ""} key={item.id} onClick={() => setTab(item.id)}>{item.label}</button>)}</div>
    {tab === "specialities" ? <Specialities canWrite={canWrite} /> : null}
    {tab === "groups" ? <StudentGroups canWrite={canWrite} /> : null}
    {tab === "reservations" ? <Reservations canWrite={canWrite} /> : null}
    {tab === "reference" ? <ReferenceData /> : null}
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
  const [creating, setCreating] = useState(false);
  const [loading, setLoading] = useState(false);
  useEffect(() => { if (!context.state.facultyId) { setItems([]); return; } setLoading(true); api.listFacultySpecialities(context.state.facultyId).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [context.state.facultyId]);
  return <><Panel className="catalog-context-card"><FacultyControl context={context} />{canWrite ? <Button className="button-primary" disabled={!context.state.facultyId} onClick={() => setCreating(true)}><Plus size={16} /> Create speciality</Button> : null}</Panel><Panel className="catalog-list-panel"><CatalogHeading icon={<BookOpen />} title="Specialities" count={items.length} />{!context.state.facultyId ? <Empty text="Choose a faculty to see its specialities." /> : loading ? <Empty text="Loading specialities…" /> : <div className="catalog-card-grid">{items.map((item) => <article className="catalog-card" key={item.id}><strong>{resourceLabel(item, "Speciality")}</strong><span>{String(item.attributes?.duration_of_study ?? "—")} years of study</span></article>)}</div>}</Panel><SpecialityInspector open={creating} facultyId={context.state.facultyId} onClose={() => setCreating(false)} onCreated={(item) => { setItems((current) => [...current, item]); setCreating(false); }} /></>;
}

function SpecialityInspector({ open, facultyId, onClose, onCreated }: { open: boolean; facultyId: string | null; onClose: () => void; onCreated: (item: JsonApiResource) => void }) {
  const [name, setName] = useState(""); const [years, setYears] = useState("4"); const [error, setError] = useState(""); const [saving, setSaving] = useState(false);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!facultyId) return; setSaving(true); setError(""); try { const response = await api.createSpeciality({ faculty_id: Number(facultyId), name, duration_of_study: Number(years) }); onCreated(response.data); setName(""); } catch { setError("The speciality could not be created."); } finally { setSaving(false); } }
  return <Inspector title="Create speciality" open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}><label><span>Name</span><input name="name" required value={name} onChange={(e) => setName(e.target.value)} /></label><label><span>Duration of study</span><input name="duration" required type="number" min="1" max="10" value={years} onChange={(e) => setYears(e.target.value)} /></label>{error ? <div className="form-error" role="alert">{error}</div> : null}<Button className="button-primary" type="submit" disabled={saving}>{saving ? "Creating…" : "Create speciality"}</Button></form></Inspector>;
}

function StudentGroups({ canWrite }: { canWrite: boolean }) {
  const context = useCatalogContext();
  const [items, setItems] = useState<JsonApiResource[]>([]); const [creating, setCreating] = useState(false); const [loading, setLoading] = useState(false);
  useEffect(() => { if (!context.state.facultyId || !context.state.semesterId) { setItems([]); return; } setLoading(true); api.listFacultyGroups(context.state.facultyId, context.state.semesterId).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [context.state.facultyId, context.state.semesterId]);
  return <><Panel className="catalog-guided-context"><FacultyControl context={context} /><div><label className="catalog-control-label">Semester</label><SemesterRail semesters={context.semesters} selectedId={context.state.semesterId} onSelect={(semester) => context.dispatch({ type: "selectSemester", semesterId: semester.id })} /></div>{canWrite ? <Button className="button-primary" disabled={!context.state.facultyId} onClick={() => setCreating(true)}><Plus size={16} /> Create student group</Button> : null}</Panel><Panel className="catalog-list-panel"><CatalogHeading icon={<GraduationCap />} title="Student groups" count={items.length} />{!context.state.facultyId || !context.state.semesterId ? <Empty text="Choose a faculty and semester to see active groups." /> : loading ? <Empty text="Loading student groups…" /> : <div className="catalog-card-grid">{items.map((item) => <article className="catalog-card" key={item.id}><strong>{groupLabel(item)}</strong><span>{String(item.attributes?.students_now ?? "—")} students</span><small>Admitted {year(item.attributes?.year_of_admission)}</small></article>)}</div>}</Panel><GroupInspector open={creating} facultyId={context.state.facultyId} onClose={() => setCreating(false)} onCreated={() => { setCreating(false); if (context.state.facultyId && context.state.semesterId) api.listFacultyGroups(context.state.facultyId, context.state.semesterId).then((response) => setItems(response.data)); }} /></>;
}

function GroupInspector({ open, facultyId, onClose, onCreated }: { open: boolean; facultyId: string | null; onClose: () => void; onCreated: () => void }) {
  const [specialities, setSpecialities] = useState<EntityOption[]>([]); const [speciality, setSpeciality] = useState<EntityOption | null>(null); const [query, setQuery] = useState(""); const [admissionYear, setAdmissionYear] = useState(String(new Date().getFullYear())); const [students, setStudents] = useState("1"); const [duration, setDuration] = useState("8"); const [error, setError] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (!open || !facultyId) return; api.listFacultySpecialities(facultyId).then((response) => setSpecialities(response.data.map((item) => ({ ...option(item, "Speciality"), description: `${String(item.attributes?.duration_of_study ?? "4")} years` })))); }, [open, facultyId]);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!speciality) return; setSaving(true); setError(""); try { await api.createStudentGroup({ speciality_id: Number(speciality.id), year_of_admission: `${admissionYear}-09-01` }, { students_now: Number(students), semester_duration: Number(duration) }); onCreated(); } catch { setError("The student group could not be created."); } finally { setSaving(false); } }
  return <Inspector title="Create student group" open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}><EntityPicker label="Speciality" query={query} onQueryChange={setQuery} options={filtered(specialities, query)} selectedId={speciality?.id ?? null} onSelect={(item) => { setSpeciality(item); setQuery(item.label); const years = Number(item.description?.split(" ")[0]) || 4; setDuration(String(years * 2)); }} /><label><span>Admission year</span><input name="admission-year" required type="number" min="2000" max="2100" value={admissionYear} onChange={(e) => setAdmissionYear(e.target.value)} /></label><label><span>Students</span><input name="students" required type="number" min="1" value={students} onChange={(e) => setStudents(e.target.value)} /></label><label><span>Generated semesters</span><input name="semester-duration" readOnly value={duration} /></label><div className="inspector-note">The number of semester records is derived from the speciality duration and starts in the admission academic year.</div>{error ? <div className="form-error" role="alert">{error}</div> : null}<Button className="button-primary" type="submit" disabled={!speciality || saving}>{saving ? "Creating…" : "Create and generate semesters"}</Button></form></Inspector>;
}

function Reservations({ canWrite }: { canWrite: boolean }) {
  const { state, dispatch } = useWorkspace(); const [rooms, setRooms] = useState<EntityOption[]>([]); const [room, setRoom] = useState<EntityOption | null>(null); const [query, setQuery] = useState(""); const [items, setItems] = useState<JsonApiResource[]>([]); const [creating, setCreating] = useState(false); const [loading, setLoading] = useState(false);
  useEffect(() => { api.listAdminCollection("rooms", "?page=1&per_page=100").then((response) => setRooms(response.data.map((item) => option(item, "Room")))); }, []);
  useEffect(() => { if (!room || !state.date) { setItems([]); return; } setLoading(true); api.listRoomReservations({ roomId: room.id, date: state.date }).then((response) => setItems(response.data)).finally(() => setLoading(false)); }, [room, state.date]);
  return <><Panel className="catalog-context-card reservation-context"><EntityPicker label="Room" query={query} onQueryChange={setQuery} options={filtered(rooms, query)} selectedId={room?.id ?? null} onSelect={(item) => { setRoom(item); setQuery(item.label); }} /><label className="date-field"><span>Date</span><input name="reservation-date" type="date" value={state.date} onChange={(e) => dispatch({ type: "selectDate", date: e.target.value })} /></label>{canWrite ? <Button className="button-primary" disabled={!room || !state.date} onClick={() => setCreating(true)}><CalendarPlus size={16} /> Create reservation</Button> : null}</Panel><Panel className="catalog-list-panel"><CatalogHeading icon={<Library />} title="Room reservations" count={items.length} />{!room ? <Empty text="Search for a room to see its reservations." /> : loading ? <Empty text="Loading reservations…" /> : <div className="catalog-card-grid">{items.map((item) => <article className="catalog-card" key={item.id}><strong>{String(item.attributes?.purpose ?? "Room reservation")}</strong><span>{state.date}</span><small>{String(item.attributes?.status ?? "Active")}</small>{canWrite ? <button className="catalog-delete" type="button" aria-label="Delete reservation" onClick={() => api.deleteRoomReservation(item.id).then(() => setItems((current) => current.filter((row) => row.id !== item.id)))}><Trash2 size={15} /></button> : null}</article>)}</div>}</Panel><ReservationInspector open={creating} room={room} date={state.date} onClose={() => setCreating(false)} onCreated={(item) => { setItems((current) => [...current, item]); setCreating(false); }} /></>;
}

function ReservationInspector({ open, room, date, onClose, onCreated }: { open: boolean; room: EntityOption | null; date: string; onClose: () => void; onCreated: (item: JsonApiResource) => void }) {
  const [teachers, setTeachers] = useState<EntityOption[]>([]); const [teacher, setTeacher] = useState<EntityOption | null>(null); const [teacherQuery, setTeacherQuery] = useState(""); const [slots, setSlots] = useState<JsonApiResource[]>([]); const [slot, setSlot] = useState(""); const [purpose, setPurpose] = useState(""); const [error, setError] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) api.listAdminCollection("time_slots").then((response) => setSlots(response.data)); }, [open]);
  useEffect(() => { if (teacherQuery.trim().length < 2) { setTeachers([]); return; } const timer = window.setTimeout(() => api.searchTeachers(teacherQuery).then((response) => setTeachers(response.data.map((item) => option(item, "Teacher")))), 250); return () => window.clearTimeout(timer); }, [teacherQuery]);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!room || !teacher || !slot) return; setSaving(true); setError(""); try { const response = await api.createRoomReservation({ room_id: Number(room.id), teacher_id: Number(teacher.id), reserved_date: date, time_slot_id: Number(slot), purpose: purpose || undefined }); onCreated(response.data); } catch { setError("The room is already occupied or the reservation could not be created."); } finally { setSaving(false); } }
  return <Inspector title="Create room reservation" open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}><div className="inspector-note"><strong>{room?.label}</strong><span>{date}</span></div><EntityPicker label="Teacher" query={teacherQuery} onQueryChange={setTeacherQuery} options={teachers} selectedId={teacher?.id ?? null} onSelect={(item) => { setTeacher(item); setTeacherQuery(item.label); }} /><label><span>Time slot</span><select name="time-slot" required value={slot} onChange={(e) => setSlot(e.target.value)}><option value="">Choose a time slot</option>{slots.map((item) => <option value={item.id} key={item.id}>{slotLabel(item)}</option>)}</select></label><label><span>Purpose</span><textarea name="purpose" value={purpose} onChange={(e) => setPurpose(e.target.value)} placeholder="Optional reservation purpose" /></label>{error ? <div className="form-error" role="alert">{error}</div> : null}<Button className="button-primary" type="submit" disabled={!teacher || !slot || saving}>{saving ? "Creating…" : "Create reservation"}</Button></form></Inspector>;
}

function ReferenceData() { const resources: Array<{ id: AdminCollection; label: string }> = [{ id: "faculties", label: "Faculties" }, { id: "semesters", label: "Semesters" }, { id: "kafedras", label: "Departments" }, { id: "teachers", label: "Teachers" }, { id: "rooms", label: "Rooms" }, { id: "subjects", label: "Disciplines" }, { id: "activity_types", label: "Activity types" }]; const [selected, setSelected] = useState(resources[0]); const [items, setItems] = useState<JsonApiResource[]>([]); useEffect(() => { api.listAdminCollection(selected.id).then((response) => setItems(response.data)); }, [selected]); return <><div className="reference-resource-grid">{resources.map((item) => <button type="button" className={selected.id === item.id ? "active" : ""} key={item.id} onClick={() => setSelected(item)}>{item.label}</button>)}</div><Panel className="catalog-list-panel"><CatalogHeading icon={<Library />} title={selected.label} count={items.length} /><div className="catalog-card-grid">{items.map((item) => <article className="catalog-card" key={item.id}><strong>{selected.id === "semesters" ? semesterResourceLabel(item) : resourceLabel(item, selected.label.slice(0, -1))}</strong></article>)}</div></Panel></>; }

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
