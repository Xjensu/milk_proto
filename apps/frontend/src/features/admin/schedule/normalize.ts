import type { AdminScheduleEvent } from "../domain";

export function normalizeScheduleEvents(source: unknown): AdminScheduleEvent[] {
  if (!isRecord(source) || !Array.isArray(source.data)) return [];
  return source.data.flatMap((item, index) => {
    if (!isRecord(item)) return [];
    const attributes = isRecord(item.attributes) ? item.attributes : item;
    const subject = recordValue(attributes.subject);
    const activity = recordValue(attributes.activity_type);
    const group = recordValue(attributes.student_group);
    const slot = recordValue(attributes.time_slot);
    const room = recordValue(attributes.room);
    const teachers = Array.isArray(attributes.teachers) ? attributes.teachers.filter(isRecord) : [];
    const teacher = teachers[0];
    return [{
      id: stringValue(item.id, String(index)),
      dayOfWeek: numberValue(attributes.day_of_week),
      startTime: stringValue(slot.start_time ?? attributes.time_slot_start_time, "—"),
      endTime: stringValue(slot.end_time ?? attributes.time_slot_end_time),
      subject: stringValue(subject.name ?? attributes.subject_name, "Untitled subject"),
      activityType: stringValue(activity.name ?? attributes.activity_type_name, "Activity"),
      group: group.code ? `${stringValue(group.code)}-${numberValue(group.course, 1)}` : stringValue(attributes.group_display_name, "Group not assigned"),
      teacher: teacher ? [teacher.last_name, teacher.first_name, teacher.middle_name].filter(Boolean).join(" ") : "Not assigned",
      room: attributes.room === null ? "Not assigned" : stringValue(room.number ?? room.name ?? attributes.room_name, "Not assigned"),
    }];
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function recordValue(value: unknown) { return isRecord(value) ? value : {}; }
function stringValue(value: unknown, fallback = "") { return typeof value === "string" || typeof value === "number" ? String(value) : fallback; }
function numberValue(value: unknown, fallback = 0) { const number = Number(value); return Number.isFinite(number) ? number : fallback; }
