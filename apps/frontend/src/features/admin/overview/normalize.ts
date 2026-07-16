import type { ActiveUserSummary, ServiceStatus } from "../domain";

export function normalizeServices(source: unknown): ServiceStatus[] {
  return dataRows(source).flatMap((row) => {
    const status = stringValue(row.status);
    if (!row.key || !row.label || !["operational", "degraded", "unavailable"].includes(status)) return [];
    const latency = typeof row.latency_ms === "number" ? row.latency_ms : undefined;
    return [{
      key: stringValue(row.key),
      label: stringValue(row.label),
      status: status as ServiceStatus["status"],
      ...(latency === undefined ? {} : { latencyMs: latency }),
    }];
  });
}

export function normalizeActiveUsers(source: unknown): ActiveUserSummary[] {
  return dataRows(source).flatMap((row) => {
    if (!row.account_id || !row.email || !row.last_seen_at) return [];
    return [{
      id: stringValue(row.account_id),
      email: stringValue(row.email),
      role: stringValue(row.role, "unknown"),
      area: stringValue(row.area, "Unknown area"),
      lastSeenAt: stringValue(row.last_seen_at),
    }];
  });
}

function dataRows(source: unknown): Record<string, unknown>[] {
  if (!isRecord(source) || !Array.isArray(source.data)) return [];
  return source.data.filter(isRecord);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function stringValue(value: unknown, fallback = "") {
  return typeof value === "string" || typeof value === "number" ? String(value) : fallback;
}
