import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_release_governance.py"
SPEC = importlib.util.spec_from_file_location("check_release_governance", SCRIPT)
assert SPEC and SPEC.loader
GOVERNANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GOVERNANCE)


class ReleaseGovernanceTests(unittest.TestCase):
    def _runner(self, output: str = "", code: int = 0):
        def run(repo: Path, args: tuple[str, ...]):
            self.assertEqual(
                args,
                (
                    "ls-tree",
                    "-r",
                    "abc123",
                    "--",
                    "docs/spec/index.md",
                    "docs/internal/release-evidence/spec-reboot-report.md",
                ),
            )
            return code, output, "tree unavailable" if code else ""

        return run

    def _repo(self, root: Path) -> None:
        for relative in GOVERNANCE.REQUIRED_PUBLICATION_FILES:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"# {relative}\n", encoding="utf-8")

    def test_working_tree_and_recorded_tree_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            output = "\n".join(
                f"100644 blob {'a' * 40}\t{name}"
                for name in GOVERNANCE.REQUIRED_PUBLICATION_FILES
            ) + "\n"
            self.assertEqual(
                GOVERNANCE.validate_publication_artifacts(root, "abc123", self._runner(output)),
                [],
            )

    def test_missing_working_tree_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            (root / "docs" / "spec" / "index.md").unlink()
            errors = GOVERNANCE.validate_publication_artifacts(root)
            self.assertEqual(
                errors,
                ["publication artifact is missing or not a regular file: docs/spec/index.md"],
            )

    def test_empty_working_tree_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            (root / "docs" / "internal" / "release-evidence" / "spec-reboot-report.md").write_text("", encoding="utf-8")
            errors = GOVERNANCE.validate_publication_artifacts(root)
            self.assertEqual(errors, ["publication artifact must not be empty: docs/internal/release-evidence/spec-reboot-report.md"])

    def test_recorded_tree_missing_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            output = "100644 blob " + "a" * 40 + "\tdocs/spec/index.md\n"
            errors = GOVERNANCE.validate_publication_artifacts(
                root, "abc123", self._runner(output)
            )
            self.assertEqual(
                errors,
                ["recorded commit abc123 is missing publication artifact: docs/internal/release-evidence/spec-reboot-report.md"],
            )

    def test_tree_inspection_failure_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            errors = GOVERNANCE.validate_publication_artifacts(
                root, "abc123", self._runner(code=1)
            )
            self.assertEqual(
                errors,
                ["could not inspect publication tree at abc123: tree unavailable"],
            )

    def test_recorded_symlink_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            output = "\n".join(
                f"{'120000' if name == 'docs/spec/index.md' else '100644'} blob {'a' * 40}\t{name}"
                for name in GOVERNANCE.REQUIRED_PUBLICATION_FILES
            ) + "\n"
            errors = GOVERNANCE.validate_publication_artifacts(
                root, "abc123", self._runner(output)
            )
            self.assertEqual(
                errors,
                [
                    "recorded commit abc123 publication artifact is not a regular blob: docs/spec/index.md",
                    "recorded commit abc123 is missing publication artifact: docs/spec/index.md",
                ],
            )

    def test_symlinked_working_tree_artifact_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._repo(root)
            outside = root / "outside.md"
            outside.write_text("outside\n", encoding="utf-8")
            path = root / "docs" / "spec" / "index.md"
            path.unlink()
            try:
                path.symlink_to(outside)
            except (OSError, NotImplementedError):
                self.skipTest("symbolic links unavailable on this platform")
            errors = GOVERNANCE.validate_publication_artifacts(root)
            self.assertEqual(
                errors,
                ["publication artifact must not be a symbolic link: docs/spec/index.md"],
            )


if __name__ == "__main__":
    unittest.main()
