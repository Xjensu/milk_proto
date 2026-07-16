import type { ScheduleEvent, ScheduleSource } from "./types";
import { normalizeSchedule } from "./normalize";

export function WeekGrid({ source, onSelect }: { source: ScheduleSource; onSelect?: (event: ScheduleEvent) => void }) {
  const days = normalizeSchedule(source);
  if (!source) return <div className="schedule-empty"><h2>Your schedule will appear here</h2><p>Complete the context above to load current schedule data.</p></div>;
  if (days.length === 0) return <div className="schedule-empty"><h2>No events found</h2><p>The API returned no events for the selected context.</p></div>;

  return <div className="week-grid-scroll" tabIndex={0} aria-label="Weekly schedule"><table className="week-grid">
    <thead><tr>{days.map((day) => <th scope="col" key={day.key}>{day.label}<small>{day.events.length} {day.events.length === 1 ? "event" : "events"}</small></th>)}</tr></thead>
    <tbody><tr>{days.map((day) => <td key={day.key}>{day.events.map((event) => {
      const content = <><div className="schedule-event-time"><strong>{event.startTime}</strong><small>{event.endTime}</small></div><div className="schedule-event-main"><strong>{event.subject}</strong><span>{event.activityType}</span><small>{[event.group, event.teacher || (event.group ? "Not assigned" : ""), event.room].filter(Boolean).join(" · ")}</small></div>{event.changed ? <span className="schedule-change-label">Changed</span> : null}</>;
      return onSelect ? <button type="button" aria-label={`View ${event.subject} details`} className={`schedule-event schedule-event-button ${event.changed ? "schedule-event-changed" : ""}`} key={event.id} onClick={() => onSelect(event)}>{content}</button> : <article className={`schedule-event ${event.changed ? "schedule-event-changed" : ""}`} key={event.id}>{content}</article>;
    })}</td>)}</tr></tbody>
  </table></div>;
}
