import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_mobile_binding.py"
SPEC = importlib.util.spec_from_file_location("check_mobile_binding", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def _kotlin(methods: tuple[str, ...]) -> str:
    body = "\n".join(
        (
            f"    @Throws(FfiException::class)\n"
            if method in CHECK.EXPECTED_MMP_NODE_THROWING_METHODS
            else ""
        )
        + f"    override fun `{method}`() {{}}"
        for method in methods
    )
    return f"interface MmpNodeInterface {{\n{body}\n}}\n"


def _swift(methods: tuple[str, ...]) -> str:
    body = "\n".join(
        f"    func {method}(){' throws' if method in CHECK.EXPECTED_MMP_NODE_THROWING_METHODS else ''}"
        for method in methods
    )
    return f"protocol MmpNodeProtocol {{\n{body}\n}}\n"


class MobileBindingTests(unittest.TestCase):
    def test_complete_kotlin_surface_and_secret_scan_pass(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "mmp_ffi.kt").write_text(
                "// prekey and ratchet are prose only\n" + _kotlin(CHECK.EXPECTED_MMP_NODE_METHODS),
                encoding="utf-8",
            )
            self.assertEqual(CHECK.scan_binding("kotlin", directory), [])

    def test_forbidden_identifier_fails_even_with_complete_api(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            (directory / "mmp_ffi.swift").write_text(
                _swift(CHECK.EXPECTED_MMP_NODE_METHODS) + "struct SnapshotVault {}\n",
                encoding="utf-8",
            )
            violations = CHECK.scan_binding("swift", directory)
            self.assertEqual(len(violations), 1)
            self.assertIn("SnapshotVault", violations[0])

    def test_missing_kotlin_throwing_contract_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            source = _kotlin(CHECK.EXPECTED_MMP_NODE_METHODS).replace(
                "    @Throws(FfiException::class)\n    override fun `setNickname`",
                "    override fun `setNickname`",
            )
            (directory / "mmp_ffi.kt").write_text(source, encoding="utf-8")
            violations = CHECK.scan_binding("kotlin", directory)
            self.assertEqual(len(violations), 1)
            self.assertIn("throwing annotation for MmpNode.setNickname", violations[0])

    def test_missing_swift_throwing_contract_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            source = _swift(CHECK.EXPECTED_MMP_NODE_METHODS).replace(
                "    func setNickname() throws",
                "    func setNickname()",
            )
            (directory / "mmp_ffi.swift").write_text(source, encoding="utf-8")
            violations = CHECK.scan_binding("swift", directory)
            self.assertEqual(len(violations), 1)
            self.assertIn("throws for MmpNode.setNickname", violations[0])

    def test_missing_language_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            self.assertTrue(CHECK.scan_binding("swift", Path(root)))


if __name__ == "__main__":
    unittest.main()
