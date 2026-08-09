#!/usr/bin/env python3
"""Verify the Android artifacts emitted by ``build-android-aar.sh``.

The AAR deliberately has a native-only shape: a manifest and one
``libmmp_ffi.so`` for each supported ABI, with no ``classes.jar``.  UniFFI
Kotlin bindings are shipped as a separate source archive and are checked with
the same API/secret-surface scanner used by the mobile-bindings CI job.

ZIP members are validated before any source is written to disk.  Absolute and
traversal paths are rejected so a malformed artifact cannot escape the
temporary extraction directory used by the Kotlin scanner.

Usage (from ``implementation/mesh-milk``)::

    python scripts/verify_android_artifacts.py \
        mobile-artifacts/android/mmp-ffi.aar \
        mobile-artifacts/android/mmp-ffi-kotlin-bindings.zip
"""

from __future__ import annotations

import argparse
import re
import stat
import sys
import tempfile
from pathlib import Path, PurePosixPath
from zipfile import BadZipFile, ZipFile

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from check_mobile_api_surface import EXPECTED_MMP_NODE_METHODS  # type: ignore[import-not-found]
from check_mobile_binding import scan_binding  # type: ignore[import-not-found]


REQUIRED_ABI_LIBRARIES = (
    "jni/arm64-v8a/libmmp_ffi.so",
    "jni/armeabi-v7a/libmmp_ffi.so",
    "jni/x86_64/libmmp_ffi.so",
)
_ALLOWED_AAR_DIRECTORIES = {
    "jni",
    "jni/arm64-v8a",
    "jni/armeabi-v7a",
    "jni/x86_64",
}
_WINDOWS_ABSOLUTE = re.compile(r"^[A-Za-z]:/")


def _normalised_member_name(name: str) -> tuple[str | None, str | None]:
    """Return a safe POSIX name or a human-readable path violation."""

    # ZIP convention uses '/', but accepting '\\' here would make a member
    # look harmless to ZIP tooling while being interpreted as traversal by a
    # consumer on Windows.  Normalize it before applying the checks.
    normalized = name.replace("\\", "/")
    if normalized.startswith("/") or _WINDOWS_ABSOLUTE.match(normalized):
        return None, f"absolute path in ZIP member: {name}"
    parts = PurePosixPath(normalized).parts
    if ".." in parts:
        return None, f"path traversal in ZIP member: {name}"
    if not normalized:
        return None, f"empty ZIP member path: {name!r}"
    # Canonicalize harmless ``.`` and repeated-slash segments so two member
    # spellings cannot target the same extraction path without being detected
    # as duplicates.  A root directory entry (``./``) is retained as ``.``.
    return ("/".join(parts) if parts else "."), None


def _index_zip(archive_path: Path, kind: str) -> tuple[ZipFile | None, dict[str, object], list[str]]:
    """Open and index a ZIP, rejecting unsafe or duplicate member names."""

    if not archive_path.is_file():
        return None, {}, [f"{kind} archive does not exist: {archive_path}"]
    try:
        archive = ZipFile(archive_path)
    except (BadZipFile, OSError) as exc:
        return None, {}, [f"{kind} archive is not a valid ZIP: {archive_path} ({exc})"]

    members: dict[str, object] = {}
    violations: list[str] = []
    for info in archive.infolist():
        if _is_symlink(info):
            violations.append(f"symbolic-link ZIP member is not permitted: {info.filename}")
            continue
        normalized, violation = _normalised_member_name(info.filename)
        if violation:
            violations.append(violation)
            continue
        assert normalized is not None
        if normalized in members:
            violations.append(f"duplicate ZIP member after normalization: {normalized}")
            continue
        members[normalized] = info
    return archive, members, violations


def _is_directory(info: object) -> bool:
    return bool(getattr(info, "is_dir")())


