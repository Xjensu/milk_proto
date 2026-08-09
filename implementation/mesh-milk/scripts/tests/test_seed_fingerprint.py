import hashlib
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
from fingerprint_seed import fingerprint_seed  # noqa: E402


class SeedFingerprintTests(unittest.TestCase):
    def test_fingerprint_hashes_decoded_seed_not_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "seed.hex"
            seed = bytes(range(32))
            path.write_text(seed.hex().upper() + "\n", encoding="ascii")
            expected = f"sha256:{hashlib.sha256(seed).hexdigest()}"
            self.assertEqual(fingerprint_seed(path), expected)

    def test_malformed_seed_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "seed.hex"
            path.write_text("00" * 31, encoding="ascii")
            with self.assertRaisesRegex(ValueError, "exactly 64"):
                fingerprint_seed(path)

    def test_symlink_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "real.seed"
            target.write_text("11" * 32, encoding="ascii")
            alias = root / "alias.seed"
            try:
                alias.symlink_to(target)
            except (OSError, NotImplementedError):
                self.skipTest("symbolic links unavailable on this platform")
            with self.assertRaisesRegex(ValueError, "symbolic link"):
                fingerprint_seed(alias)


if __name__ == "__main__":
    unittest.main()
