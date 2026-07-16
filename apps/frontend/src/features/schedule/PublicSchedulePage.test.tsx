import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PublicSchedulePage } from "./PublicSchedulePage";

describe("PublicSchedulePage", () => {
  it("starts the group flow with API-backed cascading selectors", () => {
    const markup = renderToStaticMarkup(<PublicSchedulePage mode="group" onBack={() => undefined} />);

    expect(markup).toContain("Расписание группы");
    expect(markup).toContain("Факультет");
    expect(markup).toContain("Семестр");
    expect(markup).toContain("Группа");
    expect(markup).toContain("Показать");
    expect(markup).toContain("disabled");
  });
});
