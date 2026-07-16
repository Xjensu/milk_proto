import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { WeekGrid } from "./WeekGrid";

describe("WeekGrid", () => {
  it("renders readable English schedule details and selection controls", () => {
    const markup = renderToStaticMarkup(<WeekGrid onSelect={() => undefined} source={{
      data: [{ id: "7", attributes: {
        day_of_week: 1,
        time_slot_start_time: "08:30",
        time_slot_end_time: "09:55",
        subject_name: "Mathematics",
        activity_type_name: "Lecture",
        room_name: "Room 101",
        group_display_name: "CS-21-3",
        is_manual_change: true,
      } }],
    }} />);

    expect(markup).toContain("Monday");
    expect(markup).toContain("Mathematics");
    expect(markup).toContain("CS-21-3");
    expect(markup).toContain("Changed");
    expect(markup).toContain("View Mathematics details");
  });

  it("uses valid missing-assignment copy", () => {
    const markup = renderToStaticMarkup(<WeekGrid source={{ data: [{ id: "8", attributes: { day_of_week: 2, subject_name: "Physical education", room: null, teachers: [] } }] }} />);
    expect(markup).toContain("Not assigned");
  });
});
