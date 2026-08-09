#!/usr/bin/env python3
"""Build a deterministic cargo-fuzz seed corpus from public golden vectors.

Generated files live below ``fuzz/corpus`` (which is intentionally
gitignored). Keeping the recipe, rather than opaque binary files, in the
repository makes the corpus reproducible and prevents a stale corpus from
silently drifting away from the committed wire vectors.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_VECTORS = SCRIPT_DIR.parent / "vectors"
DEFAULT_OUTPUT = SCRIPT_DIR / "corpus"
GENERATED_PREFIX = "mmp-seed-"


@dataclass(frozen=True)
class Seed:
    target: str
    name: str
    data: bytes
    source: str

    @property
    def filename(self) -> str:
        return f"{GENERATED_PREFIX}{self.name}.bin"


def _load_json(vectors: Path, filename: str) -> dict[str, Any]:
    path = vectors / filename
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load vector {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"vector {path} must contain a JSON object")
    return value


def _hex(value: Any, *, source: str) -> bytes:
    if not isinstance(value, str) or len(value) % 2:
        raise ValueError(f"{source} must be an even-length hexadecimal string")
    try:
        return bytes.fromhex(value)
    except ValueError as exc:
        raise ValueError(f"{source} is not hexadecimal: {exc}") from exc


def _seed(target: str, name: str, data: bytes, source: str) -> Seed:
    if not name or "/" in name or "\\" in name or name in {".", ".."}:
        raise ValueError(f"invalid seed name {name!r}")
    # Empty corpus files are silently discarded by some libFuzzer versions;
    # use one-byte truncations where an empty parser input is relevant.
    if not data:
        raise ValueError(f"{target}/{name} is empty")
    return Seed(target=target, name=name, data=data, source=source)


def _vector_wire(vectors: Path, filename: str, field: str = "wire") -> bytes:
    obj = _load_json(vectors, filename)
    return _hex(obj.get(field), source=f"{filename}.{field}")


def build_seed_sets(vectors: Path = DEFAULT_VECTORS) -> dict[str, tuple[Seed, ...]]:
    """Return the fixed seed set, without touching the output directory."""

    vectors = Path(vectors).resolve()
    beacon = _vector_wire(vectors, "beacon.json")
    envelope = _vector_wire(vectors, "envelope.json")
    frames_obj = _load_json(vectors, "frames.json")
    frames = frames_obj.get("frames")
    if not isinstance(frames, list) or not frames:
        raise ValueError("frames.json.frames must be a non-empty array")

    oversize_obj = _load_json(vectors, "envelope_oversize_negative.json")
    oversize = oversize_obj.get("cases")
    if not isinstance(oversize, list) or not oversize:
        raise ValueError("envelope_oversize_negative.json.cases must be a non-empty array")

    bundle_obj = _load_json(vectors, "bundles.json")
    bundles = bundle_obj.get("bundles")
    if not isinstance(bundles, list) or not bundles:
        raise ValueError("bundles.json.bundles must be a non-empty array")

    malformed_obj = _load_json(vectors, "bundle_tlv_negative.json")
    malformed = malformed_obj.get("cases")
    if not isinstance(malformed, list) or not malformed:
        raise ValueError("bundle_tlv_negative.json.cases must be a non-empty array")

    result: dict[str, list[Seed]] = {
        "fuzz_beacon": [],
        "fuzz_envelope": [],
        "fuzz_bundle": [],
        "fuzz_hs": [],
    }

    # Beacon: valid, boundary-truncated, and reserved/all-zero input.
    result["fuzz_beacon"].extend(
        (
            _seed("fuzz_beacon", "valid", beacon, "beacon.json.wire"),
            _seed("fuzz_beacon", "truncated", beacon[:-1], "beacon.json.wire[:-1]"),
            _seed("fuzz_beacon", "header-only", beacon[:3], "beacon.json.wire[:3]"),
            _seed("fuzz_beacon", "all-zero", bytes(24), "deterministic-zero-24"),
            _seed("fuzz_beacon", "all-ff", bytes([0xFF]) * 24, "deterministic-ff-24"),
        )
    )

    # Envelope: every registered frame plus exact oversized-prefix cases that
    # exercise the allocation guard before payload growth.
    result["fuzz_envelope"].extend(
        (
            _seed("fuzz_envelope", "valid-envelope", envelope, "envelope.json.wire"),
            _seed("fuzz_envelope", "truncated-envelope", envelope[:-1], "envelope.json.wire[:-1]"),
            _seed("fuzz_envelope", "oversize-prefix", bytes.fromhex("ffff20"), "deterministic-ffff-prefix"),
            # RECON_RANGE declares 255 ranges but supplies no 44-byte range;
            # this exercises the count-before-reserve guard in Frame::decode_payload.
            _seed(
                "fuzz_envelope",
                "recon-range-count-no-records",
                bytes.fromhex("02001200ff"),
                "deterministic-recon-range-count-no-records",
            ),
        )
    )
    for item in frames:
        if not isinstance(item, dict):
            raise ValueError("frames.json.frames entries must be objects")
        name = item.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("frames.json frame name must be a non-empty string")
        wire = _hex(item.get("wire"), source=f"frames.json.frames[{name}].wire")
        result["fuzz_envelope"].append(
            _seed("fuzz_envelope", f"frame-{name}", wire, f"frames.json.frames[{name}].wire")
        )
    for item in oversize:
        if not isinstance(item, dict):
            raise ValueError("envelope oversize entries must be objects")
        name = item.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("envelope oversize case name must be non-empty")
        wire = _hex(item.get("wire"), source=f"envelope_oversize_negative.json.cases[{name}].wire")
        result["fuzz_envelope"].append(
            _seed("fuzz_envelope", name, wire, f"envelope_oversize_negative.json.cases[{name}].wire")
        )

    # Bundles: all six class fixtures and the three strict-TLV rejection
    # fixtures. The latter retain valid outer framing and reach the canonical
    # TLV state machine.
    for item in bundles:
        if not isinstance(item, dict):
            raise ValueError("bundles.json.bundles entries must be objects")
        name = item.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("bundle vector name must be non-empty")
        wire = _hex(item.get("wire"), source=f"bundles.json.bundles[{name}].wire")
        result["fuzz_bundle"].append(
            _seed("fuzz_bundle", f"class-{name}", wire, f"bundles.json.bundles[{name}].wire")
        )
    legacy_bundle = _vector_wire(vectors, "bundle.json")
    result["fuzz_bundle"].extend(
        (
            _seed("fuzz_bundle", "legacy-valid", legacy_bundle, "bundle.json.wire"),
            _seed("fuzz_bundle", "legacy-truncated", legacy_bundle[:-1], "bundle.json.wire[:-1]"),
            _seed("fuzz_bundle", "legacy-prefix", legacy_bundle[:4], "bundle.json.wire[:4]"),
        )
    )
    for item in malformed:
        if not isinstance(item, dict):
            raise ValueError("bundle negative entries must be objects")
        name = item.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("bundle negative case name must be non-empty")
        wire = _hex(item.get("wire"), source=f"bundle_tlv_negative.json.cases[{name}].wire")
        result["fuzz_bundle"].append(
            _seed("fuzz_bundle", f"negative-{name}", wire, f"bundle_tlv_negative.json.cases[{name}].wire")
        )

    # Noise XX first-message input. The public vector suite intentionally does
    # not contain a session secret or generated handshake transcript. These
    # malformed-length samples still exercise the unauthenticated parser.
    result["fuzz_hs"].extend(
        (
            _seed("fuzz_hs", "short-1", b"\x00", "deterministic-short-1"),
            _seed("fuzz_hs", "short-16", bytes(range(16)), "deterministic-short-16"),
            _seed("fuzz_hs", "zero-32", bytes(32), "deterministic-zero-32"),
            _seed(
                "fuzz_hs",
                "pattern-96",
                bytes((i * 17 + 3) & 0xFF for i in range(96)),
                "deterministic-pattern-96",
            ),
        )
    )

    frozen: dict[str, tuple[Seed, ...]] = {}
    for target, seeds in result.items():
        seen: set[str] = set()
        for seed in seeds:
            if seed.name in seen:
                raise ValueError(f"duplicate seed name {target}/{seed.name}")
            seen.add(seed.name)
        frozen[target] = tuple(seeds)
    return frozen


def iter_seeds(seed_sets: dict[str, tuple[Seed, ...]]) -> Iterable[Seed]:
    for target in sorted(seed_sets):
        _validate_target_name(target)
        yield from seed_sets[target]


def _corpus_root(output: Path) -> Path:
    """Resolve a corpus root without following a caller-provided symlink."""

    output = Path(output)
    if output.is_symlink():
        raise ValueError(f"corpus output must not be a symlink: {output}")
    return output.resolve()


def _validate_target_name(target: str) -> None:
    if not isinstance(target, str) or not target or target in {".", ".."} or "/" in target or "\\" in target:
        raise ValueError(f"invalid corpus target path: {target!r}")


def _target_dir(output: Path, target: str) -> Path:
    """Return one safe target directory below ``output``."""

    _validate_target_name(target)
    target_dir = output / target
    if target_dir.is_symlink():
        raise ValueError(f"corpus target must not be a symlink: {target_dir}")
    return target_dir


def _seed_path(target_dir: Path, seed: Seed) -> Path:
    """Return one safe generated filename below a validated target directory."""

    filename = seed.filename
    if not filename or filename in {".", ".."} or Path(filename).name != filename or "/" in filename or "\\" in filename:
        raise ValueError(f"invalid corpus seed filename: {filename!r}")
    return target_dir / filename


def write_corpus(output: Path, seed_sets: dict[str, tuple[Seed, ...]]) -> None:
    """Write generated members, preserving any user-owned corpus files."""

    output = _corpus_root(output)
    # Validate every caller-provided path before deleting or replacing one
    # generated member.  A malformed later target must fail closed without
    # leaving a partially refreshed corpus behind.
    validated: list[tuple[str, tuple[Seed, ...], Path]] = []
    for target, seeds in seed_sets.items():
        target_dir = _target_dir(output, target)
        for seed in seeds:
            if seed.target != target:
                raise ValueError(f"seed target mismatch: {seed.target!r} != {target!r}")
            _seed_path(target_dir, seed)
        validated.append((target, seeds, target_dir))
    output.mkdir(parents=True, exist_ok=True)
    for _target, seeds, target_dir in validated:
        target_dir.mkdir(parents=True, exist_ok=True)
        # Only remove members created by this recipe; never wipe a corpus that
        # a developer is using for an ongoing local fuzzing campaign.
        for old in target_dir.glob(f"{GENERATED_PREFIX}*.bin"):
            old.unlink()
        for seed in seeds:
            _seed_path(target_dir, seed).write_bytes(seed.data)


def verify_corpus(output: Path, seed_sets: dict[str, tuple[Seed, ...]]) -> None:
    """Require every generated member to match the current vector recipe."""

    output = _corpus_root(output)
    errors: list[str] = []
    expected_by_target: dict[str, set[str]] = {}
    for seed in iter_seeds(seed_sets):
        target_dir = _target_dir(output, seed.target)
        expected_by_target.setdefault(seed.target, set()).add(seed.filename)
        path = _seed_path(target_dir, seed)
        if path.is_symlink():
            errors.append(f"generated corpus member must not be a symlink: {path}")
            continue
        try:
            actual = path.read_bytes()
        except OSError as exc:
            errors.append(f"missing {path}: {exc}")
            continue
        if actual != seed.data:
            errors.append(f"content mismatch for {path} (source {seed.source})")
    # The generated prefix is reserved for this recipe.  Reject stale members
    # so a local verify cannot silently run an obsolete seed alongside the
    # current vectors; developer-owned discoveries use another filename.
    for target, expected in expected_by_target.items():
        target_dir = _target_dir(output, target)
        for path in target_dir.glob(f"{GENERATED_PREFIX}*.bin"):
            if path.name not in expected:
                errors.append(f"unexpected generated corpus member {path}")
    if errors:
        raise ValueError("\n".join(errors))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "write", "verify"), nargs="?", default="check")
    parser.add_argument("--vectors", type=Path, default=DEFAULT_VECTORS)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser


def main() -> int:
    args = _parser().parse_args()
    seed_sets = build_seed_sets(args.vectors)
    count = sum(len(seeds) for seeds in seed_sets.values())
    if args.command == "check":
        print(f"validated {count} deterministic fuzz seeds")
    elif args.command == "write":
        write_corpus(args.output, seed_sets)
        print(f"wrote {count} deterministic fuzz seeds to {args.output.resolve()}")
    else:
        verify_corpus(args.output, seed_sets)
        print(f"verified {count} deterministic fuzz seeds in {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValueError as exc:
        raise SystemExit(f"seed corpus error: {exc}") from exc
