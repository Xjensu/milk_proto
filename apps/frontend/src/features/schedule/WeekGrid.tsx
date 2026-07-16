import type { ScheduleSource } from "./types";
import { normalizeSchedule } from "./normalize";

export function WeekGrid({ source }: { source: ScheduleSource }) {
  const days = normalizeSchedule(source);

  if (!source) {
    return <div className="schedule-empty"><h2>Расписание появится здесь</h2><p>Выберите параметры и загрузите актуальные данные.</p></div>;
  }

  if (days.length === 0) {
    return <div className="schedule-empty"><h2>Занятий не найдено</h2><p>Для выбранных параметров API не вернуло событий.</p></div>;
  }

  return <div className="week-grid-scroll" tabIndex={0} aria-label="Недельное расписание">
    <table className="week-grid">
      <thead>
        <tr>{days.map((day) => <th scope="col" key={day.key}>{day.label}<small>{day.events.length} занятий</small></th>)}</tr>
      </thead>
      <tbody>
        <tr>{days.map((day) => <td key={day.key}>{day.events.map((event) => <article className={`schedule-event ${event.changed ? "schedule-event-changed" : ""}`} key={event.id}>
          <div className="schedule-event-time"><strong>{event.startTime}</strong><small>{event.endTime}</small></div>
          <div className="schedule-event-main">
            <strong>{event.subject}</strong>
            <span>{event.activityType}</span>
            <small>{event.room}{event.counterpart ? ` · ${event.counterpart}` : ""}</small>
          </div>
          {event.changed ? <span className="schedule-change-label">Изменено</span> : null}
        </article>)}</td>)}</tr>
      </tbody>
    </table>
  </div>;
}
