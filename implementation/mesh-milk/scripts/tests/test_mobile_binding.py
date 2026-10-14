import importlib.util
import tempfile
import unittest
from pathlib import Path

try:
    from scripts.tests.mobile_fixtures import kotlin_surface, swift_surface
except ModuleNotFoundError:  # direct execution: python scripts/tests/test_*.py
    from mobile_fixtures import kotlin_surface, swift_surface

SCRIPT = Path(__file__).parents[1] / "check_mobile_binding.py"
SPEC = importlib.util.spec_from_file_location("check_mobile_binding", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class MobileBindingTests(unittest.TestCase):
    def test_complete_kotlin_surface_and_secret_scan_pass(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "mmp_ffi.kt").write_text(
                "// prekey and ratchet are prose only\n"
                + kotlin_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ),
                encoding="utf-8",
            )
            self.assertEqual(CHECK.scan_binding("kotlin", directory), [])

    def test_forbidden_identifier_fails_even_with_complete_api(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "mmp_ffi.swift").write_text(
                swift_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                )
                + "struct SnapshotVault {}\n",
                encoding="utf-8",
            )
            violations = CHECK.scan_binding("swift", directory)
            self.assertEqual(len(violations), 1)
            self.assertIn("SnapshotVault", violations[0])

    def test_missing_throwing_contract_is_rejected_for_each_language(self) -> None:
        cases = (
            (
                "kotlin",
                "mmp_ffi.kt",
                kotlin_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ).replace(
                    "    @Throws(FfiException::class)\n    override fun `setNickname`",
                    "    override fun `setNickname`",
                ),
                "throwing annotation for MmpNode.setNickname",
            ),
            (
                "swift",
                "mmp_ffi.swift",
                swift_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ).replace(
                    "    func setNickname() throws",
                    "    func setNickname()",
                ),
                "throws for MmpNode.setNickname",
            ),
        )
        for language, filename, source, expected in cases:
            with self.subTest(language=language):
                with tempfile.TemporaryDirectory() as root:
                    directory = Path(root)
                    (directory / filename).write_text(source, encoding="utf-8")
                    violations = CHECK.scan_binding(language, directory)
                    self.assertEqual(len(violations), 1)
                    self.assertIn(expected, violations[0])

    def test_missing_ffi_type_is_rejected_for_each_language(self) -> None:
        declarations = CHECK.EXPECTED_FFI_TYPE_DECLARATIONS
        reduced = tuple(item for item in declarations if item[0] != "FfiMetrics")
        cases = (
            (
                "kotlin",
                "mmp_ffi.kt",
                kotlin_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    reduced,
                ),
                "generated kotlin output is missing FFI type FfiMetrics",
            ),
            (
                "swift",
                "mmp_ffi.swift",
                swift_surface(
                    CHECK.EXPECTED_MMP_NODE_METHODS,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    reduced,
                ),
                "generated swift output is missing FFI type FfiMetrics",
            ),
        )
        for language, filename, source, expected in cases:
            with self.subTest(language=language):
                with tempfile.TemporaryDirectory() as root:
                    directory = Path(root)
                    (directory / filename).write_text(source, encoding="utf-8")
                    violations = CHECK.scan_binding(language, directory)
                    self.assertEqual(len(violations), 1)
                    self.assertIn(expected, violations[0])

    def test_missing_language_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            self.assertTrue(CHECK.scan_binding("swift", Path(root)))


if __name__ == "__main__":
    unittest.main()
