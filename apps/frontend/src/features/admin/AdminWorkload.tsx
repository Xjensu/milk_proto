import { useEffect, useMemo, useState } from "react";
import { Building2, GraduationCap, Plus } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type JsonApiResource } from "../../lib/api";
import type { SemesterSummary } from "./domain";
import { groupResource, resourceLabel } from "./utils";
import { EntityPicker, type EntityOption } from "./workspace/EntityPicker";
import { Inspector } from "./workspace/Inspector";
import { SemesterRail } from "./workspace/SemesterRail";
import { useWorkspace } from "./workspace/WorkspaceContext";
import { groupWorkloadRows, normalizeWorkload, type WorkloadRow } from "./workload/normalize";

type WorkloadMode = "faculty" | "department";

export function AdminWorkload() {
  const { state: workspace, dispatch } = useWorkspace();
  const [mode, setMode] = useState<WorkloadMode>("faculty");
  const [semesters, setSemesters] = useState<SemesterSummary[]>([]);
  const [faculties, setFaculties] = useState<EntityOption[]>([]);
  const [departments, setDepartments] = useState<EntityOption[]>([]);
  const [groups, setGroups] = useState<EntityOption[]>([]);
  const [facultyQuery, setFacultyQuery] = useState("");
  const [departmentQuery, setDepartmentQuery] = useState("");
  const [selectedDepartment, setSelectedDepartment] = useState<EntityOption | null>(null);
  const [selectedGroups, setSelectedGroups] = useState<string[]>([]);
  const [rows, setRows] = useState<WorkloadRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [assignment, setAssignment] = useState<WorkloadRow | null>(null);
  const [teacherAssignments, setTeacherAssignments] = useState<EntityOption[]>([]);
  const [roomOptions, setRoomOptions] = useState<EntityOption[]>([]);
  const [creatingRecord, setCreatingRecord] = useState(false);
  const [configuringColumns, setConfiguringColumns] = useState(false);
  const [visibleActivities, setVisibleActivities] = useState<string[] | null>(null);

  useEffect(() => {
    let active = true;
    Promise.allSettled([api.listAdminCollection("semesters"), api.listAdminCollection("faculties")]).then(([semesterResult, facultyResult]) => {
      if (!active) return;
      if (semesterResult.status === "fulfilled") setSemesters(semesterResult.value.data.map(toSemester).filter(isSemester));
      if (facultyResult.status === "fulfilled") setFaculties(facultyResult.value.data.map((item) => toOption(item, "Faculty")));
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!workspace.facultyId) { setDepartments([]); return; }
    api.listFacultyDepartments(workspace.facultyId).then((response) => setDepartments(response.data.map((item) => toOption(item, "Department")))).catch(() => setDepartments([]));
  }, [workspace.facultyId]);

  useEffect(() => {
    if (!workspace.facultyId || !workspace.semesterId) { setGroups([]); setSelectedGroups([]); return; }
    api.listFacultyGroups(workspace.facultyId, workspace.semesterId).then((response) => setGroups(response.data.map(groupResource))).catch(() => setGroups([]));
  }, [workspace.facultyId, workspace.semesterId]);

  useEffect(() => {
    if (!assignment || !selectedDepartment) return;
    Promise.all([api.listAdminCollection("teacher_on_kafedras", "?page=1&per_page=100"), api.listAdminCollection("rooms", "?page=1&per_page=100")]).then(([teacherResponse, roomResponse]) => {
      setTeacherAssignments(teacherAssignmentOptions(teacherResponse, selectedDepartment.id));
      setRoomOptions(roomResponse.data.map((item) => toOption(item, "Room")));
    }).catch(() => { setTeacherAssignments([]); setRoomOptions([]); });
  }, [assignment, selectedDepartment]);

  const visibleRows = useMemo(() => mode === "faculty" && selectedGroups.length ? rows.filter((row) => selectedGroups.includes(row.groupSemesterId)) : rows, [mode, rows, selectedGroups]);
  const activityTypes = useMemo(() => [...new Set(visibleRows.map((row) => row.activity))], [visibleRows]);
  const displayedActivities = visibleActivities === null ? activityTypes : activityTypes.filter((activity) => visibleActivities.includes(activity));

  async function load() {
    if (!workspace.semesterId || !workspace.facultyId || (mode === "department" && !selectedDepartment)) return;
    setLoading(true); setError("");
    try {
      const response = mode === "faculty" ? await api.facultyWorkload(workspace.facultyId, workspace.semesterId) : await api.departmentWorkload(selectedDepartment!.id, workspace.semesterId);
      setRows(normalizeWorkload(response)); setLoaded(true);
    } catch { setError("Не удалось загрузить нагрузку для выбранного контекста."); }
    finally { setLoading(false); }
  }

  const ready = Boolean(workspace.facultyId && workspace.semesterId && (mode === "faculty" || selectedDepartment));
  return <div className="admin-section workload-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Учебное планирование</span><h1>Распределение нагрузки</h1></div><p>Соберите нагрузку факультета или распределите занятия между ресурсами кафедры.</p></div>
    <div className="workload-mode-cards" role="tablist" aria-label="Вид нагрузки">
      <button type="button" role="tab" aria-selected={mode === "faculty"} className={mode === "faculty" ? "active" : ""} onClick={() => { setMode("faculty"); setLoaded(false); }}><GraduationCap /><span><strong>Нагрузка факультета</strong><small>Дисциплины, группы и часы по видам занятий</small></span></button>
      <button type="button" role="tab" aria-selected={mode === "department"} className={mode === "department" ? "active" : ""} onClick={() => { setMode("department"); setLoaded(false); }}><Building2 /><span><strong>Нагрузка кафедры</strong><small>Преподаватели, аудитории и незаполненные назначения</small></span></button>
    </div>
    <Panel className="workload-context"><div className="guided-context">
      <div className="context-step"><span>1</span><EntityPicker label="Факультет" query={facultyQuery} onQueryChange={setFacultyQuery} options={filter(faculties, facultyQuery)} selectedId={workspace.facultyId} onSelect={(option) => { dispatch({ type: "selectFaculty", facultyId: option.id }); setFacultyQuery(option.label); setSelectedDepartment(null); setLoaded(false); }} /><small>Выберите факультет, которому принадлежат группы.</small></div>
      <div className="context-step context-step-wide"><span>2</span><div><label>Семестр</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => { dispatch({ type: "selectSemester", semesterId: semester.id }); setLoaded(false); }} /></div><small>Семестр определяет курс каждой группы.</small></div>
      {mode === "department" ? <div className="context-step"><span>3</span><EntityPicker label="Кафедра" query={departmentQuery} onQueryChange={setDepartmentQuery} options={filter(departments, departmentQuery)} selectedId={selectedDepartment?.id ?? null} disabled={!workspace.facultyId} onSelect={(option) => { setSelectedDepartment(option); setDepartmentQuery(option.label); setLoaded(false); }} /><small>Показаны только кафедры выбранного факультета.</small></div> : null}
      {mode === "faculty" && workspace.facultyId && workspace.semesterId ? <div className="context-step context-step-wide"><span>3</span><div><label>Учебные группы</label><div className="group-chip-list"><button type="button" className={!selectedGroups.length ? "active" : ""} onClick={() => setSelectedGroups([])}>Все группы</button>{groups.map((group) => <button type="button" className={selectedGroups.includes(group.id) ? "active" : ""} key={group.id} onClick={() => setSelectedGroups((current) => current.includes(group.id) ? current.filter((id) => id !== group.id) : [...current, group.id])}>{group.label}</button>)}</div></div><small>Выберите отдельные группы или оставьте нагрузку по всему факультету.</small></div> : null}
    </div><div className="schedule-query-actions"><Button className="button-primary" onClick={load} disabled={!ready || loading}>{loading ? "Загрузка…" : "Открыть нагрузку"}</Button></div>{error ? <div className="form-error" role="alert">{error}</div> : null}</Panel>
    {loaded ? <Panel className="workload-results"><div className="panel-heading"><div><span className="eyebrow">Current selection</span><h2>{mode === "faculty" ? "Faculty workload" : selectedDepartment?.label ?? "Department workload"}</h2></div><div className="workload-result-actions"><span className="resource-count">{visibleRows.length}</span><Button className="button-outline button-small" onClick={() => setConfiguringColumns(true)}>Configure columns</Button>{mode === "department" ? <Button className="button-primary button-small" onClick={() => setCreatingRecord(true)}><Plus size={15} /> Create workload record</Button> : null}</div></div>{visibleRows.length ? mode === "faculty" ? <FacultyMatrix rows={visibleRows} activities={displayedActivities} /> : <DepartmentMatrix rows={visibleRows} activities={displayedActivities} onAssign={setAssignment} /> : <p className="resource-empty">No workload records match this selection.</p>}</Panel> : null}
    <AssignmentInspector row={assignment} teachers={teacherAssignments} rooms={roomOptions} onClose={() => setAssignment(null)} onSaved={() => { setAssignment(null); load(); }} />
    <WorkloadRecordInspector open={creatingRecord} groups={groups} department={selectedDepartment} onClose={() => setCreatingRecord(false)} onCreated={() => { setCreatingRecord(false); load(); }} />
    <ColumnSettingsInspector open={configuringColumns} activities={activityTypes} selected={visibleActivities ?? activityTypes} onClose={() => setConfiguringColumns(false)} onChange={setVisibleActivities} />
  </div>;
}

