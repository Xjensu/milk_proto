import importlib.util
import tempfile
import unittest
from pathlib import Path

import yaml


SCRIPT = Path(__file__).parents[1] / "check_ios_lifecycle_contract.py"
SPEC = importlib.util.spec_from_file_location("check_ios_lifecycle_contract", SCRIPT)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


ROOT = Path(__file__).parents[4]
MOBILE = ROOT / "implementation" / "mesh-milk" / "MOBILE_INTEGRATION.md"
REACT_NATIVE = ROOT / "implementation" / "mesh-milk" / "REACT_NATIVE_INTEGRATION.md"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


class IOSLifecycleContractTests(unittest.TestCase):
    def test_current_guides_preserve_ios_contract(self) -> None:
        self.assertEqual(CHECK.check_contract(MOBILE, REACT_NATIVE), [])

    def test_missing_restoration_callback_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            mobile = Path(root) / "mobile.md"
            react_native = Path(root) / "react.md"
            mobile.write_text(MOBILE.read_text(encoding="utf-8"), encoding="utf-8")
            text = REACT_NATIVE.read_text(encoding="utf-8").replace(
                "centralManager(_:willRestoreState:)", "centralManager(_:restoredState:)"
            )
            react_native.write_text(text, encoding="utf-8")

            violations = CHECK.check_contract(mobile, react_native)
            self.assertTrue(any("willRestoreState" in item for item in violations))

    def test_wall_clock_tick_is_rejected_without_rejecting_explanatory_prose(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            mobile = Path(root) / "mobile.md"
            react_native = Path(root) / "react.md"
            mobile_text = MOBILE.read_text(encoding="utf-8")
            mobile_text = mobile_text.replace(
                "node.tick(UInt64(ProcessInfo.processInfo.systemUptime * 1000))",
                "node.tick(UInt64(Date().timeIntervalSince1970 * 1000))",
            )
            mobile.write_text(mobile_text, encoding="utf-8")
            react_native.write_text(REACT_NATIVE.read_text(encoding="utf-8"), encoding="utf-8")

            violations = CHECK.check_contract(mobile, react_native)
            self.assertTrue(any("monotonic time" in item for item in violations))

    def test_mobile_bindings_ci_job_runs_the_contract_check(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["mobile-bindings"]["steps"]
        checks = [
            step.get("run")
            for step in steps
            if step.get("run") == "python scripts/check_ios_lifecycle_contract.py"
        ]
        self.assertEqual(checks, ["python scripts/check_ios_lifecycle_contract.py"])


if __name__ == "__main__":
    unittest.main()
