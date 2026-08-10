import unittest

from scripts.fast_check import _infer


class FastCheckTests(unittest.TestCase):
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
