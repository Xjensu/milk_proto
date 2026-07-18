// Normalizes /admin/student_group_semesters/:id/load entries for the schedule
// builder: one row per study_load_requirement (no merging by activity type),
// with assigned teachers, reserved room, hour counters, and stream siblings.

export type LoadTeacher = { teacherOnKafedraId: number; name: string };

export type LoadEntry = {
  id: number;
  groupSemesterId: string;
  subjectName: string;
  subjectShort: string;
  subjectId: number;
  activityName: string;
  activityShort: string;
  activityTypeId: number;
  kafedraId: number;
  hoursPerEvent: number;
  hoursAssigned: number;
  actualHours: number;
  controlSum: number;
  weekBreakdown: { both: number; even: number; odd: number };
  teachers: LoadTeacher[];
  reservedRoom: { id: number; name: string } | null;
  streamSiblingGsids: number[];
};

export function normalizeLoadEntries(source: { data?: unknown[] } | null, groupSemesterId: string): LoadEntry[] {
  if (!source || !Array.isArray(source.data)) return [];
  return source.data.flatMap((raw) => {
    if (!isRecord(raw)) return [];
    const stream = record(raw.stream);
    const subject = record(stream.subject);
    const activity = record(raw.activity_type);
    const kafedra = record(stream.kafedra);
    const week = record(raw.week_breakdown);
    const teachers = Array.isArray(raw.teachers) ? raw.teachers.filter(isRecord).map(toTeacher) : [];
    const reservations = Array.isArray(raw.reservations) ? raw.reservations.filter(isRecord) : [];
    const reservedRoom = reservations.length ? record(reservations[0].room) : null;
    return [{
      id: Number(raw.id),
      groupSemesterId,
      subjectName: str(subject.name, "Untitled subject"),
      subjectShort: str(subject.short_name, str(subject.name, "Subject")),
      subjectId: Number(subject.id ?? 0),
      activityName: str(activity.name, "Activity"),
      activityShort: str(activity.short_name, str(activity.name, "Activity")),
      activityTypeId: Number(activity.id ?? 0),
      kafedraId: Number(kafedra.id ?? 0),
      hoursPerEvent: Number(raw.hours_per_event ?? 2),
      hoursAssigned: Number(raw.hours_assigned ?? 0),
      actualHours: Number(raw.actual_hours ?? 0),
      controlSum: Number(raw.control_sum ?? 0),
      weekBreakdown: { both: Number(week.both ?? 0), even: Number(week.even ?? 0), odd: Number(week.odd ?? 0) },
      teachers,
      reservedRoom: reservedRoom && reservedRoom.id !== undefined ? { id: Number(reservedRoom.id), name: str(reservedRoom.name, `Room #${reservedRoom.id}`) } : null,
      streamSiblingGsids: Array.isArray(raw.stream_siblings) ? raw.stream_siblings.map(Number).filter(Number.isFinite) : [],
    }];
  });
}

// Finds the sibling group's requirement matching the source requirement by the
// stream tuple the backend uses: kafedra + subject + activity type + hours_per_event.
export function matchSiblingRequirement(entries: LoadEntry[], source: LoadEntry): LoadEntry | undefined {
  return entries.find((entry) =>
    entry.kafedraId === source.kafedraId
    && entry.subjectId === source.subjectId
    && entry.activityTypeId === source.activityTypeId
    && entry.hoursPerEvent === source.hoursPerEvent);
}

function toTeacher(raw: Record<string, unknown>): LoadTeacher {
  const teacher = record(raw.teacher);
  const name = [teacher.last_name, teacher.first_name, teacher.middle_name].filter(Boolean).join(" ");
  return { teacherOnKafedraId: Number(raw.teacher_on_kafedra_id ?? 0), name: name || `Teacher #${teacher.id}` };
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function record(value: unknown): Record<string, unknown> { return isRecord(value) ? value : {}; }
function str(value: unknown, fallback = ""): string { return typeof value === "string" || typeof value === "number" ? String(value) : fallback; }
