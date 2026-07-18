export type StreamGroupRef = { id: string; label: string };

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
  teachingAssignmentId: string;
  teacherOnDepartmentId: string;
  roomReferenceId: string;
  roomId: string;
  streamGroups: StreamGroupRef[];
};

export type WorkloadMatrixRow = Pick<WorkloadRow, "departmentCode" | "department" | "subject" | "group" | "groupSemesterId" | "students"> & { key: string; activities: Record<string, WorkloadRow> };

type AnyRecord = Record<string, unknown>;

export function normalizeWorkload(payload: unknown): WorkloadRow[] {
  const root = record(payload);
  const data = Array.isArray(root.data) ? root.data : [];
  const entries = data.map(record);
  const groupLabelById = buildGroupLabelIndex(entries);

  return entries.map((entry, index) => {
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
    const streamSiblingIds = Array.isArray(entry.stream_siblings) ? entry.stream_siblings.map(idText).filter(Boolean) : [];

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
      teachingAssignmentId: text(teacherAssignment.teaching_assignment_id, ""),
      teacherOnDepartmentId: text(teacherAssignment.teacher_on_kafedra_id, ""),
      roomReferenceId: text(reservation.id, ""),
      roomId: text(room.id, ""),
      streamGroups: streamSiblingIds.map((id) => ({ id, label: groupLabelById.get(id) ?? `Group semester #${id}` })),
    };
  });
}

/** Maps every group_semester_id present in this response to its readable "<Group>-<course>" label. */
function buildGroupLabelIndex(entries: AnyRecord[]): Map<string, string> {
  const index = new Map<string, string>();
  for (const entry of entries) {
    const stream = record(entry.stream);
    const groupSemester = record(stream.student_group_semester);
    const group = record(groupSemester.student_group);
    const speciality = record(group.speciality);
    const id = text(groupSemester.id, "");
    if (!id) continue;
    const course = number(groupSemester.course_number, number(stream.course, 1));
    const groupName = text(group.name, text(group.code, text(speciality.name, "Student group")));
    index.set(id, `${groupName}-${course}`);
  }
  return index;
}

/** For a grouped matrix row, returns the activities taught as a shared stream, with their sibling groups. */
export function streamedActivitiesForRow(row: WorkloadMatrixRow): { activity: string; groups: StreamGroupRef[] }[] {
  return Object.entries(row.activities)
    .filter(([, activityRow]) => activityRow.streamGroups.length > 0)
    .map(([activity, activityRow]) => ({ activity, groups: activityRow.streamGroups }));
}

export function groupWorkloadRows(rows: WorkloadRow[]): WorkloadMatrixRow[] {
  const grouped = new Map<string, WorkloadMatrixRow>();
  for (const row of rows) {
    const key = `${row.departmentCode}\u0000${row.subject}\u0000${row.groupSemesterId || row.group}`;
    const current = grouped.get(key) ?? { key, departmentCode: row.departmentCode, department: row.department, subject: row.subject, group: row.group, groupSemesterId: row.groupSemesterId, students: row.students, activities: {} };
    current.activities[row.activity] = row;
    grouped.set(key, current);
  }
  return [...grouped.values()];
}

function teacherName(value: AnyRecord) {
  const name = [value.last_name, value.first_name, value.middle_name].filter((part) => typeof part === "string" && part.trim()).join(" ");
  return name || "Not assigned";
}

function record(value: unknown): AnyRecord { return value && typeof value === "object" && !Array.isArray(value) ? value as AnyRecord : {}; }
function text(value: unknown, fallback: string) { return typeof value === "string" && value.trim() ? value : typeof value === "number" ? String(value) : fallback; }
function number(value: unknown, fallback: number) { const parsed = Number(value); return Number.isFinite(parsed) ? parsed : fallback; }
function idText(value: unknown) { return typeof value === "number" ? String(value) : typeof value === "string" ? value.trim() : ""; }
