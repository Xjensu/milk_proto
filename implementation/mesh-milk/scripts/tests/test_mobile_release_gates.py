import unittest
from pathlib import Path

import yaml


WORKFLOW = Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml"


class MobileReleaseGateTests(unittest.TestCase):
    def test_packaging_jobs_run_structural_verifiers(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        jobs = workflow["jobs"]
        android_runs = [step.get("run", "") for step in jobs["android-aar"]["steps"]]
        self.assertIn(
            "python scripts/verify_android_artifacts.py mobile-artifacts/android/mmp-ffi.aar mobile-artifacts/android/mmp-ffi-kotlin-bindings.zip",
            android_runs,
        )
        ios_runs = [step.get("run", "") for step in jobs["ios-xcframework"]["steps"]]
        self.assertIn(
            "python scripts/verify_ios_artifacts.py mobile-artifacts/ios/MmpCore.xcframework mobile-artifacts/ios/swift-bindings",
            ios_runs,
        )


if __name__ == "__main__":
    unittest.main()
