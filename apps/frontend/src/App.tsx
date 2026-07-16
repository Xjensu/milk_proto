import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { ArrowUpRight, CalendarDays, ChevronRight, GraduationCap, LogIn, MapPin, Menu, Search, ShieldCheck, Sparkles, X } from "lucide-react";
import gsap from "gsap";
import { accountFromAccessToken, api, type Account, type AccountRole } from "./lib/api";
import { Badge, Button, Panel } from "./components/ui";
import { PublicSchedulePage as ApiPublicSchedulePage } from "./features/schedule/PublicSchedulePage";
import { TeacherSelfService as ApiTeacherSelfService } from "./features/teacher/TeacherSelfService";
import { AdminPanel as ApiAdminPanel } from "./features/admin/AdminPanel";
import { PasswordChange } from "./features/auth/PasswordChange";
import { staffView } from "./lib/navigation";
import { devAdminPreview } from "./lib/devPreview";

type View = "home" | "login" | "password" | "admin" | "self" | "group" | "teacher" | "room";

const sampleSchedule = [
  { time: "08:30", title: "Математический анализ", meta: "Лекция · 204 · Корпус A", tone: "cyan" },
  { time: "10:10", title: "Программирование", meta: "Практика · 312 · Корпус B", tone: "lime" },
  { time: "11:45", title: "История науки", meta: "Семинар · 108 · Корпус A", tone: "blue" },
];

function App() {
  const previewAdmin = devAdminPreview(import.meta.env.DEV, window.location.search);
  const [view, setView] = useState<View>(previewAdmin ? "admin" : "home");
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [account, setAccount] = useState<Account | null>(() => { if (previewAdmin) return { id: "preview", email: "admin@university.test", role: "super_admin", status: "active" }; const token = sessionStorage.getItem("milk_schedule_access_token"); const email = sessionStorage.getItem("milk_schedule_email"); return token && email ? accountFromAccessToken(token, email) : null; });
  const stageRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!stageRef.current) return;
    const nodes = stageRef.current.querySelectorAll("[data-reveal]");
    const context = gsap.context(() => {
      gsap.fromTo(nodes, { autoAlpha: 0, y: 18 }, { autoAlpha: 1, y: 0, duration: 0.7, stagger: 0.08, ease: "power3.out" });
    }, stageRef);
    return () => context.revert();
  }, [view]);

  const currentLabel = useMemo(() => account ? `Роль: ${roleLabel(account.role)}` : "Открыто для всех", [account]);

  function handleLogin(nextAccount: Account) {
    setAccount(nextAccount);
    setView(staffView(nextAccount));
  }

  async function handleLogout() {
    const refreshToken = sessionStorage.getItem("milk_schedule_refresh_token");
    if (refreshToken) { try { await api.logout(refreshToken); } catch { /* Clear local session even when the API is unavailable. */ } }
    sessionStorage.removeItem("milk_schedule_access_token");
    sessionStorage.removeItem("milk_schedule_refresh_token");
    sessionStorage.removeItem("milk_schedule_email");
    setAccount(null);
    setView("home");
  }

  return (
    <div className={`app-shell ${view === "admin" ? "admin-app-shell" : ""}`} ref={stageRef}>
      <div className="ambient ambient-one" />
      <div className="ambient ambient-two" />
      {view !== "admin" ? <Header account={account} currentLabel={currentLabel} onLogin={() => setView(staffView(account))} onHome={() => setView("home")} onMenu={() => setMobileMenuOpen((value) => !value)} /> : null}
      {mobileMenuOpen && <MobileMenu onClose={() => setMobileMenuOpen(false)} onLogin={() => { setMobileMenuOpen(false); setView("login"); }} />}
      <main>
        {view === "home" && <Home onLogin={() => setView(staffView(account))} onOpenSchedule={(mode) => setView(mode)} />}
        {view === "login" && <Login onBack={() => setView("home")} onLoggedIn={handleLogin} />}
        {view === "password" && account ? <PasswordChange email={account.email} onChanged={handleLogin} onLogout={handleLogout} /> : null}
        {view === "admin" && <ApiAdminPanel account={account} onLogout={handleLogout} />}
        {view === "self" && <ApiTeacherSelfService onLogout={handleLogout} />}
        {(view === "group" || view === "teacher" || view === "room") && <ApiPublicSchedulePage mode={view} onBack={() => setView("home")} />}
      </main>
      <footer className="site-footer"><span>Milk Schedule · 2026</span><span>Расписание, которое держит ритм университета</span></footer>
    </div>
  );
}