function FacultyMatrix({ rows, activities }: { rows: WorkloadRow[]; activities: string[] }) { const matrix = groupWorkloadRows(rows); return <div className="workload-table-scroll"><table className="workload-table"><thead><tr><th>Department</th><th>Discipline</th><th>Group</th>{activities.map((activity) => <th key={activity}>{activity}</th>)}</tr></thead><tbody>{matrix.map((row) => <tr key={row.key}><td><strong>{row.departmentCode}</strong></td><td>{row.subject}</td><td>{row.group}</td>{activities.map((activity) => <td key={activity}>{row.activities[activity] ? `${row.activities[activity].plannedHours} h` : "—"}</td>)}</tr>)}</tbody></table></div>; }
function DepartmentMatrix({ rows, activities, onAssign }: { rows: WorkloadRow[]; activities: string[]; onAssign: (row: WorkloadRow) => void }) { const matrix = groupWorkloadRows(rows); const subjects = [...new Set(matrix.map((row) => row.subject))]; return <div className="department-matrix">{subjects.map((subject) => { const subjectRows = matrix.filter((row) => row.subject === subject); return <section key={subject}><header><h3>{subject}</h3><span>{subjectRows.length} groups</span></header><div className="workload-table-scroll"><table className="workload-table"><thead><tr><th>Group</th><th>Students</th>{activities.map((activity) => <th key={activity}>{activity}<small>Hours · Room · Teacher</small></th>)}</tr></thead><tbody>{subjectRows.map((row) => <tr key={row.key}><td><strong>{row.group}</strong></td><td>{row.students || "—"}</td>{activities.map((activity) => { const assignment = row.activities[activity]; return <td key={activity} className={assignment ? "assignment-cell" : "empty-assignment"}>{assignment ? <button type="button" className="assignment-cell-button" onClick={() => onAssign(assignment)}><strong>{assignment.plannedHours} h</strong><span>{assignment.room}</span><small>{assignment.teacher}</small></button> : <span>—</span>}</td>; })}</tr>)}</tbody></table></div></section>; })}</div>; }

