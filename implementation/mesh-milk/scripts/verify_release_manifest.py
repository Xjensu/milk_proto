#!/usr/bin/env python3
"""Verify one downloaded mesh-milk release digest manifest.

The manifest is an audit record, not a replacement for Sigstore verification.
This verifier checks the record's shape, source identity, and the exact bytes
of the three files that it names.  File names are deliberately restricted to
regular basenames: a release package must not use a manifest entry to follow
a symlink or escape the directory supplied by the consumer.

Usage (from ``implementation/mesh-milk``)::

    python scripts/verify_release_manifest.py \
        --manifest mmp-cli-linux-x86_64.manifest.json \
        --directory . \
        --repository Xjensu/milk_proto \
        --commit <tag-commit-sha> \
        --ref refs/tags/v1.2.3

The expected source arguments are optional for callers that only need package
integrity.  Supplying them is recommended for release automation and verifies
that the package was built from the intended repository revision and ref.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PureWindowsPath
from typing import Any


FORMAT = "mesh-milk-release-manifest-v1"
_HEX64 = re.compile(r"^[0-9a-f]{64}$")
_COMMIT = re.compile(r"^[0-9a-f]{40,64}$")
_FILE_KINDS = ("artifact", "certificate", "signature")
_ROOT_KEYS = frozenset(("format", "source", "files"))
_SOURCE_KEYS = frozenset(("repository", "commit", "ref"))
_FILE_KEYS = frozenset(("name", "size", "sha256"))


class _DuplicateKey(ValueError):
    """Raised when a JSON object contains a duplicate member name."""


def _object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateKey(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _load_json(path: Path) -> Any:
    """Read strict UTF-8 JSON, rejecting duplicate keys and non-finite values."""

    if path.is_symlink():
        raise ValueError(f"manifest must not be a symbolic link: {path}")
    if not path.is_file():
        raise ValueError(f"manifest is not a regular file: {path}")
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise ValueError(f"manifest cannot be read as UTF-8: {exc}") from exc

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-finite JSON value is not allowed: {value}")

    try:
        return json.loads(
            text,
            object_pairs_hook=_object_without_duplicates,
            parse_constant=reject_constant,
        )
    except (TypeError, ValueError, json.JSONDecodeError) as exc:
        raise ValueError(f"manifest is not valid strict JSON: {exc}") from exc


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def _require_exact_keys(value: dict[str, Any], expected: frozenset[str], label: str) -> None:
    actual = frozenset(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        details: list[str] = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if extra:
            details.append(f"unexpected {', '.join(extra)}")
        raise ValueError(f"{label} has invalid members ({'; '.join(details)})")


def _require_source_string(source: dict[str, Any], key: str) -> str:
    value = source[key]
    if not isinstance(value, str) or not value or value != value.strip() or "\x00" in value:
        raise ValueError(f"source.{key} must be a non-empty string without surrounding whitespace")
    return value


def _validate_source(source_value: Any) -> dict[str, str]:
    source = _require_object(source_value, "source")
    _require_exact_keys(source, _SOURCE_KEYS, "source")
    result = {key: _require_source_string(source, key) for key in _SOURCE_KEYS}
    if not _COMMIT.fullmatch(result["commit"]):
        raise ValueError("source.commit must be 40-64 lowercase hexadecimal characters")
    if not result["ref"].startswith("refs/"):
        raise ValueError("source.ref must start with refs/")
    return result


def _safe_basename(name_value: Any, label: str) -> str:
    if not isinstance(name_value, str) or not name_value:
        raise ValueError(f"{label}.name must be a non-empty string")
    # Check both path syntaxes so a manifest made on one platform cannot be
    # interpreted as a traversal path after being downloaded on another.
    if (
        name_value in {".", ".."}
        or "/" in name_value
        or "\\" in name_value
        or Path(name_value).is_absolute()
        or PureWindowsPath(name_value).is_absolute()
        or PureWindowsPath(name_value).drive
    ):
        raise ValueError(f"{label}.name must be a relative regular-file basename")
    return name_value


def _validate_file_record(record_value: Any, label: str) -> dict[str, Any]:
    record = _require_object(record_value, label)
    _require_exact_keys(record, _FILE_KEYS, label)
    name = _safe_basename(record["name"], label)
    size = record["size"]
    if isinstance(size, bool) or not isinstance(size, int) or size < 0:
        raise ValueError(f"{label}.size must be a non-negative integer")
    digest = record["sha256"]
    if not isinstance(digest, str) or not _HEX64.fullmatch(digest):
        raise ValueError(f"{label}.sha256 must be 64 lowercase hexadecimal characters")
    return {"name": name, "size": size, "sha256": digest}


def _resolve_child(root: Path, name: str, label: str) -> Path:
    candidate = root / name
    if candidate.is_symlink():
        raise ValueError(f"{label} resolves through a symbolic link: {name}")
    try:
        resolved_root = root.resolve(strict=True)
        resolved = candidate.resolve(strict=True)
    except OSError as exc:
        raise ValueError(f"{label} cannot be resolved: {exc}") from exc
    try:
        resolved.relative_to(resolved_root)
    except ValueError as exc:
        raise ValueError(f"{label} escapes the artifact directory: {name}") from exc
    if resolved.is_symlink() or not resolved.is_file():
        raise ValueError(f"{label} is not a regular file: {name}")
    return resolved


def _digest(path: Path) -> tuple[int, str]:
    hasher = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                hasher.update(chunk)
                size += len(chunk)
    except OSError as exc:
        raise ValueError(f"cannot read release file {path}: {exc}") from exc
    return size, hasher.hexdigest()


def verify_manifest(
    manifest_path: Path,
    *,
    directory: Path | None = None,
    repository: str | None = None,
    commit: str | None = None,
    ref: str | None = None,
) -> list[str]:
    """Return deterministic integrity errors for one release package."""

    root = (directory if directory is not None else manifest_path.parent)
    errors: list[str] = []
    try:
        if root.is_symlink() or not root.is_dir():
            raise ValueError(f"artifact directory is not a regular directory: {root}")
        if manifest_path.is_symlink():
            raise ValueError(f"manifest must not be a symbolic link: {manifest_path}")
        if not manifest_path.is_file():
            raise ValueError(f"manifest is not a regular file: {manifest_path}")
        manifest_resolved = manifest_path.resolve(strict=True)
        root_resolved = root.resolve(strict=True)
        try:
            manifest_resolved.relative_to(root_resolved)
        except ValueError as exc:
            raise ValueError("manifest escapes the artifact directory") from exc
        document = _load_json(manifest_resolved)
        root_object = _require_object(document, "manifest root")
        _require_exact_keys(root_object, _ROOT_KEYS, "manifest root")
        if root_object["format"] != FORMAT:
            raise ValueError(f"manifest format must be {FORMAT}")
        source = _validate_source(root_object["source"])
        expected = {"repository": repository, "commit": commit, "ref": ref}
        for key, wanted in expected.items():
            if wanted is not None and source[key] != wanted:
                raise ValueError(f"source.{key} does not match expected value")

        files = _require_object(root_object["files"], "files")
        _require_exact_keys(files, frozenset(_FILE_KINDS), "files")
        seen: set[str] = set()
        for kind in _FILE_KINDS:
            record = _validate_file_record(files[kind], f"files.{kind}")
            name = record["name"]
            if name in seen:
                raise ValueError(f"files contains duplicate file name: {name}")
            seen.add(name)
            actual = _resolve_child(root_resolved, name, f"files.{kind}")
            size, digest = _digest(actual)
            if size != record["size"]:
                raise ValueError(
                    f"files.{kind}.size mismatch: manifest={record['size']} actual={size}"
                )
            if digest != record["sha256"]:
                raise ValueError(
                    f"files.{kind}.sha256 mismatch: manifest={record['sha256']} actual={digest}"
                )
    except (OSError, TypeError, ValueError) as exc:
        errors.append(str(exc))
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument(
        "--directory",
        type=Path,
        help="directory containing the manifest and named release files (default: manifest parent)",
    )
    parser.add_argument("--repository")
    parser.add_argument("--commit")
    parser.add_argument("--ref")
    args = parser.parse_args(argv)
    errors = verify_manifest(
        args.manifest,
        directory=args.directory,
        repository=args.repository,
        commit=args.commit,
        ref=args.ref,
    )
    if errors:
        for error in errors:
            print(f"release manifest: {error}", file=sys.stderr)
        return 1
    print(f"release manifest: passed ({args.manifest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
