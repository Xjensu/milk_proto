import type { PropsWithChildren } from "react";

export type ScheduleMode = "group" | "teacher" | "room" | "occupancy";
export type ScopeMode = "semester" | "date";

export function ScheduleQuery({ mode, onModeChange, scope, onScopeChange, children }: PropsWithChildren<{
  mode: ScheduleMode;
  onModeChange: (mode: ScheduleMode) => void;
  scope: ScopeMode;
  onScopeChange: (scope: ScopeMode) => void;
}>) {
  return <section className="schedule-query" aria-label="Schedule query">
    <div className="admin-tabs" role="tablist" aria-label="Schedule mode">{(["group", "teacher", "room", "occupancy"] as const).map((item) => <button type="button" role="tab" aria-selected={mode === item} className={mode === item ? "active" : ""} onClick={() => onModeChange(item)} key={item}>{title(item)}</button>)}</div>
    {mode === "teacher" || mode === "room" ? <div className="scope-switch" aria-label="Time scope">{(["semester", "date"] as const).map((item) => <button type="button" aria-pressed={scope === item} className={scope === item ? "active" : ""} onClick={() => onScopeChange(item)} key={item}>By {item}</button>)}</div> : null}
    <div className="schedule-query-body">{children}</div>
  </section>;
}

function title(value: ScheduleMode) { return value.slice(0, 1).toUpperCase() + value.slice(1); }
