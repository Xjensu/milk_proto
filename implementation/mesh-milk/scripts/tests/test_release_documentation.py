import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class ReleaseDocumentationTests(unittest.TestCase):
    def test_security_dossier_does_not_restate_closed_release_gaps(self) -> None:
        dossier = (REPO_ROOT / "docs" / "SECURITY_REVIEW_DOSSIER.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("Current-state refresh", dossier)
        self.assertIn("Clippy is wired into CI", dossier)
        self.assertIn('rust-version = "1.96"', dossier)
        self.assertIn("noncanonical_s_negative.json", dossier)
        self.assertIn(
            "`AuthorityStore` non-secret state is durably validated and persisted",
            dossier,
        )
        self.assertNotIn("Clippy is not yet CI-gated", dossier)
        self.assertNotIn("No MSRV pin", dossier)
        self.assertNotIn("has no durable persistence", dossier)

    def test_roadmap_reflects_mobile_bindings_ci_job(self) -> None:
        roadmap = (REPO_ROOT / "docs" / "ROADMAP_TO_PRODUCTION.md").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "UniFFI bindgen artifacts (Kotlin/Swift) are produced by the `mobile-bindings` CI job",
            roadmap,
        )
        self.assertNotIn(
            "UniFFI bindgen artifacts (Kotlin/Swift) are not produced in CI",
            roadmap,
        )


if __name__ == "__main__":
    unittest.main()
