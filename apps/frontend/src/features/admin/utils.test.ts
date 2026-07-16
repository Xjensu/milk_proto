import { describe, expect, it } from "vitest";

import { resourceLabel, roleLabel } from "./utils";

describe("admin role labels", () => {
  it("uses concise English labels", () => {
    expect(roleLabel("super_admin")).toBe("Super administrator");
    expect(roleLabel("scheduler")).toBe("Scheduler");
    expect(roleLabel("unknown")).toBe("Role unavailable");
  });

  it("does not expose raw ids when a resource has no readable value", () => {
    expect(resourceLabel({ id: "918", type: "rooms", attributes: {} }, "Room")).toBe("Unnamed room");
  });
});
