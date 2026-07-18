import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Building2, CheckSquare, Square } from "lucide-react";

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
import { BuilderGrid, normalizeBuilderEvents, type BuilderEvent, type BuilderSlot } from "./schedule/BuilderGrid";
import { normalizeLoadEntries, matchSiblingRequirement, type LoadEntry } from "./schedule/loadPanel";
import { SlotEditor, type SlotContext, type WeekTypeOption } from "./schedule/SlotEditor";

export function AdminSchedule() {
  const { state: workspace, dispatch } = useWorkspace();
  const [mode, setMode] = useState<ScheduleMode>("group");
  const [scope, setScope] = useState<ScopeMode>("semester");
  const [semesters, setSemesters] = useState<SemesterSummary[]>([]);
  const [faculties, setFaculties] = useState<EntityOption[]>([]);
  const [groups, setGroups] = useState<EntityOption[]>([]);
  const [teachers, setTeachers] = useState<EntityOption[]>([]);
  const [rooms, setRooms] = useState<RoomSummary[]>([]);
  const [slots, setSlots] = useState<BuilderSlot[]>([]);
  const [weekTypes, setWeekTypes] = useState<WeekTypeOption[]>([]);
  const [selectedRoomIds, setSelectedRoomIds] = useState<string[]>([]);
  const [teacherQuery, setTeacherQuery] = useState("");
  const [roomQuery, setRoomQuery] = useState("");
  const [result, setResult] = useState<ScheduleSource>(null);
  const [occupancy, setOccupancy] = useState<Record<string, unknown> | null>(null);
  const [selectedEvent, setSelectedEvent] = useState<ScheduleEvent | null>(null);
  const [loadByGroup, setLoadByGroup] = useState<Record<string, LoadEntry[]>>({});
  const [scheduleByGroup, setScheduleByGroup] = useState<Record<string, BuilderEvent[]>>({});
  const [builderLoading, setBuilderLoading] = useState(false);
  const [editorContext, setEditorContext] = useState<SlotContext | null>(null);
  const [assignedEntry, setAssignedEntry] = useState<LoadEntry | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const loadCache = useRef<Record<string, LoadEntry[]>>({});

  useEffect(() => {
    let active = true;
    Promise.allSettled([
      api.listAdminCollection("semesters"),
      api.listAdminCollection("faculties"),
      api.listAdminCollection("rooms", "?page=1&per_page=100"),
      api.listAdminCollection("time_slots"),
      api.listAdminCollection("week_types"),
    ]).then(([semesterResult, facultyResult, roomResult, slotResult, weekResult]) => {
      if (!active) return;
      if (semesterResult.status === "fulfilled") setSemesters(semesterResult.value.data.map(toSemester).filter(isSemester));
      if (facultyResult.status === "fulfilled") setFaculties(facultyResult.value.data.map((item) => toOption(item, "Faculty")));
      if (roomResult.status === "fulfilled") setRooms(roomResult.value.data.map(toRoom));
      if (slotResult.status === "fulfilled") setSlots(slotResult.value.data.map(toSlot).sort((a, b) => a.callNumber - b.callNumber));
      if (weekResult.status === "fulfilled") setWeekTypes(weekResult.value.data.flatMap(toWeekType));
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
    if (teacherQuery.trim().length < 3) { setTeachers([]); return; }
    const controller = new AbortController();
    const timer = window.setTimeout(() => api.searchTeachers(teacherQuery, controller.signal).then((response) => setTeachers(response.data.map((item) => toOption(item, "Teacher")))).catch((caught) => { if ((caught as Error).name !== "AbortError") setTeachers([]); }), 250);
    return () => { window.clearTimeout(timer); controller.abort(); };
  }, [teacherQuery]);

  const weekCodeById = useMemo(() => new Map(weekTypes.map((item) => [String(item.id), item.code])), [weekTypes]);
  const roomOptions = useMemo(() => filterOptions(rooms.map((room) => ({ id: room.id, label: room.number ? `Room ${room.number}` : room.name || "Unnamed room", description: room.building || undefined })), roomQuery), [rooms, roomQuery]);
  const groupLabelById = useMemo(() => new Map(groups.map((group) => [group.id, group.label])), [groups]);

  const refreshGroup = useCallback(async (groupSemesterId: string) => {
    const [loadResponse, scheduleResponse] = await Promise.all([
      api.groupWorkload(groupSemesterId).catch(() => null),
      api.adminGroupSchedule(groupSemesterId).catch(() => null),
    ]);
    const entries = normalizeLoadEntries(loadResponse, groupSemesterId);
    loadCache.current[groupSemesterId] = entries;
    setLoadByGroup((current) => ({ ...current, [groupSemesterId]: entries }));
    setScheduleByGroup((current) => ({ ...current, [groupSemesterId]: normalizeBuilderEvents(scheduleResponse, weekCodeById) }));
  }, [weekCodeById]);

  useEffect(() => {
    if (mode !== "group" || workspace.groupIds.length === 0) return;
    let active = true;
    setBuilderLoading(true);
    Promise.all(workspace.groupIds.map((groupSemesterId) => refreshGroup(groupSemesterId)))
      .catch(() => active && setError("Load and schedule data could not be loaded for the selected groups."))
      .finally(() => active && setBuilderLoading(false));
    return () => { active = false; };
  }, [mode, workspace.groupIds, refreshGroup]);

  const resolveSiblings = useCallback(async (entry: LoadEntry) => {
    const gsids: number[] = [];
    const requirementIds: number[] = [];
    for (const siblingGsid of entry.streamSiblingGsids) {
      const key = String(siblingGsid);
      let entries = loadCache.current[key];
      if (!entries) {
        entries = normalizeLoadEntries(await api.groupWorkload(key).catch(() => null), key);
        loadCache.current[key] = entries;
      }
      const match = matchSiblingRequirement(entries, entry);
      if (match) { gsids.push(siblingGsid); requirementIds.push(match.id); }
    }
    return { gsids, requirementIds };
  }, []);

  async function loadSchedule() {
    setLoading(true); setError("");
    try {
      if (mode === "teacher" && workspace.teacherId) setResult(await api.teacherScheduleQuery(workspace.teacherId, scope === "date" ? { date: workspace.date, semesterId: workspace.semesterId ?? undefined } : { semesterId: workspace.semesterId ?? undefined }));
      if (mode === "room" && workspace.roomId) setResult(await api.roomScheduleQuery(workspace.roomId, scope === "date" ? { date: workspace.date, semesterId: workspace.semesterId ?? undefined } : { semesterId: workspace.semesterId ?? undefined }));
      if (mode === "occupancy") setOccupancy(await api.roomOccupancy(workspace.date, selectedRoomIds));
    } catch {
      setError("The schedule could not be loaded for the selected context.");
    } finally { setLoading(false); }
  }

  async function handleCreated() {
    const context = editorContext;
    const entry = assignedEntry;
    setEditorContext(null);
    setAssignedEntry(null);
    const affected = new Set<string>(context ? [context.groupSemesterId] : []);
    entry?.streamSiblingGsids.forEach((gsid) => { if (workspace.groupIds.includes(String(gsid))) affected.add(String(gsid)); });
    await Promise.all([...affected].map((gsid) => refreshGroup(gsid)));
  }

  const ready = mode === "teacher" ? Boolean(workspace.teacherId && (scope === "date" ? workspace.date : workspace.semesterId))
    : mode === "room" ? Boolean(workspace.roomId && (scope === "date" ? workspace.date : workspace.semesterId))
    : mode === "occupancy" ? Boolean(workspace.date && selectedRoomIds.length)
    : false;

  return <div className="admin-section schedule-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Schedule</span><h1>Schedule workspace</h1></div><p>Pick the semester first — the workspace remembers it across visits.</p></div>
    <ScheduleQuery mode={mode} onModeChange={(next) => { setMode(next); setResult(null); setOccupancy(null); setError(""); }} scope={scope} onScopeChange={(next) => { setScope(next); setResult(null); setError(""); }}>
      {mode === "group" ? <div className="guided-context">
        <div className="context-step context-step-wide"><span>1</span><div><label>Semester</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => dispatch({ type: "selectSemester", semesterId: semester.id })} /></div><small>The chosen semester is remembered.</small></div>
        {workspace.semesterId ? <div className="context-step context-step-wide"><span>2</span><div><label>Faculty</label><div className="faculty-card-grid">{faculties.map((faculty) => <button type="button" key={faculty.id} className={workspace.facultyId === faculty.id ? "faculty-card selected" : "faculty-card"} onClick={() => dispatch({ type: "selectFaculty", facultyId: faculty.id })}><Building2 size={17} aria-hidden="true" /><strong>{faculty.label}</strong>{faculty.description ? <span>{faculty.description}</span> : null}</button>)}</div></div><small>Pick the faculty whose groups you plan.</small></div> : null}
        {workspace.facultyId && workspace.semesterId ? <div className="context-step context-step-wide"><span>3</span><div><label>Student groups</label><div className="group-multi-list">{groups.length === 0 ? <p className="inspector-help">No groups for this faculty and semester.</p> : groups.map((group) => { const checked = workspace.groupIds.includes(group.id); return <button type="button" key={group.id} className={checked ? "group-multi-row selected" : "group-multi-row"} aria-pressed={checked} onClick={() => dispatch({ type: "toggleGroup", groupId: group.id })}>{checked ? <CheckSquare size={16} aria-hidden="true" /> : <Square size={16} aria-hidden="true" />}<span>{group.label}</span></button>; })}</div></div><small>Several groups can be planned side by side.</small></div> : null}
      </div> : null}

      {mode === "teacher" ? <div className="schedule-context-grid"><TimeScope scope={scope} semesters={semesters} workspace={workspace} dispatch={dispatch} /><EntityPicker label="Teacher" query={teacherQuery} onQueryChange={setTeacherQuery} options={teachers} selectedId={workspace.teacherId} onSelect={(option) => { dispatch({ type: "selectTeacher", teacherId: option.id }); setTeacherQuery(option.label); }} /></div> : null}
      {mode === "room" ? <div className="schedule-context-grid"><TimeScope scope={scope} semesters={semesters} workspace={workspace} dispatch={dispatch} /><EntityPicker label="Room" query={roomQuery} onQueryChange={setRoomQuery} options={roomOptions} selectedId={workspace.roomId} onSelect={(option) => { dispatch({ type: "selectRoom", roomId: option.id }); setRoomQuery(option.label); }} /></div> : null}
      {mode === "occupancy" ? <div className="schedule-context-grid"><DateField value={workspace.date} onChange={(date) => dispatch({ type: "selectDate", date })} /><div><EntityPicker label="Rooms" query={roomQuery} onQueryChange={setRoomQuery} options={roomOptions} selectedId={null} onSelect={(option) => { setSelectedRoomIds((current) => current.includes(option.id) ? current.filter((id) => id !== option.id) : [...current, option.id]); setRoomQuery(""); }} /><div className="selected-room-list">{selectedRoomIds.map((id) => <button type="button" key={id} onClick={() => setSelectedRoomIds((current) => current.filter((item) => item !== id))}>{roomOptions.find((room) => room.id === id)?.label ?? "Selected room"} ×</button>)}</div></div></div> : null}
      {mode !== "group" ? <div className="schedule-query-actions"><Button className="button-primary" disabled={!ready || loading} onClick={loadSchedule}>{loading ? "Loading…" : mode === "occupancy" ? "Show occupancy" : "Show schedule"}</Button></div> : null}
      {error ? <div className="capability-unavailable" role="alert"><strong>Something went wrong</strong><p>{error}</p></div> : null}
    </ScheduleQuery>

    {mode === "group" && workspace.groupIds.length > 0 ? <div className="builder-layout">
      <aside className="builder-load-panel" aria-label="Load distribution">
        <header><strong>Load distribution</strong><span>{builderLoading ? "Loading…" : "Drag a row onto the editor"}</span></header>
        {workspace.groupIds.map((groupSemesterId) => <section key={groupSemesterId}>
          <h3>{groupLabelById.get(groupSemesterId) ?? `Group semester #${groupSemesterId}`}</h3>
          {(loadByGroup[groupSemesterId] ?? []).map((entry) => <article
            key={entry.id}
            className={entry.actualHours >= entry.controlSum && entry.controlSum > 0 ? "load-row complete" : "load-row"}
            draggable
            onDragStart={(event) => { event.dataTransfer.setData("application/x-load-entry", String(entry.id)); event.dataTransfer.effectAllowed = "copy"; }}
          >
            <div className="load-row-main">
              <strong>{entry.subjectShort}</strong>
              <span>{entry.activityName}{entry.streamSiblingGsids.length ? ` · stream +${entry.streamSiblingGsids.length}` : ""}</span>
              <small>{entry.teachers.map((teacher) => teacher.name).join(", ") || "No teacher assigned"}{entry.reservedRoom ? ` · ${entry.reservedRoom.name}` : ""}</small>
            </div>
            <div className="load-row-hours" title="Scheduled hours / control sum"><strong>{entry.actualHours}</strong><span>/{entry.controlSum} h</span></div>
            <button type="button" className="load-row-use" disabled={!editorContext || editorContext.groupSemesterId !== groupSemesterId} onClick={() => setAssignedEntry(entry)}>Use</button>
          </article>)}
          {(loadByGroup[groupSemesterId] ?? []).length === 0 && !builderLoading ? <p className="inspector-help">No load distribution for this group.</p> : null}
        </section>)}
      </aside>
      <div className="builder-grids">
        {workspace.groupIds.map((groupSemesterId) => <BuilderGrid
          key={groupSemesterId}
          groupLabel={groupLabelById.get(groupSemesterId) ?? `Group semester #${groupSemesterId}`}
          slots={slots}
          events={scheduleByGroup[groupSemesterId] ?? []}
          onCellClick={(dayOfWeek, timeSlotId) => { setAssignedEntry(null); setEditorContext({ groupSemesterId, groupLabel: groupLabelById.get(groupSemesterId) ?? `Group semester #${groupSemesterId}`, dayOfWeek, timeSlotId }); }}
        />)}
      </div>
    </div> : null}
    {mode === "group" && workspace.groupIds.length === 0 ? <Panel className="schedule-result"><div className="schedule-empty"><h2>Select groups to start planning</h2><p>The load distribution appears on the left and one grid per group on the right.</p></div></Panel> : null}

    {mode !== "group" ? <Panel className="schedule-result">{mode === "occupancy" ? <OccupancyGrid source={occupancy} rooms={rooms.filter((room) => selectedRoomIds.includes(room.id))} /> : <WeekGrid source={result} onSelect={setSelectedEvent} />}</Panel> : null}
    <ScheduleInspector event={selectedEvent} onClose={() => setSelectedEvent(null)} onChanged={() => { setSelectedEvent(null); loadSchedule(); }} />
    <SlotEditor
      context={editorContext}
      assignedEntry={assignedEntry}
      onAssignEntry={setAssignedEntry}
      loadEntries={editorContext ? loadByGroup[editorContext.groupSemesterId] ?? [] : []}
      slots={slots}
      weekTypes={weekTypes}
      rooms={rooms.map((room) => ({ id: room.id, label: room.number ? `Room ${room.number}` : room.name || "Unnamed room" }))}
      resolveSiblings={resolveSiblings}
      onClose={() => { setEditorContext(null); setAssignedEntry(null); }}
      onCreated={() => { void handleCreated(); }}
    />
  </div>;
}

export function queryModeRequirements(mode: ScheduleMode, scope: ScopeMode) {
  if (mode === "group") return ["semester", "faculty", "groups"];
  if (mode === "occupancy") return ["date", "rooms"];
  return [scope, mode];
}

function TimeScope({ scope, semesters, workspace, dispatch }: { scope: ScopeMode; semesters: SemesterSummary[]; workspace: ReturnType<typeof useWorkspace>["state"]; dispatch: ReturnType<typeof useWorkspace>["dispatch"] }) { return scope === "date" ? <DateField value={workspace.date} onChange={(date) => dispatch({ type: "selectDate", date })} /> : <div><label>Semester</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => dispatch({ type: "selectSemester", semesterId: semester.id })} /></div>; }
function DateField({ value, onChange }: { value: string; onChange: (value: string) => void }) { return <label className="date-field"><span>Date</span><input type="date" value={value} onChange={(event) => onChange(event.target.value)} /></label>; }
function toSemester(item: JsonApiResource): SemesterSummary | null { const attrs = item.attributes ?? {}; const number = Number(attrs.number); const startYear = Number(attrs.start_year); const endYear = Number(attrs.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function toOption(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback), description: stringAttribute(item, "faculty_name") || stringAttribute(item, "post_name") || undefined }; }
function toRoom(item: JsonApiResource): RoomSummary { return { id: item.id, number: stringAttribute(item, "number"), name: stringAttribute(item, "name"), building: stringAttribute(item, "building_name") }; }
function toSlot(item: JsonApiResource): BuilderSlot { const attrs = item.attributes ?? {}; const start = timeOf(attrs.start_time); const end = timeOf(attrs.end_time); return { id: item.id, label: `${start}–${end}`, startTime: start, callNumber: Number(attrs.call_number ?? 0) }; }
function toWeekType(item: JsonApiResource): WeekTypeOption[] { const code = String(item.attributes?.code ?? ""); return code === "both" || code === "odd" || code === "even" ? [{ id: Number(item.id), code, name: String(item.attributes?.name ?? code) }] : []; }
function timeOf(value: unknown): string { return String(value ?? "").match(/\d{2}:\d{2}/)?.[0] ?? "—"; }
function stringAttribute(item: JsonApiResource, key: string) { const value = item.attributes?.[key]; return typeof value === "string" || typeof value === "number" ? String(value) : ""; }
function filterOptions(options: EntityOption[], query: string) { const normalized = query.trim().toLocaleLowerCase(); return normalized ? options.filter((option) => `${option.label} ${option.description ?? ""}`.toLocaleLowerCase().includes(normalized)).slice(0, 20) : options.slice(0, 20); }
