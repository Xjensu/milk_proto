import { useEffect, useMemo, useState } from "react";
import { Building2, CalendarDays, UsersRound } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type JsonApiResource } from "../../lib/api";
import { WeekGrid } from "../schedule/WeekGrid";
import type { ScheduleEvent, ScheduleSource } from "../schedule/types";
import type { RoomSummary, SemesterSummary } from "./domain";
import { groupResource, resourceLabel } from "./utils";
import { EntityPicker, type EntityOption } from "./workspace/EntityPicker";
import { SemesterRail } from "./workspace/SemesterRail";
import { useWorkspace } from "./workspace/WorkspaceContext";
import { OccupancyGrid } from "./schedule/OccupancyGrid";
import { ScheduleInspector } from "./schedule/ScheduleInspector";
import { ScheduleQuery, type ScheduleMode, type ScopeMode } from "./schedule/ScheduleQuery";

export function AdminSchedule() {
  const { state: workspace, dispatch } = useWorkspace();
  const [mode, setMode] = useState<ScheduleMode>("group");
  const [scope, setScope] = useState<ScopeMode>("semester");
  const [semesters, setSemesters] = useState<SemesterSummary[]>([]);
  const [faculties, setFaculties] = useState<EntityOption[]>([]);
  const [groups, setGroups] = useState<EntityOption[]>([]);
  const [teachers, setTeachers] = useState<EntityOption[]>([]);
  const [rooms, setRooms] = useState<RoomSummary[]>([]);
  const [selectedRoomIds, setSelectedRoomIds] = useState<string[]>([]);
  const [facultyQuery, setFacultyQuery] = useState("");
  const [groupQuery, setGroupQuery] = useState("");
  const [teacherQuery, setTeacherQuery] = useState("");
  const [roomQuery, setRoomQuery] = useState("");
  const [result, setResult] = useState<ScheduleSource>(null);
  const [occupancy, setOccupancy] = useState<Record<string, unknown> | null>(null);
  const [selectedEvent, setSelectedEvent] = useState<ScheduleEvent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    Promise.allSettled([api.listAdminCollection("semesters"), api.listAdminCollection("faculties"), api.listAdminCollection("rooms", "?page=1&per_page=100")]).then(([semesterResult, facultyResult, roomResult]) => {
      if (!active) return;
      if (semesterResult.status === "fulfilled") setSemesters(semesterResult.value.data.map(toSemester).filter(isSemester));
      if (facultyResult.status === "fulfilled") setFaculties(facultyResult.value.data.map((item) => toOption(item, "Faculty")));
      if (roomResult.status === "fulfilled") setRooms(roomResult.value.data.map(toRoom));
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!workspace.facultyId || !workspace.semesterId) { setGroups([]); return; }
    const controller = new AbortController();
    api.listFacultyGroups(workspace.facultyId, workspace.semesterId, controller.signal).then((response) => setGroups(response.data.map(groupResource))).catch((caught) => { if ((caught as Error).name !== "AbortError") setGroups([]); });
    return () => controller.abort();
  }, [workspace.facultyId, workspace.semesterId]);

  useEffect(() => {
    if (teacherQuery.trim().length < 2) { setTeachers([]); return; }
    const controller = new AbortController();
    const timer = window.setTimeout(() => api.searchTeachers(teacherQuery, controller.signal).then((response) => setTeachers(response.data.map((item) => toOption(item, "Teacher")))).catch((caught) => { if ((caught as Error).name !== "AbortError") setTeachers([]); }), 250);
    return () => { window.clearTimeout(timer); controller.abort(); };
  }, [teacherQuery]);

  const filteredFaculties = useMemo(() => filterOptions(faculties, facultyQuery), [faculties, facultyQuery]);
  const filteredGroups = useMemo(() => filterOptions(groups, groupQuery), [groups, groupQuery]);
  const roomOptions = useMemo(() => filterOptions(rooms.map((room) => ({ id: room.id, label: room.number ? `Room ${room.number}` : room.name || "Unnamed room", description: room.building || undefined })), roomQuery), [rooms, roomQuery]);

  async function loadSchedule() {
    setLoading(true); setError("");
    try {
      if (mode === "group" && workspace.groupId) setResult(await api.adminGroupSchedule(workspace.groupId));
      if (mode === "teacher" && workspace.teacherId) setResult(await api.teacherScheduleQuery(workspace.teacherId, scope === "date" ? { date: workspace.date } : { semesterId: workspace.semesterId ?? undefined }));
      if (mode === "room" && workspace.roomId) setResult(await api.roomScheduleQuery(workspace.roomId, scope === "date" ? { date: workspace.date } : { semesterId: workspace.semesterId ?? undefined }));
      if (mode === "occupancy") setOccupancy(await api.roomOccupancy(workspace.date, selectedRoomIds));
    } catch {
      setError(scope === "date" || mode === "occupancy" ? "This gateway does not currently expose the requested date projection." : "Schedule data could not be loaded for this context.");
    } finally { setLoading(false); }
  }

  const ready = requirementsMet(mode, scope, workspace, selectedRoomIds);
  return <div className="admin-section schedule-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Schedule</span><h1>Schedule workspace</h1></div><p>Choose readable context first; technical identifiers remain hidden.</p></div>
    <ScheduleQuery mode={mode} onModeChange={(next) => { setMode(next); setResult(null); setOccupancy(null); setError(""); }} scope={scope} onScopeChange={(next) => { setScope(next); setResult(null); setError(""); }}>
      {mode === "group" ? <div className="guided-context">
        <div className="context-step"><span>1</span><EntityPicker label="Faculty" query={facultyQuery} onQueryChange={setFacultyQuery} options={filteredFaculties} selectedId={workspace.facultyId} onSelect={(option) => { dispatch({ type: "selectFaculty", facultyId: option.id }); setFacultyQuery(option.label); }} /><small>Choose a faculty first</small></div>
        <div className="context-step context-step-wide"><span>2</span><div><label>Semester</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => dispatch({ type: "selectSemester", semesterId: semester.id })} /></div><small>Choose a semester</small></div>
        <div className="context-step"><span>3</span><EntityPicker label="Student group" query={groupQuery} onQueryChange={setGroupQuery} options={filteredGroups} selectedId={workspace.groupId} disabled={!workspace.facultyId || !workspace.semesterId} onSelect={(option) => { dispatch({ type: "selectGroup", groupId: option.id }); setGroupQuery(option.label); }} /><small>Groups are scoped to the selected faculty and semester.</small></div>
      </div> : null}

      {mode === "teacher" ? <div className="schedule-context-grid"><TimeScope scope={scope} semesters={semesters} workspace={workspace} dispatch={dispatch} /><EntityPicker label="Teacher" query={teacherQuery} onQueryChange={setTeacherQuery} options={teachers} selectedId={workspace.teacherId} onSelect={(option) => { dispatch({ type: "selectTeacher", teacherId: option.id }); setTeacherQuery(option.label); }} /></div> : null}
      {mode === "room" ? <div className="schedule-context-grid"><TimeScope scope={scope} semesters={semesters} workspace={workspace} dispatch={dispatch} /><EntityPicker label="Room" query={roomQuery} onQueryChange={setRoomQuery} options={roomOptions} selectedId={workspace.roomId} onSelect={(option) => { dispatch({ type: "selectRoom", roomId: option.id }); setRoomQuery(option.label); }} /></div> : null}
      {mode === "occupancy" ? <div className="schedule-context-grid"><DateField value={workspace.date} onChange={(date) => dispatch({ type: "selectDate", date })} /><div><EntityPicker label="Rooms" query={roomQuery} onQueryChange={setRoomQuery} options={roomOptions} selectedId={null} onSelect={(option) => { setSelectedRoomIds((current) => current.includes(option.id) ? current.filter((id) => id !== option.id) : [...current, option.id]); setRoomQuery(""); }} /><div className="selected-room-list">{selectedRoomIds.map((id) => <button type="button" key={id} onClick={() => setSelectedRoomIds((current) => current.filter((item) => item !== id))}>{roomOptions.find((room) => room.id === id)?.label ?? "Selected room"} ×</button>)}</div></div></div> : null}
      <div className="schedule-query-actions"><Button className="button-primary" disabled={!ready || loading} onClick={loadSchedule}>{loading ? "Loading…" : mode === "occupancy" ? "Show occupancy" : "Show schedule"}</Button></div>
      {error ? <div className="capability-unavailable" role="alert"><strong>Projection unavailable</strong><p>{error}</p></div> : null}
    </ScheduleQuery>

    <Panel className="schedule-result">{mode === "occupancy" ? <OccupancyGrid source={occupancy} rooms={rooms.filter((room) => selectedRoomIds.includes(room.id))} /> : <WeekGrid source={result} onSelect={setSelectedEvent} />}</Panel>
    <ScheduleInspector event={selectedEvent} onClose={() => setSelectedEvent(null)} />
  </div>;
}

