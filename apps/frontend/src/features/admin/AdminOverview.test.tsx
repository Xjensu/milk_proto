import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminOverview, describeAuditEvent } from "./AdminOverview";

describe("AdminOverview", () => {
  it("contains only operational status, active users, and event journal", () => {
    const markup = renderToStaticMarkup(<AdminOverview account={null} onOpenSchedule={() => undefined} />);
    expect(markup).toContain("Статус сервисов");
    expect(markup).toContain("Активные пользователи");
    expect(markup).toContain("Журнал событий");
    expect(markup).not.toContain("Workload rows");
    expect(markup).not.toContain("Faculties");
    expect(markup).not.toContain("Files processed");
    expect(markup).not.toContain("Semester");
  });

  it("describes audit activity without raw ids", () => {
    expect(describeAuditEvent({
      id: "91",
      type: "audit_logs",
      attributes: { action: "moved", entity_type: "schedule_event", actor_email: "anna@example.test", subject_name: "Mathematics", group_display_name: "CS-21-3" },
    })).toBe("anna@example.test moved Mathematics для CS-21-3");
  });
});
