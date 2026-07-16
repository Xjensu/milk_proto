import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminResourcePage } from "./AdminResourcePage";
import { WorkspaceProvider } from "./workspace/WorkspaceContext";

describe("AdminResourcePage", () => {
  it("exposes contextual catalog workflows without raw identifiers", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminResourcePage canWrite /></WorkspaceProvider>);
    expect(markup).toContain("University catalog");
    expect(markup).toContain("Specialities");
    expect(markup).toContain("Student groups");
    expect(markup).toContain("Reservations");
    expect(markup).toContain("Reference data");
    expect(markup).not.toContain("Identifier");
    expect(markup).not.toContain("#");
  });

  it("does not expose creation affordances to read-only roles", () => {
    const markup = renderToStaticMarkup(<WorkspaceProvider><AdminResourcePage canWrite={false} /></WorkspaceProvider>);
    expect(markup).not.toContain("Create speciality");
  });
});
