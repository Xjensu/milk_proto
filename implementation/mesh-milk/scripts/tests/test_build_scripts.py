import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]


class BuildScriptTests(unittest.TestCase):
    def test_ios_build_script_has_safe_stale_binding_cleanup(self) -> None:
        script = ROOT / "build-ios-xcframework.sh"
        text = script.read_text(encoding="utf-8")

        collision_guard = 'if [[ -e "$OUT_DIR/swift-bindings" && ! -d "$OUT_DIR/swift-bindings" ]]; then'
        cleanup = 'rm -rf "$STAGE" "$OUT_DIR/MmpCore.xcframework" "$OUT_DIR/swift-bindings"'
        copy = 'cp -r "$STAGE/swift-source" "$OUT_DIR/swift-bindings"'

        self.assertIn(collision_guard, text)
        self.assertIn(cleanup, text)
        self.assertLess(text.index(collision_guard), text.index(cleanup))
        self.assertLess(text.index(cleanup), text.index(copy))

    def test_ios_build_script_is_valid_bash(self) -> None:
        script = ROOT / "build-ios-xcframework.sh"
        result = subprocess.run(
            ["bash", "-n", script.name],
            cwd=script.parent,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
