#!/usr/bin/env python3
"""Check that the normative specification and reboot report exist.

The publication anchors are ``docs/spec/index.md`` and
``docs/internal/release-evidence/spec-reboot-report.md``. The check validates
the checkout and the recorded Git tree; it does not claim that a remote host
published the same revision.

Usage (from ``implementation/mesh-milk``)::

    python scripts/check_release_governance.py \
        --repo ../.. \
        --commit <release-commit-sha>
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Callable, Sequence


REQUIRED_PUBLICATION_FILES = (
    "docs/spec/index.md",
    "docs/internal/release-evidence/spec-reboot-report.md",
)
GitRunner = Callable[[Path, Sequence[str]], tuple[int, str, str]]


def _run_git(repo: Path, args: Sequence[str]) -> tuple[int, str, str]:
    try:
        result = subprocess.run(
            ["git", "-C", str(repo), *args],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as exc:
        return 127, "", str(exc)
    return result.returncode, result.stdout, result.stderr


def _working_tree_errors(repo: Path) -> list[str]:
    errors: list[str] = []
    for relative in REQUIRED_PUBLICATION_FILES:
        path = repo / relative
        if path.is_symlink():
            errors.append(f"publication artifact must not be a symbolic link: {relative}")
        elif not path.is_file():
            errors.append(f"publication artifact is missing or not a regular file: {relative}")
        elif path.stat().st_size == 0:
            errors.append(f"publication artifact must not be empty: {relative}")
    return errors


def validate_publication_artifacts(
    repo: Path,
    commit: str = "",
    runner: GitRunner = _run_git,
) -> list[str]:
    """Return deterministic errors for the local and recorded publication tree."""

    errors: list[str] = []
    if not repo.is_dir():
        return [f"repository directory does not exist: {repo}"]
    errors.extend(_working_tree_errors(repo))

    if not commit:
        return errors
    code, output, detail = runner(
        repo,
        ("ls-tree", "-r", commit, "--", *REQUIRED_PUBLICATION_FILES),
    )
    if code != 0:
        errors.append(f"could not inspect publication tree at {commit}: {detail.strip() or output.strip()}")
        return errors
    present: set[str] = set()
    for line in output.splitlines():
        if not line.strip():
            continue
        metadata, separator, relative = line.partition("\t")
        fields = metadata.split()
        if separator != "\t" or len(fields) != 3:
            errors.append(f"recorded commit {commit} has malformed publication tree output")
            continue
        mode, object_type, _object_id = fields
        if relative in REQUIRED_PUBLICATION_FILES:
            if mode != "100644" or object_type != "blob":
                errors.append(
                    f"recorded commit {commit} publication artifact is not a regular blob: {relative}"
                )
            else:
                present.add(relative)
    for relative in REQUIRED_PUBLICATION_FILES:
        if relative not in present:
            errors.append(f"recorded commit {commit} is missing publication artifact: {relative}")
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="repository root (default: current directory)")
    parser.add_argument("--commit", default=os.environ.get("GITHUB_SHA", ""))
    args = parser.parse_args(argv)
    errors = validate_publication_artifacts(args.repo.resolve(), args.commit)
    if errors:
        for error in errors:
            print(f"Release governance: {error}", file=sys.stderr)
        return 1
    print(f"Release governance: passed ({args.repo})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
