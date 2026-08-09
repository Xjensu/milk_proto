import unittest
from pathlib import Path

import yaml


WORKFLOW = Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml"
WORKSPACE = Path(__file__).parents[2]


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

    def test_no_std_gate_checks_embedded_shell_dependency_closure(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["no-std-size-gate"]["steps"]
        checks = [
            step.get("run", "")
            for step in steps
            if step.get("name") == "check embedded shell dependency closure (E7-S1)"
        ]
        self.assertEqual(
            checks,
            [
                "cargo check -p mmp-node-embassy -p mmp-tal-embedded -p mmp-store --target thumbv7em-none-eabihf --no-default-features"
            ],
        )

    def test_embedded_manifests_fence_default_features(self) -> None:
        node_manifest = (WORKSPACE / "crates" / "mmp-node-embassy" / "Cargo.toml").read_text(
            encoding="utf-8"
        )
        tal_manifest = (WORKSPACE / "crates" / "mmp-tal-embedded" / "Cargo.toml").read_text(
            encoding="utf-8"
        )

        # Keep this source-level fence close to the dependency declarations:
        # the target check is the authoritative proof, while this regression
        # catches an accidental feature reintroduction during review before a
        # cross target job is available.
        for dependency in ("mmp-core", "mmp-router", "mmp-tal"):
            self.assertRegex(
                node_manifest,
                rf"{dependency}\s*=\s*\{{[^\n]*default-features\s*=\s*false",
                dependency,
            )
        self.assertRegex(
            tal_manifest,
            r"mmp-tal\s*=\s*\{[^\n]*default-features\s*=\s*false",
        )
        self.assertNotRegex(node_manifest, r"(?m)^mmp-store\s*=")

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
            "release": {
                "name": "${{ matrix.artifact_name }}-signed",
                "path": {
                    "implementation/mesh-milk/${{ matrix.artifact_name }}",
                    "implementation/mesh-milk/${{ matrix.artifact_name }}.sig",
                    "implementation/mesh-milk/${{ matrix.artifact_name }}.pem",
                    "implementation/mesh-milk/${{ matrix.artifact_name }}.manifest.json",
                },
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

    def test_release_job_records_source_and_digest_manifest(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        manifest = [
            step
            for step in steps
            if step.get("name") == "record signed artifact digest manifest"
        ]
        self.assertEqual(len(manifest), 1)
        step = manifest[0]
        run = step["run"]
        self.assertIn("scripts/create_release_manifest.py", run)
        self.assertIn("--signature ${{ matrix.artifact_name }}.sig", run)
        self.assertIn("--certificate ${{ matrix.artifact_name }}.pem", run)
        self.assertIn("--output ${{ matrix.artifact_name }}.manifest.json", run)

    def test_release_job_verifies_keyless_signature_before_manifest(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        names = [step.get("name") for step in steps]
        verify_index = names.index("verify keyless artifact signature")
        manifest_index = names.index("record signed artifact digest manifest")
        self.assertLess(verify_index, manifest_index)
        run = steps[verify_index]["run"]
        self.assertIn("cosign verify-blob ${{ matrix.artifact_name }}", run)
        self.assertIn("--signature ${{ matrix.artifact_name }}.sig", run)
        self.assertIn("--certificate ${{ matrix.artifact_name }}.pem", run)
        self.assertIn(
            'https://github.com/${{ github.repository }}/.github/workflows/ci.yml@${{ github.ref }}',
            run,
        )
        self.assertIn("--certificate-oidc-issuer https://token.actions.githubusercontent.com", run)

    def test_release_job_verifies_manifest_against_source_and_files(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        verification = [
            step
            for step in steps
            if step.get("name") == "verify signed artifact digest manifest"
        ]
        self.assertEqual(len(verification), 1)
        run = verification[0]["run"]
        self.assertIn("scripts/verify_release_manifest.py", run)
        self.assertIn("--manifest ${{ matrix.artifact_name }}.manifest.json", run)
        self.assertIn("--directory .", run)
        self.assertIn('--repository "${{ github.repository }}"', run)
        self.assertIn('--commit "${{ github.sha }}"', run)
        self.assertIn('--ref "${{ github.ref }}"', run)

    def test_release_job_checks_publication_governance_artifacts(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        checks = [
            step
            for step in steps
            if step.get("name") == "validate publication governance artifacts"
        ]
        self.assertEqual(len(checks), 1)
        run = checks[0]["run"]
        self.assertIn("scripts/check_release_governance.py", run)
        self.assertIn("--repo ../..", run)
        self.assertIn('--commit "${{ github.sha }}"', run)

    def test_release_job_builds_from_the_locked_dependency_graph(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        build = next(step for step in steps if step.get("run", "").startswith("cargo build -p mmp-cli"))
        self.assertIn("--release", build["run"])
        self.assertIn("--locked", build["run"])

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
