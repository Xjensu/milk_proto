import importlib.util
import re
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_mobile_api_surface.py"
SPEC = importlib.util.spec_from_file_location("check_mobile_api_surface", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


def _kotlin(methods: tuple[str, ...]) -> str:
    body = "\n".join(f"    override fun `{method}`() {{}}" for method in methods)
    return f"interface MmpNodeInterface {{\n{body}\n}}\n"


def _swift(methods: tuple[str, ...]) -> str:
    body = "\n".join(f"    func {method}()" for method in methods)
    return f"protocol MmpNodeProtocol {{\n{body}\n}}\n"


class MobileApiSurfaceTests(unittest.TestCase):
    def test_manifest_includes_event_queue_backpressure_accessor(self) -> None:
        self.assertIn("eventQueueDropped", CHECK.EXPECTED_MMP_NODE_METHODS)

    def test_manifest_matches_rust_uniffi_mmp_node_exports(self) -> None:
        rust_source = Path(__file__).parents[2] / "crates" / "mmp-ffi" / "src" / "lib.rs"
        source = rust_source.read_text(encoding="utf-8")
        marker = "#[uniffi::export]"
        start = source.index(marker)
        end = source.index("#[cfg(test)]", start)
        exported = tuple(
            re.findall(
                r"^\s*pub fn\s+([A-Za-z_]\w*)\s*\(",
                source[start:end],
                re.MULTILINE,
            )
        )
        constructors = {"new", "new_with_store_limit"}
        rust_methods = {
            name.split("_")[0]
            + "".join(part[:1].upper() + part[1:] for part in name.split("_")[1:])
            for name in exported
            if name not in constructors
        }
        manifest_methods = set(CHECK.EXPECTED_MMP_NODE_METHODS)
        self.assertEqual(len(CHECK.EXPECTED_MMP_NODE_METHODS), len(manifest_methods))
        self.assertEqual(manifest_methods, rust_methods)

    def test_accepts_complete_generated_surface(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "mmp_ffi.kt").write_text(_kotlin(methods), encoding="utf-8")
            (swift / "mmp_ffi.swift").write_text(_swift(methods), encoding="utf-8")
            self.assertEqual(CHECK.scan_bindings(kotlin, swift), [])

    def test_reports_missing_method_per_language(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "mmp_ffi.kt").write_text(_kotlin(methods[:-1]), encoding="utf-8")
            (swift / "mmp_ffi.swift").write_text(_swift(methods[:-1]), encoding="utf-8")
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("unsubscribeScope", violations[0])
            self.assertIn("unsubscribeScope", violations[1])

    def test_requires_language_specific_generated_sources(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "not-binding.txt").write_text("x", encoding="utf-8")
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("no .kt source", violations[0])
            self.assertIn("no .swift source", violations[1])


if __name__ == "__main__":
    unittest.main()
