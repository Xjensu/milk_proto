import { describe, expect, it } from "vitest";

import { initialWorkspaceState, workspaceReducer } from "./WorkspaceContext";

describe("workspace context reducer", () => {
  it("clears an incompatible group when faculty changes", () => {
    const state = { ...initialWorkspaceState, facultyId: "1", groupId: "9" };
    expect(workspaceReducer(state, { type: "selectFaculty", facultyId: "2" }))
      .toEqual(expect.objectContaining({ facultyId: "2", groupId: null }));
  });

  it("clears an incompatible group when semester changes", () => {
    const state = { ...initialWorkspaceState, semesterId: "1", groupId: "9" };
    expect(workspaceReducer(state, { type: "selectSemester", semesterId: "2" }))
      .toEqual(expect.objectContaining({ semesterId: "2", groupId: null }));
  });

  it("keeps independent teacher and room selections", () => {
    const state = { ...initialWorkspaceState, teacherId: "4", roomId: "7" };
    expect(workspaceReducer(state, { type: "selectFaculty", facultyId: "2" }))
      .toEqual(expect.objectContaining({ teacherId: "4", roomId: "7" }));
  });
});
