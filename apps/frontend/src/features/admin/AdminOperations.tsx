import { useEffect, useMemo, useState } from "react";

import { Button, Panel } from "../../components/ui";
import { api, type AdminCollection, type JsonApiResource } from "../../lib/api";

const sections: Array<{ id: AdminCollection; label: string }> = [
  { id: "holidays", label: "Праздники" },
  { id: "weekday_transfers", label: "Переносы" },
  { id: "room_reservations", label: "Бронирования" },
  { id: "media_files", label: "Файлы" },
  { id: "audit_logs", label: "Аудит" },
];

export function AdminOperations() {
  const [selected, setSelected] = useState<AdminCollection>("holidays");
  const [items, setItems] = useState<JsonApiResource[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const current = useMemo(() => sections.find((item) => item.id === selected) ?? sections[0], [selected]);

  const load = () => {
    setLoading(true);
    setError("");
    api.listAdminCollection(selected, "?page=1&per_page=50")
      .then((response) => setItems(response.data))
      .catch(() => setError("Не удалось загрузить операционные данные."))
      .finally(() => setLoading(false));
  };
  useEffect(load, [selected]);

  return <div className="admin-section">
    <div className="admin-topline"><div><span className="eyebrow">Администрирование / Календарь</span><h1>Операционные данные<span className="brand-dot">.</span></h1><p>Календарные исключения, бронирования, загрузки и события системы.</p></div></div>
    <div className="admin-resource-tabs" role="tablist" aria-label="Операционные данные">{sections.map((item) => <button role="tab" aria-selected={selected === item.id} className={selected === item.id ? "active" : ""} onClick={() => setSelected(item.id)} key={item.id}>{item.label}</button>)}</div>
    <Panel className="admin-data-panel"><div className="panel-heading"><div><span className="eyebrow">Текущий раздел</span><h2>{current.label}</h2></div><span className="resource-count">{loading ? "…" : items.length}</span></div>
      {error ? <div className="request-error"><p>{error}</p><Button onClick={load}>Повторить</Button></div> : null}
      {!error && loading ? <p className="resource-empty">Загрузка данных…</p> : null}
      {!error && !loading && items.length === 0 ? <p className="resource-empty">Событий не найдено.</p> : null}
      {!error && !loading && items.length > 0 ? <div className="operation-list">{items.map((item) => <article key={item.id}><strong>{operationTitle(item)}</strong><span>{operationDetails(item)}</span><code>#{item.id}</code></article>)}</div> : null}
    </Panel>
  </div>;
}

function operationTitle(item: JsonApiResource) {
  const attributes = item.attributes ?? {};
  return String(attributes.name ?? attributes.original_filename ?? attributes.action ?? attributes.status ?? `${item.type} #${item.id}`);
}

function operationDetails(item: JsonApiResource) {
  return Object.entries(item.attributes ?? {}).filter(([, value]) => value !== null && typeof value !== "object").slice(0, 4).map(([key, value]) => `${key}: ${String(value)}`).join(" · ") || "Без дополнительных сведений";
}
