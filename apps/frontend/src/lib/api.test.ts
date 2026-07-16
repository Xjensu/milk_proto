import { beforeEach, describe, expect, it, vi } from "vitest";

import { API_BASE_URL, accountFromAccessToken, api } from "./api";

class MemoryStorage implements Storage {
  private values = new Map<string, string>();
  get length() { return this.values.size; }
  clear() { this.values.clear(); }
  getItem(key: string) { return this.values.get(key) ?? null; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string) { this.values.delete(key); }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

describe("API transport", () => {
  beforeEach(() => {
    vi.stubGlobal("sessionStorage", new MemoryStorage());
    vi.restoreAllMocks();
  });

  it("uses the same-origin API proxy by default", () => {
    expect(API_BASE_URL).toBe("/api");
  });

  it("includes semester context in a public room schedule request", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })));
    vi.stubGlobal("fetch", fetchMock);

    await api.roomSchedule("14", "2");

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/public/rooms/14/schedule?semester_id=2",
      expect.any(Object),
    );
  });

  it("includes semester context in teacher self-service requests", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })));
    vi.stubGlobal("fetch", fetchMock);

    await api.meSchedule("2");
    await api.meLoad("2");

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/admin/me/schedule?semester_id=2",
      "/api/admin/me/load?semester_id=2",
    ]);
  });

  it("uses only live routes for admin operational collections and accounts", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })));
    vi.stubGlobal("fetch", fetchMock);

    await api.listAdminCollection("holidays");
    await api.listAdminCollection("audit_logs", "?page=2&per_page=10");
    await api.listAccounts();

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/admin/holidays",
      "/api/admin/audit_logs?page=2&per_page=10",
      "/api/auth/accounts",
    ]);
  });

  it("uses the documented account CRUD routes and envelopes", async () => {
    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: {} }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })));
    vi.stubGlobal("fetch", fetchMock);

    await api.createAccount({ email: "scheduler@example.test", role: "scheduler" });
    await api.updateAccount("account 1", { status: "disabled" });
    await api.deleteAccount("account 1");

    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method, init.body])).toEqual([
      ["/api/auth/accounts", "POST", JSON.stringify({ account: { email: "scheduler@example.test", role: "scheduler" } })],
      ["/api/auth/accounts/account%201", "PATCH", JSON.stringify({ account: { status: "disabled" } })],
      ["/api/auth/accounts/account%201", "DELETE", undefined],
    ]);
  });

  it("preserves the forced-password-change claim and uses its dedicated route", async () => {
    const payload = btoa(JSON.stringify({ sub: "42", role: "teacher", must_change_password: true })).replace(/=/g, "");
    expect(accountFromAccessToken(`header.${payload}.signature`, "teacher@example.test").must_change_password).toBe(true);

    const fetchMock = vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ access_token: "new", refresh_token: "refresh" }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    })));
    vi.stubGlobal("fetch", fetchMock);
    await api.changePassword("old-password", "new-password");
    expect(fetchMock.mock.calls[0][0]).toBe("/api/auth/password");
    expect(fetchMock.mock.calls[0][1].body).toBe(JSON.stringify({ current_password: "old-password", new_password: "new-password" }));
  });

  it("loads groups only inside faculty and semester context", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.listFacultyGroups("faculty 4", "semester 12");

    expect(fetchMock.mock.calls[0][0]).toBe(
      "/api/admin/faculties/faculty%204/groups?semester_id=semester%2012",
    );
  });

  it("builds teacher and room schedule queries for semester or exact date", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.teacherScheduleQuery("7", { semesterId: "12" });
    await api.teacherScheduleQuery("7", { date: "2027-01-11" });
    await api.roomScheduleQuery("9", { date: "2027-01-11" });

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/admin/teachers/7/schedule?semester_id=12",
      "/api/admin/teachers/7/schedule?date=2027-01-11",
      "/api/admin/rooms/9/schedule?date=2027-01-11",
    ]);
  });

  it("uses bounded search and operational dashboard routes", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.searchTeachers("Emily Carter");
    await api.dashboardHealth();
    await api.dashboardActiveUsers();

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/admin/search/teachers?q=Emily%20Carter",
      "/api/admin/dashboard/health",
      "/api/admin/dashboard/active_users",
    ]);
  });

  it("posts a bounded room set for occupancy", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.roomOccupancy("2027-01-11", ["9", "11"]);

    expect(fetchMock.mock.calls[0]).toEqual([
      "/api/admin/schedules/rooms/occupancy-query",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ date: "2027-01-11", room_ids: ["9", "11"] }),
      }),
    ]);
  });

  it("uses scoped faculty and department workload entrypoints", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.facultyWorkload("4", "12");
    await api.departmentWorkload("7", "12");
    await api.listFacultyDepartments("4");

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "/api/admin/faculties/4/load?semester_id=12",
      "/api/admin/kafedras/7/load?semester_id=12",
      "/api/admin/faculties/4/kafedras",
    ]);
  });

  it("uses group calendar and single-occurrence transfer routes", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.groupPeriods("17");
    await api.upsertGroupPeriod("17", { period_kind: "lecture_week", start_date: "2026-09-01", end_date: "2026-09-10" });
    await api.createEventTransfer("44", { original_date: "2026-09-04", transfer_type: "moved", new_date: "2026-09-11", new_time_slot_id: 3 });

    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/admin/student_group_semesters/17/periods", undefined],
      ["/api/admin/student_group_semesters/17/periods", "POST"],
      ["/api/admin/schedule_events/44/transfers", "POST"],
    ]);
  });

  it("uses contextual speciality, group, and reservation routes", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.listFacultySpecialities("4");
    await api.createSpeciality({ faculty_id: 4, name: "Computer Science", duration_of_study: 4 });
    await api.createStudentGroup({ speciality_id: 8, year_of_admission: "2026-09-01" }, { students_now: 25, semester_duration: 8 });
    await api.listRoomReservations({ roomId: "9", date: "2026-09-01" });
    await api.createRoomReservation({ room_id: 9, teacher_id: 6, reserved_date: "2026-09-01", time_slot_id: 3, purpose: "Consultation" });

    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/admin/faculties/4/specialities", undefined],
      ["/api/admin/specialities", "POST"],
      ["/api/admin/student_groups", "POST"],
      ["/api/admin/room_reservations?room_id=9&date_from=2026-09-01&date_to=2026-09-01", undefined],
      ["/api/admin/room_reservations", "POST"],
    ]);
  });

  it("writes workload assignments through existing join resources", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.createTeachingAssignment({ load_requirement_id: 41, teacher_on_kafedra_id: 12 });
    await api.updateTeachingAssignment("22", { teacher_on_kafedra_id: 13 });
    await api.createAssignmentRoom({ teaching_assignment_id: 22, room_id: 9, priority_level: 1 });
    await api.updateAssignmentRoom("31", { room_id: 10 });

    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/admin/teaching_assignments", "POST"],
      ["/api/admin/teaching_assignments/22", "PATCH"],
      ["/api/admin/assignment_room_references", "POST"],
      ["/api/admin/assignment_room_references/31", "PATCH"],
    ]);
  });

  it("creates a curriculum stream and its workload requirement", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);

    await api.searchDisciplines("Distributed systems");
    await api.createCurriculumStream({ group_semester_id: 17, subject_id: 9, kafedra_id: 3, course: 3 });
    await api.createStudyLoadRequirement({ stream_id: 20, activity_type_id: 2, hours_assigned: 36, hours_per_event: 2 });

    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/admin/search/disciplines?q=Distributed%20systems", undefined],
      ["/api/admin/curriculum_streams", "POST"],
      ["/api/admin/study_load_requirements", "POST"],
    ]);
  });

  it("updates and deletes schedule events without exposing identifiers in the UI", async () => {
    const fetchMock = jsonFetch();
    vi.stubGlobal("fetch", fetchMock);
    await api.updateScheduleEvent("19", { day_of_week: 2, time_slot_id: 4, room_id: 9, status: "modified", is_manual_change: true });
    await api.deleteScheduleEvent("19");
    expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/admin/schedule_events/19", "PATCH"],
      ["/api/admin/schedule_events/19", "DELETE"],
    ]);
  });
});

function jsonFetch() {
  return vi.fn().mockImplementation(() => Promise.resolve(new Response(JSON.stringify({ data: [] }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  })));
}
