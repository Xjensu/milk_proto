import type { PropsWithChildren } from "react";

export type ScheduleMode = "group" | "teacher" | "room" | "occupancy";
export type ScopeMode = "semester" | "date";

export function ScheduleQuery({ mode, onModeChange, scope, onScopeChange, children }: PropsWithChildren<{
  mode: ScheduleMode;
  onModeChange: (mode: ScheduleMode) => void;
  scope: ScopeMode;
  onScopeChange: (scope: ScopeMode) => void;
}>) {
  return <section className="schedule-query" aria-label="Запрос расписания">
    <div className="admin-tabs" role="tablist" aria-label="Режим расписания">{(["group", "teacher", "room", "occupancy"] as const).map((item) => <button type="button" role="tab" aria-selected={mode === item} className={mode === item ? "active" : ""} onClick={() => onModeChange(item)} key={item}>{title(item)}</button>)}</div>
    {mode === "teacher" || mode === "room" ? <div className="scope-switch" aria-label="Период отображения">{(["semester", "date"] as const).map((item) => <button type="button" aria-pressed={scope === item} className={scope === item ? "active" : ""} onClick={() => onScopeChange(item)} key={item}>{item === "semester" ? "По семестру" : "По дате"}</button>)}</div> : null}
    <div className="schedule-query-body">{children}</div>
  </section>;
}

function title(value: ScheduleMode) { return ({ group: "Группа", teacher: "Преподаватель", room: "Аудитория", occupancy: "Занятость" } as Record<ScheduleMode, string>)[value]; }
