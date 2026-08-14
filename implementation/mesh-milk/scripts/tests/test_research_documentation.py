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
        self.assertIn("existing 1k tier", agile)
        self.assertIn("10k tier from the", agile)
        self.assertIn("remains deferred", agile)
        self.assertNotIn("existing 1k/10k tiers", agile)
        self.assertIn("uniform-random, periodic-clustered, social-clustered, and", simulation)
        self.assertIn("bounded-heavy-tail-clustered", simulation)

    def test_clustered_control_run_records_provenance_and_limits_claims(self) -> None:
        handoff = (REPO_ROOT / "docs" / "E6_S1_SOCIAL_CLUSTER_HANDOFF.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("2026-08-14 local paired control run", handoff)
        self.assertIn("rustc 1.96.1", handoff)
        self.assertIn("3f0136de53ffe6c56a0d36917d784317d764d1b2", handoff)
        self.assertIn("periodic clustered | 0.200", handoff)
        self.assertIn("social clustered | 0.100", handoff)
        self.assertIn("Reproduce the controls with", handoff)
        self.assertIn("clustered_mobility_measurement_is_reproducible", handoff)
        self.assertIn("social_clustered_mobility_measurement_is_reproducible", handoff)
        self.assertNotIn("The command was:", handoff)
        self.assertIn("do not provide trace calibration, confidence", handoff.lower())
        self.assertIn("mmp/φ improvement claim", handoff.lower())

    def test_research_baseline_seam_is_documented_as_baseline_only(self) -> None:
        handoff = (REPO_ROOT / "docs" / "MMP_PHI_EDR_HANDOFF.md").read_text(
            encoding="utf-8"
        )
        agile = (REPO_ROOT / "docs" / "AGILE_REMAINING_WORK.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("ResearchExperiment", handoff)
        self.assertIn("ResearchProvenance", handoff)
        self.assertIn("baseline-only host harness", handoff)
        self.assertIn("sidecar", handoff)
        self.assertIn("baseline-only `ResearchExperiment` seam", agile)
        self.assertIn("candidate adapters", agile)
        self.assertIn("OPEN / RESEARCH-ONLY", agile)

    def test_ews_replay_adapter_is_documented_as_a_sidecar(self) -> None:
        handoff = (REPO_ROOT / "docs" / "MMP_PHI_EDR_HANDOFF.md").read_text(
            encoding="utf-8"
        )
        agile = (REPO_ROOT / "docs" / "AGILE_REMAINING_WORK.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("evaluate_replay", handoff)
        self.assertIn("order-independent BLAKE3 fingerprint", handoff)
        self.assertIn("not candidate transport evidence", handoff)
        self.assertIn("provenance-bound, wire-neutral EWS replay sidecar", agile)
        self.assertIn("paired replay", agile)

    def test_repository_status_documents_shipped_file_plane_and_current_counts(self) -> None:
        features = (REPO_ROOT / "docs" / "USER_FEATURES.md").read_text(
            encoding="utf-8"
        )
        roadmap = (REPO_ROOT / "docs" / "ROADMAP_TO_PRODUCTION.md").read_text(
            encoding="utf-8"
        )
        index = (REPO_ROOT / "docs" / "HANDOFF_INDEX.md").read_text(
            encoding="utf-8"
        )
        agents = (REPO_ROOT / "AGENTS.md").read_text(encoding="utf-8")

        self.assertIn("class-4 symbol orchestration", features)
        self.assertIn("implemented but still bounded", features)
        self.assertNotIn("class-4 symbol bundles aren't wired", features)
        self.assertIn("69 active + 6 ignored", roadmap)
        self.assertLess(
            index.index("CONTACT_SURVIVAL_IMPLEMENTATION_PLAN.md"),
            index.index("## Supporting implementation and test documents"),
        )
        self.assertIn("final rev-B extension documents 16–18", agents)


if __name__ == "__main__":
    unittest.main()
