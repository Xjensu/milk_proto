import { describe, expect, it } from "vitest";

import { groupWorkloadRows, normalizeWorkload, streamedActivitiesForRow } from "./normalize";

describe("normalizeWorkload", () => {
  it("turns enriched load entries into readable matrix rows", () => {
    const rows = normalizeWorkload({ data: [{ id: 41, hours_assigned: 36, actual_hours: 18, activity_type: { id: 2, name: "Laboratory work", short_name: "Lab" }, stream: { subject: { id: 9, name: "Distributed systems" }, kafedra: { id: 3, name: "Computer Science", short_name: "CS" }, student_group_semester: { id: 17, students_now: 24, course_number: 3, student_group: { id: 8, name: "CS-21", year_of_admission: 2023, speciality: { name: "Computer Science" } } } }, teachers: [{ teacher: { id: 6, last_name: "Carter", first_name: "Emily" } }], reservations: [{ room: { id: 5, name: "B-204" } }] }] });

    expect(rows).toEqual([expect.objectContaining({ id: "41", departmentCode: "CS", subject: "Distributed systems", group: "CS-21-3", students: 24, activity: "Laboratory work", plannedHours: 36, actualHours: 18, teacher: "Carter Emily", room: "B-204", teachingAssignmentId: "", roomReferenceId: "" })]);
  });

  it("keeps nullable assignments readable and never emits an identifier label", () => {
    const [row] = normalizeWorkload({ data: [{ id: 1, activity_type: { name: "Exam" }, stream: { subject: { name: "Physics" }, student_group_semester: { course_number: 1, student_group: { speciality: { name: "Physics" } } } }, teachers: [], reservations: [] }] });
    expect(row.group).toBe("Physics-1");
    expect(row.teacher).toBe("Not assigned");
    expect(row.room).toBe("Not assigned");
  });

  it("combines activity requirements into one discipline and group row", () => {
    const base = { id: "1", departmentCode: "CS", department: "Computer Science", subject: "Networks", group: "CS-3", groupSemesterId: "17", students: 20, activity: "Lecture", plannedHours: 18, actualHours: 0, teacher: "Not assigned", room: "Not assigned", teachingAssignmentId: "", teacherOnDepartmentId: "", roomReferenceId: "", roomId: "", streamGroups: [] };
    const [row] = groupWorkloadRows([base, { ...base, id: "2", activity: "Laboratory work", plannedHours: 36 }]);
    expect(row.activities.Lecture.plannedHours).toBe(18);
    expect(row.activities["Laboratory work"].plannedHours).toBe(36);
  });

  it("resolves stream_siblings into readable group labels using other entries in the same response", () => {
    const rows = normalizeWorkload({
      data: [
        { id: 41, activity_type: { name: "Lecture" }, stream: { subject: { name: "Networks" }, kafedra: { short_name: "CS" }, student_group_semester: { id: 17, course_number: 3, student_group: { name: "CS-21" } } }, teachers: [], reservations: [], stream_siblings: [18, 99] },
        { id: 42, activity_type: { name: "Lecture" }, stream: { subject: { name: "Networks" }, kafedra: { short_name: "CS" }, student_group_semester: { id: 18, course_number: 3, student_group: { name: "CS-22" } } }, teachers: [], reservations: [], stream_siblings: [17] },
      ],
    });

    expect(rows[0].streamGroups).toEqual([{ id: "18", label: "CS-22-3" }, { id: "99", label: "Group semester #99" }]);
    expect(rows[1].streamGroups).toEqual([{ id: "17", label: "CS-21-3" }]);
  });

  it("leaves streamGroups empty when a requirement has no stream_siblings", () => {
    const [row] = normalizeWorkload({ data: [{ id: 1, activity_type: { name: "Exam" }, stream: { subject: { name: "Physics" }, student_group_semester: { course_number: 1, student_group: { speciality: { name: "Physics" } } } }, teachers: [], reservations: [] }] });
    expect(row.streamGroups).toEqual([]);
  });

  it("streamedActivitiesForRow surfaces only the activities within a matrix row that are streamed", () => {
    const base = { id: "1", departmentCode: "CS", department: "Computer Science", subject: "Networks", group: "CS-21-3", groupSemesterId: "17", students: 20, teacher: "Not assigned", room: "Not assigned", teachingAssignmentId: "", teacherOnDepartmentId: "", roomReferenceId: "", roomId: "", actualHours: 0, plannedHours: 18 };
    const [row] = groupWorkloadRows([
      { ...base, id: "1", activity: "Lecture", streamGroups: [{ id: "18", label: "CS-22-3" }] },
      { ...base, id: "2", activity: "Laboratory work", plannedHours: 36, streamGroups: [] },
    ]);

    expect(streamedActivitiesForRow(row)).toEqual([{ activity: "Lecture", groups: [{ id: "18", label: "CS-22-3" }] }]);
  });
});
