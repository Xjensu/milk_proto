import { describe, expect, it } from "vitest";

import { normalizeScheduleEvents } from "./normalize";

describe("admin schedule normalization", () => {
  it("keeps readable context and valid missing assignments", () => {
    expect(normalizeScheduleEvents({ data: [{
      id: "19",
      attributes: {
        day_of_week: 1,
        subject: { name: "Mathematics" },
        activity_type: { name: "Lecture" },
        student_group: { code: "CS-21", course: 3 },
        time_slot: { start_time: "09:00", end_time: "10:30" },
        teachers: [],
        room: null,
      },
    }] })).toEqual([expect.objectContaining({
      id: "19",
      subject: "Mathematics",
      activityType: "Lecture",
      group: "CS-21-3",
      teacher: "Not assigned",
      room: "Not assigned",
    })]);
  });
});
