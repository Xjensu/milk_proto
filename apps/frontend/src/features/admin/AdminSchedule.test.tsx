import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminSchedule, queryModeRequirements } from "./AdminSchedule";
import { WorkspaceProvider } from "./workspace/WorkspaceContext";

describe("AdminSchedule", () => {
  it("starts with task modes and faculty-first group guidance", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminSchedule /></WorkspaceProvider>);
    for (const label of ["Группа", "Преподаватель", "Аудитория", "Занятость"]) expect(markup).toContain(label);
    expect(markup).toContain("Сначала выберите факультет.");
    expect(markup).toContain("Выберите семестр.");
    expect(markup).toContain("Учебная группа");
    expect(markup).not.toContain("#1");
  });

  it("defines the required context for every schedule mode", () => {
    expect(queryModeRequirements("group", "semester")).toEqual(["faculty", "semester", "group"]);
    expect(queryModeRequirements("teacher", "date")).toEqual(["date", "teacher"]);
    expect(queryModeRequirements("room", "semester")).toEqual(["semester", "room"]);
    expect(queryModeRequirements("occupancy", "date")).toEqual(["date", "rooms"]);
  });
});
