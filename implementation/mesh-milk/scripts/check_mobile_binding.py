#!/usr/bin/env python3
"""Validate one generated mobile binding before packaging it.

The regular ``mobile-bindings`` CI job checks Kotlin and Swift together. The
Android AAR and iOS XCFramework packaging jobs generate only one language at a
time, so they need this one-language variant as a packaging gate. It checks
both properties that matter at this boundary:

* the generated ``MmpNode`` protocol/interface exposes every Rust FFI method;
* Rust-internal secret-state identifiers do not enter the generated source.

Usage (from ``implementation/mesh-milk``)::

    python scripts/check_mobile_binding.py kotlin path/to/kotlin-bindings
    python scripts/check_mobile_binding.py swift path/to/swift-bindings

This is a source-level check only; it does not compile Kotlin/Swift or require
native hardware.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# The scripts directory is placed on sys.path when this file is run as
# ``python scripts/check_mobile_binding.py``. Keep one API manifest and one
# secret denylist rather than maintaining packaging-specific copies.
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from check_mobile_api_surface import EXPECTED_MMP_NODE_METHODS  # type: ignore[import-not-found]
from check_mobile_secret_surface import scan_directory as scan_secret_directory  # type: ignore[import-not-found]


def _read_files(directory: Path, suffix: str) -> tuple[list[Path], list[str]]:
    if not directory.is_dir():
        return [], [f"binding directory does not exist: {directory}"]
    files = sorted(
        path
        for path in directory.rglob("*")
        if path.is_file() and path.suffix.lower() == suffix
    )
    if not files:
        return [], [f"binding directory contains no {suffix} source files: {directory}"]
    violations: list[str] = []
    for path in files:
        try:
            path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            violations.append(f"{path}: not valid UTF-8 ({exc})")
    return files, violations


def _surface_violations(language: str, directory: Path) -> list[str]:
    suffix = ".kt" if language == "kotlin" else ".swift"
    files, violations = _read_files(directory, suffix)
    if not files:
        return violations
    text = "\n".join(path.read_text(encoding="utf-8") for path in files)
    if language == "kotlin":
        if not re.search(r"\binterface\s+MmpNodeInterface\b", text):
            violations.append("generated Kotlin output has no MmpNodeInterface")
        for method in EXPECTED_MMP_NODE_METHODS:
            pattern = rf"\boverride\s+fun\s+`{re.escape(method)}`\s*\("
            if not re.search(pattern, text):
                violations.append(f"generated Kotlin output is missing MmpNode.{method}")
    else:
        if not re.search(r"\bprotocol\s+MmpNodeProtocol\b", text):
            violations.append("generated Swift output has no MmpNodeProtocol")
        for method in EXPECTED_MMP_NODE_METHODS:
            pattern = rf"\bfunc\s+{re.escape(method)}\s*\("
            if not re.search(pattern, text):
                violations.append(f"generated Swift output is missing MmpNode.{method}")
    return violations


def scan_binding(language: str, directory: Path) -> list[str]:
    """Return API and secret-surface violations for one language tree."""

    violations = _surface_violations(language, directory)
    # The existing scanner is language-agnostic and exact-identifier based; it
    # will inspect only the generated suffix present in this directory.
    violations.extend(scan_secret_directory(directory))
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("language", choices=("kotlin", "swift"))
    parser.add_argument("directory", type=Path)
    args = parser.parse_args(argv)

    violations = scan_binding(args.language, args.directory)
    if violations:
        for violation in violations:
            print(f"mobile binding check: {violation}", file=sys.stderr)
        return 1
    print(f"mobile binding check: passed ({args.language}, {args.directory})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
