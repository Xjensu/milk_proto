import { useEffect, useMemo, useState } from "react";
import { Pencil, Plus, Trash2 } from "lucide-react";

import { Button, Panel } from "../../components/ui";
import { api, type AdminCollection, type JsonApiResource } from "../../lib/api";
import { resourceLabel } from "./utils";

type ResourceConfig = {
  id: AdminCollection;
  label: string;
  fallback: string;
  editable?: boolean;
};

const resources: ResourceConfig[] = [
  { id: "faculties", label: "Факультеты", fallback: "Факультет", editable: true },
  { id: "semesters", label: "Семестры", fallback: "Семестр", editable: true },
  { id: "kafedras", label: "Кафедры", fallback: "Кафедра" },
  { id: "teachers", label: "Преподаватели", fallback: "Преподаватель" },
  { id: "rooms", label: "Аудитории", fallback: "Аудитория" },
  { id: "subjects", label: "Предметы", fallback: "Предмет" },
  { id: "activity_types", label: "Виды занятий", fallback: "Вид занятия" },
];

export function AdminResourcePage({ canWrite }: { canWrite: boolean }) {
  const [selected, setSelected] = useState<AdminCollection>("faculties");
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<JsonApiResource | "new" | null>(null);
  const config = useMemo(() => resources.find((item) => item.id === selected) ?? resources[0], [selected]);

  const load = () => {
    setLoading(true);
    setError("");
    api.listAdminCollection(selected)
      .then((response) => setItems(response.data))
      .catch(() => setError("Не удалось загрузить данные. Повторите попытку."))
      .finally(() => setLoading(false));
  };

  useEffect(load, [selected]);

  const remove = async (item: JsonApiResource) => {
    if (!config.editable || !window.confirm(`Удалить «${resourceLabel(item, config.fallback)}»?`)) return;
    try {
      await api.deleteAdmin(config.id as "faculties" | "semesters", item.id);
      load();
    } catch {
      setError("Не удалось удалить запись.");
    }
  };

  return <div className="admin-section">
    <div className="admin-topline"><div><span className="eyebrow">Администрирование / Справочники</span><h1>Данные университета<span className="brand-dot">.</span></h1><p>Выберите раздел. Изменение доступно только для ресурсов, поддержанных API.</p></div>{canWrite && config.editable ? <Button className="button-primary" onClick={() => setEditing("new")}><Plus size={16} /> Добавить</Button> : null}</div>
    <div className="admin-resource-tabs" role="tablist" aria-label="Справочники">{resources.map((item) => <button role="tab" aria-selected={selected === item.id} className={selected === item.id ? "active" : ""} key={item.id} onClick={() => { setSelected(item.id); setEditing(null); }}>{item.label}</button>)}</div>
    {editing && config.editable ? <ResourceForm resource={config.id as "faculties" | "semesters"} item={editing === "new" ? null : editing} onCancel={() => setEditing(null)} onSaved={() => { setEditing(null); load(); }} /> : null}
    <Panel className="admin-data-panel">
      <div className="panel-heading"><div><span className="eyebrow">Текущий раздел</span><h2>{config.label}</h2></div><span className="resource-count">{loading ? "…" : items.length}</span></div>
      {error ? <div className="request-error"><p>{error}</p><Button onClick={load}>Повторить</Button></div> : null}
      {!error && loading ? <p className="resource-empty">Загрузка данных…</p> : null}
      {!error && !loading && items.length === 0 ? <p className="resource-empty">Записи пока не созданы.</p> : null}
      {!error && !loading && items.length > 0 ? <div className="admin-data-table-wrap"><table className="admin-data-table"><thead><tr><th>Название</th><th>Идентификатор</th>{canWrite && config.editable ? <th><span className="sr-only">Действия</span></th> : null}</tr></thead><tbody>{items.map((item) => <tr key={item.id}><td><strong>{resourceLabel(item, config.fallback)}</strong></td><td><code>{item.id}</code></td>{canWrite && config.editable ? <td className="table-actions"><button aria-label="Изменить" onClick={() => setEditing(item)}><Pencil size={15} /></button><button aria-label="Удалить" onClick={() => remove(item)}><Trash2 size={15} /></button></td> : null}</tr>)}</tbody></table></div> : null}
    </Panel>
  </div>;
}

function ResourceForm({ resource, item, onCancel, onSaved }: { resource: "faculties" | "semesters"; item: JsonApiResource | null; onCancel: () => void; onSaved: () => void }) {
  const attributes = item?.attributes ?? {};
  const [form, setForm] = useState<Record<string, string>>(() => resource === "faculties"
    ? { name: String(attributes.name ?? ""), short_name: String(attributes.short_name ?? "") } as Record<string, string>
    : { start_year: String(attributes.start_year ?? ""), end_year: String(attributes.end_year ?? ""), number: String(attributes.number ?? "") } as Record<string, string>);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const fields: Array<{ id: string; label: string; type?: string }> = resource === "faculties"
    ? [{ id: "name", label: "Название" }, { id: "short_name", label: "Сокращение" }]
    : [{ id: "start_year", label: "Начальный год", type: "number" }, { id: "end_year", label: "Конечный год", type: "number" }, { id: "number", label: "Номер семестра", type: "number" }];

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    setError("");
    const payload = resource === "semesters" ? Object.fromEntries(Object.entries(form).map(([key, value]) => [key, Number(value)])) : form;
    try {
      if (item) await api.updateAdmin(resource, item.id, payload);
      else await api.createAdmin(resource, payload);
      onSaved();
    } catch {
      setError("Не удалось сохранить запись. Проверьте заполнение полей.");
    } finally {
      setSaving(false);
    }
  };

  return <Panel className="admin-edit-panel"><form onSubmit={submit}><div className="panel-heading"><div><span className="eyebrow">{item ? "Редактирование" : "Новая запись"}</span><h2>{resource === "faculties" ? "Факультет" : "Семестр"}</h2></div></div><div className="admin-form-grid">{fields.map((field) => <label key={field.id}><span>{field.label}</span><input required type={field.type ?? "text"} value={form[field.id]} onChange={(event) => setForm((current) => ({ ...current, [field.id]: event.target.value }))} /></label>)}</div>{error ? <p className="form-error">{error}</p> : null}<div className="form-actions"><Button type="button" onClick={onCancel}>Отмена</Button><Button type="submit" className="button-primary" disabled={saving}>{saving ? "Сохранение…" : "Сохранить"}</Button></div></form></Panel>;
}
