import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminOperations } from "./AdminOperations";

describe("AdminOperations", () => {
  it("exposes operational API collections without endpoint documentation placeholders", () => {
    const markup = renderToStaticMarkup(<AdminOperations />);
    expect(markup).toContain("Операционные данные");
    expect(markup).toContain("Праздники");
    expect(markup).toContain("Переносы");
    expect(markup).toContain("Бронирования");
    expect(markup).toContain("Файлы");
    expect(markup).toContain("Аудит");
    expect(markup).not.toContain("GET /admin");
  });
});
