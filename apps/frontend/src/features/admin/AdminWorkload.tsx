import { useEffect, useMemo, useState } from "react";
import { Building2, GraduationCap } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type JsonApiResource } from "../../lib/api";
import type { SemesterSummary } from "./domain";
import { resourceLabel } from "./utils";
import { EntityPicker, type EntityOption } from "./workspace/EntityPicker";
import { SemesterRail } from "./workspace/SemesterRail";
import { useWorkspace } from "./workspace/WorkspaceContext";
import { normalizeWorkload, type WorkloadRow } from "./workload/normalize";

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
    api.listFacultyGroups(workspace.facultyId, workspace.semesterId).then((response) => setGroups(response.data.map((item) => toOption(item, "Student group")))).catch(() => setGroups([]));
  }, [workspace.facultyId, workspace.semesterId]);

  const visibleRows = useMemo(() => mode === "faculty" && selectedGroups.length ? rows.filter((row) => selectedGroups.includes(row.groupSemesterId)) : rows, [mode, rows, selectedGroups]);
  const activityTypes = useMemo(() => [...new Set(visibleRows.map((row) => row.activity))], [visibleRows]);

  async function load() {
    if (!workspace.semesterId || !workspace.facultyId || (mode === "department" && !selectedDepartment)) return;
    setLoading(true); setError("");
    try {
      const response = mode === "faculty" ? await api.facultyWorkload(workspace.facultyId, workspace.semesterId) : await api.departmentWorkload(selectedDepartment!.id, workspace.semesterId);
      setRows(normalizeWorkload(response)); setLoaded(true);
    } catch { setError("Workload could not be loaded for this context."); }
    finally { setLoading(false); }
  }

  const ready = Boolean(workspace.facultyId && workspace.semesterId && (mode === "faculty" || selectedDepartment));
  return <div className="admin-section workload-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Academic planning</span><h1>Workload distribution</h1></div><p>Build readable faculty summaries or assign department teaching capacity.</p></div>
    <div className="workload-mode-cards" role="tablist" aria-label="Workload view">
      <button type="button" role="tab" aria-selected={mode === "faculty"} className={mode === "faculty" ? "active" : ""} onClick={() => { setMode("faculty"); setLoaded(false); }}><GraduationCap /><span><strong>Faculty workload</strong><small>Disciplines, groups, and hours by activity type</small></span></button>
      <button type="button" role="tab" aria-selected={mode === "department"} className={mode === "department" ? "active" : ""} onClick={() => { setMode("department"); setLoaded(false); }}><Building2 /><span><strong>Department workload</strong><small>Teachers, rooms, and assignment gaps</small></span></button>
    </div>
    <Panel className="workload-context"><div className="guided-context">
      <div className="context-step"><span>1</span><EntityPicker label="Faculty" query={facultyQuery} onQueryChange={setFacultyQuery} options={filter(faculties, facultyQuery)} selectedId={workspace.facultyId} onSelect={(option) => { dispatch({ type: "selectFaculty", facultyId: option.id }); setFacultyQuery(option.label); setSelectedDepartment(null); setLoaded(false); }} /><small>Choose the faculty that owns the groups.</small></div>
      <div className="context-step context-step-wide"><span>2</span><div><label>Semester</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => { dispatch({ type: "selectSemester", semesterId: semester.id }); setLoaded(false); }} /></div><small>Semester context determines group course numbers.</small></div>
      {mode === "department" ? <div className="context-step"><span>3</span><EntityPicker label="Department" query={departmentQuery} onQueryChange={setDepartmentQuery} options={filter(departments, departmentQuery)} selectedId={selectedDepartment?.id ?? null} disabled={!workspace.facultyId} onSelect={(option) => { setSelectedDepartment(option); setDepartmentQuery(option.label); setLoaded(false); }} /><small>Only departments inside the selected faculty are shown.</small></div> : null}
      {mode === "faculty" && workspace.facultyId && workspace.semesterId ? <div className="context-step context-step-wide"><span>3</span><div><label>Student groups</label><div className="group-chip-list"><button type="button" className={!selectedGroups.length ? "active" : ""} onClick={() => setSelectedGroups([])}>All groups</button>{groups.map((group) => <button type="button" className={selectedGroups.includes(group.id) ? "active" : ""} key={group.id} onClick={() => setSelectedGroups((current) => current.includes(group.id) ? current.filter((id) => id !== group.id) : [...current, group.id])}>{group.label}</button>)}</div></div><small>Select individual groups or keep the faculty-wide view.</small></div> : null}
    </div><div className="schedule-query-actions"><Button className="button-primary" onClick={load} disabled={!ready || loading}>{loading ? "Loading…" : "Open workload"}</Button></div>{error ? <div className="form-error" role="alert">{error}</div> : null}</Panel>
    {loaded ? <Panel className="workload-results"><div className="panel-heading"><div><span className="eyebrow">Current selection</span><h2>{mode === "faculty" ? "Faculty workload" : selectedDepartment?.label ?? "Department workload"}</h2></div><span className="resource-count">{visibleRows.length}</span></div>{visibleRows.length ? mode === "faculty" ? <FacultyMatrix rows={visibleRows} activities={activityTypes} /> : <DepartmentMatrix rows={visibleRows} activities={activityTypes} /> : <p className="resource-empty">No workload records match this selection.</p>}</Panel> : null}
  </div>;
}

function FacultyMatrix({ rows, activities }: { rows: WorkloadRow[]; activities: string[] }) { return <div className="workload-table-scroll"><table className="workload-table"><thead><tr><th>Department</th><th>Discipline</th><th>Group</th>{activities.map((activity) => <th key={activity}>{activity}</th>)}</tr></thead><tbody>{rows.map((row) => <tr key={row.id}><td><strong>{row.departmentCode}</strong></td><td>{row.subject}</td><td>{row.group}</td>{activities.map((activity) => <td key={activity}>{row.activity === activity ? `${row.plannedHours} h` : "—"}</td>)}</tr>)}</tbody></table></div>; }
function DepartmentMatrix({ rows, activities }: { rows: WorkloadRow[]; activities: string[] }) { const subjects = [...new Set(rows.map((row) => row.subject))]; return <div className="department-matrix">{subjects.map((subject) => <section key={subject}><header><h3>{subject}</h3><span>{rows.filter((row) => row.subject === subject).length} assignments</span></header><div className="workload-table-scroll"><table className="workload-table"><thead><tr><th>Group</th><th>Students</th>{activities.map((activity) => <th key={activity}>{activity}<small>Hours · Room · Teacher</small></th>)}</tr></thead><tbody>{rows.filter((row) => row.subject === subject).map((row) => <tr key={row.id}><td><strong>{row.group}</strong></td><td>{row.students || "—"}</td>{activities.map((activity) => <td key={activity} className={row.activity === activity ? "assignment-cell" : "empty-assignment"}>{row.activity === activity ? <><strong>{row.plannedHours} h</strong><span>{row.room}</span><small>{row.teacher}</small></> : <button type="button">Assign</button>}</td>)}</tr>)}</tbody></table></div></section>)}</div>; }

function toSemester(item: JsonApiResource): SemesterSummary | null { const a = item.attributes ?? {}; const number = Number(a.number), startYear = Number(a.start_year), endYear = Number(a.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function toOption(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback) }; }
function filter(options: EntityOption[], query: string) { const value = query.trim().toLowerCase(); return value ? options.filter((option) => option.label.toLowerCase().includes(value)).slice(0, 20) : options.slice(0, 20); }
