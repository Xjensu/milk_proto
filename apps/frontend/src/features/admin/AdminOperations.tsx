import { useEffect, useMemo, useState } from "react";
import { ArrowRight, CalendarRange } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type GroupPeriodInput, type JsonApiResource } from "../../lib/api";
import type { SemesterSummary } from "./domain";
import { groupResource, resourceLabel } from "./utils";
import { normalizeScheduleEvents } from "./schedule/normalize";
import { EntityPicker, type EntityOption } from "./workspace/EntityPicker";
import { Inspector } from "./workspace/Inspector";
import { SemesterRail } from "./workspace/SemesterRail";
import { useWorkspace } from "./workspace/WorkspaceContext";

const periodKinds: Array<{ kind: GroupPeriodInput["period_kind"]; title: string; description: string }> = [
  { kind: "lecture_week", title: "Lecture period", description: "Regular teaching dates for this group" },
  { kind: "credit_week", title: "Credit period", description: "Assessments completed as credits" },
  { kind: "exam_session", title: "Exam session", description: "Formal examination dates" },
];

export function AdminOperations() {
  const { state: workspace, dispatch } = useWorkspace();
  const [semesters, setSemesters] = useState<SemesterSummary[]>([]);
  const [faculties, setFaculties] = useState<EntityOption[]>([]);
  const [groups, setGroups] = useState<EntityOption[]>([]);
  const [facultyQuery, setFacultyQuery] = useState("");
  const [groupQuery, setGroupQuery] = useState("");
  const [periods, setPeriods] = useState<Record<string, { start: string; end: string }>>({});
  const [events, setEvents] = useState<ReturnType<typeof normalizeScheduleEvents>>([]);
  const [selectedEvent, setSelectedEvent] = useState<ReturnType<typeof normalizeScheduleEvents>[number] | null>(null);
  const [saving, setSaving] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => { let active = true; Promise.allSettled([api.listAdminCollection("semesters"), api.listAdminCollection("faculties")]).then(([s, f]) => { if (!active) return; if (s.status === "fulfilled") setSemesters(s.value.data.map(toSemester).filter(isSemester)); if (f.status === "fulfilled") setFaculties(f.value.data.map((item) => toOption(item, "Faculty"))); }); return () => { active = false; }; }, []);
  useEffect(() => { if (!workspace.facultyId || !workspace.semesterId) { setGroups([]); return; } api.listFacultyGroups(workspace.facultyId, workspace.semesterId).then((response) => setGroups(response.data.map(groupResource))).catch(() => setGroups([])); }, [workspace.facultyId, workspace.semesterId]);
  useEffect(() => {
    if (!workspace.groupId) { setPeriods({}); setEvents([]); return; }
    Promise.allSettled([api.groupPeriods(workspace.groupId), api.adminGroupSchedule(workspace.groupId)]).then(([periodResult, scheduleResult]) => {
      if (periodResult.status === "fulfilled") setPeriods(normalizePeriods(periodResult.value.data));
      if (scheduleResult.status === "fulfilled") setEvents(normalizeScheduleEvents(scheduleResult.value));
    });
  }, [workspace.groupId]);

  const filteredFaculties = useMemo(() => filter(faculties, facultyQuery), [faculties, facultyQuery]);
  const filteredGroups = useMemo(() => filter(groups, groupQuery), [groups, groupQuery]);

  async function savePeriod(kind: GroupPeriodInput["period_kind"]) {
    const value = periods[kind]; if (!workspace.groupId || !value?.start || !value.end) return;
    setSaving(kind); setMessage("");
    try { await api.upsertGroupPeriod(workspace.groupId, { period_kind: kind, start_date: value.start, end_date: value.end }); setMessage("Calendar period saved."); }
    catch { setMessage("The calendar period could not be saved."); }
    finally { setSaving(""); }
  }

  return <div className="admin-section calendar-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Учебный календарь</span><h1>Календарь группы</h1></div><p>Настройте учебные и контрольные периоды, затем перенесите отдельные занятия.</p></div>
    <Panel className="calendar-context"><div className="guided-context">
      <div className="context-step"><span>1</span><EntityPicker label="Факультет" query={facultyQuery} onQueryChange={setFacultyQuery} options={filteredFaculties} selectedId={workspace.facultyId} onSelect={(option) => { dispatch({ type: "selectFaculty", facultyId: option.id }); setFacultyQuery(option.label); }} /><small>Выберите факультет, чтобы ограничить список групп.</small></div>
      <div className="context-step context-step-wide"><span>2</span><div><label>Семестр</label><SemesterRail semesters={semesters} selectedId={workspace.semesterId} onSelect={(semester) => dispatch({ type: "selectSemester", semesterId: semester.id })} /></div><small>Выбранный семестр используется во всех административных задачах.</small></div>
      <div className="context-step"><span>3</span><EntityPicker label="Учебная группа" query={groupQuery} onQueryChange={setGroupQuery} options={filteredGroups} selectedId={workspace.groupId} disabled={!workspace.facultyId || !workspace.semesterId} onSelect={(option) => { dispatch({ type: "selectGroup", groupId: option.id }); setGroupQuery(option.label); }} /><small>Настройки календаря применяются только к выбранной группе и семестру.</small></div>
    </div></Panel>
    <div className="period-card-grid">{periodKinds.map((period) => { const value = periods[period.kind] ?? { start: "", end: "" }; return <Panel className="period-card" key={period.kind}><div className="period-card-title"><CalendarRange /><div><h2>{period.title}</h2><p>{period.description}</p></div></div><div className="period-dates"><label><span>Starts</span><input type="date" value={value.start} disabled={!workspace.groupId} onChange={(event) => setPeriods((current) => ({ ...current, [period.kind]: { ...value, start: event.target.value } }))} /></label><ArrowRight /><label><span>Ends</span><input type="date" value={value.end} disabled={!workspace.groupId} onChange={(event) => setPeriods((current) => ({ ...current, [period.kind]: { ...value, end: event.target.value } }))} /></label></div><Button className="button-primary" disabled={!workspace.groupId || !value.start || !value.end || saving === period.kind} onClick={() => savePeriod(period.kind)}>{saving === period.kind ? "Saving…" : "Save period"}</Button></Panel>; })}</div>
    {message ? <p className="request-status" role="status">{message}</p> : null}
    <Panel className="transfer-panel"><div className="panel-heading"><div><span className="eyebrow">Single occurrences</span><h2>Move a scheduled class</h2></div><span className="resource-count">{events.length}</span></div>{!workspace.groupId ? <p className="resource-empty">Choose a group to see its scheduled classes.</p> : events.length ? <div className="transfer-event-list">{events.map((event) => <button type="button" key={event.id} onClick={() => setSelectedEvent(event)}><time>{event.startTime}</time><span><strong>{event.subject}</strong><small>{event.activityType} · {event.room}</small></span><ArrowRight /></button>)}</div> : <p className="resource-empty">No scheduled classes were returned for this group.</p>}</Panel>
    <TransferInspector event={selectedEvent} onClose={() => setSelectedEvent(null)} />
  </div>;
}

