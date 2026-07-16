export type ScheduleSource = Record<string, unknown> | null;

export type ScheduleEvent = {
  id: string;
  startTime: string;
  endTime: string;
  subject: string;
  activityType: string;
  room: string;
  counterpart: string;
  changed: boolean;
};

export type ScheduleDay = {
  key: string;
  label: string;
  events: ScheduleEvent[];
};
