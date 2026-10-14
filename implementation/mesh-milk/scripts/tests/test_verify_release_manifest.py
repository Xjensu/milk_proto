import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).parents[1]


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CREATE = _load("create_release_manifest_for_verify_tests", SCRIPTS / "create_release_manifest.py")
VERIFY = _load("verify_release_manifest_for_tests", SCRIPTS / "verify_release_manifest.py")


class VerifyReleaseManifestTests(unittest.TestCase):
    def _package(self, root: Path) -> Path:
        artifact = root / "artifact"
        signature = root / "signature"
        certificate = root / "certificate"
        artifact.write_bytes(b"artifact-v1")
        signature.write_bytes(b"signature-v1")
        certificate.write_bytes(b"certificate-v1")
        manifest = CREATE.build_manifest(
            artifact,
            signature,
            certificate,
            repository="Xjensu/milk_proto",
            commit="a" * 40,
            ref="refs/tags/v1.2.3",
        )
        output = root / "release.json"
        CREATE.write_manifest(manifest, output)
        return output

    def _document(self, manifest_path: Path) -> dict:
        return json.loads(manifest_path.read_text(encoding="utf-8"))

    def _write_document(self, manifest_path: Path, document: dict) -> None:
        manifest_path.write_text(json.dumps(document), encoding="utf-8")

    def test_valid_package_and_expected_source_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            self.assertEqual(
                VERIFY.verify_manifest(
                    manifest,
                    directory=root,
                    repository="Xjensu/milk_proto",
                    commit="a" * 40,
                    ref="refs/tags/v1.2.3",
                ),
                [],
            )

    def test_digest_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            # Keep the size unchanged so this specifically exercises the
            # digest check rather than stopping at the earlier size check.
            (root / "artifact").write_bytes(b"tampered-v1")
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertTrue(any("files.artifact.sha256 mismatch" in error for error in errors))

    def test_expected_source_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            errors = VERIFY.verify_manifest(manifest, directory=root, commit="b" * 40)
            self.assertEqual(errors, ["source.commit does not match expected value"])

    def test_manifest_format_is_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            document = self._document(manifest)
            document["format"] = "mesh-milk-release-manifest-v0"
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(errors, ["manifest format must be mesh-milk-release-manifest-v1"])

    def test_manifest_schema_is_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            document = self._document(manifest)
            document["unexpected"] = True
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertTrue(any("manifest root has invalid members" in error for error in errors))

    def test_zero_length_file_record_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            document = self._document(manifest)
            document["files"]["artifact"]["size"] = 0
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(errors, ["files.artifact.size must be a positive integer"])

    def test_oversized_manifest_is_rejected_before_json_parse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "release.json"
            manifest.write_bytes(b"{" + b"x" * VERIFY.MAX_MANIFEST_BYTES + b"}")
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(
                errors,
                [f"manifest exceeds maximum size of {VERIFY.MAX_MANIFEST_BYTES} bytes"],
            )

    def test_manifest_path_traversal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            document = self._document(manifest)
            document["files"]["artifact"]["name"] = "../outside"
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(
                errors,
                ["files.artifact.name must be a relative regular-file basename"],
            )

    def test_case_insensitive_file_name_collision_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            document = self._document(manifest)
            document["files"]["certificate"]["name"] = "ARTIFACT"
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(
                errors,
                [
                    "files contains case-insensitive duplicate file name: "
                    "ARTIFACT conflicts with artifact"
                ],
            )

    def test_manifest_symlink_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self._package(root)
            alias = root / "artifact-alias"
            try:
                alias.symlink_to(root / "artifact")
            except (OSError, NotImplementedError):
                self.skipTest("symbolic links unavailable on this platform")
            document = self._document(manifest)
            document["files"]["artifact"]["name"] = alias.name
            self._write_document(manifest, document)
            errors = VERIFY.verify_manifest(manifest, directory=root)
            self.assertEqual(
                errors,
                ["files.artifact resolves through a symbolic link: artifact-alias"],
            )


if __name__ == "__main__":
    unittest.main()
