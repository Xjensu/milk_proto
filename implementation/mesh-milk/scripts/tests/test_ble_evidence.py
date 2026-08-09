import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]


class BleEvidenceTests(unittest.TestCase):
    def _manifest(self, root: Path) -> Path:
        (root / "peripheral.log").write_text(
            "advertising\nEstablished peer\nMessageReceived\nstore persisted to disk\n",
            encoding="utf-8",
        )
        (root / "central.log").write_text(
            "dialed peer -> link\nEstablished peer\nDelivered id=abcd\nstore persisted to disk\n",
            encoding="utf-8",
        )
        (root / "peripheral.inspect").write_text("bundle inventory\n", encoding="utf-8")
        (root / "central.inspect").write_text("bundle inventory\n", encoding="utf-8")
        manifest = {
            "schema": "mmp-ble-evidence-v1",
            "run_id": "linux-pair-01",
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "worktree_clean": True,
            "toolchain": {"rustc": "rustc 1.96.0", "cargo": "cargo 1.96.0"},
            "hosts": [
                {
                    "name": "alice",
                    "role": "peripheral",
                    "os": "Ubuntu 24.04",
                    "kernel": "6.8.0",
                    "adapter": "adapter-a",
                    "seed_fingerprint": "sha256:aaaaaaaa",
                    "log": "peripheral.log",
                    "markers": ["advertising", "Established", "MessageReceived"],
                },
                {
                    "name": "bob",
                    "role": "central",
                    "os": "Ubuntu 24.04",
                    "kernel": "6.8.0",
                    "adapter": "adapter-b",
                    "seed_fingerprint": "sha256:bbbbbbbb",
                    "log": "central.log",
                    "markers": ["dialed", "Established", "Delivered"],
                },
            ],
            "stores": {"peripheral_inspect": "peripheral.inspect", "central_inspect": "central.inspect"},
            "radio": {
                "mtu": {"available": True, "value": 247, "source": "btleplug"},
                "rssi_dbm": {"available": True, "samples": [-58, -55], "source": "scanner callback"},
            },
            "gates": {
                "advertisement": True,
                "gatt_connection": True,
                "identity_proof": True,
                "bundle_transfer": True,
                "receipt_returned": True,
            },
            "message": {"delivered": True, "bundle_id": "0123456789abcdef0123456789abcdef"},
            "receipt": {"returned": True},
            "reconnect": {"attempted": True, "stable": True, "cycles": 1},
        }
        path = root / "evidence.json"
        path.write_text(json.dumps(manifest), encoding="utf-8")
        return path

    def test_complete_two_host_manifest_passes(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            self.assertEqual(validate_manifest(path), [])

    def test_missing_receipt_and_reconnect_are_reported(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["receipt"]["returned"] = False
            data["reconnect"]["stable"] = False
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn("receipt.returned must be true", errors)
            self.assertIn("reconnect.stable must be true", errors)

    def test_unavailable_radio_value_requires_reason(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["radio"]["mtu"] = {"available": False, "source": "not exposed"}
            data["radio"]["rssi_dbm"] = {"available": False, "source": "not exposed", "reason": "adapter API omitted RSSI"}
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn("radio.mtu.reason is required when the measurement is unavailable", errors)

    def test_log_path_cannot_escape_manifest_directory(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][0]["log"] = "../outside.log"
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn("hosts[0].log escapes the manifest directory", errors)

    def test_role_gate_markers_cannot_be_omitted(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][0]["markers"] = ["advertising"]
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[0].markers must include role gate marker: Established",
                errors,
            )
            self.assertIn(
                "hosts[0].markers must include role gate marker: MessageReceived",
                errors,
            )

    def test_duplicate_seed_fingerprints_block_two_host_evidence(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][1]["seed_fingerprint"] = data["hosts"][0]["seed_fingerprint"]
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "duplicate host seed_fingerprint: sha256:aaaaaaaa",
                errors,
            )


if __name__ == "__main__":
    unittest.main()
