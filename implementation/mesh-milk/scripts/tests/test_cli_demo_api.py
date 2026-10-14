import json
import re
import subprocess
import unittest
from pathlib import Path


WORKSPACE = Path(__file__).parents[2]
_BINARIES = (WORKSPACE / "target" / "debug" / "mmp-cli.exe", WORKSPACE / "target" / "debug" / "mmp-cli")
MMP_CLI = next((path for path in _BINARIES if path.is_file()), None)

_REQUIRED_FIELDS = {
    "schema_version",
    "command",
    "alice_node_id",
    "bob_node_id",
    "bundle_id",
    "receiver_message",
    "sender_delivered",
    "sender_delivery_state",
    "contacts",
}
_FORBIDDEN_KEY_PARTS = ("secret", "private", "seed", "raw", "frame", "ciphertext")
_PUBLIC_ID = re.compile(r"^[0-9a-f]{32}$")
_EXPECTED_IDS = {
    "alice_node_id": "d6a7bbce57fb0b68012b47f65c889d11",
    "bob_node_id": "230fd467ca0e00d46aa3e17c57cc3149",
    "bundle_id": "6eae6631bb77863e4694ea7eb9622c02",
}


@unittest.skipUnless(MMP_CLI is not None, "target/debug/mmp-cli is not built")
class DemoJsonApiTests(unittest.TestCase):
    def _run(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(MMP_CLI), *args],
            cwd=WORKSPACE,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_demo_json_is_stable_bounded_and_public_only(self) -> None:
        reports = []
        for attempt in range(2):
            with self.subTest(attempt=attempt):
                result = self._run("demo", "--json")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(len(result.stdout.splitlines()), 1)
                report = json.loads(result.stdout)
                self.assertIsInstance(report, dict)
                self.assertTrue(_REQUIRED_FIELDS <= report.keys())
                self.assertEqual(report["schema_version"], 1)
                self.assertEqual(report["command"], "demo")
                self.assertTrue(report["receiver_message"])
                self.assertTrue(report["sender_delivered"])
                self.assertEqual(report["sender_delivery_state"], "delivered")
                for field, expected in _EXPECTED_IDS.items():
                    self.assertRegex(report[field], _PUBLIC_ID.pattern)
                    self.assertEqual(report[field], expected)
                for key, value in report.items():
                    if isinstance(value, int) and not isinstance(value, bool):
                        self.assertGreaterEqual(value, 0, key)
                        self.assertLessEqual(value, 4096, key)
                    self.assertFalse(
                        any(part in key.lower() for part in _FORBIDDEN_KEY_PARTS), key
                    )
                reports.append(report)
        self.assertEqual(reports[0], reports[1])

    def test_demo_json_rejects_extra_arguments(self) -> None:
        result = self._run("demo", "--json", "extra")
        self.assertEqual(result.returncode, 2)

    def test_process_help_and_unsupported_invocations_are_fail_closed(self) -> None:
        help_result = self._run("--help")
        self.assertEqual(help_result.returncode, 0, help_result.stderr)
        for subcommand in ("keygen", "demo", "inspect", "vectors", "connect", "send", "listen"):
            self.assertIn(f"mmp-cli {subcommand}", help_result.stdout)

        empty_result = self._run()
        self.assertEqual(empty_result.returncode, 2)
        self.assertIn("USAGE:", empty_result.stderr)

        unknown_result = self._run("unsupported")
        self.assertEqual(unknown_result.returncode, 2)
        self.assertIn("unknown subcommand 'unsupported'", unknown_result.stderr)
        self.assertIn("USAGE:", unknown_result.stderr)


if __name__ == "__main__":
    unittest.main()
