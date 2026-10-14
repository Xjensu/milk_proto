import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "check_spec_status.py"
SPEC = importlib.util.spec_from_file_location("check_spec_status", SCRIPT)
assert SPEC and SPEC.loader
STATUS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STATUS)

REPO = Path(__file__).parents[4]
MANIFEST = REPO / "docs" / "spec" / "status-manifest.json"


class SpecificationStatusTests(unittest.TestCase):
    def test_checked_in_manifest_passes(self) -> None:
        self.assertEqual(STATUS.validate_manifest(MANIFEST, REPO), [])

    def test_document_digest_drift_is_rejected(self) -> None:
        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        document["documents"][0]["sha256"] = "0" * 64
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "status.json"
            manifest.write_text(json.dumps(document), encoding="utf-8")
            errors = STATUS.validate_manifest(manifest, REPO)
        self.assertEqual(errors, ["SHA-256 mismatch for docs/spec/ble-transport.md"])

    def test_status_marker_drift_is_rejected(self) -> None:
        document = json.loads(MANIFEST.read_text(encoding="utf-8"))
        document["documents"][0]["status_marker"] = "Status: Phase 1 draft"
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "status.json"
            manifest.write_text(json.dumps(document), encoding="utf-8")
            errors = STATUS.validate_manifest(manifest, REPO)
        self.assertEqual(
            errors,
            ["status marker missing from header of docs/spec/ble-transport.md"],
        )


if __name__ == "__main__":
    unittest.main()
