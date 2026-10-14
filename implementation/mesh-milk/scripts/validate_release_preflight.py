#!/usr/bin/env python3
"""Run the repository-side preflight for a BLE evidence package.

``validate_ble_evidence.py`` checks the manifest and its evidence files.  This
companion gate adds repository facts that an evidence author must not be able
to claim only by editing JSON: the recorded commit must exist, the repository
must be a Git worktree, and the frozen ``docs/spec/`` tree must have no
staged, unstaged, or untracked changes.  It intentionally ignores unrelated
working-tree changes so a release check cannot destroy or silently adjudicate
another in-progress task.

Usage (from ``implementation/mesh-milk``)::

    python scripts/validate_release_preflight.py \
        /path/to/evidence.json --repo ../..
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Callable, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from validate_ble_evidence import MAX_MANIFEST_BYTES, validate_manifest  # type: ignore[import-not-found]


GitRunner = Callable[[Path, Sequence[str]], tuple[int, str, str]]
GIT_COMMAND_TIMEOUT_SECONDS = 30


def _run_git(repo: Path, args: Sequence[str]) -> tuple[int, str, str]:
    try:
        result = subprocess.run(
            ["git", "-C", str(repo), *args],
            capture_output=True,
            text=True,
            check=False,
            timeout=GIT_COMMAND_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as exc:
        return 124, "", f"git command timed out after {GIT_COMMAND_TIMEOUT_SECONDS}s: {exc}"
    except OSError as exc:
        return 127, "", str(exc)
    return result.returncode, result.stdout, result.stderr


def validate_repository(repo: Path, commit: str, runner: GitRunner = _run_git) -> list[str]:
    """Return repository-side preflight errors without changing Git state."""

    errors: list[str] = []
    if not repo.is_dir():
        return [f"repository directory does not exist: {repo}"]
    specification = repo / "docs" / "spec"
    if not specification.is_dir():
        errors.append(f"repository is missing docs/spec/: {specification}")

    code, root_output, detail = runner(repo, ("rev-parse", "--show-toplevel"))
    if code != 0 or not root_output.strip():
        errors.append(f"repository is not a Git worktree: {detail.strip() or root_output.strip()}")

    if commit:
        code, _output, detail = runner(repo, ("cat-file", "-e", f"{commit}^{{commit}}"))
        if code != 0:
            errors.append(f"recorded commit does not exist: {commit} ({detail.strip()})")
        else:
            # Existence alone is not enough: an evidence package recorded
            # against an older (or unrelated) object would otherwise pass
            # while this checkout runs a different implementation. Resolve
            # both sides to full commit IDs so abbreviated manifest values
            # remain supported, then require the repository under test to be
            # exactly the revision named by the package.
            resolve_code, resolved, resolve_detail = runner(
                repo, ("rev-parse", f"{commit}^{{commit}}")
            )
            head_code, head, head_detail = runner(repo, ("rev-parse", "HEAD"))
            resolved_id = resolved.strip().splitlines()[0] if resolved.strip() else ""
            head_id = head.strip().splitlines()[0] if head.strip() else ""
            if resolve_code != 0 or not resolved_id:
                errors.append(
                    f"could not resolve recorded commit: {commit} "
                    f"({resolve_detail.strip() or resolved.strip()})"
                )
            elif head_code != 0 or not head_id:
                errors.append(
                    "could not resolve checked-out HEAD: "
                    f"{head_detail.strip() or head.strip()}"
                )
            elif resolved_id.casefold() != head_id.casefold():
                errors.append(
                    "checked-out HEAD does not match recorded commit: "
                    f"HEAD={head_id}, recorded={resolved_id}"
                )

    # `status --porcelain` includes both index and worktree changes and the
    # explicit untracked flag covers a newly created specification file.
    code, output, detail = runner(repo, ("status", "--porcelain=v1", "--untracked-files=all", "--", "docs/spec"))
    if code != 0:
        errors.append(f"could not inspect docs/spec/ status: {detail.strip() or output.strip()}")
    elif output.strip():
        changed = "; ".join(line.strip() for line in output.splitlines() if line.strip())
        errors.append(f"docs/spec/ has staged, unstaged, or untracked changes: {changed}")
    return errors


def _manifest_commit(path: Path) -> str:
    try:
        if path.stat().st_size > MAX_MANIFEST_BYTES:
            return ""
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return ""
    return document.get("commit", "") if isinstance(document, dict) and isinstance(document.get("commit"), str) else ""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="JSON evidence manifest")
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="repository root (default: current directory)")
    args = parser.parse_args(argv)

    errors = validate_manifest(args.manifest)
    errors.extend(validate_repository(args.repo.resolve(), _manifest_commit(args.manifest)))
    if errors:
        for error in errors:
            print(f"Release preflight: {error}", file=sys.stderr)
        return 1
    print(f"Release preflight: passed ({args.manifest})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
