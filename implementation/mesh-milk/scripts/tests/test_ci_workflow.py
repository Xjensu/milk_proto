import unittest
from pathlib import Path

import yaml


WORKFLOW = Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml"
WORKSPACE = Path(__file__).parents[2]


class CIWorkflowTests(unittest.TestCase):
    def test_extended_simulator_tier_is_schedule_or_manual_only(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        self.assertIn("simulator-extended", workflow["jobs"])
        job = workflow["jobs"]["simulator-extended"]
        self.assertEqual(
            job["if"],
            "github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'",
        )
        runs = [step.get("run", "") for step in job["steps"]]
        self.assertIn(
            "cargo test --release --locked -p mmp-sim --all-targets --all-features -- --ignored --test-threads=1",
            runs,
        )

    def test_fuzz_smoke_is_schedule_or_manual_only(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        self.assertEqual(
            workflow["jobs"]["fuzz-run"]["if"],
            "github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'",
        )

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
            if step.get("run")
            == "cargo test --workspace --all-targets --all-features --locked -- --test-threads=1"
        ]
        self.assertEqual(len(test_steps), 1)

    def test_embedded_transactional_acceptance_double_is_a_named_ci_gate(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["host"]["steps"]
        checks = [
            step.get("run", "")
            for step in steps
            if step.get("name") == "test embedded transactional store acceptance double (E7-S1)"
        ]
        self.assertEqual(
            checks,
            ["cargo test -p mmp-store --test transactional_prototype --all-features --locked"],
        )

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
                "cargo check -p mmp-node-embassy -p mmp-tal-embedded -p mmp-store --target thumbv7em-none-eabihf --no-default-features --locked"
            ],
        )

    def test_no_std_size_build_uses_locked_dependency_graph(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["no-std-size-gate"]["steps"]
        build = next(
            step["run"]
            for step in steps
            if isinstance(step.get("run"), str)
            and "cargo build --release --target thumbv7em-none-eabihf" in step["run"]
        )
        self.assertIn("--locked", build)

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

    def test_embedded_shell_sources_keep_the_no_std_boundary(self) -> None:
        """Catch an accidental host-only import before the target job runs."""

        for crate in ("mmp-node-embassy", "mmp-tal-embedded"):
            source = (
                WORKSPACE / "crates" / crate / "src" / "lib.rs"
            ).read_text(encoding="utf-8")
            self.assertRegex(
                source,
                r"(?m)^#!\[no_std\]\s*$",
                crate,
            )
            self.assertNotRegex(
                source,
                r"(?m)^\s*(?:use|extern\s+crate)\s+std(?:[:;]|$)",
                crate,
            )
            self.assertNotIn("std::", source, crate)

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

    def test_mobile_bindings_build_uses_locked_dependency_graph(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["mobile-bindings"]["steps"]
        cargo_steps = [
            step["run"]
            for step in steps
            if step.get("run", "").startswith("cargo build -p mmp-ffi")
            or step.get("run", "").startswith("cargo run -p mmp-ffi")
        ]
        self.assertEqual(len(cargo_steps), 3)
        for command in cargo_steps:
            self.assertIn("--locked", command)

    def test_react_native_package_job_checks_source_and_npm_shape(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["react-native-package"]["steps"]
        source_check = [
            step
            for step in steps
            if step.get("name") == "test React Native package source contract (E4-S3)"
        ]
        self.assertEqual(
            [step["run"] for step in source_check],
            ["python -m unittest scripts.tests.test_react_native_mesh -v"],
        )
        package_check = [
            step
            for step in steps
            if step.get("name") == "validate React Native npm package shape (E4-S3)"
        ]
        self.assertEqual(len(package_check), 1)
        self.assertEqual(package_check[0]["run"], "npm pack --dry-run --json")
        self.assertEqual(
            package_check[0]["working-directory"],
            "implementation/mesh-milk/platform/react-native-mesh",
        )

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

    def test_release_job_requires_host_and_fuzz_build_gates(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        self.assertEqual(
            workflow["jobs"]["release"]["needs"],
            ["host", "fuzz-build", "deny", "cargo-audit"],
        )

    def test_release_job_requires_dependency_security_gates(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        needs = set(workflow["jobs"]["release"]["needs"])
        self.assertIn("deny", needs)
        self.assertIn("cargo-audit", needs)

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

    def test_release_job_checks_specification_status_manifest(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        steps = workflow["jobs"]["release"]["steps"]
        checks = [
            step
            for step in steps
            if step.get("name") == "validate specification status manifest"
        ]
        self.assertEqual(len(checks), 1)
        self.assertEqual(checks[0]["run"], "python scripts/check_spec_status.py --repo ../..")
        names = [step.get("name") for step in steps]
        self.assertLess(
            names.index("validate specification status manifest"),
            names.index("verify keyless artifact signature"),
        )

    def test_release_job_builds_from_the_locked_dependency_graph(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        release = workflow["jobs"]["release"]
        steps = release["steps"]
        build = next(step for step in steps if step.get("run", "").startswith("cargo build -p mmp-cli"))
        self.assertIn("--release", build["run"])
        self.assertIn("--locked", build["run"])

        targets = {
            entry["artifact_name"]: entry["target"]
            for entry in release["strategy"]["matrix"]["include"]
        }
        self.assertEqual(
            targets,
            {
                "mmp-cli-linux-x86_64": "x86_64-unknown-linux-gnu",
                "mmp-cli-windows-x86_64.exe": "x86_64-pc-windows-msvc",
                "mmp-cli-macos-x86_64": "x86_64-apple-darwin",
            },
        )
        toolchain = next(
            step for step in steps if step.get("uses") == "dtolnay/rust-toolchain@stable"
        )
        self.assertEqual(toolchain["with"]["targets"], "${{ matrix.target }}")
        self.assertIn("--target ${{ matrix.target }}", build["run"])

        copy = next(step for step in steps if step.get("run", "").startswith("cp target/"))
        self.assertIn("target/${{ matrix.target }}/release/${{ matrix.bin }}", copy["run"])

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

    def test_fuzz_smoke_runs_bound_each_input_and_memory(self) -> None:
        workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
        run_script = next(
            step.get("run", "")
            for step in workflow["jobs"]["fuzz-run"]["steps"]
            if "cargo fuzz run" in step.get("run", "")
        )
        self.assertIn("-max_total_time=120", run_script)
        self.assertIn("-max_len=8192", run_script)
        self.assertIn("-timeout=10", run_script)
        self.assertIn("-rss_limit_mb=2048", run_script)


if __name__ == "__main__":
    unittest.main()
