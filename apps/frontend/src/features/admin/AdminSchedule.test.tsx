import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminSchedule, queryModeRequirements } from "./AdminSchedule";
import { WorkspaceProvider } from "./workspace/WorkspaceContext";

describe("AdminSchedule", () => {
  it("starts semester-first and reveals faculty and groups only after a semester is chosen", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminSchedule /></WorkspaceProvider>);
    expect(markup).toContain("Schedule workspace");
    expect(markup).toContain("The chosen semester is remembered.");
    // Steps 2 and 3 stay hidden until a semester (and faculty) are selected.
    expect(markup).not.toContain("Pick the faculty whose groups you plan.");
    expect(markup).not.toContain("Student groups");
  });

  it("defines the required context for every schedule mode", () => {
    expect(queryModeRequirements("group", "semester")).toEqual(["semester", "faculty", "groups"]);
    expect(queryModeRequirements("teacher", "date")).toEqual(["date", "teacher"]);
    expect(queryModeRequirements("room", "semester")).toEqual(["semester", "room"]);
    expect(queryModeRequirements("occupancy", "date")).toEqual(["date", "rooms"]);
  });
});
