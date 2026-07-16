import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { CalendarDays, GraduationCap, ShieldCheck } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api } from "../../lib/api";
import { WeekGrid } from "../schedule/WeekGrid";
import { normalizeDepartments, normalizeTeacherLoad } from "./normalize";

type Tab = "schedule" | "load" | "kafedras";
type PayloadCache = Record<string, Record<string, unknown>>;

export function TeacherSelfService({ onLogout }: { onLogout: () => void }) {
  const [tab, setTab] = useState<Tab>("schedule");
  const [semesterId, setSemesterId] = useState("");
  const [semesters, setSemesters] = useState<Array<{ id: string; label: string }>>([]);
  const [payloads, setPayloads] = useState<PayloadCache>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [retryVersion, setRetryVersion] = useState(0);

  useEffect(() => {
    let active = true;
    api.listSemesters()
      .then((response) => {
        if (!active) return;
        setSemesters(response.data.map((item) => ({ id: item.id, label: semesterLabel(item.attributes) })));
      })
      .catch(() => active && setError("Не удалось загрузить список семестров."));
    return () => { active = false; };
  }, []);

  const payloadKey = tab === "kafedras" ? tab : `${tab}:${semesterId}`;
  const payload = payloads[payloadKey] ?? null;

  useEffect(() => {
    if ((tab === "schedule" || tab === "load") && !semesterId) return;
    if (payloads[payloadKey]) return;

    let active = true;
    setLoading(true);
    setError("");
    const request = tab === "schedule"
      ? api.meSchedule(semesterId)
      : tab === "load"
        ? api.meLoad(semesterId)
        : api.meKafedras();

    request
      .then((response) => active && setPayloads((current) => ({ ...current, [payloadKey]: response })))
      .catch(() => active && setError("Не удалось загрузить данные. Повторите попытку."))
      .finally(() => active && setLoading(false));
    return () => { active = false; };
  }, [payloadKey, payloads, retryVersion, semesterId, tab]);

  return <section className="self-service section-wrap">
    <div className="self-topline">
      <div>
        <span className="eyebrow">Личный кабинет преподавателя</span>
        <h1>Ваш учебный<br /><span>ритм.</span></h1>
        <p>Актуальные занятия, нагрузка и кафедры — напрямую из университетской системы.</p>
      </div>
      <Button className="button-outline" onClick={onLogout}>Выйти</Button>
    </div>

    <div className="self-toolbar">
      <label>Семестр<select value={semesterId} onChange={(event) => { setSemesterId(event.target.value); setError(""); }}><option value="">Выберите семестр</option>{semesters.map((semester) => <option value={semester.id} key={semester.id}>{semester.label}</option>)}</select></label>
    </div>

    <div className="self-tabs" role="tablist" aria-label="Разделы личного кабинета">
      <button role="tab" aria-selected={tab === "schedule"} className={tab === "schedule" ? "active" : ""} onClick={() => setTab("schedule")}><CalendarDays size={16} /> Моё расписание</button>
      <button role="tab" aria-selected={tab === "load"} className={tab === "load" ? "active" : ""} onClick={() => setTab("load")}><GraduationCap size={16} /> Моя нагрузка</button>
      <button role="tab" aria-selected={tab === "kafedras"} className={tab === "kafedras" ? "active" : ""} onClick={() => setTab("kafedras")}><ShieldCheck size={16} /> Мои кафедры</button>
    </div>

    <Panel className="self-panel">
      {(tab === "schedule" || tab === "load") && !semesterId
        ? <RequestState title="Выберите семестр" description="Расписание и нагрузка зависят от учебного периода." />
        : loading
          ? <RequestState title="Загружаем данные…" description="Получаем актуальную информацию из API." />
          : error
            ? <RequestState title="Данные недоступны" description={error} action={<Button className="button-outline" onClick={() => setRetryVersion((value) => value + 1)}>Повторить</Button>} />
            : <TeacherTabContent tab={tab} payload={payload} />}
    </Panel>
  </section>;
}

function TeacherTabContent({ tab, payload }: { tab: Tab; payload: Record<string, unknown> | null }) {
  if (tab === "schedule") return <WeekGrid source={payload} />;

  if (tab === "load") {
    const rows = payload ? normalizeTeacherLoad(payload) : [];
    if (rows.length === 0) return <RequestState title="Нагрузка не найдена" description="Для выбранного семестра нет назначенной нагрузки." />;
    return <div className="teacher-table-scroll"><table className="teacher-table"><thead><tr><th>Предмет</th><th>Вид занятия</th><th>Часы</th><th>Контекст</th><th>Аудитория</th></tr></thead><tbody>{rows.map((row) => <tr key={row.id}><td><strong>{row.subject}</strong><small>{row.teacher}</small></td><td>{row.activityType}</td><td>{row.hours}</td><td>{row.context}</td><td>{row.room}</td></tr>)}</tbody></table></div>;
  }

  const departments = payload ? normalizeDepartments(payload) : [];
  if (departments.length === 0) return <RequestState title="Кафедры не найдены" description="У преподавателя нет активных привязок к кафедрам." />;
  return <div className="department-list">{departments.map((department) => <article key={department.id}><span>{department.shortName}</span><div><strong>{department.name}</strong><small>{department.employment}</small></div></article>)}</div>;
}

function RequestState({ title, description, action }: { title: string; description: string; action?: ReactNode }) {
  return <div className="self-state"><h2>{title}</h2><p>{description}</p>{action}</div>;
}

function semesterLabel(attributes: Record<string, unknown> | undefined) {
  if (!attributes) return "Семестр";
  if (attributes.start_year && attributes.end_year) return `${attributes.start_year}/${attributes.end_year}${attributes.number ? ` · ${attributes.number} семестр` : ""}`;
  return String(attributes.name ?? attributes.number ?? "Семестр");
}
