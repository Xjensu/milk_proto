#!/usr/bin/env python3
"""Validate the checked-in specification status and publication manifest.

This is a release-governance check only.  It does not change the wire contract
or infer that a ``FINAL`` label proves implementation or production readiness.
The manifest pins each top-level specification document to its declared status
marker and SHA-256 digest, and records which publication artifacts are present
or intentionally historical references.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath
from typing import Any


FORMAT = "mesh-milk-spec-status-v1"
DEFAULT_MANIFEST = Path(__file__).resolve().parents[3] / "docs" / "SPEC_STATUS_MANIFEST.json"
MAX_MANIFEST_BYTES = 256 * 1024
SHA256 = re.compile(r"^[0-9a-f]{64}$")
ALLOWED_STATUSES = frozenset(
    {
        "final-v1-suite",
        "research-phase-1",
        "research-phase-5",
        "draft-phase-2",
        "draft-phase-3",
        "draft-phase-4",
        "draft-phase-5",
        "final-extension-rev-b",
    }
)
ROOT_KEYS = frozenset(("format", "wire_version", "generated_from", "documents", "publication_artifacts"))
DOCUMENT_KEYS = frozenset(("path", "status", "status_marker", "sha256"))
PUBLICATION_KEYS = frozenset(("required_present", "historical_references_not_shipped"))
HISTORICAL_KEYS = frozenset(("path", "reason"))


class _DuplicateKey(ValueError):
    pass


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateKey(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _load_manifest(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"manifest must be a regular file: {path}")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ValueError(f"manifest cannot be read: {exc}") from exc
    if len(raw) > MAX_MANIFEST_BYTES:
        raise ValueError(f"manifest exceeds maximum size of {MAX_MANIFEST_BYTES} bytes")
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=lambda value: (_ for _ in ()).throw(
                ValueError(f"non-finite JSON value is not allowed: {value}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, TypeError, ValueError) as exc:
        raise ValueError(f"manifest is not strict JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError("manifest root must be an object")
    return value


def _exact_keys(value: dict[str, Any], expected: frozenset[str], label: str) -> None:
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        details = []
        if missing:
            details.append(f"missing {', '.join(missing)}")
        if extra:
            details.append(f"unexpected {', '.join(extra)}")
        raise ValueError(f"{label} has invalid members ({'; '.join(details)})")


def _safe_repo_relative(value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError(f"{label} must be a non-empty POSIX relative path")
    candidate = PurePosixPath(value)
    if candidate.is_absolute() or ".." in candidate.parts or "." in candidate.parts:
        raise ValueError(f"{label} must not escape the repository")
    return Path(*candidate.parts)


def _digest(path: Path) -> str:
    hasher = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                hasher.update(chunk)
    except OSError as exc:
        raise ValueError(f"cannot read {path}: {exc}") from exc
    return hasher.hexdigest()


def _validate_documents(repo: Path, value: Any) -> None:
    if not isinstance(value, list) or not value:
        raise ValueError("documents must be a non-empty array")
    expected_paths = sorted(path.relative_to(repo).as_posix() for path in (repo / "specification").glob("*.md"))
    records: list[tuple[str, dict[str, Any]]] = []
    seen: set[str] = set()
    for index, raw_record in enumerate(value):
        if not isinstance(raw_record, dict):
            raise ValueError(f"documents[{index}] must be an object")
        _exact_keys(raw_record, DOCUMENT_KEYS, f"documents[{index}]")
        relative = _safe_repo_relative(raw_record["path"], f"documents[{index}].path")
        normalized = relative.as_posix()
        if not normalized.startswith("specification/") or relative.suffix != ".md":
            raise ValueError(f"documents[{index}].path must name a top-level specification Markdown file")
        if normalized in seen:
            raise ValueError(f"duplicate specification document: {normalized}")
        seen.add(normalized)
        status = raw_record["status"]
        if status not in ALLOWED_STATUSES:
            raise ValueError(f"documents[{index}].status is not recognized: {status!r}")
        marker = raw_record["status_marker"]
        if not isinstance(marker, str) or not marker:
            raise ValueError(f"documents[{index}].status_marker must be non-empty")
        digest = raw_record["sha256"]
        if not isinstance(digest, str) or not SHA256.fullmatch(digest):
            raise ValueError(f"documents[{index}].sha256 must be lowercase SHA-256")
        records.append((normalized, raw_record))
    listed_paths = [path for path, _record in records]
    if listed_paths != expected_paths:
        missing = sorted(set(expected_paths) - set(listed_paths))
        extra = sorted(set(listed_paths) - set(expected_paths))
        raise ValueError(f"specification document set mismatch (missing={missing}, extra={extra})")
    for normalized, record in records:
        path = repo / normalized
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"specification document must be a regular file: {normalized}")
        if _digest(path) != record["sha256"]:
            raise ValueError(f"SHA-256 mismatch for {normalized}")
        try:
            header = path.read_text(encoding="utf-8").splitlines()[:12]
        except (OSError, UnicodeDecodeError) as exc:
            raise ValueError(f"cannot read status header for {normalized}: {exc}") from exc
        if record["status_marker"] not in "\n".join(header):
            raise ValueError(f"status marker missing from header of {normalized}")


def _validate_publication(repo: Path, value: Any) -> None:
    if not isinstance(value, dict):
        raise ValueError("publication_artifacts must be an object")
    _exact_keys(value, PUBLICATION_KEYS, "publication_artifacts")
    required = value["required_present"]
    if not isinstance(required, list) or required != sorted(required):
        raise ValueError("publication_artifacts.required_present must be a sorted array")
    for raw_path in required:
        relative = _safe_repo_relative(raw_path, "required publication path")
        if len(relative.parts) != 1:
            raise ValueError(f"required publication path must be root-level: {raw_path}")
        path = repo / relative
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"required publication artifact is missing, empty, or not regular: {raw_path}")
    historical = value["historical_references_not_shipped"]
    if not isinstance(historical, list):
        raise ValueError("historical_references_not_shipped must be an array")
    seen: set[str] = set()
    for index, raw_record in enumerate(historical):
        if not isinstance(raw_record, dict):
            raise ValueError(f"historical_references_not_shipped[{index}] must be an object")
        _exact_keys(raw_record, HISTORICAL_KEYS, f"historical_references_not_shipped[{index}]")
        relative = _safe_repo_relative(raw_record["path"], f"historical_references_not_shipped[{index}].path")
        normalized = relative.as_posix()
        if len(relative.parts) != 1 or normalized in seen:
            raise ValueError(f"invalid historical publication path: {normalized}")
        seen.add(normalized)
        if not isinstance(raw_record["reason"], str) or not raw_record["reason"].strip():
            raise ValueError(f"historical_references_not_shipped[{index}].reason must be non-empty")
        if (repo / relative).exists():
            raise ValueError(f"historical publication reference is unexpectedly present: {normalized}")


def validate_manifest(manifest: Path, repo: Path) -> list[str]:
    """Return deterministic errors for the status manifest and repository."""

    errors: list[str] = []
    try:
        document = _load_manifest(manifest)
        _exact_keys(document, ROOT_KEYS, "manifest root")
        if document["format"] != FORMAT:
            raise ValueError(f"manifest format must be {FORMAT}")
        if document["wire_version"] != "MMP/1":
            raise ValueError("manifest wire_version must be MMP/1")
        if not isinstance(document["generated_from"], str) or not document["generated_from"]:
            raise ValueError("manifest generated_from must be non-empty")
        _validate_documents(repo, document["documents"])
        _validate_publication(repo, document["publication_artifacts"])
    except (OSError, TypeError, ValueError) as exc:
        errors.append(str(exc))
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    args = parser.parse_args(argv)
    errors = validate_manifest(args.manifest.resolve(), args.repo.resolve())
    if errors:
        for error in errors:
            print(f"Specification status: {error}", file=sys.stderr)
        return 1
    print(f"Specification status: passed ({args.manifest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