def _is_symlink(info: object) -> bool:
    """Return whether a ZIP member carries a Unix symbolic-link mode.

    ``zipfile`` treats symlink entries as ordinary files, so relying on
    ``is_dir()`` alone would allow a consumer that honours the external mode
    bits to materialize a link during extraction.  Artifact inputs are build
    outputs, not source archives; links are unnecessary and are rejected
    before any member is read.
    """

    external_attr = getattr(info, "external_attr", 0)
    if not isinstance(external_attr, int):
        return False
    mode = (external_attr >> 16) & 0xFFFF
    return stat.S_IFMT(mode) == stat.S_IFLNK


def _verify_aar(archive_path: Path) -> list[str]:
    archive, members, violations = _index_zip(archive_path, "AAR")
    if archive is None:
        return violations
    try:
        if "AndroidManifest.xml" not in members:
            violations.append("AAR is missing AndroidManifest.xml")
        else:
            manifest_info = members["AndroidManifest.xml"]
            if _is_directory(manifest_info):
                violations.append("AAR AndroidManifest.xml is a directory")
            else:
                manifest = archive.read(manifest_info)  # type: ignore[arg-type]
                if not manifest:
                    violations.append("AAR AndroidManifest.xml is empty")

        for path in REQUIRED_ABI_LIBRARIES:
            info = members.get(path)
            if info is None:
                violations.append(f"AAR is missing required native library: {path}")
            elif _is_directory(info):
                violations.append(f"AAR native library is a directory: {path}")
            elif not archive.read(info):  # type: ignore[arg-type]
                violations.append(f"AAR native library is empty: {path}")

        required = set(REQUIRED_ABI_LIBRARIES) | {"AndroidManifest.xml"}
        for name, info in members.items():
            if name in required or (_is_directory(info) and name in _ALLOWED_AAR_DIRECTORIES):
                continue
            if not _is_directory(info) and PurePosixPath(name).name == "classes.jar":
                violations.append(f"native-only AAR must not contain classes.jar: {name}")
            else:
                violations.append(f"unexpected member in native-only AAR: {name}")
    finally:
        archive.close()
    return violations


def _write_kotlin_sources(archive: ZipFile, members: dict[str, object], root: Path) -> list[str]:
    """Write only validated Kotlin members into ``root`` for source scanning."""

    source_names = [
        name for name, info in members.items()
        if not _is_directory(info) and PurePosixPath(name).suffix.lower() == ".kt"
    ]
    if not source_names:
        return ["Kotlin bindings archive contains no .kt source files"]

    violations: list[str] = []
    for name in sorted(source_names):
        info = members[name]
        try:
            content = archive.read(info)  # type: ignore[arg-type]
            content.decode("utf-8")
        except UnicodeDecodeError as exc:
            violations.append(f"Kotlin source is not valid UTF-8: {name} ({exc})")
            continue
        destination = root.joinpath(*PurePosixPath(name).parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
    return violations


def _verify_kotlin_bindings(archive_path: Path) -> list[str]:
    archive, members, violations = _index_zip(archive_path, "Kotlin bindings")
    if archive is None:
        return violations
    try:
        with tempfile.TemporaryDirectory(prefix="mmp-kotlin-bindings-") as root:
            extraction_violations = _write_kotlin_sources(archive, members, Path(root))
            violations.extend(extraction_violations)
            if not extraction_violations:
                violations.extend(scan_binding("kotlin", Path(root)))
    finally:
        archive.close()
    return violations


def verify_artifacts(aar_path: Path, kotlin_bindings_path: Path) -> list[str]:
    """Return all deterministic violations for the two Android artifacts."""

    violations = [
        f"AAR: {item}" for item in _verify_aar(aar_path)
    ]
    violations.extend(
        f"Kotlin bindings: {item}"
        for item in _verify_kotlin_bindings(kotlin_bindings_path)
    )
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("aar", type=Path)
    parser.add_argument("kotlin_bindings", type=Path)
    args = parser.parse_args(argv)

    violations = verify_artifacts(args.aar, args.kotlin_bindings)
    if violations:
        for violation in violations:
            print(f"Android artifact verification: {violation}", file=sys.stderr)
        return 1
    print(f"Android artifact verification: passed ({args.aar}, {args.kotlin_bindings})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
