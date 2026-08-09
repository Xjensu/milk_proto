#!/usr/bin/env python3
"""Verify the iOS artifacts emitted by ``build-ios-xcframework.sh``.

The check is intentionally structural: it does not pretend to compile Swift
or exercise CoreBluetooth on a host without a native app and physical device.
It does ensure that the produced XCFramework contains both the device and
Apple-Silicon simulator slices named by the build script, that each slice has
a usable static library and headers, and that the generated Swift source is
present and UTF-8.  The existing generated-binding API and secret-surface
checks remain the authoritative source-level gates before assembly.

Usage (from ``implementation/mesh-milk``)::

    python scripts/verify_ios_artifacts.py \
        mobile-artifacts/ios/MmpCore.xcframework \
        mobile-artifacts/ios/swift-bindings
"""

from __future__ import annotations

import argparse
import plistlib
import sys
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from check_mobile_binding import scan_binding  # type: ignore[import-not-found]


EXPECTED_SLICES = {
    "ios-arm64": {"variant": None},
    "ios-arm64-simulator": {"variant": "simulator"},
}


def _safe_relative_path(value: Any, *, field: str, violations: list[str]) -> Path | None:
    if not isinstance(value, str) or not value:
        violations.append(f"Info.plist {field} must be a non-empty string")
        return None
    if "\\" in value:
        violations.append(f"Info.plist {field} must use POSIX separators: {value!r}")
        return None
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts:
        violations.append(f"Info.plist {field} escapes its slice: {value!r}")
        return None
    return Path(*path.parts)


def _safe_slice_path(
    slice_root: Path,
    relative: Path,
    *,
    field: str,
    violations: list[str],
) -> Path | None:
    """Resolve a plist path without following symlinks out of its slice.

    An XCFramework is a build artifact, so a symlink is never needed for the
    library or headers.  Rejecting links is stronger than merely checking the
    lexical ``..`` rule: ``slice/libmmp_ffi.a`` could otherwise be a link to an
    arbitrary file outside the slice and still pass ``Path.is_file()``.  The
    resolved containment check also catches Windows junctions, which are not
    always reported as POSIX-style symlinks.
    """

    candidate = slice_root.joinpath(relative)
    current = slice_root
    try:
        if current.is_symlink():
            violations.append(f"{field} slice root must not be a symbolic link")
            return None
        for component in relative.parts:
            current = current / component
            if current.is_symlink():
                violations.append(f"{field} must not traverse a symbolic link: {relative}")
                return None
            # XCFrameworks are consumed on both case-sensitive and
            # case-insensitive filesystems.  A metadata path whose spelling
            # differs from the packaged entry can pass on macOS/Windows and
            # then select a different file (or fail) on Linux.  Reject it at
            # the artifact boundary instead of relying on host filesystem
            # semantics.
            if current.exists():
                try:
                    matching_entries = [
                        entry
                        for entry in current.parent.iterdir()
                        if entry.name.casefold() == component.casefold()
                    ]
                except OSError as exc:
                    violations.append(f"{field} cannot inspect path casing: {relative} ({exc})")
                    return None
                if len(matching_entries) == 1 and matching_entries[0].name != component:
                    violations.append(
                        f"{field} has non-canonical path casing: {relative} "
                        f"(filesystem entry is {matching_entries[0].name!r})"
                    )
                    return None

        root_resolved = slice_root.resolve(strict=True)
        candidate_resolved = candidate.resolve(strict=False)
        candidate_resolved.relative_to(root_resolved)
    except (OSError, ValueError) as exc:
        violations.append(f"{field} escapes its slice: {relative!s} ({exc})")
        return None
    return candidate


def _first_symlink(path: Path) -> Path | None:
    """Return the first symlink below ``path`` in deterministic order."""

    for descendant in sorted(path.rglob("*")):
        if descendant.is_symlink():
            return descendant
    return None


def _verify_plist(xcframework: Path, violations: list[str]) -> list[dict[str, Any]]:
    plist_path = xcframework / "Info.plist"
    if plist_path.is_symlink():
        violations.append("XCFramework Info.plist must not be a symbolic link")
        return []
    if not plist_path.is_file():
        violations.append("XCFramework is missing Info.plist")
        return []
    try:
        with plist_path.open("rb") as stream:
            plist = plistlib.load(stream)
    except (OSError, plistlib.InvalidFileException, ValueError) as exc:
        violations.append(f"XCFramework Info.plist is unreadable: {exc}")
        return []
    if not isinstance(plist, dict):
        violations.append("XCFramework Info.plist root must be a dictionary")
        return []
    libraries = plist.get("AvailableLibraries")
    if not isinstance(libraries, list):
        violations.append("XCFramework Info.plist has no AvailableLibraries array")
        return []
    if plist.get("CFBundlePackageType") not in (None, "XFWK"):
        violations.append("XCFramework Info.plist has unexpected CFBundlePackageType")
    return [item for item in libraries if isinstance(item, dict)]


