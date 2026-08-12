import unittest
from pathlib import Path


ROOT = Path(__file__).parents[2]
PACKAGE = ROOT / "platform" / "react-native-mesh"
BETA_SESSION = PACKAGE / "src" / "BetaTestSession.ts"
INDEX = PACKAGE / "src" / "index.ts"
README = PACKAGE / "README.md"


class BetaReportSchemaSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.session = BETA_SESSION.read_text(encoding="utf-8")
        cls.index = INDEX.read_text(encoding="utf-8")
        cls.readme = README.read_text(encoding="utf-8")

    def test_report_has_local_compatibility_fence(self) -> None:
        self.assertIn(
            "export const BETA_TEST_REPORT_SCHEMA_VERSION = 1 as const;",
            self.session,
        )
        self.assertIn(
            "schemaVersion: typeof BETA_TEST_REPORT_SCHEMA_VERSION;",
            self.session,
        )
        self.assertIn("schemaVersion: BETA_TEST_REPORT_SCHEMA_VERSION,", self.session)
        self.assertIn("BETA_TEST_REPORT_SCHEMA_VERSION", self.index)
        self.assertIn("schemaVersion` is the local report", self.readme)

    def test_beta_event_wait_is_bounded_and_cleans_up(self) -> None:
        self.assertIn("waitForEvent<T extends MmpEvent['type']>", self.session)
        self.assertIn("timeoutMs = 10_000", self.session)
        self.assertIn("timeoutMs < 1 || timeoutMs > 60_000", self.session)
        self.assertIn("BetaTestSessionError", self.session)
        self.assertIn("clearTimeout(timer)", self.session)
        self.assertIn("unsubscribe?.()", self.session)
        self.assertIn("Timed out waiting for event", self.session)


if __name__ == "__main__":
    unittest.main()