export function queryModeRequirements(mode: ScheduleMode, scope: ScopeMode) {
  if (mode === "group") return ["faculty", "semester", "group"];
  if (mode === "occupancy") return ["date", "rooms"];
  return [scope, mode];
}

function requirementsMet(mode: ScheduleMode, scope: ScopeMode, state: ReturnType<typeof useWorkspace>["state"], rooms: string[]) { if (mode === "group") return Boolean(state.facultyId && state.semesterId && state.groupId); if (mode === "teacher") return Boolean(state.teacherId && (scope === "date" ? state.date : state.semesterId)); if (mode === "room") return Boolean(state.roomId && (scope === "date" ? state.date : state.semesterId)); return Boolean(state.date && rooms.length); }
function TimeScope({ scope, semesters, workspace, dispatch }: { scope: ScopeMode; semesters: SemesterSummary[]; workspace: ReturnType<typeof useWorkspace>["state"]; dispatch: ReturnType<typeof useWorkspace>["dispatch"] }) { return scope === "date" ? <DateField value={workspace.date} onChange={(date) => dispatch({ type: "selectDate", date })} /> : <div><label>Semester</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => dispatch({ type: "selectSemester", semesterId: semester.id })} /></div>; }
function DateField({ value, onChange }: { value: string; onChange: (value: string) => void }) { return <label className="date-field"><span>Date</span><input type="date" value={value} onChange={(event) => onChange(event.target.value)} /></label>; }
function toSemester(item: JsonApiResource): SemesterSummary | null { const attrs = item.attributes ?? {}; const number = Number(attrs.number); const startYear = Number(attrs.start_year); const endYear = Number(attrs.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function toOption(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback), description: stringAttribute(item, "faculty_name") || stringAttribute(item, "post_name") || undefined }; }
function toRoom(item: JsonApiResource): RoomSummary { return { id: item.id, number: stringAttribute(item, "number"), name: stringAttribute(item, "name"), building: stringAttribute(item, "building_name") }; }
function stringAttribute(item: JsonApiResource, key: string) { const value = item.attributes?.[key]; return typeof value === "string" || typeof value === "number" ? String(value) : ""; }
function filterOptions(options: EntityOption[], query: string) { const normalized = query.trim().toLocaleLowerCase(); return normalized ? options.filter((option) => `${option.label} ${option.description ?? ""}`.toLocaleLowerCase().includes(normalized)).slice(0, 20) : options.slice(0, 20); }
