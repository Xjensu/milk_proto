#!/usr/bin/env python3
"""Create a deterministic digest manifest for one signed release artifact.

The release workflow signs the executable with keyless Sigstore.  This small
companion record makes the exact bytes and source revision visible to a
consumer without relying on a platform-specific checksum command.  It does
not replace signature verification: the binary signature remains the
authenticity check, while this manifest is a reproducible identity and audit
record for the uploaded artifact set.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any


FORMAT = "mesh-milk-release-manifest-v1"
_COMMIT = re.compile(r"^[0-9a-f]{40,64}$")


def _source_string(value: str, *, label: str) -> str:
    if not isinstance(value, str) or not value or value != value.strip() or "\x00" in value:
        raise ValueError(f"{label} must be a non-empty string without surrounding whitespace")
    return value


def _record(path: Path, *, kind: str) -> dict[str, Any]:
    """Return a stable record for one regular, non-symlink file."""

    if path.is_symlink():
        raise ValueError(f"{kind} must not be a symbolic link: {path}")
    if not path.is_file():
        raise ValueError(f"{kind} is not a regular file: {path}")

    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    if size == 0:
        raise ValueError(f"{kind} must not be empty: {path}")
    return {"name": path.name, "size": size, "sha256": digest.hexdigest()}


def build_manifest(
    artifact: Path,
    signature: Path,
    certificate: Path,
    *,
    repository: str,
    commit: str,
    ref: str,
) -> dict[str, Any]:
    """Build the JSON-compatible manifest without writing any files."""

    repository = _source_string(repository, label="repository")
    commit = _source_string(commit, label="commit")
    ref = _source_string(ref, label="ref")
    if not _COMMIT.fullmatch(commit):
        raise ValueError("commit must be 40-64 lowercase hexadecimal characters")
    if not ref.startswith("refs/"):
        raise ValueError("ref must start with refs/")
    return {
        "format": FORMAT,
        "source": {"repository": repository, "commit": commit, "ref": ref},
        "files": {
            "artifact": _record(artifact, kind="artifact"),
            "certificate": _record(certificate, kind="certificate"),
            "signature": _record(signature, kind="signature"),
        },
    }


def write_manifest(manifest: dict[str, Any], output: Path) -> None:
    """Write a manifest atomically and reject symlinked output paths."""

    if output.is_symlink():
        raise ValueError(f"manifest output must not be a symbolic link: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=output.parent, prefix=f".{output.name}.", delete=False
    ) as stream:
        temporary = Path(stream.name)
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
    try:
        os.replace(temporary, output)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--signature", type=Path, required=True)
    parser.add_argument("--certificate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA", ""))
    parser.add_argument("--ref", default=os.environ.get("GITHUB_REF", ""))
    args = parser.parse_args(argv)

    try:
        manifest = build_manifest(
            args.artifact,
            args.signature,
            args.certificate,
            repository=args.repository,
            commit=args.commit,
            ref=args.ref,
        )
        write_manifest(manifest, args.output)
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(f"wrote release manifest: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
