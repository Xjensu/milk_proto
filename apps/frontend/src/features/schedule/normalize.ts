import type { ScheduleDay, ScheduleEvent, ScheduleSource } from "./types";

const DAY_LABELS: Record<string, string> = { "1": "Monday", "2": "Tuesday", "3": "Wednesday", "4": "Thursday", "5": "Friday", "6": "Saturday", "7": "Sunday" };

export function normalizeSchedule(source: ScheduleSource): ScheduleDay[] {
  if (!source) return [];
  const data = source.data;
  if (Array.isArray(data)) return normalizeLiveRows(data);
  if (isRecord(data) && Array.isArray(data.days)) return normalizeLegacyDays(data.days);
  if (Array.isArray(source.days)) return normalizeLegacyDays(source.days);
  return [];
}

function normalizeLiveRows(rows: unknown[]): ScheduleDay[] {
  const days = new Map<string, ScheduleDay>();
  rows.forEach((row, index) => {
    if (!isRecord(row)) return;
    const attributes = isRecord(row.attributes) ? row.attributes : row;
    const key = stringValue(attributes.day_of_week, "0");
    const day = days.get(key) ?? { key, label: DAY_LABELS[key] ?? "Day", events: [] };
    day.events.push(normalizeEvent(attributes, stringValue(row.id, String(index))));
    days.set(key, day);
  });
  return [...days.values()].sort((left, right) => Number(left.key) - Number(right.key)).map(sortDayEvents);
}

function normalizeLegacyDays(rows: unknown[]): ScheduleDay[] {
  return rows.flatMap((row, dayIndex) => {
    if (!isRecord(row)) return [];
    const key = stringValue(row.date ?? row.day_of_week, String(dayIndex));
    const label = stringValue(row.date, DAY_LABELS[stringValue(row.day_of_week)] ?? "Day");
    const events = Array.isArray(row.events) ? row.events.flatMap((event, eventIndex) => isRecord(event) ? [normalizeEvent(event, `${key}-${eventIndex}`)] : []) : [];
    return [{ key, label, events }];
  }).map(sortDayEvents);
}

function normalizeEvent(event: Record<string, unknown>, id: string): ScheduleEvent {
  const timeSlot = recordValue(event.time_slot);
  const subject = recordValue(event.subject);
  const activityType = recordValue(event.activity_type);
  const room = recordValue(event.room);
  const studentGroup = recordValue(event.student_group);
  const teachers = Array.isArray(event.teachers) ? event.teachers.filter(isRecord) : [];
  const teacher = teachers[0] ?? recordValue(event.teacher);
  const teacherName = stringValue(teacher.full_name, [teacher.last_name, teacher.first_name, teacher.middle_name].filter(Boolean).join(" "));
  const groupName = stringValue(event.group_display_name ?? studentGroup.display_name ?? studentGroup.code);
  return {
    id,
    startTime: stringValue(event.time_slot_start_time ?? timeSlot.start_time, "—"),
    endTime: stringValue(event.time_slot_end_time ?? timeSlot.end_time),
    subject: stringValue(event.subject_name ?? subject.name ?? subject.short_name, "Untitled subject"),
    activityType: stringValue(event.activity_type_name ?? activityType.name ?? activityType.short_name, "Activity"),
    room: stringValue(event.room_name ?? room.number ?? room.name, "Not assigned"),
    counterpart: groupName || teacherName,
    group: groupName,
    teacher: teacherName,
    changed: Boolean(event.is_manual_change || event.is_transferred || event.status === "modified" || event.status === "transferred"),
    dayOfWeek: Number(event.day_of_week) || undefined,
    timeSlotId: stringValue(event.time_slot_id ?? timeSlot.id),
    roomId: stringValue(event.room_id ?? room.id),
    status: stringValue(event.status, "scheduled"),
  };
}

function sortDayEvents(day: ScheduleDay): ScheduleDay { return { ...day, events: [...day.events].sort((left, right) => left.startTime.localeCompare(right.startTime)) }; }
function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function recordValue(value: unknown): Record<string, unknown> { return isRecord(value) ? value : {}; }
function stringValue(value: unknown, fallback = ""): string { return typeof value === "string" || typeof value === "number" ? String(value) : fallback; }
