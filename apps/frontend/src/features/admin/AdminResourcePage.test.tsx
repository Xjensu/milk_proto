import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminResourcePage } from "./AdminResourcePage";

describe("AdminResourcePage", () => {
  it("exposes live resource categories and authorized editing", () => {
    const markup = renderToStaticMarkup(<AdminResourcePage canWrite />);
    expect(markup).toContain("Данные университета");
    expect(markup).toContain("Факультеты");
    expect(markup).toContain("Семестры");
    expect(markup).toContain("Преподаватели");
    expect(markup).toContain("Аудитории");
    expect(markup).toContain("Добавить");
  });

  it("does not expose write controls to read-only roles", () => {
    const markup = renderToStaticMarkup(<AdminResourcePage canWrite={false} />);
    expect(markup).not.toContain("Добавить");
  });
});
