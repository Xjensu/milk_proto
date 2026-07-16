import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { WorkspaceProvider } from "./workspace/WorkspaceContext";
import { AdminWorkload } from "./AdminWorkload";

describe("AdminWorkload", () => {
  it("starts with readable faculty and department workflows", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminWorkload /></WorkspaceProvider>);
    expect(markup).toContain("Распределение нагрузки");
    expect(markup).toContain("Нагрузка факультета");
    expect(markup).toContain("Нагрузка кафедры");
    expect(markup).toContain("Поиск: факультет");
    expect(markup).not.toContain("semester_id");
    expect(markup).not.toContain("#");
  });
});
