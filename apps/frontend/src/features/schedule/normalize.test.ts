import { describe, expect, it } from "vitest";

import { normalizeSchedule } from "./normalize";

describe("normalizeSchedule", () => {
  it("groups live JSON:API rows by day and sorts them by start time", () => {
    const result = normalizeSchedule({
      data: [
        { id: "2", attributes: { day_of_week: 1, time_slot_start_time: "10:10", subject_name: "Физика", activity_type_name: "Лекция", teachers: [{ full_name: "Иванов И.И." }], is_manual_change: true } },
        { id: "1", attributes: { day_of_week: 1, time_slot_start_time: "08:30", subject_name: "Математика", room_name: "101" } },
        { id: "3", attributes: { day_of_week: 2, time_slot_start_time: "08:30", subject_name: "Информатика" } },
      ],
    });

    expect(result.map((day) => day.label)).toEqual(["Понедельник", "Вторник"]);
    expect(result[0].events.map((event) => event.subject)).toEqual(["Математика", "Физика"]);
    expect(result[0].events[1]).toMatchObject({ changed: true, counterpart: "Иванов И.И." });
  });

  it("normalizes the documented legacy day envelope", () => {
    const result = normalizeSchedule({
      data: {
        days: [{
          date: "2026-07-16",
          events: [{
            time_slot: { start_time: "08:30", end_time: "09:55" },
            subject: { name: "Базы данных" },
            activity_type: { short_name: "Лек" },
            room: null,
            student_group: { display_name: "ПИ-23-1" },
            is_transferred: true,
          }],
        }],
      },
    });

    expect(result[0].events[0]).toMatchObject({
      startTime: "08:30",
      endTime: "09:55",
      subject: "Базы данных",
      activityType: "Лек",
      room: "Аудитория не указана",
      counterpart: "ПИ-23-1",
      changed: true,
    });
  });
});
