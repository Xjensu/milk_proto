import type { AccountRole, JsonApiResource } from "../../lib/api";

export function roleLabel(role: AccountRole) {
  return {
    super_admin: "Главный администратор",
    faculty_admin: "Администратор факультета",
    kafedra_admin: "Администратор кафедры",
    scheduler: "Диспетчер расписания",
    teacher: "Преподаватель",
    student: "Студент",
    unknown: "Роль не определена",
  }[role];
}

export function resourceLabel(item: JsonApiResource, fallback: string) {
  const attributes = item.attributes ?? {};
  const fullName = [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
  if (attributes.start_year && attributes.end_year && attributes.number) {
    return `${attributes.number} семестр ${attributes.start_year}/${attributes.end_year}`;
  }
  const value = [attributes.display_name, attributes.name, attributes.full_name, fullName, attributes.short_name, attributes.number]
    .find((candidate) => (typeof candidate === "string" && candidate.length > 0) || typeof candidate === "number");
  return String(value ?? `Без названия: ${fallback.toLowerCase()}`);
}

export function groupResource(item: JsonApiResource) {
  const attributes = item.attributes ?? {};
  return {
    id: String(attributes.group_semester_id ?? item.id),
    label: `${String(attributes.speciality_name ?? "Student group")}-${String(attributes.course ?? "—")}`,
  };
}
