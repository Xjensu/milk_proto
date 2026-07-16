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

type AdminTab = "overview" | "schedule" | "workload" | "data" | "calendar" | "accounts";

export function AdminPanel({ account, onLogout }: { account: Account | null; onLogout: () => void }) {
  const [tab, setTab] = useState<AdminTab>("overview");
  const nav = [
    { id: "overview" as const, label: "Обзор", icon: <Sparkles size={16} /> },
    { id: "schedule" as const, label: "Расписание", icon: <CalendarDays size={16} /> },
    { id: "workload" as const, label: "Нагрузка", icon: <GraduationCap size={16} /> },
    { id: "data" as const, label: "Справочники", icon: <Database size={16} /> },
    { id: "calendar" as const, label: "Календарь", icon: <Search size={16} /> },
    ...(account?.role === "super_admin" ? [{ id: "accounts" as const, label: "Пользователи", icon: <ShieldCheck size={16} /> }] : []),
  ];

  return <section className="admin-layout section-wrap" data-reveal>
    <aside className="admin-sidebar">
      <div className="sidebar-title"><span className="eyebrow">Рабочий контур</span><strong>Управление</strong></div>
      <nav aria-label="Разделы панели">{nav.map((item) => <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>{item.icon}<span>{item.label}</span>{tab === item.id ? <ChevronRight size={14} /> : null}</button>)}</nav>
      <div className="sidebar-bottom"><div className="sidebar-user"><span className="user-avatar">{(account?.email ?? "A").slice(0, 1).toUpperCase()}</span><div><strong>{account?.email ?? "Администратор"}</strong><small>{roleLabel(account?.role ?? "unknown")}</small></div></div><button className="sidebar-logout" onClick={onLogout}>Выйти</button></div>
    </aside>
    <div className="admin-content">
      {tab === "overview" ? <AdminOverview account={account} onOpenSchedule={() => setTab("schedule")} /> : null}
      {tab === "schedule" ? <AdminSchedule /> : null}
      {tab === "workload" ? <AdminWorkload /> : null}
      {tab === "data" ? <AdminResourcePage canWrite={account?.role === "super_admin"} /> : null}
      {tab === "calendar" ? <AdminOperations /> : null}
      {tab === "accounts" && account?.role === "super_admin" ? <AdminAccounts /> : null}
    </div>
  </section>;
}
