import type { AccountRole, JsonApiResource } from "../../lib/api";

export function roleLabel(role: AccountRole) {
  return {
    super_admin: "Super administrator",
    faculty_admin: "Faculty administrator",
    kafedra_admin: "Department administrator",
    scheduler: "Scheduler",
    teacher: "Teacher",
    student: "Student",
    unknown: "Role unavailable",
  }[role];
}

export function resourceLabel(item: JsonApiResource, fallback: string) {
  const attributes = item.attributes ?? {};
  const fullName = [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
  if (attributes.start_year && attributes.end_year && attributes.number) {
    return `${attributes.number} semester ${attributes.start_year}/${attributes.end_year}`;
  }
  const value = [attributes.display_name, attributes.name, attributes.full_name, fullName, attributes.short_name, attributes.number]
    .find((candidate) => (typeof candidate === "string" && candidate.length > 0) || typeof candidate === "number");
  return String(value ?? `Unnamed ${fallback.toLowerCase()}`);
}
