import { useEffect, useState } from "react";

import { Button, Panel } from "../../components/ui";
import { api, type JsonApiResource } from "../../lib/api";
import { WeekGrid } from "../schedule/WeekGrid";
import type { ScheduleSource } from "../schedule/types";

type Mode = "group" | "teacher" | "room";

export function AdminSchedule() {
  const [mode, setMode] = useState<Mode>("group");
  const [semesters, setSemesters] = useState<JsonApiResource[]>([]);
  const [entities, setEntities] = useState<JsonApiResource[]>([]);
  const [semesterId, setSemesterId] = useState("");
  const [entityId, setEntityId] = useState("");
  const [result, setResult] = useState<ScheduleSource>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    setLoading(true);
    setEntityId("");
    setResult(null);
    const resource = mode === "group" ? "student_groups" : mode === "teacher" ? "teachers" : "rooms";
    Promise.all([api.listAdminCollection("semesters"), api.listAdminCollection(resource)])
      .then(([semesterResponse, entityResponse]) => {
        if (!active) return;
        setSemesters(semesterResponse.data);
        setEntities(entityResponse.data);
      })
      .catch(() => active && setError("Не удалось загрузить параметры расписания."))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [mode]);

  async function loadSchedule() {
    if (!semesterId || !entityId) return;
    setLoading(true);
    setError("");
    try {
      let response: Record<string, unknown>;
      if (mode === "group") {
        const groupSemesters = await api.studentGroupSemesters(entityId);
        const groupSemester = groupSemesters.data.find((item) => relationshipId(item, "semester") === semesterId);
        response = groupSemester ? await api.adminGroupSchedule(groupSemester.id) : { data: [] };
      } else if (mode === "teacher") {
        response = await api.adminTeacherSchedule(entityId, semesterId);
      } else {
        response = await api.adminRoomSchedule(entityId, semesterId);
      }
      setResult(response);
    } catch {
      setError("Не удалось загрузить рабочую сетку.");
    } finally {
      setLoading(false);
    }
  }

  return <div className="admin-section">
    <SectionHeading title="Рабочая сетка расписания" description="Выберите семестр и сущность — данные загружаются из защищённых проекций API." />
    <div className="admin-tabs" role="tablist">{(["group", "teacher", "room"] as Mode[]).map((item) => <button role="tab" aria-selected={mode === item} className={mode === item ? "active" : ""} onClick={() => setMode(item)} key={item}>{item === "group" ? "Группа" : item === "teacher" ? "Преподаватель" : "Аудитория"}</button>)}</div>
    <Panel className="admin-query-panel">
      <div className="selector-row">
        <label>Семестр<select value={semesterId} onChange={(event) => { setSemesterId(event.target.value); setResult(null); }}><option value="">Выберите семестр</option>{semesters.map((item) => <option value={item.id} key={item.id}>{label(item, "Семестр")}</option>)}</select></label>
        <label>{mode === "group" ? "Группа" : mode === "teacher" ? "Преподаватель" : "Аудитория"}<select value={entityId} onChange={(event) => { setEntityId(event.target.value); setResult(null); }}><option value="">Выберите вариант</option>{entities.map((item) => <option value={item.id} key={item.id}>{label(item, mode === "group" ? "Группа" : mode === "teacher" ? "Преподаватель" : "Аудитория")}</option>)}</select></label>
        <Button className="button-primary" onClick={loadSchedule} disabled={loading || !semesterId || !entityId}>{loading ? "Загрузка…" : "Показать"}</Button>
      </div>
      {error ? <div className="form-error" role="alert">{error}</div> : null}
    </Panel>
    <Panel className="schedule-result"><WeekGrid source={result} /></Panel>
  </div>;
}

function SectionHeading({ title, description }: { title: string; description: string }) { return <div className="admin-section-heading"><div><span className="eyebrow">Расписание</span><h2>{title}</h2></div><p>{description}</p></div>; }

function relationshipId(resource: JsonApiResource, name: string) {
  const relation = resource.relationships?.[name] as { data?: { id?: string } } | undefined;
  return relation?.data?.id ?? "";
}

function label(resource: JsonApiResource, fallback: string) {
  const attributes = resource.attributes ?? {};
  const fullName = [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
  if (attributes.start_year && attributes.end_year) return `${attributes.start_year}/${attributes.end_year}${attributes.number ? ` · ${attributes.number}` : ""}`;
  const value = [attributes.display_name, attributes.name, attributes.full_name, fullName, attributes.number]
    .find((candidate) => typeof candidate === "string" && candidate.length > 0 || typeof candidate === "number");
  return String(value ?? `${fallback} #${resource.id}`);
}
