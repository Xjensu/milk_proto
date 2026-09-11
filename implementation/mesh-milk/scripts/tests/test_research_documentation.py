import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class ResearchDocumentationTests(unittest.TestCase):
    def test_research_documents_state_scope_and_evidence(self) -> None:
        index = (REPO_ROOT / "docs" / "research" / "README.md").read_text(encoding="utf-8")
        open_questions = (REPO_ROOT / "docs" / "research" / "open-questions.md").read_text(
            encoding="utf-8"
        )
        performance = (REPO_ROOT / "docs" / "research" / "performance-model.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("do not add wire requirements", index)
        self.assertIn("Open technical questions", open_questions)
        self.assertIn("hardware-calibrated", performance)
        self.assertIn("1,000-node", performance)

    def test_internal_records_are_neutral_and_linked(self) -> None:
        handoff = (REPO_ROOT / "docs" / "internal" / "handoff" / "current-state.md").read_text(
            encoding="utf-8"
        )
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

        self.assertIn("workspace contains", handoff)
        self.assertIn("physical BLE", gates)
        self.assertIn("Implementation mismatches", report)
        self.assertNotIn("previous agent", handoff.lower())
        self.assertNotIn("model council", report.lower())


if __name__ == "__main__":
    unittest.main()
