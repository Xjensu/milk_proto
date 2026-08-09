import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]


class ReleasePreflightTests(unittest.TestCase):
    def _runner(self, status: str = "", cat_file_code: int = 0):
        def run(repo: Path, args: tuple[str, ...]):
            if args[:2] == ("rev-parse", "--show-toplevel"):
                return 0, str(repo), ""
            if args[:2] == ("cat-file", "-e"):
                return cat_file_code, "", "missing commit" if cat_file_code else ""
            if args[0] == "status":
                return 0, status, ""
            raise AssertionError(f"unexpected git command: {args}")

        return run

    def test_clean_specification_and_existing_commit_pass(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "specification").mkdir()
            self.assertEqual(validate_repository(repo, "0123456", self._runner()), [])

    def test_any_specification_status_line_blocks_release(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "specification").mkdir()
            errors = validate_repository(repo, "0123456", self._runner(" M specification/03-protocol-spec.md\n"))
            self.assertTrue(any("specification/ has staged" in error for error in errors))

    def test_missing_recorded_commit_blocks_release(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import validate_repository

        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "specification").mkdir()
            errors = validate_repository(repo, "deadbeef", self._runner(cat_file_code=1))
            self.assertTrue(any("recorded commit does not exist" in error for error in errors))

    def test_oversized_manifest_commit_lookup_is_bounded(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_release_preflight import MAX_MANIFEST_BYTES, _manifest_commit

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            path.write_bytes(b"{" + b"x" * MAX_MANIFEST_BYTES + b"}")
            self.assertEqual(_manifest_commit(path), "")


if __name__ == "__main__":
    unittest.main()
