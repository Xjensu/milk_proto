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


if __name__ == "__main__":
    unittest.main()