def _verify_slice(
    xcframework: Path,
    item: dict[str, Any],
    violations: list[str],
) -> None:
    identifier = item.get("LibraryIdentifier")
    if not isinstance(identifier, str) or not identifier:
        violations.append("XCFramework slice has no LibraryIdentifier")
        return
    expected = EXPECTED_SLICES.get(identifier)
    if expected is None:
        return
    slice_root = xcframework / identifier
    if slice_root.is_symlink():
        violations.append(f"XCFramework slice must not be a symbolic link: {identifier}")
        return
    if not slice_root.is_dir():
        violations.append(f"missing XCFramework slice directory: {identifier}")
        return
    if item.get("SupportedPlatform") not in (None, "ios"):
        violations.append(f"slice {identifier} is not marked for iOS")
    if item.get("SupportedPlatformVariant") != expected["variant"]:
        violations.append(
            f"slice {identifier} has unexpected SupportedPlatformVariant: "
            f"{item.get('SupportedPlatformVariant')!r}"
        )
    architectures = item.get("SupportedArchitectures")
    if not isinstance(architectures, list) or set(architectures) != {"arm64"}:
        violations.append(
            f"slice {identifier} has invalid SupportedArchitectures; expected "
            "exactly ['arm64']: "
            f"{architectures!r}"
        )

    library = _safe_relative_path(item.get("LibraryPath"), field=f"{identifier}.LibraryPath", violations=violations)
    if library is not None:
        library_path = _safe_slice_path(
            slice_root,
            library,
            field=f"{identifier}.LibraryPath",
            violations=violations,
        )
        if library_path is None:
            pass
        elif not library_path.is_file():
            violations.append(f"slice {identifier} is missing static library: {library}")
        elif library_path.stat().st_size == 0:
            violations.append(f"slice {identifier} static library is empty: {library}")

    headers = _safe_relative_path(item.get("HeadersPath"), field=f"{identifier}.HeadersPath", violations=violations)
    if headers is not None:
        headers_path = _safe_slice_path(
            slice_root,
            headers,
            field=f"{identifier}.HeadersPath",
            violations=violations,
        )
        if headers_path is None:
            pass
        elif not headers_path.is_dir():
            violations.append(f"slice {identifier} is missing headers directory: {headers}")
        else:
            symlink = _first_symlink(headers_path)
            if symlink is not None:
                violations.append(
                    f"slice {identifier} headers directory contains a symbolic link: {symlink}"
                )
            elif not any(
                path.is_file() and path.suffix.lower() in {".h", ".modulemap"}
                for path in headers_path.rglob("*")
            ):
                violations.append(f"slice {identifier} headers directory has no header/module map")


def _verify_swift_bindings(bindings: Path, violations: list[str]) -> None:
    if not bindings.is_dir():
        violations.append(f"Swift bindings directory does not exist: {bindings}")
        return
    if bindings.is_symlink():
        violations.append("Swift bindings directory must not be a symbolic link")
        return
    symlink = _first_symlink(bindings)
    if symlink is not None:
        violations.append(f"Swift bindings directory contains a symbolic link: {symlink}")
        return
    swift_files = sorted(path for path in bindings.rglob("*") if path.is_file() and path.suffix == ".swift")
    if not swift_files:
        violations.append(f"Swift bindings directory contains no .swift source: {bindings}")
        return
    for path in swift_files:
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            violations.append(f"{path}: not valid UTF-8 ({exc})")
            continue
        if not text.strip():
            violations.append(f"{path}: Swift source is empty")
    # The build script performs this check before assembly, but the verifier
    # is also the gate used when an XCFramework and its source artifact are
    # copied between hosts.  Re-run the API/secret scan here so a copied or
    # modified binding cannot pass a structural-only check.  Avoid invoking
    # the scanner after local UTF-8/empty checks: its API pass reads source
    # text directly and should not mask the deterministic diagnostics above.
    if not any(
        item.startswith(f"{path}:") or item == f"{path}: Swift source is empty"
        for path in swift_files
        for item in violations
    ):
        violations.extend(scan_binding("swift", bindings))


def verify_artifacts(xcframework: Path, swift_bindings: Path) -> list[str]:
    """Return deterministic violations for one XCFramework output pair."""

    violations: list[str] = []
    if not xcframework.is_dir():
        violations.append(f"XCFramework directory does not exist: {xcframework}")
    else:
        libraries = _verify_plist(xcframework, violations)
        by_id: dict[str, dict[str, Any]] = {}
        by_id_casefold: dict[str, str] = {}
        for item in libraries:
            identifier = item.get("LibraryIdentifier")
            if not isinstance(identifier, str):
                continue
            if identifier in by_id:
                violations.append(f"Info.plist contains duplicate slice: {identifier}")
                continue
            folded = identifier.casefold()
            prior = by_id_casefold.get(folded)
            if prior is not None:
                violations.append(
                    "Info.plist contains case-insensitive slice identifier collision: "
                    f"{prior} vs {identifier}"
                )
                continue
            by_id_casefold[folded] = identifier
            by_id[identifier] = item
        for identifier in sorted(set(by_id) - set(EXPECTED_SLICES)):
            violations.append(f"Info.plist contains unexpected slice: {identifier}")
        for identifier in EXPECTED_SLICES:
            item = by_id.get(identifier)
            if item is None:
                violations.append(f"Info.plist is missing required slice: {identifier}")
            else:
                _verify_slice(xcframework, item, violations)
    _verify_swift_bindings(swift_bindings, violations)
    return violations


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("xcframework", type=Path, help="path to MmpCore.xcframework")
    parser.add_argument("swift_bindings", type=Path, help="path to generated Swift source directory")
    args = parser.parse_args(argv)
    violations = verify_artifacts(args.xcframework, args.swift_bindings)
    if violations:
        for violation in violations:
            print(f"iOS artifact verification: {violation}", file=sys.stderr)
        return 1
    print(f"iOS artifact verification: passed ({args.xcframework}, {args.swift_bindings})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