function Header({ account, currentLabel, onLogin, onHome, onMenu }: { account: Account | null; currentLabel: string; onLogin: () => void; onHome: () => void; onMenu: () => void }) {
  return <header className="site-header">
    <button className="brand" onClick={onHome} aria-label="На главную"><span className="brand-mark"><span /></span><span>milk<span className="brand-dot">.</span>schedule</span></button>
    <nav className="desktop-nav" aria-label="Основная навигация"><button onClick={onHome}>Расписание</button><button onClick={onHome}>Возможности</button><button onClick={onHome}>О системе</button></nav>
    <div className="header-actions"><Badge tone="positive">{currentLabel}</Badge>{account ? <Button className="button-quiet" onClick={onLogin}>Панель</Button> : <Button className="button-outline" onClick={onLogin}><LogIn size={16} /> Войти</Button>}<button className="menu-button" onClick={onMenu} aria-label="Открыть меню"><Menu size={21} /></button></div>
  </header>;
}

function MobileMenu({ onClose, onLogin }: { onClose: () => void; onLogin: () => void }) {
  return <div className="mobile-menu"><button onClick={onClose} className="close-button" aria-label="Закрыть меню"><X size={20} /></button><button onClick={onClose}>Расписание</button><button onClick={onClose}>Возможности</button><button onClick={onLogin}>Войти в систему <ArrowUpRight size={18} /></button></div>;
}

function Home({ onLogin, onOpenSchedule }: { onLogin: () => void; onOpenSchedule: (mode: "group" | "teacher" | "room") => void }) {
  return <div className="home-page">
    <section className="hero section-wrap"><div className="hero-copy" data-reveal><Badge tone="accent"><Sparkles size={14} /> Умная система расписаний</Badge><h1>Ритм университета<br /><span>в одном экране.</span></h1><p className="hero-lede">Находите занятия, управляйте нагрузкой и собирайте расписание без лишних таблиц и ручной рутины.</p><div className="hero-actions"><Button className="button-primary" onClick={() => document.getElementById("finder")?.scrollIntoView({ behavior: "smooth" })}>Открыть расписание <ArrowUpRight size={18} /></Button><button className="text-link" onClick={onLogin}>Для сотрудников <ChevronRight size={17} /></button></div><div className="hero-stats"><span><strong>24/7</strong><small>доступ к расписанию</small></span><span><strong>1 экран</strong><small>для всех ролей</small></span><span><strong>0 хаоса</strong><small>в учебном ритме</small></span></div></div><div className="hero-visual" data-reveal><div className="orbit orbit-one" /><div className="orbit orbit-two" /><div className="visual-card"><div className="visual-card-top"><span>Недельный обзор</span><span className="live-dot">● LIVE</span></div><div className="mini-calendar"><span className="mini-day active">ПН<small>14</small></span><span className="mini-day">ВТ<small>15</small></span><span className="mini-day">СР<small>16</small></span><span className="mini-day">ЧТ<small>17</small></span><span className="mini-day">ПТ<small>18</small></span></div>{sampleSchedule.map((item) => <div className={`mini-event event-${item.tone}`} key={item.time}><span>{item.time}</span><div><strong>{item.title}</strong><small>{item.meta}</small></div></div>)}</div><div className="floating-chip chip-top"><CalendarDays size={15} /><span>Семестр 2026</span></div><div className="floating-chip chip-bottom"><span className="signal-icon">◌</span><span>Система в норме</span></div></div></section>
    <section className="finder section-wrap" id="finder" data-reveal><div className="section-heading"><div><Badge>Быстрый доступ</Badge><h2>Что ищем сегодня?</h2></div><p>Выберите сценарий — нужный экран откроется за несколько секунд.</p></div><div className="finder-grid"><FinderCard icon={<GraduationCap />} title="Группа" description="Расписание вашей группы" onClick={() => onOpenSchedule("group")} /><FinderCard icon={<ShieldCheck />} title="Преподаватель" description="Занятия и нагрузка преподавателя" onClick={() => onOpenSchedule("teacher")} /><FinderCard icon={<MapPin />} title="Аудитория" description="Свободные окна и занятость" onClick={() => onOpenSchedule("room")} /></div></section>
    <section className="proof section-wrap" data-reveal><Panel className="proof-panel"><div><Badge tone="positive">Собрано для университета</Badge><h2>Сложное становится<br /><span>очевидным.</span></h2></div><div className="proof-grid"><div><strong>01</strong><span>Единая точка правды для расписания</span></div><div><strong>02</strong><span>Умные рекомендации для диспетчера</span></div><div><strong>03</strong><span>Понятный интерфейс для каждого</span></div></div></Panel></section>
  </div>;
}

