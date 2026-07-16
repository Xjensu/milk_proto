import { useState } from "react";
import { CalendarDays, ChevronRight, Database, GraduationCap, Search, ShieldCheck, Sparkles } from "lucide-react";

import type { Account } from "../../lib/api";
import { AdminAccounts } from "./AdminAccounts";
import { AdminOperations } from "./AdminOperations";
import { AdminOverview } from "./AdminOverview";
import { AdminResourcePage } from "./AdminResourcePage";
import { AdminSchedule } from "./AdminSchedule";
import { AdminWorkload } from "./AdminWorkload";
import { roleLabel } from "./utils";
import { WorkspaceProvider } from "./workspace/WorkspaceContext";

type AdminTab = "overview" | "schedule" | "workload" | "data" | "calendar" | "accounts";

export function AdminPanel({ account, onLogout }: { account: Account | null; onLogout: () => void }) {
  const [tab, setTab] = useState<AdminTab>("overview");
  const nav = [
    { id: "overview" as const, label: "Overview", icon: <Sparkles size={16} /> },
    { id: "schedule" as const, label: "Schedule", icon: <CalendarDays size={16} /> },
    { id: "workload" as const, label: "Workload", icon: <GraduationCap size={16} /> },
    { id: "data" as const, label: "Catalog", icon: <Database size={16} /> },
    { id: "calendar" as const, label: "Calendar", icon: <Search size={16} /> },
    ...(account?.role === "super_admin" ? [{ id: "accounts" as const, label: "Accounts", icon: <ShieldCheck size={16} /> }] : []),
  ];

  return <WorkspaceProvider><section className="admin-layout section-wrap" data-reveal>
    <aside className="admin-sidebar">
      <div className="sidebar-title"><span className="eyebrow">University workspace</span><strong>Administration</strong></div>
      <nav aria-label="Administration sections">{nav.map((item) => <button type="button" key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>{item.icon}<span>{item.label}</span>{tab === item.id ? <ChevronRight size={14} /> : null}</button>)}</nav>
      <div className="sidebar-bottom"><div className="sidebar-user"><span className="user-avatar">{(account?.email ?? "A").slice(0, 1).toUpperCase()}</span><div><strong>{account?.email ?? "Administrator"}</strong><small>{roleLabel(account?.role ?? "unknown")}</small></div></div><button type="button" className="sidebar-logout" onClick={onLogout}>Sign out</button></div>
    </aside>
    <div className="admin-content">
      {tab === "overview" ? <AdminOverview account={account} onOpenSchedule={() => setTab("schedule")} /> : null}
      {tab === "schedule" ? <AdminSchedule /> : null}
      {tab === "workload" ? <AdminWorkload /> : null}
      {tab === "data" ? <AdminResourcePage canWrite={account?.role === "super_admin"} /> : null}
      {tab === "calendar" ? <AdminOperations /> : null}
      {tab === "accounts" && account?.role === "super_admin" ? <AdminAccounts /> : null}
    </div>
  </section></WorkspaceProvider>;
}
