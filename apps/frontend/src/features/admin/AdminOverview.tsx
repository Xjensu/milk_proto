import { useEffect, useState } from "react";
import { ArrowUpRight, CalendarDays } from "lucide-react";

import { Badge, Button, Panel } from "../../components/ui";
import { api, type Account, type JsonApiResource } from "../../lib/api";
import { resourceLabel, roleLabel } from "./utils";

type OverviewState = { semesters: JsonApiResource[]; faculties: JsonApiResource[]; audit: JsonApiResource[]; media: JsonApiResource[] };

export function AdminOverview({ account, onOpenSchedule }: { account: Account | null; onOpenSchedule: () => void }) {
  const [data, setData] = useState<OverviewState>({ semesters: [], faculties: [], audit: [], media: [] });
  const [semesterId, setSemesterId] = useState("");
  const [loadCount, setLoadCount] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    Promise.allSettled([
      api.listAdminCollection("semesters"),
      api.listAdminCollection("faculties"),
      api.listAdminCollection("audit_logs", "?page=1&per_page=5"),
      api.listAdminCollection("media_files", "?page=1&per_page=20"),
    ]).then((results) => {
      if (!active) return;
      const collections = results.map((result) => result.status === "fulfilled" ? result.value.data : []);
      const next = { semesters: collections[0], faculties: collections[1], audit: collections[2], media: collections[3] };
      setData(next);
      setSemesterId((current) => current || next.semesters[0]?.id || "");
      if (results.every((result) => result.status === "rejected")) setError("Не удалось загрузить обзор системы.");
      setLoading(false);
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!semesterId) return;
    let active = true;
    api.loadOverview(semesterId)
      .then((response) => active && setLoadCount(response.data.length))
      .catch(() => active && setLoadCount(null));
    return () => { active = false; };
  }, [semesterId]);

  const cleanMedia = data.media.filter((item) => item.attributes?.status === "clean").length;

  return <div className="admin-section">
    <div className="admin-topline"><div><span className="eyebrow">Рабочий контур / Обзор</span><h1>Добрый день<span className="brand-dot">.</span></h1><p>Показатели ниже получены из API и отражают текущее состояние данных.</p></div><Badge tone="positive">{roleLabel(account?.role ?? "unknown")}</Badge></div>
    <div className="admin-context"><label><span>Семестр</span><select value={semesterId} onChange={(event) => setSemesterId(event.target.value)}><option value="">Не выбран</option>{data.semesters.map((item) => <option value={item.id} key={item.id}>{resourceLabel(item, "Семестр")}</option>)}</select></label><div><span>Статус данных</span><strong className={error ? "" : "status-ok"}>{loading ? "Загрузка…" : error || "API отвечает"}</strong></div></div>
    <div className="admin-live-metrics">
      <Metric label="Факультеты" value={loading ? "…" : String(data.faculties.length)} />
      <Metric label="Строки нагрузки" value={loadCount === null ? "—" : String(loadCount)} />
      <Metric label="Файлы обработаны" value={`${cleanMedia}/${data.media.length}`} />
      <Metric label="События аудита" value={loading ? "…" : String(data.audit.length)} />
    </div>
    <div className="admin-grid">
      <Panel className="admin-focus"><div className="panel-heading"><div><Badge tone="accent">Основная задача</Badge><h2>Проверить расписание выбранного семестра</h2></div><CalendarDays size={25} /></div><p>Откройте рабочую сетку и выберите группу, преподавателя или аудиторию.</p><Button className="button-primary" onClick={onOpenSchedule}>Открыть расписание <ArrowUpRight size={17} /></Button></Panel>
      <Panel className="activity-card"><div className="panel-heading"><div><span className="eyebrow">Последние изменения</span><h3>Журнал событий</h3></div></div>{data.audit.length === 0 ? <p className="resource-empty">Событий не найдено.</p> : data.audit.map((item) => <div className="activity-row" key={item.id}><span className="activity-marker marker-0" /><span>{String(item.attributes?.action ?? item.attributes?.entity_type ?? "Изменение")}</span><small>{formatDate(item.attributes?.created_at)}</small></div>)}</Panel>
    </div>
  </div>;
}

function Metric({ label, value }: { label: string; value: string }) { return <Panel className="live-metric"><span>{label}</span><strong>{value}</strong></Panel>; }

function formatDate(value: unknown) { if (typeof value !== "string") return "—"; return new Intl.DateTimeFormat("ru-RU", { dateStyle: "short", timeStyle: "short" }).format(new Date(value)); }
