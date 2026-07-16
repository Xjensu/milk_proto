import { useState } from "react";
import { ArrowLeft, CalendarDays, ChevronRight, Database, GraduationCap, Search, ShieldCheck, Sparkles } from "lucide-react";

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

export function AdminPanel({ account, onLogout, onBack }: { account: Account | null; onLogout: () => void; onBack?: () => void }) {
  const [tab, setTab] = useState<AdminTab>("overview");
  const nav = [
    { id: "overview" as const, label: "Обзор", icon: <Sparkles size={16} /> },
    { id: "schedule" as const, label: "Расписание", icon: <CalendarDays size={16} /> },
    { id: "workload" as const, label: "Нагрузка", icon: <GraduationCap size={16} /> },
    { id: "data" as const, label: "Справочники", icon: <Database size={16} /> },
    { id: "calendar" as const, label: "Календарь", icon: <Search size={16} /> },
    ...(account?.role === "super_admin" ? [{ id: "accounts" as const, label: "Учётные записи", icon: <ShieldCheck size={16} /> }] : []),
  ];

  return <WorkspaceProvider><section className="admin-product-shell admin-layout" data-reveal>
    <aside className="admin-sidebar">
      <div className="sidebar-title"><span className="eyebrow">Университетская система</span><strong>Администрирование</strong></div>
      <button type="button" className="sidebar-back" onClick={() => onBack?.()}><ArrowLeft size={18} /> На главную</button>
      <nav aria-label="Разделы администрирования">{nav.map((item) => <button type="button" key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>{item.icon}<span>{item.label}</span>{tab === item.id ? <ChevronRight size={14} /> : null}</button>)}</nav>
      <div className="sidebar-bottom"><div className="sidebar-user"><span className="user-avatar">{(account?.email ?? "A").slice(0, 1).toUpperCase()}</span><div><strong>{account?.email ?? "Администратор"}</strong><small>{roleLabel(account?.role ?? "unknown")}</small></div></div><button type="button" className="sidebar-logout" onClick={onLogout}>Выйти</button></div>
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