function TransferInspector({ event, onClose }: { event: ReturnType<typeof normalizeScheduleEvents>[number] | null; onClose: () => void }) {
  const [originalDate, setOriginalDate] = useState(""); const [newDate, setNewDate] = useState(""); const [timeSlots, setTimeSlots] = useState<JsonApiResource[]>([]); const [timeSlot, setTimeSlot] = useState(""); const [rooms, setRooms] = useState<EntityOption[]>([]); const [room, setRoom] = useState<EntityOption | null>(null); const [roomQuery, setRoomQuery] = useState(""); const [reason, setReason] = useState(""); const [saving, setSaving] = useState(false); const [error, setError] = useState("");
  useEffect(() => { if (!event) return; Promise.all([api.listAdminCollection("time_slots"), api.listAdminCollection("rooms", "?page=1&per_page=100")]).then(([slotResponse, roomResponse]) => { setTimeSlots(slotResponse.data); setRooms(roomResponse.data.map((item) => toOption(item, "Room"))); }); }, [event]);
  if (!event) return null;
  async function submit(submitEvent: React.FormEvent) { submitEvent.preventDefault(); if (!originalDate || !newDate || !timeSlot) return; setSaving(true); setError(""); try { await api.createEventTransfer(event!.id, { original_date: originalDate, transfer_type: "moved", new_date: newDate, new_time_slot_id: Number(timeSlot), new_room_id: room ? Number(room.id) : undefined, reason: reason || undefined }); onClose(); } catch { setError("This class occurrence could not be moved."); } finally { setSaving(false); } }
  return <Inspector title="Move scheduled class" open={Boolean(event)} onClose={onClose}><div className="transfer-summary"><strong>{event.subject}</strong><span>{event.startTime}–{event.endTime}</span><small>{event.group} · {event.room}</small></div><form className="inspector-form" onSubmit={submit}><label><span>Original date</span><input name="original-date" required type="date" value={originalDate} onChange={(e) => setOriginalDate(e.target.value)} /></label><label><span>New date</span><input name="new-date" required type="date" value={newDate} onChange={(e) => setNewDate(e.target.value)} /></label><label><span>New time</span><select name="new-time" required value={timeSlot} onChange={(e) => setTimeSlot(e.target.value)}><option value="">Choose a time slot</option>{timeSlots.map((item) => <option value={item.id} key={item.id}>{timeSlotLabel(item)}</option>)}</select></label><EntityPicker label="New room (optional)" query={roomQuery} onQueryChange={setRoomQuery} options={filter(rooms, roomQuery)} selectedId={room?.id ?? null} onSelect={(item) => { setRoom(item); setRoomQuery(item.label); }} /><label><span>Reason</span><textarea name="reason" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Optional note" /></label>{error ? <div className="form-error">{error}</div> : null}<Button className="button-primary" type="submit" disabled={saving}>{saving ? "Moving…" : "Move class"}</Button></form></Inspector>;
}

function normalizePeriods(items: JsonApiResource[]) { return Object.fromEntries(items.map((item) => { const a = item.attributes ?? {}; return [String(a.period_kind ?? ""), { start: String(a.start_date ?? ""), end: String(a.end_date ?? "") }]; })); }
function toSemester(item: JsonApiResource): SemesterSummary | null { const a = item.attributes ?? {}; const number = Number(a.number), startYear = Number(a.start_year), endYear = Number(a.end_year); return Number.isFinite(number) && Number.isFinite(startYear) && Number.isFinite(endYear) ? { id: item.id, number, startYear, endYear } : null; }
function isSemester(value: SemesterSummary | null): value is SemesterSummary { return value !== null; }
function toOption(item: JsonApiResource, fallback: string): EntityOption { return { id: item.id, label: resourceLabel(item, fallback) }; }
function filter(options: EntityOption[], query: string) { const value = query.trim().toLowerCase(); return value ? options.filter((option) => option.label.toLowerCase().includes(value)).slice(0, 20) : options.slice(0, 20); }
function timeSlotLabel(item: JsonApiResource) { const a = item.attributes ?? {}; return `${String(a.call_number ?? "Class")} · ${String(a.start_time ?? "")}-${String(a.end_time ?? "")}`; }
