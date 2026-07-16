import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { WeekGrid } from "./WeekGrid";

describe("WeekGrid", () => {
  it("renders normalized schedule details and the changed state", () => {
    const markup = renderToStaticMarkup(<WeekGrid source={{
      data: [{
        id: "7",
        attributes: {
          day_of_week: 1,
          time_slot_start_time: "08:30",
          time_slot_end_time: "09:55",
          subject_name: "Математика",
          activity_type_name: "Лекция",
          room_name: "101",
          group_display_name: "ПИ-23-1",
          is_manual_change: true,
        },
      }],
    }} />);

    expect(markup).toContain("Понедельник");
    expect(markup).toContain("Математика");
    expect(markup).toContain("ПИ-23-1");
    expect(markup).toContain("Изменено");
    expect(markup).toContain("schedule-event-changed");
  });
});
