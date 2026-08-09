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
        # Windows ships a `bash.exe` launcher that emits WSL installation
        # guidance instead of parsing shell scripts when WSL is absent. That
        # is an environment prerequisite, not evidence that this script has a
        # syntax error. Keep the real syntax gate strict on Linux/macOS and
        # skip only this unmistakable launcher response on Windows.
        launcher_output = result.stdout.replace("\x00", "").casefold()
        if result.returncode != 0 and "wsl.exe --install" in launcher_output:
            self.skipTest("Windows bash launcher found; WSL is not installed")
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
