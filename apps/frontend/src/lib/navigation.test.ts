import { describe, expect, it } from "vitest";

import type { Account } from "./api";
import { staffView } from "./navigation";

const account: Account = { id: "1", email: "user@example.test", role: "super_admin", status: "active" };

describe("staffView", () => {
  it("routes anonymous and authenticated roles consistently", () => {
    expect(staffView(null)).toBe("login");
    expect(staffView(account)).toBe("admin");
    expect(staffView({ ...account, role: "teacher" })).toBe("self");
    expect(staffView({ ...account, must_change_password: true })).toBe("password");
  });
});
