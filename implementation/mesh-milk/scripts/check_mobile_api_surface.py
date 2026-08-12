#!/usr/bin/env python3
"""Check that generated Kotlin and Swift bindings expose the complete MmpNode API.

Usage (from ``implementation/mesh-milk``)::

    python scripts/check_mobile_api_surface.py bindings/kotlin bindings/swift

This is a source-level smoke check, not a native compiler or hardware test. It
protects the mobile integration boundary from silent UniFFI surface drift: the
generated language bindings must contain every public ``MmpNode`` method that
the Rust FFI exports. Update ``EXPECTED_MMP_NODE_METHODS`` deliberately when a
method is added or removed from ``mmp-ffi``.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


# Keep this manifest in sync with the #[uniffi::export] impl MmpNode in
# crates/mmp-ffi/src/lib.rs. These are the methods a native Android/iOS host
# can call; lifecycle methods such as an RN module's start/stop are not part
# of the Rust object and therefore do not belong here.
EXPECTED_MMP_NODE_METHODS = (
    "allowFriendshipFrom",
    "beacon",
    "betaSnapshot",
    "block",
    "capabilities",
    "cancelFetch",
    "closeActionsDropped",
    "createGroup",
    "diagDump",
    "enableDiagnostics",
    "eventQueueDropped",
    "fetchFile",
    "fileProgress",
    "flush",
    "groupInvite",
    "groupMembers",
    "groupRemove",
    "groupSend",
    "ingestClaim",
    "lastPersistError",
    "linkData",
    "linkDown",
    "linkUp",
    "linkUpWithQuality",
    "messageState",
    "metrics",
    "mintAuthCert",
    "nodeId",
    "onBeacon",
    "ownClaims",
    "pinAuthorityRoot",
    "pollClose",
    "pollEvent",
    "pollSend",
    "publicKey",
    "publishAnnouncement",
    "requestFriendship",
    "revokeIdentity",
    "revokeIdentityWithSuccessor",
    "sendFile",
    "sendText",
    "setNickname",
    "setTier",
    "setWallClock",
    "status",
    "storeLen",
    "subscribeScope",
    "tick",
    "tryRevokeIdentity",
    "unsubscribeScope",
)

# UniFFI maps Rust ``Result<_, FfiError>`` methods to throwing operations in
# the generated host bindings.  Keep this list explicit: a name-only surface
# check would accept a stale binding after a Rust method changes from a
# fallible call to an infallible one (or vice versa), which is a source-level
# compatibility break for Kotlin/Swift callers.
EXPECTED_MMP_NODE_THROWING_METHODS = (
    "allowFriendshipFrom",
    "block",
    "cancelFetch",
    "fetchFile",
    "fileProgress",
    "flush",
    "groupInvite",
    "groupMembers",
    "groupRemove",
    "groupSend",
    "ingestClaim",
    "linkUpWithQuality",
    "messageState",
    "mintAuthCert",
    "pinAuthorityRoot",
    "publishAnnouncement",
    "requestFriendship",
    "revokeIdentityWithSuccessor",
    "sendFile",
    "sendText",
    "setNickname",
    "unsubscribeScope",
    "tryRevokeIdentity",
)

SOURCE_SUFFIXES = {".kt", ".kts", ".swift"}


def _read_sources(directory: Path, suffix: str) -> tuple[list[Path], list[str]]:
    """Read one generated-language tree, returning files and violations."""

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


def _joined_text(files: list[Path]) -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in files)


def scan_bindings(kotlin_dir: Path, swift_dir: Path) -> list[str]:
    """Return deterministic violations for the generated Kotlin/Swift trees."""

    kotlin_files, violations = _read_sources(kotlin_dir, ".kt")
    swift_files, swift_violations = _read_sources(swift_dir, ".swift")
    violations.extend(swift_violations)
    if not kotlin_files or not swift_files:
        return violations

    kotlin_text = _joined_text(kotlin_files)
    swift_text = _joined_text(swift_files)
    if not re.search(r"\binterface\s+MmpNodeInterface\b", kotlin_text):
        violations.append("generated Kotlin output has no MmpNodeInterface")
    if not re.search(r"\bprotocol\s+MmpNodeProtocol\b", swift_text):
        violations.append("generated Swift output has no MmpNodeProtocol")

    for method in EXPECTED_MMP_NODE_METHODS:
        kotlin_pattern = rf"\boverride\s+fun\s+`{re.escape(method)}`\s*\("
        kotlin_has_method = re.search(kotlin_pattern, kotlin_text) is not None
        if not kotlin_has_method:
            violations.append(f"generated Kotlin output is missing MmpNode.{method}")

        swift_pattern = rf"\bfunc\s+{re.escape(method)}\s*\("
        swift_has_method = re.search(swift_pattern, swift_text) is not None
        if not swift_has_method:
            violations.append(f"generated Swift output is missing MmpNode.{method}")

    # Check the error-bearing part of the ABI separately from method names.
    # The Kotlin annotation is emitted on the concrete implementation (not
    # the interface declaration), while Swift puts ``throws`` on both the
    # protocol requirement and implementation.  Search the whole generated
    # tree so either layout remains supported.
    for method in EXPECTED_MMP_NODE_THROWING_METHODS:
        kotlin_method_pattern = rf"\boverride\s+fun\s+`{re.escape(method)}`\s*\("
        swift_method_pattern = rf"\bfunc\s+{re.escape(method)}\s*\("
        kotlin_throwing_pattern = (
            rf"@Throws\s*\(\s*FfiException\s*::\s*class\s*\)\s*"
            rf"override\s+fun\s+`{re.escape(method)}`\s*\("
        )
        if re.search(kotlin_method_pattern, kotlin_text) and not re.search(
            kotlin_throwing_pattern, kotlin_text
        ):
            violations.append(
                "generated Kotlin output is missing FfiError throwing annotation "
                f"for MmpNode.{method}"
            )

        swift_throwing_pattern = rf"\bfunc\s+{re.escape(method)}\s*\([^)]*\)\s*throws\b"
        if re.search(swift_method_pattern, swift_text) and not re.search(
            swift_throwing_pattern, swift_text
        ):
            violations.append(
                f"generated Swift output is missing throws for MmpNode.{method}"
            )

    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kotlin_dir", type=Path, help="generated Kotlin binding directory")
    parser.add_argument("swift_dir", type=Path, help="generated Swift binding directory")
    args = parser.parse_args(argv)

    violations = scan_bindings(args.kotlin_dir, args.swift_dir)
    if violations:
        for violation in violations:
            print(f"mobile API-surface check: {violation}", file=sys.stderr)
        return 1
    print(f"mobile API-surface check: passed ({args.kotlin_dir}, {args.swift_dir})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
