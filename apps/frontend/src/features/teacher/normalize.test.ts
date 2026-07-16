import { describe, expect, it } from "vitest";

import { normalizeDepartments, normalizeTeacherLoad } from "./normalize";

describe("teacher self-service normalization", () => {
  it("turns load projection rows into readable workload records", () => {
    const rows = normalizeTeacherLoad({ data: [{
      id: 7,
      hours_assigned: 36,
      actual_hours: 18,
      activity_type: { name: "Лекция" },
      stream: {
        subject: { name: "Математика" },
        student_group_semester: { course_number: 2, semester: { number: 3, start_year: 2026, end_year: 2027 } },
      },
      teachers: [{ teacher: { last_name: "Иванов", first_name: "Иван", middle_name: "Иванович" } }],
      reservations: [{ room: { name: "101" } }],
    }] });

    expect(rows[0]).toEqual({
      id: "7",
      subject: "Математика",
      activityType: "Лекция",
      hours: "18 / 36 ч",
      context: "2 курс · 3 семестр · 2026/2027",
      room: "101",
      teacher: "Иванов Иван Иванович",
    });
  });

  it("joins department relationships with JSON:API included resources", () => {
    const rows = normalizeDepartments({
      data: [{ id: "5", attributes: { pochasovoy: true }, relationships: { kafedra: { data: { id: "3", type: "kafedra" } } } }],
      included: [{ id: "3", type: "kafedra", attributes: { name: "Информационные технологии", short_name: "ИТ" } }],
    });

    expect(rows[0]).toEqual({ id: "5", name: "Информационные технологии", shortName: "ИТ", employment: "Почасовая работа" });
  });
});
