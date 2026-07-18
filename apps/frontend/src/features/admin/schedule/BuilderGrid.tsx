import { Plus } from "lucide-react";

export type BuilderWeekCode = "both" | "odd" | "even";

export type BuilderEvent = {
  id: string;
  dayOfWeek: number;
  timeSlotId: string;
  weekCode: BuilderWeekCode;
  subgroup: number; // 0 = whole group, 1/2 = split subgroup
  subject: string;
  activity: string;
  room: string;
  teachers: string;
  status: string;
};

export type BuilderSlot = { id: string; label: string; startTime: string; callNumber: number };

const DAYS: Array<{ day: number; label: string }> = [
  { day: 1, label: "Mon" }, { day: 2, label: "Tue" }, { day: 3, label: "Wed" },
  { day: 4, label: "Thu" }, { day: 5, label: "Fri" }, { day: 6, label: "Sat" },
];

// One grid per student group: rows are time slots, columns are weekdays.
// A cell is a 2×2 area — columns split subgroup 1/2, rows split odd/even weeks —
// so a whole-group both-weeks event spans the full cell, while split events
// occupy their half above/below the parity divider or left/right of the
// subgroup divider.
export function BuilderGrid({ groupLabel, slots, events, onCellClick }: {
  groupLabel: string;
  slots: BuilderSlot[];
  events: BuilderEvent[];
  onCellClick: (dayOfWeek: number, timeSlotId: string) => void;
}) {
  return <section className="builder-grid-block" aria-label={`Schedule grid for ${groupLabel}`}>
    <header className="builder-grid-title"><strong>{groupLabel}</strong></header>
    <div className="builder-grid-scroll">
      <table className="builder-grid">
        <thead><tr><th scope="col">Time</th>{DAYS.map((day) => <th scope="col" key={day.day}>{day.label}</th>)}</tr></thead>
        <tbody>
          {slots.map((slot) => <tr key={slot.id}>
            <th scope="row">{slot.label}</th>
            {DAYS.map(({ day }) => {
              const cellEvents = events.filter((event) => event.dayOfWeek === day && event.timeSlotId === slot.id);
              return <td key={day} className={cellEvents.length ? "builder-cell filled" : "builder-cell"}>
                <div className="builder-cell-area">
                  {cellEvents.map((event) => <article
                    key={event.id}
                    className={`builder-event week-${event.weekCode} subgroup-${event.subgroup} ${event.status === "cancelled" ? "is-cancelled" : ""}`}
                  >
                    <strong>{event.subject}</strong>
                    <span>{event.activity}{event.subgroup ? ` · SG${event.subgroup}` : ""}{event.weekCode !== "both" ? ` · ${event.weekCode}` : ""}</span>
                    <small>{event.room}{event.teachers ? ` · ${event.teachers}` : ""}</small>
                  </article>)}
                </div>
                <button type="button" className="builder-cell-add" aria-label={`Add class: ${groupLabel}, ${DAYS.find((d) => d.day === day)?.label} ${slot.label}`} onClick={() => onCellClick(day, slot.id)}><Plus size={13} aria-hidden="true" /></button>
              </td>;
            })}
          </tr>)}
        </tbody>
      </table>
    </div>
  </section>;
}

export function normalizeBuilderEvents(source: Record<string, unknown> | null, weekCodeById: Map<string, string>): BuilderEvent[] {
  if (!source || !Array.isArray(source.data)) return [];
  return source.data.flatMap((raw) => {
    if (!raw || typeof raw !== "object") return [];
    const row = raw as Record<string, unknown>;
    const attrs = (row.attributes && typeof row.attributes === "object" ? row.attributes : row) as Record<string, unknown>;
    const teachers = Array.isArray(attrs.teachers) ? attrs.teachers as Array<Record<string, unknown>> : [];
    const code = weekCodeById.get(String(attrs.week_type_id)) ?? "both";
    return [{
      id: String(row.id ?? attrs.id ?? Math.random()),
      dayOfWeek: Number(attrs.day_of_week) || 0,
      timeSlotId: String(attrs.time_slot_id ?? ""),
      weekCode: (code === "odd" || code === "even" ? code : "both") as BuilderWeekCode,
      subgroup: Number(attrs.subgroup_number) || 0,
      subject: text(attrs.subject_short_name) || text(attrs.subject_name) || "Class",
      activity: text(attrs.activity_type_name) || "Activity",
      room: text(attrs.room_name) || text(attrs.room_number) || "No room",
      teachers: teachers.map((teacher) => [teacher.last_name, initial(teacher.first_name)].filter(Boolean).join(" ")).join(", "),
      status: text(attrs.status) || "scheduled",
    }];
  });
}

function text(value: unknown): string { return typeof value === "string" || typeof value === "number" ? String(value) : ""; }
function initial(value: unknown): string { const name = text(value); return name ? `${name[0]}.` : ""; }
