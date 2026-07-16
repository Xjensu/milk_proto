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
  { key: "background", label: "Background jobs", status: "degraded" },
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
    <div className="admin-section-heading"><div><span className="eyebrow">Operations</span><h1>System overview</h1></div><p>Live operational state and recent administrative activity.</p></div>

    <section className="service-strip" aria-labelledby="service-status-title">
      <div className="section-inline-title"><Server aria-hidden="true" size={19} /><h2 id="service-status-title">Service status</h2></div>
      <div className="service-list">{services.map((service) => <div className={`service-item status-${healthUnavailable ? "unavailable" : service.status}`} key={service.key}>
        <span className="status-dot" aria-hidden="true" /><strong>{service.label}</strong><span>{healthPending ? "Checking" : healthUnavailable ? "Unavailable" : statusLabel(service.status)}</span>
        {service.latencyMs !== undefined ? <small>{service.latencyMs} ms</small> : null}
      </div>)}</div>
    </section>

    <div className="overview-columns">
      <Panel className="overview-presence">
        <div className="panel-heading"><div className="section-inline-title"><CircleUserRound aria-hidden="true" size={20} /><div><span className="eyebrow">Presence</span><h2>Active users</h2></div></div><Badge>{users.length}</Badge></div>
        {usersPending ? <p className="resource-empty">Checking active sessions…</p> : usersUnavailable ? <CapabilityUnavailable label="Active-user presence" /> : users.length === 0 ? <p className="resource-empty">No active users reported.</p> : <div className="presence-list">{users.map((user) => <button type="button" className="presence-row" key={user.id} onClick={() => setSelected(userSelection(user))}>
          <span className="user-avatar">{user.email.slice(0, 1).toUpperCase()}</span><span><strong>{user.email}</strong><small>{user.role.replaceAll("_", " ")} · {user.area}</small></span><time dateTime={user.lastSeenAt}>{relativeTime(user.lastSeenAt)}</time>
        </button>)}</div>}
      </Panel>

      <Panel className="overview-journal">
        <div className="panel-heading"><div className="section-inline-title"><Activity aria-hidden="true" size={20} /><div><span className="eyebrow">Recent changes</span><h2>Event journal</h2></div></div><Radio aria-hidden="true" size={18} /></div>
        {auditPending ? <p className="resource-empty">Loading recent activity…</p> : auditUnavailable ? <CapabilityUnavailable label="Event journal" /> : audit.length === 0 ? <p className="resource-empty">No events found.</p> : <div className="journal-list">{audit.map((item) => <button type="button" className="journal-row" key={item.id} onClick={() => setSelected(auditSelection(item))}>
          <span className="activity-marker" aria-hidden="true" /><span><strong>{describeAuditEvent(item)}</strong><small>{auditContext(item)}</small></span><time dateTime={stringAttribute(item, "created_at")}>{formatDate(stringAttribute(item, "created_at"))}</time>
        </button>)}</div>}
      </Panel>
    </div>

    <Inspector title={selected?.title ?? "Details"} open={selected !== null} onClose={() => setSelected(null)}>
      <dl className="inspector-details">{selected?.rows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
    </Inspector>
  </div>;
}

export function describeAuditEvent(item: JsonApiResource) {
  const actor = stringAttribute(item, "actor_name") || stringAttribute(item, "actor_email") || "Administrator";
  const action = stringAttribute(item, "action") || "updated";
  const subject = stringAttribute(item, "subject_name") || humanize(stringAttribute(item, "entity_type") || "record");
  const group = stringAttribute(item, "group_display_name");
  return `${actor} ${action} ${subject}${group ? ` for ${group}` : ""}`;
}

function CapabilityUnavailable({ label }: { label: string }) {
  return <div className="capability-unavailable"><strong>{label} is unavailable</strong><p>The current gateway does not expose this projection. No substitute data is shown.</p></div>;
}

function statusLabel(status: ServiceStatus["status"]) { return status === "operational" ? "Operational" : status === "degraded" ? "Degraded" : "Unavailable"; }
function humanize(value: string) { return value.replaceAll("_", " "); }
function stringAttribute(item: JsonApiResource, key: string) { const value = item.attributes?.[key]; return typeof value === "string" || typeof value === "number" ? String(value) : ""; }
function auditContext(item: JsonApiResource) { return [humanize(stringAttribute(item, "entity_type")), stringAttribute(item, "area")].filter(Boolean).join(" · ") || "Administrative change"; }
function formatDate(value: string) { if (!value) return "Time unavailable"; return new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }
function relativeTime(value: string) { const seconds = Math.max(0, Math.round((Date.now() - new Date(value).getTime()) / 1000)); if (seconds < 60) return "Now"; if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`; return `${Math.floor(seconds / 3600)}h ago`; }
function userSelection(user: ActiveUserSummary): SelectedItem { return { title: user.email, rows: [["Role", humanize(user.role)], ["Current area", user.area], ["Last activity", formatDate(user.lastSeenAt)]] }; }
function auditSelection(item: JsonApiResource): SelectedItem { return { title: "Event details", rows: [["Description", describeAuditEvent(item)], ["Area", auditContext(item)], ["Occurred", formatDate(stringAttribute(item, "created_at"))]] }; }
