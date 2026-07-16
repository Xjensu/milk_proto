import { useEffect, useState } from "react";
import { Activity, CircleUserRound, Radio, Server } from "lucide-react";

import { Badge, Panel } from "../../components/ui";
import { api, type Account, type JsonApiResource } from "../../lib/api";
import type { ActiveUserSummary, ServiceStatus } from "./domain";
import { normalizeActiveUsers, normalizeServices } from "./overview/normalize";
import { Inspector } from "./workspace/Inspector";

type SelectedItem = { title: string; rows: Array<[string, string]> } | null;

const CHECKING_SERVICES: ServiceStatus[] = [
  { key: "api", label: "API", status: "degraded" },
  { key: "database", label: "Database", status: "degraded" },
  { key: "streaming", label: "Streaming", status: "degraded" },
  { key: "background", label: "Фоновые задачи", status: "degraded" },
];

export function AdminOverview({ account: _account, onOpenSchedule: _onOpenSchedule }: { account: Account | null; onOpenSchedule: () => void }) {
  const [services, setServices] = useState<ServiceStatus[]>(CHECKING_SERVICES);
  const [users, setUsers] = useState<ActiveUserSummary[]>([]);
  const [audit, setAudit] = useState<JsonApiResource[]>([]);
  const [healthPending, setHealthPending] = useState(true);
  const [usersPending, setUsersPending] = useState(true);
  const [auditPending, setAuditPending] = useState(true);
  const [healthUnavailable, setHealthUnavailable] = useState(false);
  const [usersUnavailable, setUsersUnavailable] = useState(false);
  const [auditUnavailable, setAuditUnavailable] = useState(false);
  const [selected, setSelected] = useState<SelectedItem>(null);

  useEffect(() => {
    let active = true;
    Promise.allSettled([
      api.dashboardHealth(),
      api.dashboardActiveUsers(),
      api.listAdminCollection("audit_logs", "?page=1&per_page=12"),
    ]).then(([healthResult, usersResult, auditResult]) => {
      if (!active) return;
      if (healthResult.status === "fulfilled") setServices(normalizeServices(healthResult.value));
      else setHealthUnavailable(true);
      if (usersResult.status === "fulfilled") setUsers(normalizeActiveUsers(usersResult.value));
      else setUsersUnavailable(true);
      if (auditResult.status === "fulfilled") setAudit(auditResult.value.data);
      else setAuditUnavailable(true);
      setHealthPending(false);
      setUsersPending(false);
      setAuditPending(false);
    });
    return () => { active = false; };
  }, []);

  return <div className="admin-section overview-workspace">
    <div className="admin-section-heading"><div><span className="eyebrow">Операции</span><h1>Обзор системы</h1></div><p>Текущее состояние сервисов и последние действия администраторов.</p></div>

    <section className="service-strip" aria-labelledby="service-status-title">
      <div className="section-inline-title"><Server aria-hidden="true" size={19} /><h2 id="service-status-title">Статус сервисов</h2></div>
      <div className="service-list">{services.map((service) => <div className={`service-item status-${healthUnavailable ? "unavailable" : service.status}`} key={service.key}>
        <span className="status-dot" aria-hidden="true" /><strong>{service.label}</strong><span>{healthPending ? "Проверка" : healthUnavailable ? "Недоступен" : statusLabel(service.status)}</span>
        {service.latencyMs !== undefined ? <small>{service.latencyMs} ms</small> : null}
      </div>)}</div>
    </section>

    <div className="overview-columns">
      <Panel className="overview-presence">
        <div className="panel-heading"><div className="section-inline-title"><CircleUserRound aria-hidden="true" size={20} /><div><span className="eyebrow">Присутствие</span><h2>Активные пользователи</h2></div></div><Badge>{users.length}</Badge></div>
        {usersPending ? <p className="resource-empty">Проверяем активные сессии…</p> : usersUnavailable ? <CapabilityUnavailable label="Список активных пользователей" /> : users.length === 0 ? <p className="resource-empty">Активных пользователей нет.</p> : <div className="presence-list">{users.map((user) => <button type="button" className="presence-row" key={user.id} onClick={() => setSelected(userSelection(user))}>
          <span className="user-avatar">{user.email.slice(0, 1).toUpperCase()}</span><span><strong>{user.email}</strong><small>{user.role.replaceAll("_", " ")} · {user.area}</small></span><time dateTime={user.lastSeenAt}>{relativeTime(user.lastSeenAt)}</time>
        </button>)}</div>}
      </Panel>

      <Panel className="overview-journal">
        <div className="panel-heading"><div className="section-inline-title"><Activity aria-hidden="true" size={20} /><div><span className="eyebrow">Последние изменения</span><h2>Журнал событий</h2></div></div><Radio aria-hidden="true" size={18} /></div>
        {auditPending ? <p className="resource-empty">Загружаем последние действия…</p> : auditUnavailable ? <CapabilityUnavailable label="Журнал событий" /> : audit.length === 0 ? <p className="resource-empty">Событий не найдено.</p> : <div className="journal-list">{audit.map((item) => <button type="button" className="journal-row" key={item.id} onClick={() => setSelected(auditSelection(item))}>
          <span className="activity-marker" aria-hidden="true" /><span><strong>{describeAuditEvent(item)}</strong><small>{auditContext(item)}</small></span><time dateTime={stringAttribute(item, "created_at")}>{formatDate(stringAttribute(item, "created_at"))}</time>
        </button>)}</div>}
      </Panel>
    </div>

    <Inspector title={selected?.title ?? "Подробности"} open={selected !== null} onClose={() => setSelected(null)}>
      <dl className="inspector-details">{selected?.rows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
    </Inspector>
  </div>;
}

export function describeAuditEvent(item: JsonApiResource) {
  const actor = stringAttribute(item, "actor_name") || stringAttribute(item, "actor_email") || "Администратор";
  const action = stringAttribute(item, "action") || "обновил";
  const subject = stringAttribute(item, "subject_name") || humanize(stringAttribute(item, "entity_type") || "запись");
  const group = stringAttribute(item, "group_display_name");
  return `${actor} ${action} ${subject}${group ? ` для ${group}` : ""}`;
}

function CapabilityUnavailable({ label }: { label: string }) {
  return <div className="capability-unavailable"><strong>{label}: недоступно</strong><p>Текущий шлюз не предоставляет это представление. Замещающие данные не отображаются.</p></div>;
}

function statusLabel(status: ServiceStatus["status"]) { return status === "operational" ? "Работает" : status === "degraded" ? "Ограниченно" : "Недоступен"; }
function humanize(value: string) { return value.replaceAll("_", " "); }
function stringAttribute(item: JsonApiResource, key: string) { const value = item.attributes?.[key]; return typeof value === "string" || typeof value === "number" ? String(value) : ""; }
function auditContext(item: JsonApiResource) { return [humanize(stringAttribute(item, "entity_type")), stringAttribute(item, "area")].filter(Boolean).join(" · ") || "Административное изменение"; }
function formatDate(value: string) { if (!value) return "Время не указано"; return new Intl.DateTimeFormat("ru", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }
function relativeTime(value: string) { const seconds = Math.max(0, Math.round((Date.now() - new Date(value).getTime()) / 1000)); if (seconds < 60) return "Сейчас"; if (seconds < 3600) return `${Math.floor(seconds / 60)} мин. назад`; return `${Math.floor(seconds / 3600)} ч. назад`; }
function userSelection(user: ActiveUserSummary): SelectedItem { return { title: user.email, rows: [["Роль", humanize(user.role)], ["Текущий раздел", user.area], ["Последняя активность", formatDate(user.lastSeenAt)]] }; }
function auditSelection(item: JsonApiResource): SelectedItem { return { title: "Подробности события", rows: [["Описание", describeAuditEvent(item)], ["Раздел", auditContext(item)], ["Время", formatDate(stringAttribute(item, "created_at"))]] }; }
