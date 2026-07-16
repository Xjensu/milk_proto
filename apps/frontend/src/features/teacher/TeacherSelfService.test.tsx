import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { TeacherSelfService } from "./TeacherSelfService";

describe("TeacherSelfService", () => {
  it("requires semester context and exposes structured self-service tabs", () => {
    const markup = renderToStaticMarkup(<TeacherSelfService onLogout={() => undefined} />);

    expect(markup).toContain("Личный кабинет преподавателя");
    expect(markup).toContain("Выберите семестр");
    expect(markup).toContain("Моё расписание");
    expect(markup).toContain("Моя нагрузка");
    expect(markup).toContain("Мои кафедры");
    expect(markup).not.toContain("<pre");
  });
});
