import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_mobile_secret_surface.py"
SPEC = importlib.util.spec_from_file_location("check_mobile_secret_surface", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class MobileSecretSurfaceTests(unittest.TestCase):
    def test_allows_normal_generated_api_and_prose(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "MmpNode.kt").write_text(
                """class MmpNode {
  // prekey and ratchet are protocol concepts, not exported state
  fun sendText(peer: String, text: String): String
}
""",
                encoding="utf-8",
            )
            self.assertEqual(CHECK.scan_directory(directory), [])

    def test_rejects_secret_state_types_and_methods(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "MmpNode.swift").write_text(
                """struct SnapshotVault {}
func restoreSecretState(_ bytes: Data) {}
""",
                encoding="utf-8",
            )
            violations = CHECK.scan_directory(directory)
            self.assertEqual(len(violations), 2)
            self.assertIn("SnapshotVault", violations[0])
            self.assertIn("restoreSecretState", violations[1])

    def test_requires_nonempty_source_directory(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            self.assertTrue(CHECK.scan_directory(Path(root)))

    def test_current_uniffi_exports_contain_no_secret_state_identifiers(self) -> None:
        """Keep the Rust export boundary closed as the native vault evolves."""

        rust_source = Path(__file__).parents[2] / "crates" / "mmp-ffi" / "src" / "lib.rs"
        source = rust_source.read_text(encoding="utf-8")
        start = source.index("#[uniffi::export]")
        end = source.index("#[cfg(test)]", start)
        exported_block = source[start:end]
        violations = [match.group(0) for match in CHECK._FORBIDDEN.finditer(exported_block)]
        self.assertEqual(violations, [])


if __name__ == "__main__":
    unittest.main()
