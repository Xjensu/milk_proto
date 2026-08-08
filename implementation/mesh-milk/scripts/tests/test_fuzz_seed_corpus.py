import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[2] / "fuzz" / "seed_corpus.py"
SPEC = importlib.util.spec_from_file_location("seed_corpus", SCRIPT)
assert SPEC and SPEC.loader
CORPUS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CORPUS
SPEC.loader.exec_module(CORPUS)


class FuzzSeedCorpusTests(unittest.TestCase):
    def test_vector_recipe_is_stable_and_covers_all_targets(self) -> None:
        first = CORPUS.build_seed_sets()
        second = CORPUS.build_seed_sets()
        self.assertEqual(first, second)
        self.assertEqual(set(first), {"fuzz_beacon", "fuzz_envelope", "fuzz_bundle", "fuzz_hs"})
        self.assertGreaterEqual(len(first["fuzz_beacon"]), 5)
        self.assertGreaterEqual(len(first["fuzz_envelope"]), 20)
        self.assertGreaterEqual(len(first["fuzz_bundle"]), 12)
        self.assertGreaterEqual(len(first["fuzz_hs"]), 4)
        self.assertTrue(any(len(seed.data) > 4096 for seed in first["fuzz_envelope"]))
        self.assertTrue(any("recon-range-count-no-records" in seed.name for seed in first["fuzz_envelope"]))
        self.assertTrue(any("unknown_critical_tlv" in seed.name for seed in first["fuzz_bundle"]))

    def test_write_and_verify_round_trip(self) -> None:
        seeds = CORPUS.build_seed_sets()
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "corpus"
            CORPUS.write_corpus(output, seeds)
            CORPUS.verify_corpus(output, seeds)
            path = output / "fuzz_bundle" / "mmp-seed-legacy-valid.bin"
            path.write_bytes(path.read_bytes() + b"\x00")
            with self.assertRaises(ValueError):
                CORPUS.verify_corpus(output, seeds)

    def test_write_preserves_unmanaged_corpus_members(self) -> None:
        seeds = CORPUS.build_seed_sets()
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "corpus"
            user_file = output / "fuzz_bundle" / "local-discovery.bin"
            user_file.parent.mkdir(parents=True)
            user_file.write_bytes(b"developer corpus")
            CORPUS.write_corpus(output, seeds)
            self.assertEqual(user_file.read_bytes(), b"developer corpus")
            CORPUS.verify_corpus(output, seeds)

    def test_invalid_hex_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            CORPUS._hex("abc", source="test")
        with self.assertRaises(ValueError):
            CORPUS._hex("zz", source="test")


if __name__ == "__main__":
    unittest.main()
