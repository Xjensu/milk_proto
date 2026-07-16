export type SemesterSummary = {
  id: string;
  number: number;
  startYear: number;
  endYear: number;
};

export type GroupSummary = {
  id: string;
  code: string;
  course: number;
  students?: number;
};

export type TeacherSummary = {
  id: string;
  firstName: string;
  lastName: string;
  middleName?: string | null;
  post?: string | null;
};

export type RoomSummary = {
  id: string;
  number?: string | null;
  name?: string | null;
  building?: string | null;
};

export type CourseInput = {
  admissionYear: number;
  semesterStartYear: number;
};

export type WorkspaceSelection = {
  semester: SemesterSummary | null;
  facultyId: string | null;
  group: GroupSummary | null;
  teacher: TeacherSummary | null;
  room: RoomSummary | null;
  date: string;
};

export type ServiceStatus = {
  key: string;
  label: string;
  status: "operational" | "degraded" | "unavailable";
  latencyMs?: number;
};

export type ActiveUserSummary = {
  id: string;
  email: string;
  role: string;
  area: string;
  lastSeenAt: string;
};

export type AdminScheduleEvent = {
  id: string;
  dayOfWeek: number;
  startTime: string;
  endTime: string;
  subject: string;
  activityType: string;
  group: string;
  teacher: string;
  room: string;
};
