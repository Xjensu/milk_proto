import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { Account } from "../../lib/api";
import { AdminPanel } from "./AdminPanel";

const account: Account = { id: "1", email: "admin@example.test", role: "super_admin", status: "active" };

describe("AdminPanel", () => {
  it("renders task-oriented API modules and super-admin account navigation", () => {
    const markup = renderToStaticMarkup(<AdminPanel account={account} onLogout={() => undefined} />);

    expect(markup).toContain("Обзор");
    expect(markup).toContain("Расписание");
    expect(markup).toContain("Нагрузка");
    expect(markup).toContain("Справочники");
    expect(markup).toContain("Календарь");
    expect(markup).toContain("Пользователи");
    expect(markup).not.toContain("86%");
  });

  it("hides account administration from non-super-admin roles", () => {
    const markup = renderToStaticMarkup(<AdminPanel account={{ ...account, role: "scheduler" }} onLogout={() => undefined} />);
    expect(markup).not.toContain("Пользователи");
  });
});
