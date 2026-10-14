import importlib.util
import re
import tempfile
import unittest
from pathlib import Path

try:
    from scripts.tests.mobile_fixtures import kotlin_surface, swift_surface
except ModuleNotFoundError:  # direct execution: python scripts/tests/test_*.py
    from mobile_fixtures import kotlin_surface, swift_surface

SCRIPT = Path(__file__).parents[1] / "check_mobile_api_surface.py"
SPEC = importlib.util.spec_from_file_location("check_mobile_api_surface", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class MobileApiSurfaceTests(unittest.TestCase):
    def test_manifest_includes_event_queue_backpressure_accessor(self) -> None:
        self.assertIn("eventQueueDropped", CHECK.EXPECTED_MMP_NODE_METHODS)

    def test_manifest_includes_fallible_nickname_lookup(self) -> None:
        self.assertIn("nicknameOf", CHECK.EXPECTED_MMP_NODE_METHODS)
        self.assertIn("nicknameOf", CHECK.EXPECTED_MMP_NODE_THROWING_METHODS)

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

    def test_type_manifest_matches_public_rust_records_and_enums(self) -> None:
        rust_source = Path(__file__).parents[2] / "crates" / "mmp-ffi" / "src" / "lib.rs"
        source = rust_source.read_text(encoding="utf-8")
        rust_types = {
            (name, "enum" if kind == "enum" else "record")
            for kind, name in re.findall(r"^pub\s+(enum|struct)\s+([A-Za-z_]\w*)", source, re.MULTILINE)
            if name != "MmpNode"
        }
        self.assertEqual(
            set(CHECK.EXPECTED_FFI_TYPE_DECLARATIONS),
            rust_types,
        )

    def test_throwing_manifest_matches_rust_result_exports(self) -> None:
        rust_source = Path(__file__).parents[2] / "crates" / "mmp-ffi" / "src" / "lib.rs"
        source = rust_source.read_text(encoding="utf-8")
        marker = "#[uniffi::export]"
        start = source.index(marker)
        end = source.index("#[cfg(test)]", start)
        block = source[start:end]

        # A small signature scanner is enough here: each exported method's
        # opening brace follows its complete declaration, and no parameter
        # type in this FFI surface contains a brace.  Keeping this assertion
        # source-derived prevents a future Result-returning method from being
        # added to the name manifest without also gaining the host-language
        # throws contract.
        result_exports: set[str] = set()
        for match in re.finditer(r"(?m)^\s*pub fn\s+([A-Za-z_]\w*)\s*\(", block):
            brace = block.find("{", match.end())
            self.assertNotEqual(brace, -1, match.group(1))
            signature = block[match.start() : brace]
            if (
                match.group(1) not in {"new", "new_with_store_limit"}
                and "-> Result<" in signature
                and "FfiError" in signature
            ):
                snake = match.group(1)
                camel = snake.split("_")[0] + "".join(
                    part[:1].upper() + part[1:] for part in snake.split("_")[1:]
                )
                result_exports.add(camel)

        self.assertEqual(set(CHECK.EXPECTED_MMP_NODE_THROWING_METHODS), result_exports)

    def test_accepts_complete_generated_surface(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "mmp_ffi.kt").write_text(
                kotlin_surface(
                    methods,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ),
                encoding="utf-8",
            )
            (swift / "mmp_ffi.swift").write_text(
                swift_surface(
                    methods,
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ),
                encoding="utf-8",
            )
            self.assertEqual(CHECK.scan_bindings(kotlin, swift), [])

    def test_reports_missing_method_per_language(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "mmp_ffi.kt").write_text(
                kotlin_surface(
                    methods[:-1],
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ),
                encoding="utf-8",
            )
            (swift / "mmp_ffi.swift").write_text(
                swift_surface(
                    methods[:-1],
                    CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                    CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
                ),
                encoding="utf-8",
            )
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("unsubscribeScope", violations[0])
            self.assertIn("unsubscribeScope", violations[1])

    def test_reports_missing_throwing_contract_per_language(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            kotlin_source = kotlin_surface(
                methods,
                CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
            ).replace(
                "    @Throws(FfiException::class)\n    override fun `setNickname`",
                "    override fun `setNickname`",
            )
            swift_source = swift_surface(
                methods,
                CHECK.EXPECTED_MMP_NODE_THROWING_METHODS,
                CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
            ).replace(
                "    func setNickname() throws",
                "    func setNickname()",
            )
            (kotlin / "mmp_ffi.kt").write_text(kotlin_source, encoding="utf-8")
            (swift / "mmp_ffi.swift").write_text(swift_source, encoding="utf-8")
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("throwing annotation for MmpNode.setNickname", violations[0])
            self.assertIn("throws for MmpNode.setNickname", violations[1])

    def test_reports_missing_nickname_lookup_throwing_contract_per_language(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        throwing_methods = CHECK.EXPECTED_MMP_NODE_THROWING_METHODS
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            kotlin_source = kotlin_surface(
                methods,
                throwing_methods,
                CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
            ).replace(
                "    @Throws(FfiException::class)\n    override fun `nicknameOf`",
                "    override fun `nicknameOf`",
            )
            swift_source = swift_surface(
                methods,
                throwing_methods,
                CHECK.EXPECTED_FFI_TYPE_DECLARATIONS,
            ).replace(
                "    func nicknameOf() throws",
                "    func nicknameOf()",
            )
            (kotlin / "mmp_ffi.kt").write_text(kotlin_source, encoding="utf-8")
            (swift / "mmp_ffi.swift").write_text(swift_source, encoding="utf-8")
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("throwing annotation for MmpNode.nicknameOf", violations[0])
            self.assertIn("throws for MmpNode.nicknameOf", violations[1])

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

    def test_reports_missing_ffi_type_per_language(self) -> None:
        methods = CHECK.EXPECTED_MMP_NODE_METHODS
        declarations = CHECK.EXPECTED_FFI_TYPE_DECLARATIONS
        missing_name = "FfiBetaSnapshot"
        reduced = tuple(item for item in declarations if item[0] != missing_name)
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            kotlin = root_path / "kotlin"
            swift = root_path / "swift"
            kotlin.mkdir()
            swift.mkdir()
            (kotlin / "mmp_ffi.kt").write_text(
                kotlin_surface(methods, CHECK.EXPECTED_MMP_NODE_THROWING_METHODS, reduced),
                encoding="utf-8",
            )
            (swift / "mmp_ffi.swift").write_text(
                swift_surface(methods, CHECK.EXPECTED_MMP_NODE_THROWING_METHODS, reduced),
                encoding="utf-8",
            )
            violations = CHECK.scan_bindings(kotlin, swift)
            self.assertEqual(len(violations), 2)
            self.assertIn("generated kotlin output is missing FFI type FfiBetaSnapshot", violations[0])
            self.assertIn("generated swift output is missing FFI type FfiBetaSnapshot", violations[1])


if __name__ == "__main__":
    unittest.main()
