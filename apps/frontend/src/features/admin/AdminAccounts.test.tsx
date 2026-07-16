import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminAccounts } from "./AdminAccounts";

describe("AdminAccounts", () => {
  it("renders English account administration without raw scope inputs", () => {
    const markup = renderToStaticMarkup(<AdminAccounts />);
    expect(markup).toContain("Account management");
    expect(markup).toContain("Create account");
    expect(markup).toContain("Loading accounts");
    expect(markup).not.toContain("Faculty ID");
    expect(markup).not.toContain("Department ID");
  });
});
