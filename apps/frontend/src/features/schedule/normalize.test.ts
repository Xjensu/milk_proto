import { describe, expect, it } from "vitest";

import { normalizeSchedule } from "./normalize";

describe("normalizeSchedule", () => {
  it("groups live JSON:API rows by English day and sorts by start time", () => {
    const result = normalizeSchedule({ data: [
      { id: "2", attributes: { day_of_week: 1, time_slot_start_time: "10:10", subject_name: "Physics", activity_type_name: "Lecture", teachers: [{ full_name: "Carter Emily" }], is_manual_change: true } },
      { id: "1", attributes: { day_of_week: 1, time_slot_start_time: "08:30", subject_name: "Mathematics", room_name: "101" } },
      { id: "3", attributes: { day_of_week: 2, time_slot_start_time: "08:30", subject_name: "Computer Science" } },
    ] });
    expect(result.map((day) => day.label)).toEqual(["Monday", "Tuesday"]);
    expect(result[0].events.map((event) => event.subject)).toEqual(["Mathematics", "Physics"]);
    expect(result[0].events[1]).toMatchObject({ changed: true, counterpart: "Carter Emily" });
  });

  it("normalizes the documented legacy envelope with optional assignments", () => {
    const result = normalizeSchedule({ data: { days: [{ date: "2026-07-16", events: [{
      time_slot: { start_time: "08:30", end_time: "09:55" }, subject: { name: "Databases" },
      activity_type: { short_name: "Lecture" }, room: null,
      student_group: { display_name: "CS-21-3" }, is_transferred: true,
    }] }] } });
    expect(result[0].events[0]).toMatchObject({ startTime: "08:30", endTime: "09:55", subject: "Databases", activityType: "Lecture", room: "Not assigned", counterpart: "CS-21-3", changed: true });
  });
});
