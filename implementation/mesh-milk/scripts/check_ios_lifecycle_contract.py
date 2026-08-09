#!/usr/bin/env python3
"""Check the documented iOS/CoreBluetooth lifecycle contract.

This is deliberately a source-level gate.  It does not claim that Swift,
CoreBluetooth, or a physical iOS device was compiled or exercised.  It keeps
the two integration guides aligned on the invariants a native iOS shell must
preserve before those external gates are available:

* restoration identifiers and restoration callbacks are present;
* the node is owned by the serial drive queue;
* monotonic time and Keychain-backed identity are used; and
* BLE back-pressure and link lifecycle are forwarded to the Rust node.

Usage (from ``implementation/mesh-milk``)::

    python scripts/check_ios_lifecycle_contract.py

Optional positional arguments select the mobile and React Native guides.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Iterable


_SECTION_RE = re.compile(r"(?m)^##\s+(?P<title>[^\n]+)\n")

# Keep these requirements intentionally tied to the platform boundary rather
# than to prose wording.  The docs may be edited for clarity without changing
# the contract, while deleting one of these markers becomes a review-visible
# failure before an iOS host exists to catch it.
MOBILE_IOS_MARKERS = (
    "CBCentralManagerOptionRestoreIdentifierKey",
    "CBPeripheralManagerOptionRestoreIdentifierKey",
    "ProcessInfo.processInfo.systemUptime",
    "Keychain.loadOrCreateSeed()",
    "canSendWriteWithoutResponse",
    "node.linkUp",
    "node.linkData",
    "node.linkDown",
)

REACT_NATIVE_IOS_MARKERS = (
    "CBCentralManagerOptionRestoreIdentifierKey",
    "CBPeripheralManagerOptionRestoreIdentifierKey",
    "centralManager(_:willRestoreState:)",
    "peripheralManager(_:willRestoreState:)",
    "driveQueue",
    "ProcessInfo.processInfo.systemUptime",
    "Keychain.loadOrCreateSeed()",
    "flush()",
)

# A wall-clock call in the node tick itself would violate the sans-IO timing
# contract.  The guides may mention Date() while explicitly forbidding it, so
# only executable-looking tick expressions are rejected.
_WALL_CLOCK_TICK = re.compile(r"\bnode\.tick\s*\([^\n`]*(?:Date\s*\(|Date\.\s*\()")


def _section(text: str, title_fragment: str) -> str | None:
    """Return one level-2 section whose title contains ``title_fragment``."""

    matches = list(_SECTION_RE.finditer(text))
    for index, match in enumerate(matches):
        if title_fragment.lower() not in match.group("title").lower():
            continue
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        return text[match.end() : end]
    return None


def _check_section(path: Path, title: str, markers: Iterable[str]) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"{path}: cannot read integration guide ({exc})"]
    except UnicodeDecodeError as exc:
        return [f"{path}: integration guide is not UTF-8 ({exc})"]

    section = _section(text, title)
    if section is None:
        return [f"{path}: missing iOS section containing {title!r}"]

    violations = [
        f"{path}: iOS section is missing required lifecycle marker {marker!r}"
        for marker in markers
        if marker not in section
    ]
    if _WALL_CLOCK_TICK.search(section):
        violations.append(
            f"{path}: iOS node.tick() must use monotonic time, not a wall-clock Date()"
        )
    return violations


def check_contract(mobile_guide: Path, react_native_guide: Path) -> list[str]:
    """Return deterministic violations for the two iOS integration guides."""

    return _check_section(mobile_guide, "iOS (Swift)", MOBILE_IOS_MARKERS) + _check_section(
        react_native_guide, "iOS (Swift) native module", REACT_NATIVE_IOS_MARKERS
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "mobile_guide",
        nargs="?",
        type=Path,
        default=Path("MOBILE_INTEGRATION.md"),
        help="path to MOBILE_INTEGRATION.md",
    )
    parser.add_argument(
        "react_native_guide",
        nargs="?",
        type=Path,
        default=Path("REACT_NATIVE_INTEGRATION.md"),
        help="path to REACT_NATIVE_INTEGRATION.md",
    )
    args = parser.parse_args(argv)
    violations = check_contract(args.mobile_guide, args.react_native_guide)
    if violations:
        for violation in violations:
            print(f"iOS lifecycle-contract check: {violation}", file=sys.stderr)
        return 1
    print(
        "iOS lifecycle-contract check: passed "
        f"({args.mobile_guide}, {args.react_native_guide})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
