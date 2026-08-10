import importlib.util
import stat
import tempfile
import unittest
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

try:
    from scripts.tests.check_mobile_api_surface import EXPECTED_MMP_NODE_THROWING_METHODS
except ModuleNotFoundError:  # direct execution: python scripts/tests/test_*.py
    import sys

    sys.path.insert(0, str(Path(__file__).parents[1]))
    from check_mobile_api_surface import EXPECTED_MMP_NODE_THROWING_METHODS
try:
    from scripts.tests.mobile_fixtures import kotlin_surface
except ModuleNotFoundError:  # direct execution: python scripts/tests/test_*.py
    from mobile_fixtures import kotlin_surface


SCRIPT = Path(__file__).parents[1] / "verify_android_artifacts.py"
SPEC = importlib.util.spec_from_file_location("verify_android_artifacts", SCRIPT)
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


def _kotlin() -> str:
    return kotlin_surface(
        VERIFY.EXPECTED_MMP_NODE_METHODS,
        EXPECTED_MMP_NODE_THROWING_METHODS,
    )


def _write_zip(path: Path, members: dict[str, bytes | str]) -> None:
    with ZipFile(path, "w", ZIP_DEFLATED) as archive:
        for name, payload in members.items():
            if isinstance(payload, str):
                payload = payload.encode("utf-8")
            archive.writestr(name, payload)


def _valid_aar_members() -> dict[str, bytes | str]:
    return {
        "AndroidManifest.xml": "<manifest package=\"org.meshmilk.ffi\" />",
        "jni/arm64-v8a/libmmp_ffi.so": b"arm64",
        "jni/armeabi-v7a/libmmp_ffi.so": b"armv7",
        "jni/x86_64/libmmp_ffi.so": b"x86_64",
    }


class MobileArtifactTests(unittest.TestCase):
    def test_native_only_aar_and_kotlin_source_zip_pass(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "mmp-ffi-kotlin-bindings.zip"
            _write_zip(aar, _valid_aar_members())
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            self.assertEqual(VERIFY.verify_artifacts(aar, bindings), [])

    def test_missing_required_abi_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            del members["jni/x86_64/libmmp_ffi.so"]
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("jni/x86_64/libmmp_ffi.so" in item for item in violations))

    def test_malformed_android_manifest_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            members["AndroidManifest.xml"] = b"<manifest"
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("not valid XML" in item for item in violations))

    def test_android_manifest_package_identity_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            members["AndroidManifest.xml"] = '<manifest package="org.example.wrong" />'
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("package must be org.meshmilk.ffi" in item for item in violations))

    def test_classes_jar_is_rejected_from_native_only_aar(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            members["classes.jar"] = b"bytecode"
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("classes.jar" in item for item in violations))

    def test_unexpected_native_aar_payload_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            members["META-INF/old-signature.SF"] = b"stale payload"
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("unexpected member" in item for item in violations))

    def test_zip_path_traversal_is_rejected_before_extraction(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            members["../outside.txt"] = b"must not extract"
            _write_zip(aar, members)
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("path traversal" in item for item in violations))

    def test_zip_symbolic_link_member_is_rejected_before_reading(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            members = _valid_aar_members()
            del members["jni/arm64-v8a/libmmp_ffi.so"]
            _write_zip(aar, members)
            with ZipFile(aar, "a", ZIP_DEFLATED) as archive:
                info = ZipInfo("jni/arm64-v8a/libmmp_ffi.so")
                info.create_system = 3
                info.external_attr = (stat.S_IFLNK | 0o777) << 16
                archive.writestr(info, b"../../outside/libmmp_ffi.so")
            _write_zip(bindings, {"mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("symbolic-link ZIP member" in item for item in violations))

    def test_absolute_kotlin_zip_path_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            _write_zip(aar, _valid_aar_members())
            _write_zip(bindings, {"/tmp/mmp_ffi.kt": _kotlin()})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("absolute path" in item for item in violations))

    def test_case_insensitive_kotlin_member_collision_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            _write_zip(aar, _valid_aar_members())
            source = _kotlin()
            _write_zip(bindings, {"Mmp_ffi.kt": source, "mmp_ffi.kt": source})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(
                any("case-insensitive duplicate ZIP member" in item for item in violations)
            )

    def test_kotlin_secret_surface_failure_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            _write_zip(aar, _valid_aar_members())
            _write_zip(
                bindings,
                {"mmp_ffi.kt": _kotlin() + "class SnapshotVault {}\n"},
            )

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("SnapshotVault" in item for item in violations))

    def test_kotlin_zip_without_source_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root)
            aar = directory / "mmp-ffi.aar"
            bindings = directory / "bindings.zip"
            _write_zip(aar, _valid_aar_members())
            _write_zip(bindings, {"README.txt": "source omitted"})

            violations = VERIFY.verify_artifacts(aar, bindings)
            self.assertTrue(any("no .kt source" in item for item in violations))


if __name__ == "__main__":
    unittest.main()