function AssignmentInspector({ row, teachers, rooms, onClose, onSaved }: { row: WorkloadRow | null; teachers: EntityOption[]; rooms: EntityOption[]; onClose: () => void; onSaved: () => void }) {
  const [teacher, setTeacher] = useState<EntityOption | null>(null); const [teacherQuery, setTeacherQuery] = useState(""); const [room, setRoom] = useState<EntityOption | null>(null); const [roomQuery, setRoomQuery] = useState(""); const [saving, setSaving] = useState(false); const [error, setError] = useState("");
  useEffect(() => { if (!row) return; const currentTeacher = teachers.find((item) => item.id === row.teacherOnDepartmentId) ?? null; const currentRoom = rooms.find((item) => item.id === row.roomId) ?? null; setTeacher(currentTeacher); setTeacherQuery(currentTeacher?.label ?? ""); setRoom(currentRoom); setRoomQuery(currentRoom?.label ?? ""); }, [row, teachers, rooms]);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!row || !teacher) return; setSaving(true); setError(""); try { let assignmentId = row.teachingAssignmentId; if (assignmentId) await api.updateTeachingAssignment(assignmentId, { teacher_on_kafedra_id: Number(teacher.id) }); else { const response = await api.createTeachingAssignment({ load_requirement_id: Number(row.id), teacher_on_kafedra_id: Number(teacher.id) }); assignmentId = response.data.id; } if (room && assignmentId) { if (row.roomReferenceId) await api.updateAssignmentRoom(row.roomReferenceId, { room_id: Number(room.id) }); else await api.createAssignmentRoom({ teaching_assignment_id: Number(assignmentId), room_id: Number(room.id), priority_level: 1 }); } onSaved(); } catch { setError("The workload assignment could not be saved."); } finally { setSaving(false); } }
  return <Inspector title="Edit workload assignment" open={Boolean(row)} onClose={onClose}><form className="inspector-form" onSubmit={submit}><div className="inspector-note"><strong>{row?.subject}</strong><span>{row?.group} · {row?.activity} · {row?.plannedHours} hours</span></div><EntityPicker label="Teacher" query={teacherQuery} onQueryChange={setTeacherQuery} options={filter(teachers, teacherQuery)} selectedId={teacher?.id ?? null} onSelect={(item) => { setTeacher(item); setTeacherQuery(item.label); }} /><EntityPicker label="Room (optional)" query={roomQuery} onQueryChange={setRoomQuery} options={filter(rooms, roomQuery)} selectedId={room?.id ?? null} onSelect={(item) => { setRoom(item); setRoomQuery(item.label); }} />{error ? <p className="form-error" role="alert">{error}</p> : null}<Button className="button-primary" type="submit" disabled={!teacher || saving}>{saving ? "Saving…" : "Save assignment"}</Button></form></Inspector>;
}

