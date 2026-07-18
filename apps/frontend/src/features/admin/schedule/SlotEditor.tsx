import { useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Sparkles, Wand2 } from "lucide-react";

import { Button } from "../../../components/ui";
import { api, type JsonApiResource } from "../../../lib/api";
import { Inspector } from "../workspace/Inspector";
import type { EntityOption } from "../workspace/EntityPicker";
import type { BuilderSlot, BuilderWeekCode } from "./BuilderGrid";
import type { LoadEntry } from "./loadPanel";

export type SlotContext = { groupSemesterId: string; groupLabel: string; dayOfWeek: number; timeSlotId: string };
export type WeekTypeOption = { id: number; code: BuilderWeekCode; name: string };

const DAY_NAMES = ["", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

// The editor for one grid cell. Two collapsible sections:
//  - Manual: drop (or pick) a load requirement, choose week type and subgroup;
//    teacher and room lock to the requirement's assignment/reservation when present.
//  - Recommendations: engine placements for this exact cell, applied as-is.
// Both paths POST /schedule_events/from_recommendation so stream siblings are
// mirrored transactionally and an occupied slot is rejected with 409.
export function SlotEditor({ context, assignedEntry, onAssignEntry, loadEntries, slots, weekTypes, rooms, resolveSiblings, onClose, onCreated }: {
  context: SlotContext | null;
  assignedEntry: LoadEntry | null;
  onAssignEntry: (entry: LoadEntry | null) => void;
  loadEntries: LoadEntry[];
  slots: BuilderSlot[];
  weekTypes: WeekTypeOption[];
  rooms: EntityOption[];
  resolveSiblings: (entry: LoadEntry) => Promise<{ gsids: number[]; requirementIds: number[] }>;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [manualOpen, setManualOpen] = useState(true);
  const [recsOpen, setRecsOpen] = useState(false);
  const [weekTypeId, setWeekTypeId] = useState<number | null>(null);
  const [subgroup, setSubgroup] = useState(0);
  const [roomId, setRoomId] = useState("");
  const [teacherTokId, setTeacherTokId] = useState("");
  const [fallbackTeachers, setFallbackTeachers] = useState<EntityOption[]>([]);
  const [recommendations, setRecommendations] = useState<RecommendationEntry[]>([]);
  const [recsLoading, setRecsLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const slot = slots.find((item) => item.id === context?.timeSlotId);
  const bothType = useMemo(() => weekTypes.find((item) => item.code === "both") ?? null, [weekTypes]);

  useEffect(() => {
    if (!context) return;
    setError("");
    setSubgroup(0);
    setWeekTypeId(bothType?.id ?? null);
    setRecommendations([]);
    setRecsOpen(false);
    setManualOpen(true);
  }, [context, bothType]);

  useEffect(() => {
    setRoomId(assignedEntry?.reservedRoom ? String(assignedEntry.reservedRoom.id) : "");
    setTeacherTokId(assignedEntry?.teachers[0] ? String(assignedEntry.teachers[0].teacherOnKafedraId) : "");
    setFallbackTeachers([]);
    if (assignedEntry && assignedEntry.teachers.length === 0) void loadKafedraTeachers(assignedEntry.kafedraId, setFallbackTeachers);
  }, [assignedEntry]);

  useEffect(() => {
    if (!recsOpen || !context || recommendations.length) return;
    setRecsLoading(true);
    api.scheduleRecommendations(context.groupSemesterId, context.dayOfWeek, Number(context.timeSlotId))
      .then((response) => setRecommendations(normalizeRecommendations(response)))
      .catch(() => setError("Recommendations could not be loaded for this slot."))
      .finally(() => setRecsLoading(false));
  }, [recsOpen, context, recommendations.length]);

  if (!context) return null;

  const teacherLocked = Boolean(assignedEntry && assignedEntry.teachers.length > 0);
  const roomLocked = Boolean(assignedEntry?.reservedRoom);

  async function submitManual(event: React.FormEvent) {
    event.preventDefault();
    if (!context || !assignedEntry || !weekTypeId) return;
    const weekType = weekTypes.find((item) => item.id === weekTypeId);
    if (!weekType) return;
    setSaving(true);
    setError("");
    try {
      const siblings = assignedEntry.streamSiblingGsids.length ? await resolveSiblings(assignedEntry) : { gsids: [], requirementIds: [] };
      await api.createEventFromRecommendation({
        score: 0,
        day_of_week: context.dayOfWeek,
        time_slot_id: Number(context.timeSlotId),
        call_number: slot?.callNumber ?? 0,
        week_type_id: weekType.id,
        week_type_code: weekType.code,
        room_id: Number(roomId),
        teacher_on_kafedra_ids: teacherTokId ? [Number(teacherTokId)] : [],
        group_semester_ids: [Number(context.groupSemesterId), ...siblings.gsids],
        load_requirement_ids: [assignedEntry.id, ...siblings.requirementIds],
        is_day_edge: false,
        week_type_recommendation: weekType.code,
        subgroup_number: subgroup,
      });
      onCreated();
    } catch (caught) {
      setError(readableCreateError(caught));
    } finally {
      setSaving(false);
    }
  }

  async function applyPlacement(placement: Record<string, unknown>) {
    setSaving(true);
    setError("");
    try {
      await api.createEventFromRecommendation(placement);
      onCreated();
    } catch (caught) {
      setError(readableCreateError(caught));
    } finally {
      setSaving(false);
    }
  }

  return <Inspector title="Plan a class" open onClose={onClose}>
    <div className="slot-editor-context">
      <strong>{context.groupLabel}</strong>
      <span>{DAY_NAMES[context.dayOfWeek]}{slot ? ` · ${slot.label}` : ""}</span>
    </div>

    <section className="editor-section">
      <button type="button" className="editor-section-toggle" aria-expanded={manualOpen} onClick={() => setManualOpen((open) => !open)}>
        {manualOpen ? <ChevronDown size={15} aria-hidden="true" /> : <ChevronRight size={15} aria-hidden="true" />}<Wand2 size={15} aria-hidden="true" /> Manual planning
      </button>
      {manualOpen ? <form className="inspector-form" onSubmit={submitManual}>
        <div
          className={assignedEntry ? "load-drop-zone has-entry" : "load-drop-zone"}
          onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = "copy"; }}
          onDrop={(event) => {
            event.preventDefault();
            const id = Number(event.dataTransfer.getData("application/x-load-entry"));
            const entry = loadEntries.find((item) => item.id === id && item.groupSemesterId === context.groupSemesterId);
            if (entry) onAssignEntry(entry);
          }}
        >
          {assignedEntry
            ? <><strong>{assignedEntry.subjectName}</strong><span>{assignedEntry.activityName} · {assignedEntry.actualHours}/{assignedEntry.controlSum} h{assignedEntry.streamSiblingGsids.length ? ` · stream of ${assignedEntry.streamSiblingGsids.length + 1} groups` : ""}</span><button type="button" onClick={() => onAssignEntry(null)}>Clear</button></>
            : <span>Drag a load row here, or press "Use" on a load row of this group.</span>}
        </div>
        <label><span>Week type</span><select required value={weekTypeId ?? ""} onChange={(event) => setWeekTypeId(Number(event.target.value))}>{weekTypes.map((item) => <option value={item.id} key={item.id}>{item.code === "both" ? "Every week" : item.code === "odd" ? "Odd weeks (above the line)" : "Even weeks (below the line)"}</option>)}</select></label>
        <label><span>Subgroup</span><select value={subgroup} onChange={(event) => setSubgroup(Number(event.target.value))}><option value={0}>Whole group</option><option value={1}>Subgroup 1</option><option value={2}>Subgroup 2</option></select></label>
        <label><span>Teacher{teacherLocked ? " (from load distribution)" : ""}</span>
          {teacherLocked
            ? <select value={teacherTokId} onChange={(event) => setTeacherTokId(event.target.value)} disabled={assignedEntry!.teachers.length === 1}>{assignedEntry!.teachers.map((teacher) => <option value={teacher.teacherOnKafedraId} key={teacher.teacherOnKafedraId}>{teacher.name}</option>)}</select>
            : <select value={teacherTokId} onChange={(event) => setTeacherTokId(event.target.value)} disabled={!assignedEntry}><option value="">No teacher</option>{fallbackTeachers.map((teacher) => <option value={teacher.id} key={teacher.id}>{teacher.label}</option>)}</select>}
        </label>
        <label><span>Room{roomLocked ? " (reserved)" : ""}</span>
          <select required value={roomId} onChange={(event) => setRoomId(event.target.value)} disabled={roomLocked || !assignedEntry}>
            {roomLocked ? <option value={String(assignedEntry!.reservedRoom!.id)}>{assignedEntry!.reservedRoom!.name}</option> : <><option value="">Choose a room</option>{rooms.map((room) => <option value={room.id} key={room.id}>{room.label}</option>)}</>}
          </select>
        </label>
        {error ? <div className="form-error" role="alert">{error}</div> : null}
        <Button className="button-primary" type="submit" disabled={saving || !assignedEntry || !roomId}>{saving ? "Creating…" : "Create class"}</Button>
      </form> : null}
    </section>

    <section className="editor-section">
      <button type="button" className="editor-section-toggle" aria-expanded={recsOpen} onClick={() => setRecsOpen((open) => !open)}>
        {recsOpen ? <ChevronDown size={15} aria-hidden="true" /> : <ChevronRight size={15} aria-hidden="true" />}<Sparkles size={15} aria-hidden="true" /> Recommendations
      </button>
      {recsOpen ? <div className="recommendation-list">
        {recsLoading ? <p className="inspector-help">Looking for placements…</p> : null}
        {!recsLoading && recommendations.length === 0 ? <p className="inspector-help">No recommendations for this slot.</p> : null}
        {recommendations.map((entry) => <article key={entry.loadRequirementId} className="recommendation-entry">
          <strong>{entry.subject}</strong><span>{entry.activity} · {entry.actualHours}/{entry.controlSum} h</span>
          {entry.placements.map((placement, index) => <div className="recommendation-placement" key={index}>
            <span>{placementLabel(placement)}</span>
            <Button className="button-small" disabled={saving} onClick={() => applyPlacement(placement)}>Apply</Button>
          </div>)}
        </article>)}
        {error && recsOpen && !manualOpen ? <div className="form-error" role="alert">{error}</div> : null}
      </div> : null}
    </section>
  </Inspector>;
}

type RecommendationEntry = {
  loadRequirementId: number;
  subject: string;
  activity: string;
  actualHours: number;
  controlSum: number;
  placements: Array<Record<string, unknown>>;
};

function normalizeRecommendations(response: { data: unknown[] }): RecommendationEntry[] {
  return (response.data ?? []).flatMap((raw) => {
    if (!raw || typeof raw !== "object") return [];
    const entry = raw as Record<string, unknown>;
    const subject = (entry.subject ?? {}) as Record<string, unknown>;
    const activity = (entry.activity_type ?? {}) as Record<string, unknown>;
    return [{
      loadRequirementId: Number(entry.load_requirement_id),
      subject: String(subject.short_name ?? subject.name ?? "Class"),
      activity: String(activity.name ?? "Activity"),
      actualHours: Number(entry.actual_hours ?? 0),
      controlSum: Number(entry.control_sum ?? 0),
      placements: Array.isArray(entry.placements) ? entry.placements.filter((p): p is Record<string, unknown> => Boolean(p) && typeof p === "object") : [],
    }];
  });
}

function placementLabel(placement: Record<string, unknown>) {
  const week = String(placement.week_type_code ?? "both");
  const subgroupNumber = Number(placement.subgroup_number ?? 0);
  const parts = [
    String(placement.room_name ?? `Room #${placement.room_id}`),
    week === "both" ? "every week" : `${week} weeks`,
    subgroupNumber ? `subgroup ${subgroupNumber}` : "whole group",
    `score ${Number(placement.score ?? 0).toFixed(2)}`,
  ];
  return parts.join(" · ");
}

async function loadKafedraTeachers(kafedraId: number, setOptions: (options: EntityOption[]) => void) {
  try {
    const [tokResponse, teacherResponse] = await Promise.all([
      api.listAdminCollection("teacher_on_kafedras", "?page=1&per_page=500"),
      api.listAdminCollection("teachers", "?page=1&per_page=500"),
    ]);
    const names = new Map(teacherResponse.data.map((teacher) => [teacher.id, teacherName(teacher)]));
    setOptions(tokResponse.data
      .filter((tok) => relationId(tok, "kafedra") === String(kafedraId))
      .map((tok) => ({ id: tok.id, label: names.get(relationId(tok, "teacher")) ?? `Teacher link #${tok.id}` })));
  } catch {
    setOptions([]);
  }
}

function teacherName(teacher: JsonApiResource) {
  const attrs = teacher.attributes ?? {};
  return [attrs.last_name, attrs.first_name, attrs.middle_name].filter(Boolean).join(" ") || `Teacher #${teacher.id}`;
}

function relationId(resource: JsonApiResource, name: string) {
  const relation = resource.relationships?.[name] as { data?: { id?: string } } | undefined;
  return String(relation?.data?.id ?? "");
}

function readableCreateError(caught: unknown) {
  const message = caught instanceof Error ? caught.message : "";
  if (message.includes("409") || message.toLowerCase().includes("slot_taken") || message.toLowerCase().includes("taken")) {
    return "This slot is already taken for the group or one of its stream groups. The class was not created.";
  }
  return message || "The class could not be created for this slot.";
}
