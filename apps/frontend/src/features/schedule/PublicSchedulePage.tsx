import { useEffect, useState } from "react";

import { Button, Badge, Panel } from "../../components/ui";
import { api } from "../../lib/api";
import { WeekGrid } from "./WeekGrid";
import type { ScheduleSource } from "./types";

type Mode = "group" | "teacher" | "room";
type Option = { id: string; label: string };
type ApiItem = { id: string; attributes?: Record<string, unknown> };

const TITLES: Record<Mode, string> = {
  group: "Расписание группы",
  teacher: "Расписание преподавателя",
  room: "Занятость аудитории",
};

export function PublicSchedulePage({ mode, onBack }: { mode: Mode; onBack: () => void }) {
  const [selection, setSelection] = useState({ facultyId: "", semesterId: "", entityId: "" });
  const [faculties, setFaculties] = useState<Option[]>([]);
  const [semesters, setSemesters] = useState<Option[]>([]);
  const [entities, setEntities] = useState<Option[]>([]);
  const [result, setResult] = useState<ScheduleSource>(null);
  const [referenceLoading, setReferenceLoading] = useState(true);
  const [scheduleLoading, setScheduleLoading] = useState(false);
  const [error, setError] = useState("");
  const [referenceVersion, setReferenceVersion] = useState(0);

  useEffect(() => {
    let active = true;
    setReferenceLoading(true);
    setError("");

    const entityRequest = mode === "teacher"
      ? api.listPublicTeachers()
      : mode === "room"
        ? api.listPublicRooms()
        : Promise.resolve({ data: [] as ApiItem[] });

    Promise.all([api.listFaculties(), api.listSemesters(), entityRequest])
      .then(([facultyResponse, semesterResponse, entityResponse]) => {
        if (!active) return;
        setFaculties(toOptions(facultyResponse.data, "Факультет"));
        setSemesters(toOptions(semesterResponse.data, "Семестр"));
        setEntities(toOptions(entityResponse.data, mode === "teacher" ? "Преподаватель" : "Аудитория"));
      })
      .catch(() => active && setError("Не удалось загрузить данные для выбора."))
      .finally(() => active && setReferenceLoading(false));

    return () => { active = false; };
  }, [mode, referenceVersion]);

  useEffect(() => {
    if (mode !== "group" || !selection.facultyId || !selection.semesterId) {
      if (mode === "group") setEntities([]);
      return;
    }

    let active = true;
    setReferenceLoading(true);
    api.listPublicGroups(selection.facultyId, selection.semesterId)
      .then((response) => active && setEntities(toOptions(response.data, "Группа")))
      .catch(() => active && setError("Не удалось загрузить группы."))
      .finally(() => active && setReferenceLoading(false));
    return () => { active = false; };
  }, [mode, selection.facultyId, selection.semesterId]);

  const canLoad = Boolean(selection.entityId && selection.semesterId && (mode !== "group" || selection.facultyId));

  async function loadSchedule() {
    if (!canLoad) return;
    setScheduleLoading(true);
    setError("");
    try {
      const response = mode === "group"
        ? await api.groupSchedule(selection.entityId)
        : mode === "teacher"
          ? await api.teacherSchedule(selection.entityId, selection.semesterId)
          : await api.roomSchedule(selection.entityId, selection.semesterId);
      setResult(response);
    } catch {
      setError("Не удалось загрузить расписание. Повторите попытку.");
    } finally {
      setScheduleLoading(false);
    }
  }

  return <section className="public-schedule section-wrap" data-reveal>
    <div className="schedule-heading">
      <div>
        <button className="text-link" onClick={onBack}>← На главную</button>
        <span className="eyebrow">Публичное расписание</span>
        <h1>{TITLES[mode]}<span>.</span></h1>
        <p>Выберите параметры — расписание будет загружено из актуальных данных университета.</p>
      </div>
      <Badge tone="positive">Без входа</Badge>
    </div>

    <Panel className="public-selector">
      <div className="public-selector-grid">
        {mode === "group" ? <Selector label="Факультет" value={selection.facultyId} options={faculties} placeholder="Выберите факультет" onChange={(facultyId) => {
          setSelection({ facultyId, semesterId: "", entityId: "" });
          setResult(null);
        }} /> : null}
        <Selector label="Семестр" value={selection.semesterId} options={semesters} placeholder="Выберите семестр" onChange={(semesterId) => {
          setSelection((current) => ({ ...current, semesterId, entityId: mode === "group" ? "" : current.entityId }));
          setResult(null);
        }} />
        <Selector label={mode === "group" ? "Группа" : mode === "teacher" ? "Преподаватель" : "Аудитория"} value={selection.entityId} options={entities} placeholder="Выберите вариант" onChange={(entityId) => {
          setSelection((current) => ({ ...current, entityId }));
          setResult(null);
        }} />
        <Button className="button-primary" onClick={loadSchedule} disabled={!canLoad || scheduleLoading}>
          {scheduleLoading ? "Загружаем…" : "Показать"}
        </Button>
      </div>
      {referenceLoading ? <p className="request-status" role="status">Загружаем варианты…</p> : null}
      {error ? <div className="form-error request-error" role="alert"><span>{error}</span><Button className="button-outline" onClick={() => setReferenceVersion((value) => value + 1)}>Повторить</Button></div> : null}
    </Panel>

    <Panel className="schedule-result"><WeekGrid source={result} /></Panel>
  </section>;
}

function Selector({ label, value, options, placeholder, onChange }: { label: string; value: string; options: Option[]; placeholder: string; onChange: (value: string) => void }) {
  return <label>{label}<select value={value} onChange={(event) => onChange(event.target.value)}><option value="">{placeholder}</option>{options.map((option) => <option value={option.id} key={option.id}>{option.label}</option>)}</select></label>;
}

function toOptions(items: ApiItem[], fallback: string): Option[] {
  return items.map((item) => ({ id: item.id, label: displayLabel(item.attributes, fallback) }));
}

function displayLabel(attributes: Record<string, unknown> | undefined, fallback: string): string {
  if (!attributes) return fallback;
  const fullName = [attributes.last_name, attributes.first_name, attributes.middle_name].filter(Boolean).join(" ");
  const semester = attributes.start_year && attributes.end_year
    ? `${attributes.start_year}/${attributes.end_year}${attributes.number ? ` · ${attributes.number} семестр` : ""}`
    : "";
  const value = [attributes.display_name, attributes.name, attributes.full_name, fullName, semester, attributes.short_name]
    .find((candidate) => typeof candidate === "string" && candidate.length > 0);
  return String(value ?? fallback);
}
