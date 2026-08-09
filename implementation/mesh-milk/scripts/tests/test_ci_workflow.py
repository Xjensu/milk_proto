import unittest
from pathlib import Path

import yaml


WORKFLOW = Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml"


class CIWorkflowTests(unittest.TestCase):
    def test_android_check_provisions_ndk_toolchain(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["android-check"]["steps"]
        setup = [step for step in steps if step.get("uses") == "nttld/setup-ndk@v1"]
        self.assertEqual(len(setup), 1)
        self.assertEqual(setup[0]["with"]["ndk-version"], "r26d")
        self.assertTrue(
            any(step.get("run") == "cargo install cargo-ndk --locked" for step in steps)
        )
        ndk_checks = [step for step in steps if step.get("run") == "cargo ndk -t arm64-v8a check --workspace"]
        self.assertEqual(len(ndk_checks), 1)
        self.assertEqual(ndk_checks[0]["env"]["ANDROID_NDK_HOME"], "${{ steps.setup-ndk.outputs.ndk-path }}")

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

    def test_fuzz_jobs_resolve_paths_from_workspace_working_directory(self) -> None:
        """Keep fuzz commands relative to the workflow's default directory."""

        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        self.assertEqual(
            workflow["defaults"]["run"]["working-directory"],
            "implementation/mesh-milk",
        )

        fuzz_build_runs = [
            step.get("run", "")
            for step in workflow["jobs"]["fuzz-build"]["steps"]
            if "run" in step
        ]
        self.assertIn("python fuzz/seed_corpus.py check", fuzz_build_runs)
        build_script = next(run for run in fuzz_build_runs if "cargo fuzz build" in run)
        self.assertIn("cd fuzz", build_script)
        self.assertNotIn("cd implementation/mesh-milk/fuzz", build_script)

        fuzz_run_runs = [
            step.get("run", "")
            for step in workflow["jobs"]["fuzz-run"]["steps"]
            if "run" in step
        ]
        run_script = next(run for run in fuzz_run_runs if "cargo fuzz run" in run)
        self.assertIn("cd fuzz", run_script)
        self.assertNotIn("cd implementation/mesh-milk/fuzz", run_script)


if __name__ == "__main__":
    unittest.main()
