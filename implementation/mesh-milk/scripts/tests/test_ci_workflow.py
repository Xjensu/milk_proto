import unittest
from pathlib import Path

import yaml


WORKFLOW = Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml"


class CIWorkflowTests(unittest.TestCase):
    def test_host_workspace_tests_use_serial_threads(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        test_steps = [
            step
            for step in workflow["jobs"]["host"]["steps"]
            if step.get("run") == "cargo test --workspace --all-features -- --test-threads=1"
        ]
        self.assertEqual(len(test_steps), 1)

    def test_mobile_artifact_uploads_fail_on_missing_paths(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        jobs = workflow["jobs"]
        expected = {
            "mobile-bindings": {
                "name": "mmp-mobile-bindings",
                "path": {
                    "implementation/mesh-milk/bindings/kotlin",
                    "implementation/mesh-milk/bindings/swift",
                },
            },
            "android-aar": {
                "name": "mmp-ffi-aar",
                "path": "implementation/mesh-milk/mobile-artifacts/android",
            },
            "ios-xcframework": {
                "name": "mmp-core-xcframework",
                "path": "implementation/mesh-milk/mobile-artifacts/ios",
            },
        }

        for job_id, requirement in expected.items():
            uploads = [
                step
                for step in jobs[job_id]["steps"]
                if step.get("uses") == "actions/upload-artifact@v4"
            ]
            self.assertEqual(len(uploads), 1, job_id)
            with_args = uploads[0]["with"]
            self.assertEqual(with_args["name"], requirement["name"])
            self.assertEqual(with_args["if-no-files-found"], "error", job_id)
            path = with_args["path"]
            if isinstance(requirement["path"], set):
                self.assertEqual(set(path.splitlines()), requirement["path"])
            else:
                self.assertEqual(path, requirement["path"])


if __name__ == "__main__":
    unittest.main()
