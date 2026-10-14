import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class ReleaseDocumentationTests(unittest.TestCase):
    def test_public_documentation_has_one_normative_entry_point(self) -> None:
        index = (REPO_ROOT / "docs" / "spec" / "index.md").read_text(encoding="utf-8")
        root_readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("normative source", index)
        self.assertIn("docs/spec/index.md", root_readme)
        self.assertNotIn("production-ready", root_readme.lower())

    def test_extensions_and_research_are_separate_from_core(self) -> None:
        extensions = (REPO_ROOT / "docs" / "extensions" / "README.md").read_text(
            encoding="utf-8"
        )
        research = (REPO_ROOT / "docs" / "research" / "README.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("outside the minimum direct-message milestone", extensions)
        self.assertIn("hypotheses", research.lower())

    def test_release_evidence_records_unvalidated_gates(self) -> None:
        gates = (REPO_ROOT / "docs" / "internal" / "handoff" / "release-gates.md").read_text(
            encoding="utf-8"
        )
        report = (
            REPO_ROOT
            / "docs"
            / "internal"
            / "release-evidence"
            / "spec-reboot-report.md"
        ).read_text(encoding="utf-8")
        self.assertIn("Android, iOS, Linux, and embedded", gates)
        self.assertIn("Open blockers", report)
        self.assertIn("File symbols are relay-visible plaintext", report)


if __name__ == "__main__":
    unittest.main()
