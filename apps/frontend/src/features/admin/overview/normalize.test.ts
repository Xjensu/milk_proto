import { describe, expect, it } from "vitest";

import { normalizeActiveUsers, normalizeServices } from "./normalize";

describe("overview normalization", () => {
  it("accepts safe service states and rejects malformed rows", () => {
    expect(normalizeServices({ data: [
      { key: "api", label: "API", status: "operational", latency_ms: 12 },
      { key: "secret", status: "unknown" },
    ] })).toEqual([{ key: "api", label: "API", status: "operational", latencyMs: 12 }]);
  });

  it("normalizes active users without exposing unknown fields", () => {
    expect(normalizeActiveUsers({ data: [{
      account_id: "2", email: "scheduler@example.test", role: "scheduler",
      area: "Schedule", last_seen_at: "2027-01-11T09:30:00Z", token: "secret",
    }] })).toEqual([{ id: "2", email: "scheduler@example.test", role: "scheduler", area: "Schedule", lastSeenAt: "2027-01-11T09:30:00Z" }]);
  });
});
