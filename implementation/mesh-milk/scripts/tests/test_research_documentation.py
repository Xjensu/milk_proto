import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class ResearchDocumentationTests(unittest.TestCase):
    def test_clustered_research_status_matches_shipped_matrix(self) -> None:
        handoff = (REPO_ROOT / "docs" / "E6_S1_SOCIAL_CLUSTER_HANDOFF.md").read_text(
            encoding="utf-8"
        )
        agile = (REPO_ROOT / "docs" / "AGILE_REMAINING_WORK.md").read_text(
            encoding="utf-8"
        )
        simulation = (REPO_ROOT / "docs" / "MMP_PHI_SIMULATION.md").read_text(
            encoding="utf-8"
        )

        # The simulator comparison matrix now has four controlled schedule
        # families. Keep the handoff and Agile queue from describing the
        # shipped heavy-tail control as an unimplemented SLAW task.
        self.assertIn("bounded-heavy-tail-clustered", "".join(handoff.split()))
        self.assertIn("PARTIALLY DONE / RESEARCH-ONLY", agile)
        self.assertIn("uniform-random, periodic-clustered, social-clustered, and", simulation)
        self.assertIn("bounded-heavy-tail-clustered", simulation)


if __name__ == "__main__":
    unittest.main()
