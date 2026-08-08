#!/usr/bin/env python3
"""Reject secret-state symbols from generated Kotlin/Swift bindings.

Usage (from ``implementation/mesh-milk``)::

    python scripts/check_mobile_secret_surface.py bindings/kotlin bindings/swift

The first directory is scanned as Kotlin output and the second as Swift output.
The check deliberately matches exact API identifiers rather than generic words
such as ``prekey`` or ``secret``; generated documentation may legitimately
mention those concepts while the secret-state types and methods must remain
Rust-internal.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


# Keep this list in sync with the Rust-internal names in mmp-node/src/vault.rs
# and mmp-node/src/secret_state.rs.  Camel-case and snake-case spellings cover
# Kotlin/Swift codegen without rejecting ordinary prose in generated comments.
FORBIDDEN_IDENTIFIERS = (
    "SnapshotVault",
    "SealedRecord",
    "SealedCandidate",
    "SecretState",
    "RestoredState",
    "VaultError",
    "wrapped_dek",
    "wrappedDek",
    "wrappedDEK",
    "chain_key",
    "chainKey",
    "skipped_key",
    "skippedKey",
    "ratchet_state",
    "ratchetState",
    "prekey_secret",
    "prekeySecret",
    "secret_state_snapshot",
    "secretStateSnapshot",
    "restore_secret_state",
    "restoreSecretState",
    "snapshot_vault",
    "snapshotVault",
)

_FORBIDDEN = re.compile(
    r"(?<![A-Za-z0-9_])(?:"
    + "|".join(re.escape(identifier) for identifier in FORBIDDEN_IDENTIFIERS)
    + r")(?![A-Za-z0-9_])"
)

SOURCE_SUFFIXES = {".kt", ".kts", ".swift"}


def scan_directory(directory: Path) -> list[str]:
    """Return deterministic, human-readable violations for one binding tree."""

    if not directory.is_dir():
        return [f"binding directory does not exist: {directory}"]

    files = sorted(
        path
        for path in directory.rglob("*")
        if path.is_file() and path.suffix.lower() in SOURCE_SUFFIXES
    )
    if not files:
        return [f"binding directory contains no Kotlin/Swift source files: {directory}"]

    violations: list[str] = []
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            violations.append(f"{path}: not valid UTF-8 ({exc})")
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            for match in _FORBIDDEN.finditer(line):
                violations.append(f"{path}:{line_number}: forbidden secret-state identifier {match.group(0)!r}")
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kotlin_dir", type=Path, help="generated Kotlin binding directory")
    parser.add_argument("swift_dir", type=Path, help="generated Swift binding directory")
    args = parser.parse_args(argv)

    violations = scan_directory(args.kotlin_dir) + scan_directory(args.swift_dir)
    if violations:
        for violation in violations:
            print(f"mobile secret-surface check: {violation}", file=sys.stderr)
        return 1
    print(f"mobile secret-surface check: passed ({args.kotlin_dir}, {args.swift_dir})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
