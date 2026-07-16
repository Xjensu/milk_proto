export type AccountRole = "super_admin" | "faculty_admin" | "kafedra_admin" | "scheduler" | "teacher" | "student" | "unknown";

export type Account = {
  id: string;
  email: string;
  role: AccountRole;
  status: "active" | "disabled";
  must_change_password?: boolean;
};

type LoginResponse = {
  access_token: string;
  refresh_token: string;
};

export type JsonApiResource = {
  id: string;
  type: string;
  attributes?: Record<string, unknown>;
  relationships?: Record<string, unknown>;
  links?: Record<string, unknown>;
};

export type JsonApiCollection = {
  data: JsonApiResource[];
  included?: JsonApiResource[];
  meta?: Record<string, unknown>;
  links?: Record<string, unknown>;
};

export type ScheduleQuery = { semesterId?: string; date?: string };
export type GroupPeriodInput = { period_kind: "lecture_week" | "credit_week" | "exam_session"; start_date: string; end_date: string };
export type EventTransferInput = { original_date: string; transfer_type: "moved" | "cancelled"; new_date?: string; new_time_slot_id?: number; new_room_id?: number; reason?: string };
export type SpecialityInput = { faculty_id: number; name: string; duration_of_study: number };
export type StudentGroupInput = { speciality_id: number; year_of_admission: string };
export type RoomReservationInput = { room_id: number; teacher_id: number; reserved_date: string; time_slot_id: number; purpose?: string };

export type DashboardService = {
  key: string;
  label: string;
  status: "operational" | "degraded" | "unavailable";
  latency_ms?: number;
};

export type ActiveUser = {
  account_id: string;
  email: string;
  role: AccountRole;
  area: string;
  last_seen_at: string;
};

export type AdminCollection = "faculties" | "semesters" | "teachers" | "rooms" | "kafedras" | "student_groups" | "subjects" | "activity_types" | "time_slots" | "teacher_on_kafedras" | "holidays" | "weekday_transfers" | "room_reservations" | "media_files" | "audit_logs";

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(/\/$/, "") ?? "/api";

