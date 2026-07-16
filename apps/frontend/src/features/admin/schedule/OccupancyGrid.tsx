import type { RoomSummary } from "../domain";
import { formatRoom } from "../formatters";

export function OccupancyGrid({ source, rooms }: { source: Record<string, unknown> | null; rooms: RoomSummary[] }) {
  if (!source) return <div className="schedule-empty"><h2>Room occupancy will appear here</h2><p>Select a date and at least one room.</p></div>;
  const rows = Array.isArray(source.data) ? source.data.filter(isRecord) : [];
  if (rows.length === 0) return <div className="schedule-empty"><h2>No occupancy found</h2><p>The selected rooms have no reported teaching events or reservations.</p></div>;
  const slots = [...new Set(rows.map((row) => stringValue(row.time_label ?? row.start_time)).filter(Boolean))];
  return <div className="occupancy-scroll"><table className="occupancy-grid"><thead><tr><th scope="col">Room</th>{slots.map((slot) => <th scope="col" key={slot}>{slot}</th>)}</tr></thead><tbody>{rooms.map((room) => <tr key={room.id}><th scope="row">{formatRoom(room)}</th>{slots.map((slot) => { const event = rows.find((row) => stringValue(row.room_id) === room.id && stringValue(row.time_label ?? row.start_time) === slot); return <td key={slot}>{event ? <><strong>{stringValue(event.subject_name ?? event.purpose, "Occupied")}</strong><small>{stringValue(event.group_display_name, "Reservation")}</small></> : <span>Available</span>}</td>; })}</tr>)}</tbody></table></div>;
}

function isRecord(value: unknown): value is Record<string, unknown> { return Boolean(value) && typeof value === "object" && !Array.isArray(value); }
function stringValue(value: unknown, fallback = "") { return typeof value === "string" || typeof value === "number" ? String(value) : fallback; }
