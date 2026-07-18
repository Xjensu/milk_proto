import { createContext, type Dispatch, type PropsWithChildren, useContext, useEffect, useMemo, useReducer } from "react";

export type WorkspaceState = {
  semesterId: string | null;
  facultyId: string | null;
  departmentId: string | null;
  groupId: string | null;
  groupIds: string[];
  teacherId: string | null;
  roomId: string | null;
  date: string;
};

export type WorkspaceAction =
  | { type: "selectSemester"; semesterId: string | null }
  | { type: "selectFaculty"; facultyId: string | null }
  | { type: "selectDepartment"; departmentId: string | null }
  | { type: "selectGroup"; groupId: string | null }
  | { type: "toggleGroup"; groupId: string }
  | { type: "selectTeacher"; teacherId: string | null }
  | { type: "selectRoom"; roomId: string | null }
  | { type: "selectDate"; date: string };

export const initialWorkspaceState: WorkspaceState = {
  semesterId: null,
  facultyId: null,
  departmentId: null,
  groupId: null,
  groupIds: [],
  teacherId: null,
  roomId: null,
  date: new Date().toISOString().slice(0, 10),
};

export function workspaceReducer(state: WorkspaceState, action: WorkspaceAction): WorkspaceState {
  switch (action.type) {
    case "selectSemester": return { ...state, semesterId: action.semesterId, groupId: null, groupIds: [] };
    case "selectFaculty": return { ...state, facultyId: action.facultyId, departmentId: null, groupId: null, groupIds: [] };
    case "selectDepartment": return { ...state, departmentId: action.departmentId };
    case "selectGroup": return { ...state, groupId: action.groupId };
    case "toggleGroup": {
      const groupIds = state.groupIds.includes(action.groupId)
        ? state.groupIds.filter((id) => id !== action.groupId)
        : [...state.groupIds, action.groupId];
      return { ...state, groupIds, groupId: groupIds[0] ?? null };
    }
    case "selectTeacher": return { ...state, teacherId: action.teacherId };
    case "selectRoom": return { ...state, roomId: action.roomId };
    case "selectDate": return { ...state, date: action.date };
  }
}

const WorkspaceContext = createContext<{ state: WorkspaceState; dispatch: Dispatch<WorkspaceAction> } | null>(null);
const STORAGE_KEY = "milk_schedule_admin_workspace_v1";

export function WorkspaceProvider({ children }: PropsWithChildren) {
  const [state, dispatch] = useReducer(workspaceReducer, initialWorkspaceState, restoreWorkspace);

  useEffect(() => {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ version: 1, state }));
    const params = new URLSearchParams(window.location.search);
    setParam(params, "semester", state.semesterId);
    setParam(params, "faculty", state.facultyId);
    setParam(params, "group", state.groupId);
    setParam(params, "groups", state.groupIds.length ? state.groupIds.join(",") : null);
    setParam(params, "teacher", state.teacherId);
    setParam(params, "room", state.roomId);
    setParam(params, "date", state.date);
    window.history.replaceState(null, "", `${window.location.pathname}?${params.toString()}${window.location.hash}`);
  }, [state]);

  const value = useMemo(() => ({ state, dispatch }), [state]);
  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace() {
  const value = useContext(WorkspaceContext);
  if (!value) throw new Error("useWorkspace must be used inside WorkspaceProvider");
  return value;
}

function restoreWorkspace(state: WorkspaceState) {
  if (typeof window === "undefined") return state;
  try {
    const saved = JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? "null") as { version?: number; state?: Partial<WorkspaceState> } | null;
    const params = new URLSearchParams(window.location.search);
    return {
      ...state,
      ...(saved?.version === 1 ? saved.state : {}),
      semesterId: params.get("semester") ?? saved?.state?.semesterId ?? null,
      facultyId: params.get("faculty") ?? saved?.state?.facultyId ?? null,
      groupId: params.get("group") ?? saved?.state?.groupId ?? null,
      groupIds: params.get("groups")?.split(",").filter(Boolean) ?? saved?.state?.groupIds ?? [],
      teacherId: params.get("teacher") ?? saved?.state?.teacherId ?? null,
      roomId: params.get("room") ?? saved?.state?.roomId ?? null,
      date: params.get("date") ?? saved?.state?.date ?? state.date,
    };
  } catch {
    return state;
  }
}

function setParam(params: URLSearchParams, key: string, value: string | null) {
  if (value) params.set(key, value);
  else params.delete(key);
}
