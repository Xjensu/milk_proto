import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminWorkload } from "./AdminWorkload";

describe("AdminWorkload", () => {
  it("requires a live semester before loading the overview", () => {
    const markup = renderToStaticMarkup(<AdminWorkload />);
    expect(markup).toContain("Контроль учебной нагрузки");
    expect(markup).toContain("Выберите семестр");
    expect(markup).toContain("Загрузить нагрузку");
    expect(markup).not.toContain("Сводная нагрузка");
  });
});
