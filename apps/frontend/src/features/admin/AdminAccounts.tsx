import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";

import { Badge, Button, Panel } from "../../components/ui";
import { api, type AccountRole, type JsonApiResource } from "../../lib/api";

const roles: AccountRole[] = ["super_admin", "faculty_admin", "kafedra_admin", "scheduler", "teacher", "student"];

export function AdminAccounts() {
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [initialPassword, setInitialPassword] = useState("");

  const load = () => {
    setLoading(true);
    setError("");
    api.listAccounts().then((response) => setItems(response.data)).catch(() => setError("Не удалось загрузить аккаунты.")).finally(() => setLoading(false));
  };
  useEffect(load, []);

  const setStatus = async (item: JsonApiResource) => {
    const status = item.attributes?.status === "disabled" ? "active" : "disabled";
    try { await api.updateAccount(item.id, { status }); load(); } catch { setError("Не удалось изменить статус аккаунта."); }
  };
  const remove = async (item: JsonApiResource) => {
    if (!window.confirm(`Удалить аккаунт ${String(item.attributes?.email ?? item.id)}?`)) return;
    try { await api.deleteAccount(item.id); load(); } catch { setError("Не удалось удалить аккаунт."); }
  };

  return <div className="admin-section">
    <div className="admin-topline"><div><span className="eyebrow">Только super_admin</span><h1>Управление аккаунтами<span className="brand-dot">.</span></h1><p>Роли, статусы и области доступа пользователей системы.</p></div><Button className="button-primary" onClick={() => setCreating(true)}><Plus size={16} /> Создать аккаунт</Button></div>
    {initialPassword ? <Panel className="notice-panel"><strong>Временный пароль</strong><span>{initialPassword}</span><small>Скопируйте его сейчас: API показывает пароль только один раз.</small></Panel> : null}
    {creating ? <AccountForm onCancel={() => setCreating(false)} onCreated={(password) => { setCreating(false); setInitialPassword(password); load(); }} /> : null}
    <Panel className="admin-data-panel"><div className="panel-heading"><div><span className="eyebrow">Доступы</span><h2>Аккаунты</h2></div><span className="resource-count">{loading ? "…" : items.length}</span></div>
      {error ? <div className="request-error"><p>{error}</p><Button onClick={load}>Повторить</Button></div> : null}
      {!error && loading ? <p className="resource-empty">Загрузка аккаунтов…</p> : null}
      {!error && !loading && items.length === 0 ? <p className="resource-empty">Аккаунты не найдены.</p> : null}
      {!error && !loading && items.length > 0 ? <div className="admin-data-table-wrap"><table className="admin-data-table"><thead><tr><th>Пользователь</th><th>Роль</th><th>Область</th><th>Статус</th><th><span className="sr-only">Действия</span></th></tr></thead><tbody>{items.map((item) => <tr key={item.id}><td><strong>{String(item.attributes?.email ?? "—")}</strong></td><td>{roleName(String(item.attributes?.role ?? ""))}</td><td>{scopeName(item.attributes?.scopes)}</td><td><Badge tone={item.attributes?.status === "active" ? "positive" : "neutral"}>{item.attributes?.status === "active" ? "Активен" : "Отключён"}</Badge></td><td className="table-actions"><button onClick={() => setStatus(item)}>{item.attributes?.status === "active" ? "Отключить" : "Включить"}</button><button aria-label="Удалить" onClick={() => remove(item)}><Trash2 size={15} /></button></td></tr>)}</tbody></table></div> : null}
    </Panel>
  </div>;
}

function AccountForm({ onCancel, onCreated }: { onCancel: () => void; onCreated: (password: string) => void }) {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<AccountRole>("scheduler");
  const [password, setPassword] = useState("");
  const [scopeId, setScopeId] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const scoped = role === "faculty_admin" || role === "kafedra_admin";

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    const payload: Record<string, unknown> = { email, role };
    if (password) payload.password = password;
    if (scoped) payload.scopes = [{ [role === "faculty_admin" ? "faculty_id" : "kafedra_id"]: Number(scopeId) }];
    try {
      const response = await api.createAccount(payload);
      onCreated(String(response.data.attributes?.initial_password ?? ""));
    } catch {
      setError("Не удалось создать аккаунт. Проверьте роль и область доступа.");
    } finally { setSaving(false); }
  };

  return <Panel className="admin-edit-panel"><form onSubmit={submit}><div className="panel-heading"><div><span className="eyebrow">Новый пользователь</span><h2>Создание аккаунта</h2></div></div><div className="admin-form-grid"><label><span>Email</span><input required type="email" value={email} onChange={(event) => setEmail(event.target.value)} /></label><label><span>Роль</span><select value={role} onChange={(event) => setRole(event.target.value as AccountRole)}>{roles.map((item) => <option value={item} key={item}>{roleName(item)}</option>)}</select></label>{scoped ? <label><span>{role === "faculty_admin" ? "ID факультета" : "ID кафедры"}</span><input required type="number" min="1" value={scopeId} onChange={(event) => setScopeId(event.target.value)} /></label> : null}<label><span>Пароль (необязательно)</span><input type="password" minLength={8} value={password} onChange={(event) => setPassword(event.target.value)} placeholder="API создаст временный" /></label></div>{error ? <p className="form-error">{error}</p> : null}<div className="form-actions"><Button type="button" onClick={onCancel}>Отмена</Button><Button className="button-primary" type="submit" disabled={saving}>{saving ? "Создание…" : "Создать"}</Button></div></form></Panel>;
}

function roleName(role: string) { return ({ super_admin: "Супер-администратор", faculty_admin: "Администратор факультета", kafedra_admin: "Администратор кафедры", scheduler: "Диспетчер", teacher: "Преподаватель", student: "Студент" } as Record<string, string>)[role] ?? role; }
function scopeName(value: unknown) { if (!Array.isArray(value) || value.length === 0) return "Глобальная"; return value.map((scope) => typeof scope === "object" && scope ? Object.entries(scope).filter(([, id]) => id !== null).map(([key, id]) => `${key}: ${id}`).join(", ") : "").join("; "); }