function WorkloadRecordInspector({ open, groups, department, onClose, onCreated }: { open: boolean; groups: EntityOption[]; department: EntityOption | null; onClose: () => void; onCreated: () => void }) {
  const [group, setGroup] = useState<EntityOption | null>(null); const [groupQuery, setGroupQuery] = useState(""); const [disciplines, setDisciplines] = useState<EntityOption[]>([]); const [discipline, setDiscipline] = useState<EntityOption | null>(null); const [disciplineQuery, setDisciplineQuery] = useState(""); const [activityTypes, setActivityTypes] = useState<EntityOption[]>([]); const [activityType, setActivityType] = useState(""); const [hours, setHours] = useState(""); const [hoursPerEvent, setHoursPerEvent] = useState("2"); const [saving, setSaving] = useState(false); const [error, setError] = useState("");
  useEffect(() => { if (open) api.listAdminCollection("activity_types").then((response) => setActivityTypes(response.data.map((item) => toOption(item, "Activity type")))); }, [open]);
  useEffect(() => { if (disciplineQuery.trim().length < 2) { setDisciplines([]); return; } const controller = new AbortController(); const timer = window.setTimeout(() => api.searchDisciplines(disciplineQuery, controller.signal).then((response) => setDisciplines(response.data.map((item) => toOption(item, "Discipline")))).catch(() => setDisciplines([])), 250); return () => { window.clearTimeout(timer); controller.abort(); }; }, [disciplineQuery]);
  async function submit(event: React.FormEvent) { event.preventDefault(); if (!group || !discipline || !activityType || !department) return; setSaving(true); setError(""); try { const course = Number(group.label.split("-").at(-1)) || 1; const stream = await api.createCurriculumStream({ group_semester_id: Number(group.id), subject_id: Number(discipline.id), kafedra_id: Number(department.id), course }); await api.createStudyLoadRequirement({ stream_id: Number(stream.data.id), activity_type_id: Number(activityType), hours_assigned: Number(hours), hours_per_event: Number(hoursPerEvent) || undefined }); onCreated(); } catch { setError("The workload record could not be created."); } finally { setSaving(false); } }
  return <Inspector title="Create workload record" open={open} onClose={onClose}><form className="inspector-form" onSubmit={submit}><div className="inspector-note"><strong>{department?.label}</strong><span>The new curriculum stream and workload requirement will be created together.</span></div><EntityPicker label="Student group" query={groupQuery} onQueryChange={setGroupQuery} options={filter(groups, groupQuery)} selectedId={group?.id ?? null} onSelect={(item) => { setGroup(item); setGroupQuery(item.label); }} /><EntityPicker label="Discipline" query={disciplineQuery} onQueryChange={setDisciplineQuery} options={disciplines} selectedId={discipline?.id ?? null} onSelect={(item) => { setDiscipline(item); setDisciplineQuery(item.label); }} /><label><span>Activity type</span><select name="activity-type" required value={activityType} onChange={(e) => setActivityType(e.target.value)}><option value="">Choose an activity type</option>{activityTypes.map((item) => <option value={item.id} key={item.id}>{item.label}</option>)}</select></label><label><span>Total hours</span><input name="hours" required type="number" min="1" value={hours} onChange={(e) => setHours(e.target.value)} /></label><label><span>Hours per event</span><input name="hours-per-event" type="number" min="1" value={hoursPerEvent} onChange={(e) => setHoursPerEvent(e.target.value)} /></label>{error ? <p className="form-error" role="alert">{error}</p> : null}<Button className="button-primary" type="submit" disabled={!group || !discipline || !activityType || saving}>{saving ? "Creating…" : "Create workload record"}</Button></form></Inspector>;
}

