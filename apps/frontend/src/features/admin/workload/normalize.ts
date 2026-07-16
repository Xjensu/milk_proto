export type WorkloadRow = {
  id: string;
  departmentCode: string;
  department: string;
  subject: string;
  group: string;
  groupSemesterId: string;
  students: number;
  activity: string;
  plannedHours: number;
  actualHours: number;
  teacher: string;
  room: string;
};

type AnyRecord = Record<string, unknown>;

export function normalizeWorkload(payload: unknown): WorkloadRow[] {
  const root = record(payload);
  const data = Array.isArray(root.data) ? root.data : [];
  return data.map((value, index) => {
    const entry = record(value);
    const stream = record(entry.stream);
    const subject = record(stream.subject);
    const department = record(stream.kafedra);
    const groupSemester = record(stream.student_group_semester);
    const group = record(groupSemester.student_group);
    const speciality = record(group.speciality);
    const activity = record(entry.activity_type);
    const teacherAssignment = Array.isArray(entry.teachers) ? record(entry.teachers[0]) : {};
    const teacher = record(teacherAssignment.teacher);
    const reservation = Array.isArray(entry.reservations) ? record(entry.reservations[0]) : {};
    const room = record(reservation.room);
    const course = number(groupSemester.course_number, number(stream.course, 1));
    const groupName = text(group.name, text(group.code, text(speciality.name, "Student group")));

    return {
      id: text(entry.id, String(index)),
      departmentCode: text(department.short_name, text(department.name, "Department")),
      department: text(department.name, "Department"),
      subject: text(subject.name, "Unnamed discipline"),
      group: `${groupName}-${course}`,
      groupSemesterId: text(groupSemester.id, ""),
      students: number(groupSemester.students_now, 0),
      activity: text(activity.name, text(activity.short_name, "Activity")),
      plannedHours: number(entry.hours_assigned, number(entry.hours_per_event, 0)),
      actualHours: number(entry.actual_hours, 0),
      teacher: teacherName(teacher),
      room: text(room.name, text(room.number, "Not assigned")),
    };
  });
}

function teacherName(value: AnyRecord) {
  const name = [value.last_name, value.first_name, value.middle_name].filter((part) => typeof part === "string" && part.trim()).join(" ");
  return name || "Not assigned";
}

function record(value: unknown): AnyRecord { return value && typeof value === "object" && !Array.isArray(value) ? value as AnyRecord : {}; }
function text(value: unknown, fallback: string) { return typeof value === "string" && value.trim() ? value : typeof value === "number" ? String(value) : fallback; }
function number(value: unknown, fallback: number) { const parsed = Number(value); return Number.isFinite(parsed) ? parsed : fallback; }
