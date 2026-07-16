import { Button } from "../../../components/ui";
import type { ScheduleEvent } from "../../schedule/types";
import { Inspector } from "../workspace/Inspector";

export function ScheduleInspector({ event, onClose }: { event: ScheduleEvent | null; onClose: () => void }) {
  return <Inspector title={event?.subject ?? "Event details"} open={event !== null} onClose={onClose}>
    {event ? <><dl className="inspector-details">
      <Detail label="Activity" value={event.activityType} />
      <Detail label="Group" value={event.group || "Not assigned"} />
      <Detail label="Teacher" value={event.teacher || "Not assigned"} />
      <Detail label="Room" value={event.room || "Not assigned"} />
      <Detail label="Time" value={`${event.startTime}${event.endTime ? `–${event.endTime}` : ""}`} />
      <Detail label="State" value={event.changed ? "Changed occurrence" : "Scheduled"} />
    </dl><div className="inspector-actions"><Button className="button-primary">Move occurrence</Button><Button className="button-outline">Edit event</Button></div></> : null}
  </Inspector>;
}

function Detail({ label, value }: { label: string; value: string }) { return <div><dt>{label}</dt><dd>{value}</dd></div>; }
