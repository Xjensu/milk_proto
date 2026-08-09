import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[4]


class InteropGuideTests(unittest.TestCase):
    def test_pc_ble_guide_matches_supported_role_matrix(self) -> None:
        guide = (REPO_ROOT / "docs" / "INTEROPERABILITY_TEST_GUIDE.md").read_text(
            encoding="utf-8"
        )

        # The code has a Linux/BlueZ peripheral backend and a btleplug central
        # backend for Linux, Windows, and macOS. Keep the guide from silently
        # narrowing the supported cross-platform test matrix to two Linux hosts.
        self.assertIn(
            "A real Linux/BlueZ peripheral and a real Linux/Windows/macOS central",
            guide,
        )
        self.assertIn(
            "may run Linux, Windows, or macOS through the",
            guide,
        )
        self.assertIn(
            "The current peripheral implementation is not\navailable on Windows or macOS",
            guide,
        )
        self.assertNotIn(
            "A real Linux/BlueZ peripheral and a real Linux/BlueZ central",
            guide,
        )

    def test_pc_ble_guide_passes_snapshot_files_not_directories(self) -> None:
        guide = (REPO_ROOT / "docs" / "INTEROPERABILITY_TEST_GUIDE.md").read_text(
            encoding="utf-8"
        )

        # SnapshotStore::persist replaces a file and its sibling .tmp recovery
        # candidate. Creating the --store path as a directory makes the run
        # fail closed on shutdown, so the manual procedure must use file paths.
        self.assertIn("--store /tmp/mmp-alice.snapshot", guide)
        self.assertIn("--store /tmp/mmp-bob.snapshot", guide)
        self.assertIn("`--store` is not a directory", guide)
        self.assertNotIn("mkdir -p /tmp/mmp-alice.store", guide)
        self.assertNotIn("mkdir -p /tmp/mmp-bob.store", guide)

    def test_release_preflight_is_described_as_revision_exact(self) -> None:
        guide = (REPO_ROOT / "docs" / "INTEROPERABILITY_TEST_GUIDE.md").read_text(
            encoding="utf-8"
        )
        manual = (REPO_ROOT / "docs" / "MANUAL_INTEROP_TESTING.md").read_text(
            encoding="utf-8"
        )

        # The repository-side wrapper binds evidence to the checkout's
        # current revision; documentation must not imply that object existence
        # alone is sufficient.
        self.assertIn("requires it to equal the checkout's current `HEAD`", guide)
        self.assertIn("requires it to equal the checkout's current `HEAD`", manual)


if __name__ == "__main__":
    unittest.main()
