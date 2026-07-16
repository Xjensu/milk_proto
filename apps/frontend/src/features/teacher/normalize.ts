export type TeacherLoadRow = { id: string; subject: string; activityType: string; hours: string; context: string; room: string; teacher: string };
export type DepartmentRow = { id: string; name: string; shortName: string; employment: string };

export function normalizeTeacherLoad(source: Record<string, unknown>): TeacherLoadRow[] {
  if (!Array.isArray(source.data)) return [];

  return source.data.flatMap((value) => {
    if (!isRecord(value)) return [];
    const stream = recordValue(value.stream);
    const subject = recordValue(stream.subject);
    const groupSemester = recordValue(stream.student_group_semester);
    const semester = recordValue(groupSemester.semester);
    const activityType = recordValue(value.activity_type);
    const teachers = Array.isArray(value.teachers) ? value.teachers.filter(isRecord) : [];
    const teacher = recordValue(teachers[0]?.teacher);
    const reservations = Array.isArray(value.reservations) ? value.reservations.filter(isRecord) : [];
    const room = recordValue(reservations[0]?.room);
    const assigned = text(value.hours_assigned, "—");
    const actual = text(value.actual_hours);
    const context = [
      groupSemester.course_number ? `${groupSemester.course_number} курс` : "",
      semester.number ? `${semester.number} семестр` : "",
      semester.start_year && semester.end_year ? `${semester.start_year}/${semester.end_year}` : "",
    ].filter(Boolean).join(" · ");

    return [{
      id: text(value.id),
      subject: text(subject.name ?? subject.short_name, "Предмет не указан"),
      activityType: text(activityType.name ?? activityType.short_name, "Вид занятия не указан"),
      hours: `${actual ? `${actual} / ` : ""}${assigned} ч`,
      context: context || "Контекст группы не указан",
      room: text(room.name ?? room.number, "—"),
      teacher: [teacher.last_name, teacher.first_name, teacher.middle_name].filter(Boolean).join(" ") || "—",
    }];
  });
}

export function normalizeDepartments(source: Record<string, unknown>): DepartmentRow[] {
  if (!Array.isArray(source.data)) return [];
  const included = new Map<string, Record<string, unknown>>();
  if (Array.isArray(source.included)) {
    source.included.filter(isRecord).forEach((resource) => included.set(`${resource.type}:${resource.id}`, resource));
  }

  return source.data.flatMap((value) => {
    if (!isRecord(value)) return [];
    const attributes = recordValue(value.attributes);
    const relationships = recordValue(value.relationships);
    const kafedraRelationship = recordValue(recordValue(relationships.kafedra).data);
    const kafedra = included.get(`${kafedraRelationship.type}:${kafedraRelationship.id}`) ?? {};
    const kafedraAttributes = recordValue(kafedra.attributes);
    const kafedraId = text(kafedraRelationship.id);

    return [{
      id: text(value.id),
      name: text(kafedraAttributes.name, kafedraId ? `Кафедра #${kafedraId}` : "Кафедра"),
      shortName: text(kafedraAttributes.short_name, "—"),
      employment: attributes.pochasovoy ? "Почасовая работа" : "Основное место работы",
    }];
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function recordValue(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function text(value: unknown, fallback = ""): string {
  return typeof value === "string" || typeof value === "number" ? String(value) : fallback;
}