function ColumnSettingsInspector({ open, activities, selected, onClose, onChange }: { open: boolean; activities: string[]; selected: string[]; onClose: () => void; onChange: (value: string[]) => void }) {
  return <Inspector title="Configure workload columns" open={open} onClose={onClose}><p className="inspector-help">Choose which activity types appear in this workload table. Source data is not changed.</p><fieldset className="column-checkboxes"><legend>Activity columns</legend>{activities.map((activity) => <label key={activity}><input type="checkbox" checked={selected.includes(activity)} onChange={() => onChange(selected.includes(activity) ? selected.filter((item) => item !== activity) : [...selected, activity])} /><span>{activity}</span></label>)}</fieldset><div className="inspector-actions"><Button className="button-primary" onClick={onClose}>Apply columns</Button><Button className="button-outline" onClick={() => onChange(activities)}>Show all</Button></div></Inspector>;
}

function teacherAssignmentOptions(response: { data: JsonApiResource[]; included?: JsonApiResource[] }, departmentId: string): EntityOption[] {
  const included = response.included ?? [];
  return response.data.flatMap((item) => {
    const department = relationshipId(item, "kafedra"); if (department && department !== departmentId) return [];
    const teacherId = relationshipId(item, "teacher"); const teacher = included.find((entry) => entry.type === "teacher" && entry.id === teacherId); if (!teacher) return [];
    const a = teacher.attributes ?? {}; const label = [a.last_name, a.first_name, a.middle_name].filter(Boolean).join(" "); return label ? [{ id: item.id, label }] : [];
  });
}
function relationshipId(item: JsonApiResource, name: string) { const relation = item.relationships?.[name] as { data?: { id?: string } } | undefined; return relation?.data?.id ? String(relation.data.id) : ""; }

function toSemester(item: JsonApiResource): SemesterSummary | null { const a = item.attributes ?? {}; const number = Number(a.number), startYear = Number(a.start_year), endYear = Number(a.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function toOption(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback) }; }
function filter(options: EntityOption[], query: string) { const value = query.trim().toLowerCase(); return value ? options.filter((option) => option.label.toLowerCase().includes(value)).slice(0, 20) : options.slice(0, 20); }
