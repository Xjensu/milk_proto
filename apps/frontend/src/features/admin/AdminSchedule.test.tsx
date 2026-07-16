import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminSchedule } from "./AdminSchedule";

describe("AdminSchedule", () => {
  it("starts with live semester and entity selectors", () => {
    const markup = renderToStaticMarkup(<AdminSchedule />);
    expect(markup).toContain("Рабочая сетка расписания");
    expect(markup).toContain("Группа");
    expect(markup).toContain("Преподаватель");
    expect(markup).toContain("Аудитория");
    expect(markup).toContain("Выберите семестр");
    expect(markup).not.toContain("Весна 2026");
  });
});
