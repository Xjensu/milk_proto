import { useEffect, useState } from "react";

import { Button, Panel } from "../../components/ui";
import { api, type JsonApiResource } from "../../lib/api";
import { normalizeTeacherLoad, type TeacherLoadRow } from "../teacher/normalize";

export function AdminWorkload() {
  const [semesters, setSemesters] = useState<JsonApiResource[]>([]);
  const [semesterId, setSemesterId] = useState("");
  const [rows, setRows] = useState<TeacherLoadRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    api.listAdminCollection("semesters")
      .then((response) => active && setSemesters(response.data))
      .catch(() => active && setError("Не удалось загрузить семестры."));
    return () => { active = false; };
  }, []);

  async function load() {
    if (!semesterId) return;
    setLoading(true);
    setError("");
    try {
      const response = await api.loadOverview(semesterId);
      setRows(normalizeTeacherLoad(response as Record<string, unknown>));
      setLoaded(true);
    } catch {
      setError("Не удалось загрузить нагрузку выбранного семестра.");
    } finally {
      setLoading(false);
    }
  }

  return <div className="admin-section">
    <div className="admin-section-heading"><div><span className="eyebrow">Нагрузка</span><h2>Контроль учебной нагрузки</h2></div><p>Сводка строится из `/admin/schedule/load_overview` и всегда содержит контекст семестра.</p></div>
    <Panel className="admin-query-panel"><div className="selector-row"><label>Семестр<select value={semesterId} onChange={(event) => { setSemesterId(event.target.value); setLoaded(false); setRows([]); }}><option value="">Выберите семестр</option>{semesters.map((item) => <option value={item.id} key={item.id}>{semesterLabel(item)}</option>)}</select></label><Button className="button-primary" onClick={load} disabled={!semesterId || loading}>{loading ? "Загрузка…" : "Загрузить нагрузку"}</Button></div>{error ? <div className="form-error" role="alert">{error}</div> : null}</Panel>
    {loaded ? <Panel className="resource-table"><div className="panel-heading"><div><span className="eyebrow">Актуальные данные</span><h3>Строки нагрузки</h3></div><span className="badge badge-neutral">{rows.length}</span></div>{rows.length === 0 ? <p className="resource-empty">Для выбранного семестра нагрузки нет.</p> : <div className="teacher-table-scroll"><table className="teacher-table"><thead><tr><th>Предмет</th><th>Вид занятия</th><th>Часы</th><th>Группа / семестр</th><th>Преподаватель</th><th>Аудитория</th></tr></thead><tbody>{rows.map((row) => <tr key={row.id}><td><strong>{row.subject}</strong></td><td>{row.activityType}</td><td>{row.hours}</td><td>{row.context}</td><td>{row.teacher}</td><td>{row.room}</td></tr>)}</tbody></table></div>}</Panel> : null}
  </div>;
}

function semesterLabel(resource: JsonApiResource) {
  const attributes = resource.attributes ?? {};
  return attributes.start_year && attributes.end_year ? `${attributes.start_year}/${attributes.end_year}${attributes.number ? ` · ${attributes.number} семестр` : ""}` : `Семестр #${resource.id}`;
}
