import type { AccountRole, JsonApiResource } from "../../lib/api";

export function roleLabel(role: AccountRole) {
  return { super_admin: "Супер-администратор", faculty_admin: "Администратор факультета", kafedra_admin: "Администратор кафедры", scheduler: "Диспетчер расписания", teacher: "Преподаватель", student: "Студент", unknown: "Роль не определена" }[role];
}

export function resourceLabel(item: JsonApiResource, fallback: string) {
  const attributes = item.attributes ?? {};
  const fullName = [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
  if (attributes.start_year && attributes.end_year) return `${attributes.start_year}/${attributes.end_year}${attributes.number ? ` · ${attributes.number}` : ""}`;
  const value = [attributes.display_name, attributes.name, attributes.full_name, fullName, attributes.short_name]
    .find((candidate) => typeof candidate === "string" && candidate.length > 0);
  return String(value ?? `${fallback} #${item.id}`);
}
