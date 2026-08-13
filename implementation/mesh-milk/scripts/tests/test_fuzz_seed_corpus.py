import importlib.util
import json
import os
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
    def test_ci_pins_the_cargo_fuzz_frontend_for_both_fuzz_jobs(self) -> None:
        workflow = (Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        command = "cargo install cargo-fuzz --version 0.13.2 --locked"
        self.assertEqual(workflow.count(command), 2)
        self.assertNotIn("cargo install cargo-fuzz --locked", workflow)

    def test_fuzz_run_fails_if_corpus_artifact_is_not_found(self) -> None:
        workflow = (Path(__file__).parents[4] / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        start = workflow.index("  fuzz-run:")
        end = workflow.index("\n  simulator-extended:", start)
        job = workflow[start:end]
        self.assertIn("uses: actions/upload-artifact@v4", job)
        self.assertIn("if: always()", job)
        self.assertIn("if-no-files-found: error", job)
        self.assertIn("path: implementation/mesh-milk/fuzz/corpus", job)

    def test_fuzz_run_status_documents_both_ci_triggers(self) -> None:
        """Keep fuzz status docs aligned with the scheduled fuzz gate."""

        repository = Path(__file__).parents[4]
        workflow = (repository / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        dossier = (repository / "docs" / "SECURITY_REVIEW_DOSSIER.md").read_text(
            encoding="utf-8"
        )
        readme = (repository / "implementation" / "mesh-milk" / "fuzz" / "README.md").read_text(
            encoding="utf-8"
        )
        self.assertIn('cron: "17 3 * * 0"', workflow)
        self.assertIn(
            "if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'",
            workflow,
        )
        self.assertIn(
            "**CI `fuzz-run`** (weekly scheduled run or manual `workflow_dispatch`)",
            dossier,
        )
        self.assertNotIn("**CI `fuzz-run`** (manual `workflow_dispatch` only)", dossier)
        self.assertIn(
            "- `fuzz-run` (weekly scheduled run or manual `workflow_dispatch`, not on every push):",
            readme,
        )
        self.assertNotIn("- `fuzz-run` (manual `workflow_dispatch` only", readme)

    def test_cargo_target_inventory_matches_seed_mapping(self) -> None:
        inventory = CORPUS.validate_target_inventory()
        self.assertEqual(tuple(inventory), CORPUS.TARGETS)
        self.assertEqual(set(CORPUS.build_seed_sets()), set(inventory))
        for target, source in inventory.items():
            self.assertEqual(source, f"fuzz_targets/{target}.rs")

    def test_vector_recipe_is_stable_and_covers_all_targets(self) -> None:
        first = CORPUS.build_seed_sets()
        second = CORPUS.build_seed_sets()
        self.assertEqual(first, second)
        self.assertEqual(set(first), {"fuzz_beacon", "fuzz_envelope", "fuzz_bundle", "fuzz_hs"})
        self.assertGreaterEqual(len(first["fuzz_beacon"]), 5)
        self.assertGreaterEqual(len(first["fuzz_envelope"]), 20)
        self.assertGreaterEqual(len(first["fuzz_bundle"]), 12)
        self.assertGreaterEqual(len(first["fuzz_hs"]), 4)
        self.assertTrue(any(seed.name == "max-256" and len(seed.data) == 256 for seed in first["fuzz_hs"]))
        self.assertTrue(any(seed.name == "over-max-257" and len(seed.data) == 257 for seed in first["fuzz_hs"]))
        self.assertTrue(any(len(seed.data) > 4096 for seed in first["fuzz_envelope"]))
        self.assertTrue(any("recon-range-count-no-records" in seed.name for seed in first["fuzz_envelope"]))
        self.assertTrue(any("unknown_critical_tlv" in seed.name for seed in first["fuzz_bundle"]))
        reserved = [seed for seed in first["fuzz_bundle"] if seed.name == "negative-reserved-bits"]
        self.assertEqual(len(reserved), 1)
        self.assertEqual(reserved[0].source, "reserved_bits_negative.json.wire")
        expected = json.loads(
            (CORPUS.DEFAULT_VECTORS / "reserved_bits_negative.json").read_text(encoding="utf-8")
        )["wire"]
        self.assertEqual(reserved[0].data, bytes.fromhex(expected))

    def test_corpus_fingerprint_is_stable_and_content_sensitive(self) -> None:
        seeds = CORPUS.build_seed_sets()
        self.assertEqual(CORPUS.corpus_fingerprint(seeds), CORPUS.corpus_fingerprint(seeds))
        changed = dict(seeds)
        first = list(changed[CORPUS.TARGETS[0]])
        original = first[0]
        first[0] = CORPUS.Seed(
            target=original.target,
            name=original.name,
            data=original.data + b"\x00",
            source=original.source,
        )
        changed[CORPUS.TARGETS[0]] = tuple(first)
        self.assertNotEqual(CORPUS.corpus_fingerprint(seeds), CORPUS.corpus_fingerprint(changed))

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
            reserved = output / "fuzz_bundle" / "mmp-seed-negative-reserved-bits.bin"
            self.assertEqual(
                reserved.read_bytes(),
                next(seed.data for seed in seeds["fuzz_bundle"] if seed.name == "negative-reserved-bits"),
            )

    def test_verify_rejects_stale_generated_member(self) -> None:
        seeds = CORPUS.build_seed_sets()
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "corpus"
            CORPUS.write_corpus(output, seeds)
            stale = output / "fuzz_bundle" / "mmp-seed-retired-vector.bin"
            stale.write_bytes(b"retired")
            with self.assertRaisesRegex(ValueError, "unexpected generated corpus member"):
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

    def test_write_rejects_target_path_traversal(self) -> None:
        seed = CORPUS.Seed(target="../escape", name="owned", data=b"seed", source="test")
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "corpus"
            with self.assertRaisesRegex(ValueError, "invalid corpus target path"):
                CORPUS.write_corpus(output, {"../escape": (seed,)})
            self.assertFalse((Path(root) / "escape").exists())

    def test_verify_rejects_seed_path_traversal(self) -> None:
        seed = CORPUS.Seed(target="../escape", name="owned", data=b"seed", source="test")
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaisesRegex(ValueError, "invalid corpus target path"):
                CORPUS.verify_corpus(Path(root) / "corpus", {"fuzz_bundle": (seed,)})

    def test_write_rejects_seed_filename_path_traversal(self) -> None:
        seed = CORPUS.Seed(target="fuzz_bundle", name="../escape", data=b"seed", source="test")
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaisesRegex(ValueError, "invalid corpus seed filename"):
                CORPUS.write_corpus(Path(root) / "corpus", {"fuzz_bundle": (seed,)})

    def test_write_validates_all_paths_before_refreshing_members(self) -> None:
        seeds = CORPUS.build_seed_sets()
        with tempfile.TemporaryDirectory() as root:
            output = Path(root) / "corpus"
            CORPUS.write_corpus(output, seeds)
            generated = next((output / "fuzz_bundle").glob("mmp-seed-*.bin"))
            original = generated.read_bytes()
            bad = CORPUS.Seed(target="fuzz_bundle", name="../escape", data=b"bad", source="test")
            with self.assertRaisesRegex(ValueError, "invalid corpus seed filename"):
                CORPUS.write_corpus(output, {"fuzz_bundle": (seeds["fuzz_bundle"][0], bad)})
            self.assertEqual(generated.read_bytes(), original)

    def test_write_rejects_case_insensitive_generated_name_collisions(self) -> None:
        seeds = (
            CORPUS.Seed(target="fuzz_bundle", name="Case", data=b"one", source="test"),
            CORPUS.Seed(target="fuzz_bundle", name="case", data=b"two", source="test"),
        )
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaisesRegex(ValueError, "case-insensitive duplicate seed name"):
                CORPUS.write_corpus(Path(root) / "corpus", {"fuzz_bundle": seeds})

    def test_write_rejects_windows_invalid_seed_filename_components(self) -> None:
        for name in ("contains:colon", "contains*glob", "contains\ncontrol", "trailing.", "trailing "):
            seed = CORPUS.Seed(target="fuzz_bundle", name=name, data=b"seed", source="test")
            with self.subTest(name=name), tempfile.TemporaryDirectory() as root:
                with self.assertRaisesRegex(ValueError, "invalid corpus seed filename"):
                    CORPUS.write_corpus(Path(root) / "corpus", {"fuzz_bundle": (seed,)})

    def test_write_rejects_unicode_normalization_seed_name_collisions(self) -> None:
        seeds = (
            CORPUS.Seed(target="fuzz_bundle", name="café", data=b"one", source="test"),
            CORPUS.Seed(target="fuzz_bundle", name="cafe\u0301", data=b"two", source="test"),
        )
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaisesRegex(ValueError, "case-insensitive duplicate seed name"):
                CORPUS.write_corpus(Path(root) / "corpus", {"fuzz_bundle": seeds})

    def test_write_rejects_symlinked_target_when_supported(self) -> None:
        if not hasattr(os, "symlink"):
            self.skipTest("symlink support is unavailable")
        seeds = CORPUS.build_seed_sets()
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            output = root_path / "corpus"
            output.mkdir()
            outside = root_path / "outside"
            outside.mkdir()
            target = output / "fuzz_bundle"
            try:
                target.symlink_to(outside, target_is_directory=True)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symlink creation unavailable: {exc}")
            with self.assertRaisesRegex(ValueError, "corpus target must not be a symlink"):
                CORPUS.write_corpus(output, seeds)

    def test_invalid_hex_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            CORPUS._hex("abc", source="test")
        with self.assertRaises(ValueError):
            CORPUS._hex("zz", source="test")

    def test_hostile_vector_file_is_bounded_before_json_parse(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            vectors = Path(root)
            oversized = vectors / "beacon.json"
            oversized.write_bytes(b"{" + b"x" * CORPUS.MAX_VECTOR_FILE_BYTES + b"}")
            with self.assertRaisesRegex(ValueError, "exceeds maximum size"):
                CORPUS._load_json(vectors, "beacon.json")

    def test_hostile_wire_and_entry_counts_are_bounded(self) -> None:
        with self.assertRaisesRegex(ValueError, "exceeds maximum seed size"):
            CORPUS._hex("00" * (CORPUS.MAX_SEED_BYTES + 1), source="test")

        with tempfile.TemporaryDirectory() as root:
            vectors = Path(root)
            vectors.joinpath("beacon.json").write_text(
                json.dumps({"wire": "00"}), encoding="utf-8"
            )
            vectors.joinpath("envelope.json").write_text(
                json.dumps({"wire": "00"}), encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "too many entries"):
                # The remaining required vector files are intentionally absent:
                # this check must reject the oversized frame list first.
                vectors.joinpath("frames.json").write_text(
                    json.dumps({
                        "frames": [
                            {"name": str(index), "wire": "00"}
                            for index in range(CORPUS.MAX_VECTOR_ENTRIES + 1)
                        ]
                    }),
                    encoding="utf-8",
                )
                CORPUS.build_seed_sets(vectors)


if __name__ == "__main__":
    unittest.main()
