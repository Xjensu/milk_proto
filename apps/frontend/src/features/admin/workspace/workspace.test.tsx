import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { EntityPicker } from "./EntityPicker";
import { Inspector } from "./Inspector";
import { SemesterRail } from "./SemesterRail";

describe("workspace selection primitives", () => {
  it("renders semester cards as one labelled radio group", () => {
    const markup = renderToStaticMarkup(<SemesterRail
      semesters={[{ id: "1", number: 1, startYear: 2026, endYear: 2027 }]}
      selectedId="1"
      onSelect={() => undefined}
    />);
    expect(markup).toContain('role="radiogroup"');
    expect(markup).toContain('aria-checked="true"');
    expect(markup).toContain("1 semester 2026/2027");
  });

  it("renders a labelled searchable picker with readable options", () => {
    const markup = renderToStaticMarkup(<EntityPicker
      label="Teacher"
      query="Emily"
      onQueryChange={() => undefined}
      options={[{ id: "7", label: "Carter Emily", description: "Computer Science" }]}
      selectedId={null}
      onSelect={() => undefined}
    />);
    expect(markup).toContain('role="combobox"');
    expect(markup).toContain("Carter Emily");
    expect(markup).not.toContain(">7<");
  });

  it("renders inspectors as labelled dialogs", () => {
    const markup = renderToStaticMarkup(<Inspector title="Mathematics" open onClose={() => undefined}>Details</Inspector>);
    expect(markup).toContain('role="dialog"');
    expect(markup).toContain('aria-modal="true"');
    expect(markup).toContain("Mathematics");
  });
});
