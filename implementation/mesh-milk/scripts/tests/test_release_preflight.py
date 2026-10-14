import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]


class ReleasePreflightTests(unittest.TestCase):
    def _runner(
        self,
        status: str = "",
        cat_file_code: int = 0,
        head: str = "0123456",
        resolved_commit: str = "0123456",
    ):
        def run(repo: Path, args: tuple[str, ...]):
            if args[:2] == ("rev-parse", "--show-toplevel"):
                return 0, str(repo), ""
            if args[:2] == ("rev-parse", "HEAD"):
                return 0, f"{head}\n", ""
            if (
                len(args) == 2
                and args[0] == "rev-parse"
                and args[1].endswith("^{commit}")
            ):
                return 0, f"{resolved_commit}\n", ""
            if args[:2] == ("cat-file", "-e"):
                return cat_file_code, "", "missing commit" if cat_file_code else ""
            if args[0] == "status":
                return 0, status, ""
            raise AssertionError(f"unexpected git command: {args}")

        return run

    def test_clean_spec_tree_and_existing_commit_pass(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "docs" / "spec").mkdir(parents=True)
            self.assertEqual(validate_repository(repo, "0123456", self._runner()), [])

    def test_any_spec_status_line_blocks_release(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "docs" / "spec").mkdir(parents=True)
            errors = validate_repository(repo, "0123456", self._runner(" M docs/spec/protocol-model.md\n"))
            self.assertTrue(any("docs/spec/ has staged" in error for error in errors))

    def test_missing_recorded_commit_blocks_release(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "docs" / "spec").mkdir(parents=True)
            errors = validate_repository(repo, "deadbeef", self._runner(cat_file_code=1))
            self.assertTrue(any("recorded commit does not exist" in error for error in errors))

    def test_checked_out_head_must_match_recorded_commit(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "docs" / "spec").mkdir(parents=True)
            errors = validate_repository(
                repo,
                "0123456",
                self._runner(head="abcdef0", resolved_commit="0123456"),
            )
            self.assertTrue(
                any("checked-out HEAD does not match recorded commit" in error for error in errors),
                errors,
            )

    def test_oversized_manifest_commit_lookup_is_bounded(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import MAX_MANIFEST_BYTES, _manifest_commit

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            path.write_bytes(b"{" + b"x" * MAX_MANIFEST_BYTES + b"}")
            self.assertEqual(_manifest_commit(path), "")

    def test_git_runner_converts_hung_command_to_bounded_failure(self) -> None:
        import sys
        from unittest.mock import patch

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import _run_git

        timeout = subprocess.TimeoutExpired(["git", "status"], 30)
        with patch("validate_release_preflight.subprocess.run", side_effect=timeout) as run:
            code, output, detail = _run_git(Path("/repo"), ("status",))
        self.assertEqual(code, 124)
        self.assertEqual(output, "")
        self.assertIn("timed out", detail)
        run.assert_called_once()
        self.assertEqual(run.call_args.kwargs["timeout"], 30)


if __name__ == "__main__":
    unittest.main()
