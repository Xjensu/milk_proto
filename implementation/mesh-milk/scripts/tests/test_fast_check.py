import unittest

from scripts.fast_check import _format_command, _infer, _rust_files_for_scope


class FastCheckTests(unittest.TestCase):
    def test_package_scope_formats_only_changed_rust_files_in_selected_packages(self) -> None:
        paths = [
            "implementation/mesh-milk/crates/mmp-router/src/lib.rs",
            "implementation/mesh-milk/crates/mmp-router/tests/handoff.rs",
            "implementation/mesh-milk/crates/mmp-cli/src/main.rs",
            "implementation/mesh-milk/docs/example.rs",
        ]

        self.assertEqual(
            _rust_files_for_scope(paths, {"mmp-router"}),
            ["crates/mmp-router/src/lib.rs", "crates/mmp-router/tests/handoff.rs"],
        )
        self.assertEqual(
            _format_command({"mmp-router"}, paths, all_mode=False),
            [
                "rustfmt",
                "--check",
                "--edition",
                "2021",
                "crates/mmp-router/src/lib.rs",
                "crates/mmp-router/tests/handoff.rs",
            ],
        )

    def test_package_scope_skips_format_when_no_selected_rust_file_changed(self) -> None:
        paths = ["implementation/mesh-milk/crates/mmp-cli/src/main.rs"]

        self.assertEqual(_rust_files_for_scope(paths, {"mmp-router"}), [])
        self.assertIsNone(_format_command({"mmp-router"}, paths, all_mode=False))

    def test_all_scope_keeps_workspace_cargo_fmt(self) -> None:
        self.assertEqual(
            _format_command(set(), [], all_mode=True),
            ["cargo", "fmt", "--all", "--", "--check"],
        )

    def test_infers_rust_package_and_python_changes(self) -> None:
        packages, workspace, python_change, fuzz_change = _infer(
            [
                "implementation/mesh-milk/crates/mmp-node/src/lib.rs",
                "implementation/mesh-milk/scripts/check_mobile_binding.py",
            ]
        )
        self.assertEqual(packages, {"mmp-node"})
        self.assertFalse(workspace)
        self.assertTrue(python_change)
        self.assertFalse(fuzz_change)

    def test_manifest_and_fuzz_changes_are_not_silently_fast_scoped(self) -> None:
        packages, workspace, python_change, fuzz_change = _infer(
            [
                "implementation/mesh-milk/Cargo.lock",
                "implementation/mesh-milk/fuzz/seed_corpus.py",
            ]
        )
        self.assertEqual(packages, set())
        self.assertTrue(workspace)
        self.assertFalse(python_change)
        self.assertTrue(fuzz_change)


if __name__ == "__main__":
    unittest.main()
