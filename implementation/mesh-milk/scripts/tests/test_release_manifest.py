import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "create_release_manifest.py"
SPEC = importlib.util.spec_from_file_location("create_release_manifest", SCRIPT)
assert SPEC and SPEC.loader
MANIFEST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MANIFEST)


class ReleaseManifestTests(unittest.TestCase):
    def test_manifest_records_stable_hashes_and_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "mmp-cli-linux-x86_64"
            signature = root / f"{artifact.name}.sig"
            certificate = root / f"{artifact.name}.pem"
            artifact.write_bytes(b"binary-v1")
            signature.write_bytes(b"signature-v1")
            certificate.write_bytes(b"certificate-v1")

            manifest = MANIFEST.build_manifest(
                artifact,
                signature,
                certificate,
                repository="Xjensu/milk_proto",
                commit="a" * 40,
                ref="refs/tags/v1.2.3",
            )
            output = root / "release.json"
            MANIFEST.write_manifest(manifest, output)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8")), manifest)
            self.assertEqual(manifest["format"], "mesh-milk-release-manifest-v1")
            self.assertEqual(manifest["source"]["ref"], "refs/tags/v1.2.3")
            self.assertEqual(manifest["files"]["artifact"]["size"], len(b"binary-v1"))
            self.assertEqual(
                manifest["files"]["artifact"]["sha256"],
                "ce6ad72522124416f55e00535b3d7a87231fa76caea47a2e9a161fe6aee45756",
            )

    def test_manifest_rejects_symlinked_input(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "artifact"
            signature = root / "artifact.sig"
            certificate = root / "artifact.pem"
            artifact.write_bytes(b"artifact")
            signature.write_bytes(b"signature")
            certificate.write_bytes(b"certificate")
            alias = root / "alias"
            try:
                alias.symlink_to(artifact)
            except (OSError, NotImplementedError):
                self.skipTest("symbolic links unavailable on this platform")
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                MANIFEST.build_manifest(
                    alias,
                    signature,
                    certificate,
                    repository="repo",
                    commit="commit",
                    ref="ref",
                )

    def test_manifest_rejects_empty_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = {
                "artifact": root / "artifact",
                "signature": root / "artifact.sig",
                "certificate": root / "artifact.pem",
            }
            for path in paths.values():
                path.write_bytes(b"non-empty")

            for kind, path in paths.items():
                path.write_bytes(b"")
                with self.subTest(kind=kind):
                    with self.assertRaisesRegex(ValueError, rf"{kind} must not be empty"):
                        MANIFEST.build_manifest(
                            paths["artifact"],
                            paths["signature"],
                            paths["certificate"],
                            repository="repo",
                            commit="a" * 40,
                            ref="refs/tags/v1.2.3",
                        )
                path.write_bytes(b"non-empty")

    def test_manifest_requires_source_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = []
            for name in ("artifact", "signature", "certificate"):
                path = root / name
                path.write_bytes(name.encode())
                files.append(path)
            with self.assertRaisesRegex(ValueError, "commit"):
                MANIFEST.build_manifest(
                    *files,
                    repository="repo",
                    commit="",
                    ref="ref",
                )

    def test_manifest_rejects_source_identity_the_verifier_would_reject(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = []
            for name in ("artifact", "signature", "certificate"):
                path = root / name
                path.write_bytes(name.encode())
                files.append(path)

            with self.assertRaisesRegex(ValueError, "40-64 lowercase hexadecimal"):
                MANIFEST.build_manifest(
                    *files,
                    repository="repo",
                    commit="not-a-commit",
                    ref="refs/tags/v1.2.3",
                )
            with self.assertRaisesRegex(ValueError, "must start with refs/"):
                MANIFEST.build_manifest(
                    *files,
                    repository="repo",
                    commit="a" * 40,
                    ref="tag-v1.2.3",
                )


if __name__ == "__main__":
    unittest.main()
