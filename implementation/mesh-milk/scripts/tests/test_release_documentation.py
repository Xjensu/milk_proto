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

    def test_store_compliance_dossier_and_release_status_are_consistent(self) -> None:
        dossier = (REPO_ROOT / "docs" / "E8-S3_STORE_COMPLIANCE.md").read_text(
            encoding="utf-8"
        )
        backlog = (REPO_ROOT / "docs" / "AGILE_BACKLOG.md").read_text(
            encoding="utf-8"
        )
        roadmap = (REPO_ROOT / "docs" / "ROADMAP_TO_PRODUCTION.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("# E8-S3 Store-Compliance Dossier", dossier)
        self.assertIn("### BGTaskScheduler scope", dossier)
        self.assertIn("does **not** claim an iOS BGTask entitlement", dossier)
        self.assertIn("iOS 26", dossier)
        normalized_dossier = " ".join(dossier.split())
        self.assertIn(
            "does not claim that `mmp-ffi` is a compliant mobile application",
            normalized_dossier,
        )
        self.assertIn("DOSSIER DONE (E8-S3, 2026-08-09)", roadmap)
        self.assertIn("does not use `BGTaskScheduler`", roadmap)
        self.assertIn("E8-S3 · Store-compliance dossier", backlog)
        self.assertIn("DOSSIER DONE (2026-08-09; app-store gates remain open)", backlog)

    def test_backlog_peripheral_status_does_not_reopen_shipped_role(self) -> None:
        backlog = (REPO_ROOT / "docs" / "AGILE_BACKLOG.md").read_text(
            encoding="utf-8"
        )
        remaining = (REPO_ROOT / "docs" / "AGILE_REMAINING_WORK.md").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            "host-testable peripheral role shipped; Linux/BlueZ runtime and real exchange remain open",
            backlog,
        )
        self.assertNotIn("API complete; peripheral role missing (E3-S1)", backlog)
        self.assertIn(
            "E3-S1 host-testable implementation shipped; Linux/BlueZ",
            remaining,
        )

    def test_backlog_react_native_status_matches_shipped_codegen_metadata(self) -> None:
        backlog = (REPO_ROOT / "docs" / "AGILE_BACKLOG.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("PACKAGE + FIXTURE TYPECHECK + CODEGEN METADATA SHIPPED", backlog)
        self.assertIn("Codegen metadata `96c96c1`", backlog)
        self.assertIn("native consumer declarations/runtime", backlog)
        self.assertNotIn("Still open: native React Native declarations/codegen", backlog)
        self.assertNotIn("under a new `platforms/` directory", backlog)


if __name__ == "__main__":
    unittest.main()