function FinderCard({ icon, title, description, onClick }: { icon: ReactNode; title: string; description: string; onClick: () => void }) {
  return <button className="finder-card" onClick={onClick}><span className="finder-icon">{icon}</span><span><strong>{title}</strong><small>{description}</small></span><ArrowUpRight size={18} className="finder-arrow" /></button>;
}

function PublicSchedulePage({ mode, onBack }: { mode: "group" | "teacher" | "room"; onBack: () => void }) {
  const [selection, setSelection] = useState({ facultyId: "", semesterId: "", entityId: "" });
  const [faculties, setFaculties] = useState<Array<{ id: string; label: string }>>([]);
  const [semesters, setSemesters] = useState<Array<{ id: string; label: string }>>([]);
  const [entities, setEntities] = useState<Array<{ id: string; label: string }>>([]);
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const title = mode === "group" ? "Расписание группы" : mode === "teacher" ? "Расписание преподавателя" : "Занятость аудитории";

  useEffect(() => { api.listFaculties().then((response) => setFaculties(response.data.map((item) => ({ id: item.id, label: displayAttributes(item.attributes, "Факультет") })))).catch(() => setError("Не удалось загрузить факультеты")); api.listSemesters().then((response) => setSemesters(response.data.map((item) => ({ id: item.id, label: displayAttributes(item.attributes, "Семестр") })))).catch(() => setError("Не удалось загрузить семестры")); if (mode === "teacher") api.listPublicTeachers().then((response) => setEntities(response.data.map((item) => ({ id: item.id, label: displayAttributes(item.attributes, "Преподаватель") })))).catch(() => setError("Не удалось загрузить преподавателей")); if (mode === "room") api.listPublicRooms().then((response) => setEntities(response.data.map((item) => ({ id: item.id, label: displayAttributes(item.attributes, "Аудитория") })))).catch(() => setError("Не удалось загрузить аудитории")); }, [mode]);
  useEffect(() => { if (mode === "group" && selection.facultyId && selection.semesterId) api.listPublicGroups(selection.facultyId, selection.semesterId).then((response) => setEntities(response.data.map((item) => ({ id: item.id, label: displayAttributes(item.attributes, "Группа") })))).catch(() => setError("Не удалось загрузить группы")); }, [mode, selection.facultyId, selection.semesterId]);

  async function loadSchedule() { if (!selection.entityId) return; setLoading(true); setError(""); try { const response = mode === "group" ? await api.groupSchedule(selection.entityId) : mode === "teacher" ? await api.teacherSchedule(selection.entityId, selection.semesterId) : await api.roomSchedule(selection.entityId); setResult(response); } catch (requestError) { setError(requestError instanceof Error ? requestError.message : "Не удалось загрузить расписание"); } finally { setLoading(false); } }
  return <section className="public-schedule section-wrap" data-reveal><div className="schedule-heading"><div><button className="text-link" onClick={onBack}>← На главную</button><span className="eyebrow">Публичный портал / {mode}</span><h1>{title}<span>.</span></h1><p>Выберите параметры — данные будут загружены из публичного API без авторизации.</p></div><Badge tone="positive">Без входа</Badge></div><Panel className="public-selector"><div className="public-selector-grid">{mode === "group" && <label>Факультет<select value={selection.facultyId} onChange={(event) => setSelection((current) => ({ ...current, facultyId: event.target.value, entityId: "" }))}><option value="">Выберите факультет</option>{faculties.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>}<label>Семестр<select value={selection.semesterId} onChange={(event) => setSelection((current) => ({ ...current, semesterId: event.target.value }))}><option value="">Выберите семестр</option>{semesters.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label><label>{mode === "group" ? "Группа" : mode === "teacher" ? "Преподаватель" : "Аудитория"}<select value={selection.entityId} onChange={(event) => setSelection((current) => ({ ...current, entityId: event.target.value }))}><option value="">Выберите вариант</option>{entities.map((entity) => <option key={entity.id} value={entity.id}>{entity.label}</option>)}</select></label><Button className="button-primary" onClick={loadSchedule} disabled={loading || !selection.entityId}>{loading ? "Загружаем…" : "Показать"}</Button></div>{error && <div className="form-error" role="alert">{error}</div>}</Panel><ScheduleResult result={result} /></section>;
}

type ScheduleEvent = Record<string, unknown>;
type ScheduleDay = { key: string; label: string; events: ScheduleEvent[] };

function ScheduleResult({ result }: { result: Record<string, unknown> | null }) { const days = normalizeSchedule(result); return <Panel className="schedule-result">{!result ? <><div className="empty-orbit"><CalendarDays size={28} /></div><h2>Ваше расписание появится здесь</h2><p>Сетка сохраняет привычную структуру: день, время, предмет, преподаватель и аудитория.</p></> : days.length === 0 ? <><h2>Занятий не найдено</h2><p>Для выбранных параметров API не вернуло событий на текущий период.</p></> : <div className="schedule-grid">{days.map((day) => <div className="schedule-day" key={day.key}><div className="schedule-day-heading"><span>{day.label}</span><small>{day.events.length} {pluralize(day.events.length, "занятие", "занятия", "занятий")}</small></div>{day.events.map((event, index) => <div className={`schedule-event ${isChanged(event) ? "schedule-event-changed" : ""}`} key={`${day.key}-${index}`}><div className="schedule-event-time"><strong>{readTime(event)}</strong><small>{readEndTime(event)}</small></div><div className="schedule-event-main"><strong>{readText(event, "subject", "Занятие")}</strong><span>{readText(event, "activity_type", "Занятие")}</span><small>{readText(event, "room", "Аудитория не указана")} · {readGroupOrTeacher(event)}</small></div>{isChanged(event) && <Badge tone="accent">Изменено</Badge>}</div>)}</div>)}</div>}</Panel>; }

function displayAttributes(attributes: Record<string, unknown> | undefined, fallback: string) { if (!attributes) return fallback; return String(attributes.name ?? attributes.full_name ?? ([attributes.last_name, attributes.first_name].filter(Boolean).join(" ") || fallback)); }
function normalizeSchedule(result: Record<string, unknown> | null): ScheduleDay[] { if (!result) return []; const data = result.data; if (data && !Array.isArray(data) && typeof data === "object" && "days" in data) return normalizeLegacyDays((data as { days?: unknown[] }).days); if (Array.isArray(data)) { const grouped = new Map<string, ScheduleDay>(); for (const item of data) { const record = item as Record<string, unknown>; const attributes = (record.attributes ?? record) as Record<string, unknown>; const day = String(attributes.day_of_week ?? "0"); const existing = grouped.get(day) ?? { key: day, label: dayLabel(day), events: [] }; existing.events.push({ ...attributes, id: record.id }); grouped.set(day, existing); } return [...grouped.values()].sort((a, b) => Number(a.key) - Number(b.key)); } if (Array.isArray(result.days)) return normalizeLegacyDays(result.days); return []; }
function normalizeLegacyDays(days: unknown[] | undefined): ScheduleDay[] { return (days ?? []).map((rawDay) => { const day = rawDay as { date?: string; day_of_week?: number; events?: ScheduleEvent[] }; return { key: String(day.date ?? day.day_of_week ?? Math.random()), label: day.date ?? dayLabel(String(day.day_of_week ?? "")), events: day.events ?? [] }; }); }
function dayLabel(day: string) { return ({ "1": "Понедельник", "2": "Вторник", "3": "Среда", "4": "Четверг", "5": "Пятница", "6": "Суббота", "7": "Воскресенье" } as Record<string, string>)[day] ?? "День"; }
function readTime(event: ScheduleEvent) { const timeSlot = event.time_slot as Record<string, unknown> | undefined; return String(event.time_slot_start_time ?? timeSlot?.start_time ?? "—"); }
function readEndTime(event: ScheduleEvent) { const timeSlot = event.time_slot as Record<string, unknown> | undefined; return String(event.time_slot_end_time ?? timeSlot?.end_time ?? ""); }
function readText(event: ScheduleEvent, key: string, fallback: string) { const liveKey = key === "subject" ? "subject_name" : key === "room" ? "room_name" : key === "activity_type" ? "activity_type_name" : key; const value = event[key] as Record<string, unknown> | undefined; return String(event[liveKey] ?? value?.name ?? value?.short_name ?? event[key] ?? fallback); }
function readGroupOrTeacher(event: ScheduleEvent) { return String(event.group_display_name ?? event.student_group ?? (Array.isArray(event.teachers) ? (event.teachers[0] as Record<string, unknown> | undefined)?.full_name : undefined) ?? ""); }
function isChanged(event: ScheduleEvent) { return Boolean(event.is_transferred ?? event.is_manual_change ?? event.status === "transferred"); }
function pluralize(value: number, one: string, few: string, many: string) { const mod = value % 100; if (mod >= 11 && mod <= 14) return many; const last = value % 10; return last === 1 ? one : last >= 2 && last <= 4 ? few : many; }

function TeacherSelfService({ onLogout }: { onLogout: () => void }) {
  const [tab, setTab] = useState<"schedule" | "load" | "kafedras">("schedule");
  const [payload, setPayload] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  useEffect(() => { setLoading(true); setError(""); const load = tab === "schedule" ? api.meSchedule() : tab === "load" ? api.meLoad() : api.meKafedras(); load.then(setPayload).catch((requestError) => setError(requestError instanceof Error ? requestError.message : "Не удалось загрузить данные")).finally(() => setLoading(false)); }, [tab]);
  return <section className="self-service section-wrap"><div className="self-topline"><div><span className="eyebrow">Личный кабинет преподавателя</span><h1>Ваш учебный<br /><span>ритм.</span></h1><p>Только ваши занятия, нагрузка и кафедры — без административного шума.</p></div><Button className="button-outline" onClick={onLogout}>Выйти</Button></div><div className="self-tabs"><button className={tab === "schedule" ? "active" : ""} onClick={() => setTab("schedule")}><CalendarDays size={16} /> Моё расписание</button><button className={tab === "load" ? "active" : ""} onClick={() => setTab("load")}><GraduationCap size={16} /> Моя нагрузка</button><button className={tab === "kafedras" ? "active" : ""} onClick={() => setTab("kafedras")}><ShieldCheck size={16} /> Мои кафедры</button></div><Panel className="self-panel">{loading ? <div className="self-state">Загружаем данные…</div> : error ? <div className="form-error" role="alert">{error}</div> : <SelfServiceContent tab={tab} payload={payload} />}</Panel></section>;
}

function SelfServiceContent({ tab, payload }: { tab: "schedule" | "load" | "kafedras"; payload: Record<string, unknown> | null }) { if (tab === "schedule") return <ScheduleResult result={payload} />; const data = payload?.data; return <><div className="self-state"><div className="empty-orbit"><Sparkles size={24} /></div><h2>{tab === "load" ? "Нагрузка загружена" : "Кафедры загружены"}</h2><p>Данные получены из `/admin/me/{tab}` и готовы к дальнейшему представлению.</p></div><pre className="self-json">{JSON.stringify(data ?? payload ?? {}, null, 2)}</pre></>; }

function Login({ onBack, onLoggedIn }: { onBack: () => void; onLoggedIn: (account: Account) => void }) {
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setError(""); setLoading(true);
    try {
      const response = await api.login(email, password);
      sessionStorage.setItem("milk_schedule_access_token", response.access_token);
      sessionStorage.setItem("milk_schedule_refresh_token", response.refresh_token);
      sessionStorage.setItem("milk_schedule_email", email);
      onLoggedIn(accountFromAccessToken(response.access_token, email));
    } catch (requestError) {
      setError(requestError instanceof Error ? requestError.message : "Не удалось войти");
    } finally { setLoading(false); }
  }

  return <section className="auth-page section-wrap"><div className="auth-intro" data-reveal><Badge tone="accent"><ShieldCheck size={14} /> Защищённый доступ</Badge><h1>Вход в рабочий<br /><span>контур.</span></h1><p>Управляйте учебным ритмом университета из одной понятной панели.</p><button className="text-link" onClick={onBack}>← Вернуться к расписанию</button></div><Panel className="auth-card" data-reveal><div className="auth-card-heading"><span className="eyebrow">Milk Schedule / Staff</span><h2>Добро пожаловать</h2><p>Войдите под своей рабочей учётной записью.</p></div><form onSubmit={submit}><label>Email<input type="email" value={email} onChange={(event) => setEmail(event.target.value)} autoComplete="username" required /></label><label>Пароль<input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" required /></label>{error && <div className="form-error" role="alert">{error}</div>}<Button className="button-primary button-full" type="submit" disabled={loading}>{loading ? "Проверяем…" : "Войти в систему"}<ArrowUpRight size={17} /></Button></form><div className="auth-note"><span className="signal-icon">●</span><span>Сессия защищена токенами API</span></div></Panel></section>;
}

type AdminTab = "overview" | "schedule" | "workload" | "data" | "calendar" | "accounts";

function AdminOverview({ account, onLogout }: { account: Account | null; onLogout: () => void }) {
  const [tab, setTab] = useState<AdminTab>("overview");
  const canManageAccounts = account?.role === "super_admin";
  const nav = [
    { id: "overview" as const, label: "Обзор", icon: <Sparkles size={16} /> },
    { id: "schedule" as const, label: "Расписание", icon: <CalendarDays size={16} /> },
    { id: "workload" as const, label: "Нагрузка", icon: <GraduationCap size={16} /> },
    { id: "data" as const, label: "Справочники", icon: <Search size={16} /> },
    { id: "calendar" as const, label: "Календарь", icon: <CalendarDays size={16} /> },
    ...(canManageAccounts ? [{ id: "accounts" as const, label: "Пользователи", icon: <ShieldCheck size={16} /> }] : []),
  ];
  return <section className="admin-layout section-wrap" data-reveal><aside className="admin-sidebar"><div className="sidebar-title"><span className="eyebrow">Рабочий контур</span><strong>Управление</strong></div><nav aria-label="Разделы панели">{nav.map((item) => <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>{item.icon}<span>{item.label}</span>{tab === item.id && <ChevronRight size={14} />}</button>)}</nav><div className="sidebar-bottom"><div className="sidebar-user"><span className="user-avatar">{(account?.email ?? "A").slice(0, 1).toUpperCase()}</span><div><strong>{account?.email ?? "Администратор"}</strong><small>{account ? roleLabel(account.role) : "Роль из токена"}</small></div></div><button className="sidebar-logout" onClick={onLogout}>Выйти</button></div></aside><div className="admin-content">{tab === "overview" && <AdminDashboard account={account} onOpenSchedule={() => setTab("schedule")} />}{tab === "schedule" && <AdminSchedule />} {tab === "workload" && <AdminWorkload />} {tab === "data" && <AdminReferenceData canWrite={account?.role === "super_admin"} />} {tab === "calendar" && <AdminCalendar />} {tab === "accounts" && <AdminAccounts />}</div></section>;
}

function AdminDashboard({ account, onOpenSchedule }: { account: Account | null; onOpenSchedule: () => void }) {
  return <><div className="admin-topline"><div><span className="eyebrow">Рабочий контур / Обзор</span><h1>Добрый день<span className="brand-dot">.</span></h1><p>Главное по учебному процессу — на одном экране.</p></div><Badge tone="positive">{account ? roleLabel(account.role) : "Роль из токена"}</Badge></div><div className="admin-context"><div><span>Текущий семестр</span><strong>Весна 2026 <ChevronRight size={17} /></strong></div><div><span>Ваша роль</span><strong>{account ? roleLabel(account.role) : "Администратор"}</strong></div><div><span>Статус системы</span><strong className="status-ok"><span /> Всё работает</strong></div></div><div className="admin-grid"><Panel className="admin-focus"><div className="panel-heading"><div><Badge tone="accent">Фокус дня</Badge><h2>Расписание готово<br />к проверке</h2></div><CalendarDays size={25} /></div><p>12 групп ждут финального подтверждения после последних изменений нагрузки.</p><Button className="button-primary" onClick={onOpenSchedule}>Открыть расписание <ArrowUpRight size={17} /></Button></Panel><Panel className="metric-card"><span className="eyebrow">Заполненность</span><strong>86<span>%</span></strong><div className="progress"><span style={{ width: "86%" }} /></div><small>+8% за последние 7 дней</small></Panel><Panel className="metric-card"><span className="eyebrow">Активные группы</span><strong>148</strong><div className="metric-spark"><i /><i /><i /><i /><i /><i /><i /></div><small>Все факультеты подключены</small></Panel><Panel className="activity-card"><div className="panel-heading"><div><span className="eyebrow">Последние изменения</span><h3>Журнал событий</h3></div><button className="icon-button" aria-label="Поиск"><Search size={18} /></button></div>{["Нагрузка группы ИВТ-21 обновлена", "Добавлено 4 занятия в корпус B", "Преподаватель привязан к кафедре"].map((item, index) => <div className="activity-row" key={item}><span className={`activity-marker marker-${index}`} /><span>{item}</span><small>{index + 1}ч назад</small></div>)}</Panel></div></>;
}

function AdminSchedule() {
  const [mode, setMode] = useState("group");
  return <div className="admin-section"><AdminSectionHeading eyebrow="Расписание" title="Соберите день без пересечений" description="Выберите режим просмотра. Сетка подключается к существующим schedule endpoints после выбора сущности и семестра." /><div className="admin-tabs"><button className={mode === "group" ? "active" : ""} onClick={() => setMode("group")}>Группа</button><button className={mode === "teacher" ? "active" : ""} onClick={() => setMode("teacher")}>Преподаватель</button><button className={mode === "room" ? "active" : ""} onClick={() => setMode("room")}>Аудитория</button></div><Panel className="schedule-empty"><div className="empty-orbit"><CalendarDays size={30} /></div><h2>{mode === "group" ? "Расписание группы" : mode === "teacher" ? "Расписание преподавателя" : "Занятость аудитории"}</h2><p>Выберите факультет, семестр и нужную сущность, чтобы загрузить данные из API.</p><div className="selector-row"><label>Семестр<select defaultValue=""><option value="" disabled>Выберите семестр</option><option value="spring-2026">Весна 2026</option></select></label><label>{mode === "group" ? "Группа" : mode === "teacher" ? "Преподаватель" : "Аудитория"}<div className="fake-select">Начните вводить название <Search size={15} /></div></label><Button className="button-primary">Показать</Button></div></Panel></div>;
}

function AdminWorkload() { return <div className="admin-section"><AdminSectionHeading eyebrow="Учебная нагрузка" title="Нагрузка под контролем" description="Факультетская и кафедральная нагрузка с обязательным контекстом семестра." /><div className="workload-grid"><Panel className="workload-card"><Badge tone="accent">Факультет</Badge><strong>Сводная нагрузка</strong><span>Кафедры, часы и контрольные суммы</span><Button className="button-outline">Открыть сводную</Button></Panel><Panel className="workload-card"><Badge>Кафедра</Badge><strong>Ячейки нагрузки</strong><span>Предметы, виды занятий и распределение</span><Button className="button-outline">Открыть кафедру</Button></Panel></div><Panel className="notice-panel"><span className="signal-icon">●</span><span>Перед запросами нагрузки клиент всегда передаёт `semester_id`, как требует API contract.</span></Panel></div>; }

function AdminReferenceData({ canWrite }: { canWrite: boolean }) {
  const items = [{ title: "Факультеты", resource: "faculties", route: "/admin/faculties", icon: "01" }, { title: "Семестры", resource: "semesters", route: "/admin/semesters", icon: "02" }, { title: "Преподаватели", resource: "teachers", route: "/admin/teachers", icon: "03" }, { title: "Аудитории", resource: "rooms", route: "/admin/rooms", icon: "04" } ] as const;
  const [selected, setSelected] = useState<(typeof items)[number]["resource"]>("faculties");
  const [records, setRecords] = useState<Array<{ id: string; attributes?: Record<string, unknown> }>>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [editor, setEditor] = useState<{ id?: string; fields: Record<string, string> } | null>(null);
  useEffect(() => { setLoading(true); setError(""); api.listAdmin(selected).then((response) => setRecords(response.data)).catch((requestError) => setError(requestError instanceof Error ? requestError.message : "Не удалось загрузить справочник")).finally(() => setLoading(false)); }, [selected]);
  const current = items.find((item) => item.resource === selected)!;
  async function saveEditor(event: FormEvent<HTMLFormElement>) { event.preventDefault(); if (!editor || (selected !== "faculties" && selected !== "semesters")) return; setLoading(true); setError(""); try { if (editor.id) await api.updateAdmin(selected, editor.id, normalizeEditorFields(selected, editor.fields)); else await api.createAdmin(selected, normalizeEditorFields(selected, editor.fields)); setEditor(null); const response = await api.listAdmin(selected); setRecords(response.data); } catch (requestError) { setError(requestError instanceof Error ? requestError.message : "Не удалось сохранить запись"); } finally { setLoading(false); } }
  async function removeRecord(id: string) { if (!window.confirm("Удалить запись?")) return; setLoading(true); setError(""); try { await api.deleteAdmin(selected as "faculties" | "semesters", id); setRecords((currentRecords) => currentRecords.filter((record) => record.id !== id)); } catch (requestError) { setError(requestError instanceof Error ? requestError.message : "Не удалось удалить запись"); } finally { setLoading(false); } }
  const editable = selected === "faculties" || selected === "semesters";
  return <div className="admin-section"><AdminSectionHeading eyebrow="Справочники" title="Данные, на которых держится система" description="GET-запросы идут только к ресурсам, объявленным в admin routes." /><div className="reference-list">{items.map((item) => <button className={`reference-row ${selected === item.resource ? "selected" : ""}`} onClick={() => setSelected(item.resource)} key={item.route}><span>{item.icon}</span><strong>{item.title}</strong><code>{item.route}</code><ChevronRight size={17} /></button>)}</div><Panel className="resource-table"><div className="panel-heading"><div><span className="eyebrow">{current.route}</span><h3>{current.title}</h3></div><div className="resource-actions"><Badge>{loading ? "Загрузка" : `${records.length} записей`}</Badge>{canWrite && editable && <Button className="button-primary button-small" onClick={() => setEditor({ fields: {} })}>Добавить</Button>}</div></div>{error && <div className="form-error" role="alert">{error}</div>}{!error && !loading && <div className="resource-rows">{records.length === 0 ? <div className="resource-empty">API вернул пустую коллекцию.</div> : records.slice(0, 50).map((record) => <div className="resource-row" key={record.id}><span>#{record.id}</span><strong>{displayAttributes(record.attributes, current.title)}</strong><small>{Object.entries(record.attributes ?? {}).filter(([key]) => key !== "name").slice(0, 2).map(([key, value]) => `${key}: ${String(value)}`).join(" · ")}</small>{canWrite && editable && <span className="row-actions"><button onClick={() => setEditor({ id: record.id, fields: Object.fromEntries(Object.entries(record.attributes ?? {}).map(([key, value]) => [key, String(value ?? "")])) })}>Изменить</button><button onClick={() => removeRecord(record.id)}>Удалить</button></span>}</div>)}</div>}</Panel>{editor && <div className="modal-backdrop" role="presentation"><Panel className="editor-modal"><div className="panel-heading"><div><span className="eyebrow">{editor.id ? "Изменение" : "Новая запись"}</span><h3>{current.title}</h3></div><button className="icon-button" onClick={() => setEditor(null)} aria-label="Закрыть"><X size={18} /></button></div><form onSubmit={saveEditor} className="editor-form">{(selected === "faculties" ? ["name", "short_name"] : ["start_year", "end_year", "number"]).map((field) => <label key={field}>{fieldLabel(field)}<input required value={editor.fields[field] ?? ""} onChange={(event) => setEditor((value) => value ? { ...value, fields: { ...value.fields, [field]: event.target.value } } : value)} /></label>)}<Button className="button-primary button-full" type="submit">Сохранить <ArrowUpRight size={16} /></Button></form></Panel></div>}</div>;
}

function normalizeEditorFields(resource: "faculties" | "semesters", fields: Record<string, string>) { if (resource === "faculties") return { name: fields.name, short_name: fields.short_name }; return { start_year: Number(fields.start_year), end_year: Number(fields.end_year), number: Number(fields.number) }; }
function fieldLabel(field: string) { return { name: "Название", short_name: "Короткое название", start_year: "Начальный год", end_year: "Конечный год", number: "Номер семестра" }[field] ?? field; }

function AdminCalendar() { return <div className="admin-section"><AdminSectionHeading eyebrow="Календарь" title="Учебный год без сюрпризов" description="Праздники, переносы дней и периодические окна групп." /><div className="calendar-grid"><Panel><span className="eyebrow">Праздники</span><strong>Holidays</strong><small>GET /admin/holidays<br />POST /admin/holidays<br />DELETE /admin/holidays/:id</small></Panel><Panel><span className="eyebrow">Переносы</span><strong>Weekday transfers</strong><small>GET /admin/weekday_transfers<br />POST /admin/weekday_transfers<br />DELETE /admin/weekday_transfers/:id</small></Panel><Panel><span className="eyebrow">Периоды</span><strong>Периоды группы</strong><small>GET /admin/student_group_semesters/:id/periods<br />POST /admin/student_group_semesters/:id/periods</small></Panel></div></div>; }

function AdminAccounts() { return <div className="admin-section"><AdminSectionHeading eyebrow="Пользователи" title="Доступы и роли" description="Этот раздел доступен только super_admin и использует /auth/accounts." /><Panel className="accounts-panel"><div className="panel-heading"><div><Badge tone="positive">Super admin only</Badge><h3>Управление аккаунтами</h3></div><Button className="button-primary">Создать аккаунт</Button></div><div className="account-placeholder"><ShieldCheck size={25} /><span>Список аккаунтов будет загружен из GET /auth/accounts с JSON-ответом API.</span></div></Panel></div>; }

function AdminSectionHeading({ eyebrow, title, description }: { eyebrow: string; title: string; description: string }) { return <div className="admin-section-heading"><div><span className="eyebrow">{eyebrow}</span><h2>{title}</h2></div><p>{description}</p></div>; }

function roleLabel(role: AccountRole) {
  return { super_admin: "Супер-администратор", faculty_admin: "Администратор факультета", kafedra_admin: "Администратор кафедры", scheduler: "Диспетчер расписания", teacher: "Преподаватель", student: "Студент", unknown: "Роль из токена" }[role];
}

export default App;
