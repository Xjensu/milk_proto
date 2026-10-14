#!/usr/bin/env python3
"""Print a non-secret, canonical fingerprint for an MMP identity seed.

``mmp-cli keygen`` stores a 32-byte seed as exactly 64 hexadecimal characters.
This helper hashes the decoded seed bytes (rather than the file's text
formatting) so the evidence value is stable across hosts and line endings.
It never prints the seed itself.  The result is an evidence-package label, not
an MMP node identifier and not a substitute for protecting the seed file.

Usage (from ``implementation/mesh-milk``)::

    python scripts/fingerprint_seed.py /secure/path/alice.seed
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path


_SEED_HEX = re.compile(r"^[0-9a-fA-F]{64}$")
_MAX_SEED_FILE_BYTES = 4096


def fingerprint_seed(path: Path) -> str:
    """Return ``sha256:<hex>`` for one canonical 32-byte seed file."""

    if path.is_symlink():
        raise ValueError(f"seed file must not be a symbolic link: {path}")
    if not path.is_file():
        raise ValueError(f"seed file is not a regular file: {path}")
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise ValueError(f"seed file cannot be read: {exc}") from exc
    if len(raw) > _MAX_SEED_FILE_BYTES:
        raise ValueError(f"seed file exceeds {_MAX_SEED_FILE_BYTES} bytes")
    try:
        text = raw.decode("ascii").strip()
    except UnicodeDecodeError as exc:
        raise ValueError("seed file must contain ASCII hexadecimal text") from exc
    if not _SEED_HEX.fullmatch(text):
        raise ValueError("seed file must contain exactly 64 hexadecimal characters")
    seed = bytes.fromhex(text)
    return f"sha256:{hashlib.sha256(seed).hexdigest()}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("seed", type=Path, help="mmp-cli keygen seed file")
    args = parser.parse_args(argv)
    try:
        print(fingerprint_seed(args.seed))
    except ValueError as exc:
        print(f"seed fingerprint: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
