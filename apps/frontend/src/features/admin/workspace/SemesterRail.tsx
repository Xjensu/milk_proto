import type { SemesterSummary } from "../domain";
import { formatSemester } from "../formatters";

export function SemesterRail({ semesters, selectedId, onSelect }: {
  semesters: SemesterSummary[];
  selectedId: string | null;
  onSelect: (semester: SemesterSummary) => void;
}) {
  return <div className="semester-rail" role="radiogroup" aria-label="Семестр">
    {semesters.map((semester) => <button
      type="button"
      role="radio"
      aria-checked={semester.id === selectedId}
      className={`semester-card${semester.id === selectedId ? " is-selected" : ""}`}
      key={semester.id}
      onClick={() => onSelect(semester)}
    >
      <strong>{formatSemester(semester)}</strong>
      <span>{semester.startYear} учебный год</span>
    </button>)}
  </div>;
}
