import type {
  CourseInput,
  GroupSummary,
  RoomSummary,
  SemesterSummary,
  TeacherSummary,
} from "./domain";

export function formatSemester(value: SemesterSummary) {
  return `${value.number} semester ${value.startYear}/${value.endYear}`;
}

export function calculateCourse({ admissionYear, semesterStartYear }: CourseInput) {
  return Math.max(1, semesterStartYear - admissionYear + 1);
}

export function formatGroup(value: GroupSummary) {
  return `${value.code}-${value.course}`;
}

export function formatTeacher(value: TeacherSummary | null | undefined) {
  if (!value) return "Not assigned";
  return [value.lastName, value.firstName, value.middleName].filter(Boolean).join(" ");
}

export function formatRoom(value: RoomSummary | null | undefined) {
  if (!value) return "Not assigned";
  const identity = value.number ? `Room ${value.number}` : value.name || "Room";
  return value.building ? `${identity} · ${value.building}` : identity;
}
