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
                    "command": "cargo run --release -p mmp-cli -- listen /tmp/alice.seed --store /tmp/mmp-alice.store",
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
                    "command": "cargo run --release -p mmp-cli -- send /tmp/bob.seed 0123456789abcdef0123456789abcdef --store /tmp/mmp-bob.store",
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

    def test_exact_host_command_is_required(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            del data["hosts"][1]["command"]
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn("hosts[1].command must be recorded", errors)

    def test_host_command_must_match_the_recorded_role(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][0]["command"] = "cargo run --release -p mmp-cli -- connect /tmp/alice.seed"
            data["hosts"][1]["command"] = "cargo run --release -p mmp-cli -- listen /tmp/bob.seed"
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[0].command must invoke mmp-cli listen for the peripheral role",
                errors,
            )
            self.assertIn(
                "hosts[1].command must invoke mmp-cli send for the central role",
                errors,
            )

            # A bootstrap command must not pass merely because a free-form
            # argument contains the delivery subcommand word.
            data["hosts"][1]["command"] = (
                "cargo run --release -p mmp-cli -- connect /tmp/bob.seed "
                "--store /tmp/mmp-bob.store 'send'"
            )
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[1].command must invoke mmp-cli send for the central role",
                errors,
            )

    def test_host_command_marker_inside_echo_does_not_count_as_invocation(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][1]["command"] = "echo 'mmp-cli send /tmp/bob.seed destination'"
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[1].command must invoke mmp-cli send for the central role",
                errors,
            )

    def test_quoted_windows_cli_path_is_accepted(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = self._manifest(Path(directory))
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][1]["command"] = (
                r'& "C:\Program Files\mmp-cli.exe" send C:\bob.seed destination '
                r'--store C:\mmp-bob.snapshot'
            )
            path.write_text(json.dumps(data), encoding="utf-8")
            self.assertEqual(validate_manifest(path), [])

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

    def test_evidence_files_cannot_be_reused_between_roles(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self._manifest(root)
            # Make the shared log satisfy both role marker sets so this test
            # isolates the path-reuse guard rather than marker validation.
            (root / "peripheral.log").write_text(
                (root / "peripheral.log").read_text(encoding="utf-8")
                + "dialed\nDelivered\n",
                encoding="utf-8",
            )
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][1]["log"] = "./peripheral.log"
            data["stores"]["central_inspect"] = "./peripheral.inspect"
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[1].log reuses evidence path already assigned to hosts[0].log",
                errors,
            )
            self.assertIn(
                "stores.central_inspect reuses evidence path already assigned to stores.peripheral_inspect",
                errors,
            )

    def test_evidence_symlink_is_rejected_even_when_target_stays_inside_package(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self._manifest(root)
            source = root / "unreferenced.log"
            source.write_text("dialed\nEstablished\nDelivered\n", encoding="utf-8")
            alias = root / "central-alias.log"
            try:
                alias.symlink_to(source)
            except (OSError, NotImplementedError):
                self.skipTest("symbolic links unavailable on this platform")
            data = json.loads(path.read_text(encoding="utf-8"))
            data["hosts"][1]["log"] = alias.name
            path.write_text(json.dumps(data), encoding="utf-8")
            errors = validate_manifest(path)
            self.assertIn(
                "hosts[1].log must not use symbolic links: central-alias.log",
                errors,
            )

    def test_oversized_host_log_is_rejected_before_read(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import MAX_EVIDENCE_FILE_BYTES, validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self._manifest(root)
            (root / "peripheral.log").write_bytes(b"x" * (MAX_EVIDENCE_FILE_BYTES + 1))
            errors = validate_manifest(path)
            self.assertIn(
                f"hosts[0].log exceeds maximum size of {MAX_EVIDENCE_FILE_BYTES} bytes",
                errors,
            )

    def test_oversized_manifest_is_rejected_before_json_parse(self) -> None:
        import sys

        sys.path.insert(0, str(ROOT))
        from validate_ble_evidence import MAX_MANIFEST_BYTES, validate_manifest

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            path.write_bytes(b"{" + b"x" * MAX_MANIFEST_BYTES + b"}")
            self.assertEqual(
                validate_manifest(path),
                [f"manifest exceeds maximum size of {MAX_MANIFEST_BYTES} bytes"],
            )


if __name__ == "__main__":
    unittest.main()