async function request<T>(path: string, init: RequestInit = {}, allowRefresh = true): Promise<T> {
  const accessToken = sessionStorage.getItem("milk_schedule_access_token");
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
      ...(path.startsWith("/admin/") ? { Accept: "application/vnd.api+json" } : {}),
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
      ...(init.headers ?? {}),
    },
  });

  if (!response.ok && response.status === 401 && allowRefresh && !path.endsWith("/auth/sessions/refresh")) {
    const refreshToken = sessionStorage.getItem("milk_schedule_refresh_token");
    if (refreshToken) {
      const refreshResponse = await fetch(`${API_BASE_URL}/auth/sessions/refresh`, { method: "POST", headers: { Accept: "application/json", "Content-Type": "application/json" }, body: JSON.stringify({ refresh_token: refreshToken }) });
      if (refreshResponse.ok) {
        const refreshed = await refreshResponse.json() as LoginResponse;
        sessionStorage.setItem("milk_schedule_access_token", refreshed.access_token);
        sessionStorage.setItem("milk_schedule_refresh_token", refreshed.refresh_token);
        return request<T>(path, init, false);
      }
    }
  }

  if (!response.ok) {
    let message = `Request failed with status ${response.status}`;
    try {
      const body = (await response.json()) as { error?: string; message?: string };
      message = body.error ?? body.message ?? message;
    } catch {
      // Keep the HTTP status when the API has no JSON error body.
    }
    throw new Error(message);
  }

  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export const api = {
  login(email: string, password: string) {
    return request<LoginResponse>("/auth/sessions", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    });
  },
  changePassword(currentPassword: string, newPassword: string) {
    return request<LoginResponse>("/auth/password", { method: "POST", body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }) });
  },
  listFaculties() {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>("/public/faculties");
  },
  listSemesters() {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>("/public/semesters");
  },
  listPublicGroups(facultyId: string, semesterId: string) {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>(`/public/faculties/${encodeURIComponent(facultyId)}/groups?semester_id=${encodeURIComponent(semesterId)}`);
  },
  listPublicTeachers(query = "") {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>(`/public/teachers${query ? `?q=${encodeURIComponent(query)}` : ""}`);
  },
  listPublicRooms(query = "") {
    return request<{ data: Array<{ id: string; attributes?: Record<string, unknown> }> }>(`/public/rooms${query ? `?q=${encodeURIComponent(query)}` : ""}`);
  },
  groupSchedule(groupSemesterId: string) {
    return request<Record<string, unknown>>(`/public/groups/${encodeURIComponent(groupSemesterId)}/schedule`);
  },
  teacherSchedule(teacherId: string, semesterId: string) {
    return request<Record<string, unknown>>(`/public/teachers/${encodeURIComponent(teacherId)}/schedule?semester_id=${encodeURIComponent(semesterId)}`);
  },
  roomSchedule(roomId: string, semesterId = "") {
    const query = semesterId ? `?semester_id=${encodeURIComponent(semesterId)}` : "";
    return request<Record<string, unknown>>(`/public/rooms/${encodeURIComponent(roomId)}/schedule${query}`);
  },
  me() {
    return request<Record<string, unknown>>("/admin/me");
  },
  meSchedule(semesterId = "") {
    const query = semesterId ? `?semester_id=${encodeURIComponent(semesterId)}` : "";
    return request<Record<string, unknown>>(`/admin/me/schedule${query}`);
  },
  meLoad(semesterId = "") {
    const query = semesterId ? `?semester_id=${encodeURIComponent(semesterId)}` : "";
    return request<Record<string, unknown>>(`/admin/me/load${query}`);
  },
  meKafedras() {
    return request<Record<string, unknown>>("/admin/me/kafedras");
  },
  listAdmin(resource: "faculties" | "semesters" | "teachers" | "rooms") {
    return request<{ data: Array<{ id: string; type: string; attributes: Record<string, unknown> }> }>(`/admin/${resource}`);
  },
  listAdminCollection(resource: AdminCollection, query = "") {
    return request<JsonApiCollection>(`/admin/${resource}${query}`);
  },
  listFacultyGroups(facultyId: string, semesterId: string, signal?: AbortSignal) {
    return request<JsonApiCollection>(`/admin/faculties/${encodeURIComponent(facultyId)}/groups?semester_id=${encodeURIComponent(semesterId)}`, { signal });
  },
  listFacultyDepartments(facultyId: string) {
    return request<JsonApiCollection>(`/admin/faculties/${encodeURIComponent(facultyId)}/kafedras`);
  },
  facultyWorkload(facultyId: string, semesterId: string) {
    return request<{ data: unknown[] }>(`/admin/faculties/${encodeURIComponent(facultyId)}/load?semester_id=${encodeURIComponent(semesterId)}`);
  },
  departmentWorkload(departmentId: string, semesterId: string) {
    return request<{ data: unknown[] }>(`/admin/kafedras/${encodeURIComponent(departmentId)}/load?semester_id=${encodeURIComponent(semesterId)}`);
  },
  groupPeriods(groupSemesterId: string) {
    return request<JsonApiCollection>(`/admin/student_group_semesters/${encodeURIComponent(groupSemesterId)}/periods`);
  },
  upsertGroupPeriod(groupSemesterId: string, period: GroupPeriodInput) {
    return request<{ data: JsonApiResource }>(`/admin/student_group_semesters/${encodeURIComponent(groupSemesterId)}/periods`, { method: "POST", body: JSON.stringify({ period }) });
  },
  createEventTransfer(eventId: string, eventTransfer: EventTransferInput) {
    return request<{ data: JsonApiResource }>(`/admin/schedule_events/${encodeURIComponent(eventId)}/transfers`, { method: "POST", body: JSON.stringify({ event_transfer: eventTransfer }) });
  },
  listFacultySpecialities(facultyId: string) {
    return request<JsonApiCollection>(`/admin/faculties/${encodeURIComponent(facultyId)}/specialities`);
  },
  createSpeciality(speciality: SpecialityInput) {
    return request<{ data: JsonApiResource }>("/admin/specialities", { method: "POST", body: JSON.stringify({ speciality }) });
  },
  createStudentGroup(studentGroup: StudentGroupInput, options: { students_now: number; semester_duration: number }) {
    return request<{ data: JsonApiResource }>("/admin/student_groups", { method: "POST", body: JSON.stringify({ student_group: studentGroup, auto_provision: true, ...options }) });
  },
  listRoomReservations({ roomId, date }: { roomId: string; date: string }) {
    const query = new URLSearchParams({ room_id: roomId, date_from: date, date_to: date });
    return request<JsonApiCollection>(`/admin/room_reservations?${query.toString()}`);
  },
  createRoomReservation(roomReservation: RoomReservationInput) {
    return request<{ data: JsonApiResource }>("/admin/room_reservations", { method: "POST", body: JSON.stringify({ room_reservation: roomReservation }) });
  },
  deleteRoomReservation(id: string) {
    return request<void>(`/admin/room_reservations/${encodeURIComponent(id)}`, { method: "DELETE" });
  },
  createTeachingAssignment(teachingAssignment: { load_requirement_id: number; teacher_on_kafedra_id: number }) {
    return request<{ data: JsonApiResource }>("/admin/teaching_assignments", { method: "POST", body: JSON.stringify({ teaching_assignment: teachingAssignment }) });
  },
  updateTeachingAssignment(id: string, teachingAssignment: { teacher_on_kafedra_id: number }) {
    return request<{ data: JsonApiResource }>(`/admin/teaching_assignments/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ teaching_assignment: teachingAssignment }) });
  },
  createAssignmentRoom(assignmentRoomReference: { teaching_assignment_id: number; room_id: number; priority_level?: number }) {
    return request<{ data: JsonApiResource }>("/admin/assignment_room_references", { method: "POST", body: JSON.stringify({ assignment_room_reference: assignmentRoomReference }) });
  },
  updateAssignmentRoom(id: string, assignmentRoomReference: { room_id: number }) {
    return request<{ data: JsonApiResource }>(`/admin/assignment_room_references/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ assignment_room_reference: assignmentRoomReference }) });
  },
  searchTeachers(query: string, signal?: AbortSignal) {
    return request<JsonApiCollection>(`/admin/search/teachers?q=${encodeURIComponent(query.trim())}`, { signal });
  },
  teacherScheduleQuery(teacherId: string, query: ScheduleQuery, signal?: AbortSignal) {
    return request<Record<string, unknown>>(`/admin/teachers/${encodeURIComponent(teacherId)}/schedule${scheduleQuery(query)}`, { signal });
  },
  roomScheduleQuery(roomId: string, query: ScheduleQuery, signal?: AbortSignal) {
    return request<Record<string, unknown>>(`/admin/rooms/${encodeURIComponent(roomId)}/schedule${scheduleQuery(query)}`, { signal });
  },
  dashboardHealth() {
    return request<{ data: DashboardService[] }>("/admin/dashboard/health");
  },
  dashboardActiveUsers() {
    return request<{ data: ActiveUser[] }>("/admin/dashboard/active_users");
  },
  roomOccupancy(date: string, roomIds: string[]) {
    return request<Record<string, unknown>>("/admin/schedules/rooms/occupancy-query", {
      method: "POST",
      body: JSON.stringify({ date, room_ids: roomIds }),
    });
  },
  listAccounts() {
    return request<JsonApiCollection>("/auth/accounts");
  },
  createAccount(attributes: Record<string, unknown>) {
    return request<{ data: JsonApiResource }>("/auth/accounts", { method: "POST", body: JSON.stringify({ account: attributes }) });
  },
  updateAccount(id: string, attributes: Record<string, unknown>) {
    return request<{ data: JsonApiResource }>(`/auth/accounts/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ account: attributes }) });
  },
  deleteAccount(id: string) {
    return request<void>(`/auth/accounts/${encodeURIComponent(id)}`, { method: "DELETE" });
  },
  studentGroupSemesters(groupId: string) {
    return request<JsonApiCollection>(`/admin/student_groups/${encodeURIComponent(groupId)}/semesters`);
  },
  adminGroupSchedule(groupSemesterId: string) {
    return request<Record<string, unknown>>(`/admin/student_group_semesters/${encodeURIComponent(groupSemesterId)}/schedule`);
  },
  adminTeacherSchedule(teacherId: string, semesterId: string) {
    return request<Record<string, unknown>>(`/admin/teachers/${encodeURIComponent(teacherId)}/schedule?semester_id=${encodeURIComponent(semesterId)}`);
  },
  adminRoomSchedule(roomId: string, semesterId: string) {
    return request<Record<string, unknown>>(`/admin/rooms/${encodeURIComponent(roomId)}/schedule?semester_id=${encodeURIComponent(semesterId)}`);
  },
  createAdmin(resource: "faculties" | "semesters", attributes: Record<string, unknown>) {
    const key = resource === "faculties" ? "faculty" : "semester";
    return request<{ data: { id: string; attributes: Record<string, unknown> } }>(`/admin/${resource}`, { method: "POST", body: JSON.stringify({ [key]: attributes }) });
  },
  updateAdmin(resource: "faculties" | "semesters", id: string, attributes: Record<string, unknown>) {
    const key = resource === "faculties" ? "faculty" : "semester";
    return request<{ data: { id: string; attributes: Record<string, unknown> } }>(`/admin/${resource}/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ [key]: attributes }) });
  },
  deleteAdmin(resource: "faculties" | "semesters", id: string) {
    return request<void>(`/admin/${resource}/${encodeURIComponent(id)}`, { method: "DELETE" });
  },
  loadOverview(semesterId: string) {
    return request<{ data: unknown[] }>(`/admin/schedule/load_overview?semester_id=${encodeURIComponent(semesterId)}`);
  },
  logout(refreshToken: string) {
    return request<void>("/auth/sessions", { method: "DELETE", body: JSON.stringify({ refresh_token: refreshToken }) });
  },
};

function scheduleQuery(query: ScheduleQuery) {
  if (query.date) return `?date=${encodeURIComponent(query.date)}`;
  if (query.semesterId) return `?semester_id=${encodeURIComponent(query.semesterId)}`;
  return "";
}

export function accountFromAccessToken(token: string, email: string): Account {
  try {
    const encodedPayload = token.split(".")[1].replace(/-/g, "+").replace(/_/g, "/");
    const payload = JSON.parse(atob(encodedPayload.padEnd(Math.ceil(encodedPayload.length / 4) * 4, "="))) as { sub?: string; role?: AccountRole; must_change_password?: boolean };
    return { id: payload.sub ?? "unknown", email, role: payload.role ?? "unknown", status: "active", must_change_password: payload.must_change_password === true };
  } catch {
    return { id: "unknown", email, role: "unknown", status: "active" };
  }
}

export { API_BASE_URL };
