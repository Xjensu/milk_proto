import importlib.util
import os
import plistlib
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPT = Path(__file__).parents[1] / "verify_ios_artifacts.py"
SPEC = importlib.util.spec_from_file_location("verify_ios_artifacts", SCRIPT)
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)
from check_mobile_api_surface import EXPECTED_MMP_NODE_METHODS


def _write_fixture(root: Path, *, include_simulator: bool = True) -> tuple[Path, Path]:
    xcframework = root / "MmpCore.xcframework"
    xcframework.mkdir()
    libraries = [
        {
            "LibraryIdentifier": "ios-arm64",
            "LibraryPath": "libmmp_ffi.a",
            "HeadersPath": "Headers",
            "SupportedArchitectures": ["arm64"],
            "SupportedPlatform": "ios",
        }
    ]
    if include_simulator:
        libraries.append(
            {
                "LibraryIdentifier": "ios-arm64-simulator",
                "LibraryPath": "libmmp_ffi.a",
                "HeadersPath": "Headers",
                "SupportedArchitectures": ["arm64"],
                "SupportedPlatform": "ios",
                "SupportedPlatformVariant": "simulator",
            }
        )
    with (xcframework / "Info.plist").open("wb") as stream:
        plistlib.dump(
            {"CFBundlePackageType": "XFWK", "AvailableLibraries": libraries},
            stream,
        )
    for item in libraries:
        slice_root = xcframework / item["LibraryIdentifier"]
        headers = slice_root / "Headers"
        headers.mkdir(parents=True)
        (headers / "mmp_ffi.h").write_text("void mmp_ffi(void);\n", encoding="utf-8")
        (slice_root / "libmmp_ffi.a").write_bytes(b"archive")

    bindings = root / "swift-bindings"
    bindings.mkdir()
    methods = "\n".join(
        f"    func {method}() {{}}"
        for method in EXPECTED_MMP_NODE_METHODS
    )
    (bindings / "mmp_ffi.swift").write_text(
        f"public protocol MmpNodeProtocol {{\n{methods}\n}}\n",
        encoding="utf-8",
    )
    return xcframework, bindings


class IOSArtifactTests(unittest.TestCase):
    def test_device_and_simulator_fixture_passes(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            self.assertEqual(VERIFY.verify_artifacts(xcframework, bindings), [])

    def test_missing_simulator_slice_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root), include_simulator=False)
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("ios-arm64-simulator" in item for item in violations))

    def test_empty_static_library_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            (xcframework / "ios-arm64" / "libmmp_ffi.a").write_bytes(b"")
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("static library is empty" in item for item in violations))

    def test_slice_without_arm64_architecture_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            plist["AvailableLibraries"][0]["SupportedArchitectures"] = ["x86_64"]
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("SupportedArchitectures" in item for item in violations))

    def test_unsafe_plist_library_path_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            plist["AvailableLibraries"][0]["LibraryPath"] = "../outside.a"
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("escapes its slice" in item for item in violations))

    def test_symlinked_library_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            xcframework, bindings = _write_fixture(directory)
            target = directory / "outside.a"
            target.write_bytes(b"not part of the slice")
            library = xcframework / "ios-arm64" / "libmmp_ffi.a"
            library.unlink()
            try:
                os.symlink(target, library)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symbolic links unavailable: {exc}")

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("must not traverse a symbolic link" in item for item in violations))

    def test_symlinked_info_plist_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            xcframework, bindings = _write_fixture(directory)
            target = directory / "outside.plist"
            target.write_bytes((xcframework / "Info.plist").read_bytes())
            plist_path = xcframework / "Info.plist"
            plist_path.unlink()
            try:
                os.symlink(target, plist_path)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symbolic links unavailable: {exc}")

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("Info.plist must not be a symbolic link" in item for item in violations))

    def test_symlinked_header_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            xcframework, bindings = _write_fixture(directory)
            target = directory / "outside.h"
            target.write_text("void outside(void);\n", encoding="utf-8")
            header = xcframework / "ios-arm64" / "Headers" / "mmp_ffi.h"
            header.unlink()
            try:
                os.symlink(target, header)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symbolic links unavailable: {exc}")

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("headers directory contains a symbolic link" in item for item in violations))

    def test_symlinked_swift_binding_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            xcframework, bindings = _write_fixture(directory)
            target = directory / "outside.swift"
            target.write_text((bindings / "mmp_ffi.swift").read_text(encoding="utf-8"), encoding="utf-8")
            source = bindings / "mmp_ffi.swift"
            source.unlink()
            try:
                os.symlink(target, source)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symbolic links unavailable: {exc}")

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(
                any("Swift bindings directory contains a symbolic link" in item for item in violations)
            )

    def test_swift_binding_link_scan_is_enforced_before_source_read(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            with patch.object(VERIFY, "_first_symlink", return_value=bindings / "alias.swift"):
                violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(
                any("Swift bindings directory contains a symbolic link" in item for item in violations)
            )

    def test_duplicate_slice_identifier_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            plist["AvailableLibraries"].append(dict(plist["AvailableLibraries"][0]))
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("duplicate slice: ios-arm64" in item for item in violations))

    def test_case_insensitive_slice_identifier_collision_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            collision = dict(plist["AvailableLibraries"][0])
            collision["LibraryIdentifier"] = "IOS-ARM64"
            plist["AvailableLibraries"].append(collision)
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(
                any("case-insensitive slice identifier collision" in item for item in violations)
            )

    def test_noncanonical_library_path_casing_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            plist["AvailableLibraries"][0]["LibraryPath"] = "LIBMMP_FFI.A"
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            # On a case-sensitive host the path is simply absent; on a
            # case-insensitive host the canonical-casing fence is the proof.
            self.assertTrue(
                any(
                    "static library is missing" in item
                    or "non-canonical path casing" in item
                    for item in violations
                )
            )

    def test_unexpected_slice_identifier_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            plist_path = xcframework / "Info.plist"
            with plist_path.open("rb") as stream:
                plist = plistlib.load(stream)
            extra = {
                "LibraryIdentifier": "ios-x86_64-simulator",
                "LibraryPath": "libmmp_ffi.a",
                "HeadersPath": "Headers",
                "SupportedArchitectures": ["x86_64"],
                "SupportedPlatform": "ios",
                "SupportedPlatformVariant": "simulator",
            }
            plist["AvailableLibraries"].append(extra)
            with plist_path.open("wb") as stream:
                plistlib.dump(plist, stream)

            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("unexpected slice: ios-x86_64-simulator" in item for item in violations))

    def test_empty_swift_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            (bindings / "mmp_ffi.swift").write_text("", encoding="utf-8")
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("Swift source is empty" in item for item in violations))

    def test_swift_binding_missing_api_method_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            source = bindings / "mmp_ffi.swift"
            text = source.read_text(encoding="utf-8")
            text = text.replace("    func flush() {}\n", "")
            source.write_text(text, encoding="utf-8")
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("MmpNode.flush" in item for item in violations))

    def test_swift_binding_secret_surface_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            xcframework, bindings = _write_fixture(Path(root))
            source = bindings / "mmp_ffi.swift"
            source.write_text(
                source.read_text(encoding="utf-8") + "struct SnapshotVault {}\n",
                encoding="utf-8",
            )
            violations = VERIFY.verify_artifacts(xcframework, bindings)
            self.assertTrue(any("SnapshotVault" in item for item in violations))


if __name__ == "__main__":
    unittest.main()
