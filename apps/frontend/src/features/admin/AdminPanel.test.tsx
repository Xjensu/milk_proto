import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { Account } from "../../lib/api";
import { AdminPanel } from "./AdminPanel";

const account: Account = { id: "1", email: "admin@example.test", role: "super_admin", status: "active" };

describe("AdminPanel", () => {
  it("renders English task navigation and super-admin accounts", () => {
    const markup = renderToStaticMarkup(<AdminPanel account={account} onLogout={() => undefined} />);
    expect(markup).toContain("admin-product-shell");
    for (const label of ["Overview", "Schedule", "Workload", "Catalog", "Calendar", "Accounts"]) {
      expect(markup).toContain(label);
    }
    expect(markup).not.toContain("86%");
  });

  it("hides account administration from non-super-admin roles", () => {
    const markup = renderToStaticMarkup(<AdminPanel account={{ ...account, role: "scheduler" }} onLogout={() => undefined} />);
    expect(markup).not.toContain(">Accounts<");
  });
});
