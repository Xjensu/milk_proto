import { describe, expect, it } from "vitest";

import {
  calculateCourse,
  formatGroup,
  formatRoom,
  formatSemester,
  formatTeacher,
} from "./formatters";

describe("admin domain formatters", () => {
  it("formats semesters with number and academic years", () => {
    expect(formatSemester({ id: "3", number: 3, startYear: 2026, endYear: 2027 }))
      .toBe("3 semester 2026/2027");
  });

  it("calculates a group course in the selected academic year", () => {
    expect(calculateCourse({ admissionYear: 2023, semesterStartYear: 2025 })).toBe(3);
    expect(calculateCourse({ admissionYear: 2026, semesterStartYear: 2025 })).toBe(1);
  });

  it("formats groups as code-course", () => {
    expect(formatGroup({ id: "8", code: "CS-21", course: 3 })).toBe("CS-21-3");
  });

  it("uses readable empty assignment labels", () => {
    expect(formatTeacher(null)).toBe("Not assigned");
    expect(formatRoom(null)).toBe("Not assigned");
  });

  it("formats teacher and room details without exposing ids", () => {
    expect(formatTeacher({ id: "7", firstName: "Emily", lastName: "Carter", middleName: "Jane" }))
      .toBe("Carter Emily Jane");
    expect(formatRoom({ id: "4", number: "201", building: "Main building" }))
      .toBe("Room 201 · Main building");
  });
});
