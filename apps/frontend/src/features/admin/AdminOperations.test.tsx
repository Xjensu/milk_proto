import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { WorkspaceProvider } from "./workspace/WorkspaceContext";
import { AdminOperations } from "./AdminOperations";

describe("AdminOperations", () => {
  it("guides operators from faculty and semester to a group calendar", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminOperations /></WorkspaceProvider>);
    expect(markup).toContain("Group calendar");
    expect(markup).toContain("Lecture period");
    expect(markup).toContain("Credit period");
    expect(markup).toContain("Exam session");
    expect(markup).toContain("Search faculty");
    expect(markup).not.toContain("GET /admin");
    expect(markup).not.toContain("#");
  });
});
