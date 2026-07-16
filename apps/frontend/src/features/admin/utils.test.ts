import { describe, expect, it } from "vitest";

import { groupResource, resourceLabel, roleLabel } from "./utils";

describe("admin role labels", () => {
  it("uses concise Russian labels", () => {
    expect(roleLabel("super_admin")).toBe("Главный администратор");
    expect(roleLabel("scheduler")).toBe("Диспетчер расписания");
    expect(roleLabel("unknown")).toBe("Роль не определена");
  });

  it("does not expose raw ids when a resource has no readable value", () => {
    expect(resourceLabel({ id: "918", type: "rooms", attributes: {} }, "Room")).toBe("Без названия: room");
  });

  it("uses the group-semester identity and readable course label", () => {
    expect(groupResource({ id: "8", type: "studying_group", attributes: { group_semester_id: 17, speciality_name: "Computer Science", course: 3 } })).toEqual({ id: "17", label: "Computer Science-3" });
  });
});
