import { describe, expect, it } from "vitest";

import { devAdminPreview } from "./devPreview";

describe("devAdminPreview", () => {
  it("only enables the admin preview for an explicit development query", () => {
    expect(devAdminPreview(true, "?admin-preview=1")).toBe(true);
    expect(devAdminPreview(true, "?admin-preview=0")).toBe(false);
    expect(devAdminPreview(false, "?admin-preview=1")).toBe(false);
  });
});
