#!/usr/bin/env python3
"""Run the fast, changed-scope local verification tier.

The full workspace matrix belongs to CI. This command keeps an edit/compile
loop small by checking only the selected Rust package(s) and their Cargo
dependencies, while still running formatting and warnings-as-errors Clippy.
Use ``--package`` explicitly when a change crosses a package boundary; without
it the script infers packages from the working tree and refuses to silently
turn a manifest change into an unexpectedly long workspace run.

Examples (from ``implementation/mesh-milk``)::

    python scripts/fast_check.py --package mmp-node
    python scripts/fast_check.py --package mmp-types --package mmp-crypto
    python scripts/fast_check.py --python
    python scripts/fast_check.py --all       # deliberate full local matrix
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


WORKSPACE = Path(__file__).resolve().parents[1]
REPO_ROOT = WORKSPACE.parents[1]
# Package-scoped libtest runs may use a few workers locally. The full workspace
# matrix remains serial until every workspace test family has an independent
# fixture/isolation review.
PACKAGE_TEST_THREADS = 4


def _run(command: list[str], *, cwd: Path) -> int:
    print("+", " ".join(command), flush=True)
    return subprocess.run(command, cwd=cwd, check=False).returncode


def _git_paths(*args: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(REPO_ROOT), *args],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return []
    return [line for line in result.stdout.splitlines() if line]


def _changed_paths() -> list[str]:
    paths = set(_git_paths("diff", "--name-only"))
    paths.update(_git_paths("diff", "--cached", "--name-only"))
    status = _git_paths("status", "--porcelain=v1")
    for line in status:
        if len(line) >= 4:
            path = line[3:]
            if " -> " in path:
                path = path.rsplit(" -> ", 1)[-1]
            paths.add(path)
    return sorted(paths)


def _infer(paths: list[str]) -> tuple[set[str], bool, bool, bool]:
    packages: set[str] = set()
    workspace_change = False
    python_change = False
    fuzz_change = False
    for path in paths:
        parts = Path(path).parts
        # Python bytecode is an ignored build artefact, not a source change.
        # Do not let an untracked interpreter cache expand an otherwise small
        # automatic fast-check invocation into the complete Python tier.
        if "__pycache__" in parts or Path(path).suffix == ".pyc":
            continue
        if path == ".github/workflows/ci.yml":
            python_change = True
        elif path in {
            "implementation/mesh-milk/Cargo.toml",
            "implementation/mesh-milk/Cargo.lock",
            "implementation/mesh-milk/rust-toolchain.toml",
        }:
            workspace_change = True
        elif len(parts) >= 4 and parts[:3] == ("implementation", "mesh-milk", "crates"):
            packages.add(parts[3])
        elif len(parts) >= 4 and parts[:3] == (
            "implementation",
            "mesh-milk",
            "vectors",
        ):
            # Golden vectors are exercised by mmp-cli's independent reader and
            # generator tests.  Keep edits to committed vector bytes in the
            # mandatory local conformance tier instead of treating them as
            # documentation-only changes.
            packages.add("mmp-cli")
        elif len(parts) >= 3 and parts[:2] == ("implementation", "mesh-milk"):
            if parts[2] == "tools" and len(parts) >= 4 and parts[3] == "layer-check":
                packages.add("layer-check")
            elif parts[2] == "scripts":
                python_change = True
            elif parts[2] == "fuzz":
                fuzz_change = True
    return packages, workspace_change, python_change, fuzz_change


def _rust_files_for_scope(paths: list[str], packages: set[str]) -> list[str]:
    """Return changed Rust files under the selected workspace crates.

    Git reports repository-relative paths.  Rustfmt runs from ``WORKSPACE``,
    so the returned paths intentionally use the workspace-relative
    ``crates/<package>/...`` form rather than absolute paths.
    """

    selected: set[str] = set()
    for raw_path in paths:
        path = Path(raw_path)
        parts = path.parts
        if (
            len(parts) < 5
            or parts[:3] != ("implementation", "mesh-milk", "crates")
            or parts[3] not in packages
            or path.suffix != ".rs"
        ):
            continue
        selected.add(Path(*parts[2:]).as_posix())
    return sorted(selected)


def _format_command(
    packages: set[str], changed_paths: list[str], *, all_mode: bool
) -> list[str] | None:
    """Build the formatter command for the selected fast-check scope."""

    if all_mode:
        return ["cargo", "fmt", "--all", "--", "--check"]
    rust_files = _rust_files_for_scope(changed_paths, packages)
    if not rust_files:
        return None
    return ["rustfmt", "--check", "--edition", "2021", *rust_files]


def _test_command(packages: set[str], *, all_mode: bool) -> list[str]:
    """Build the Cargo test command for a full or package-scoped check.

    Explicit package scopes are the developer fast tier and use a bounded
    number of libtest workers.  ``--all`` is the reproducible workspace/CI
    matrix and intentionally keeps the serial test setting.
    """

    if all_mode:
        return [
            "cargo",
            "test",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "--test-threads=1",
        ]

    package_args = sorted(packages)
    test_threads = PACKAGE_TEST_THREADS
    return [
        "cargo",
        "test",
        "--locked",
        *sum((["-p", package] for package in package_args), []),
        "--all-targets",
        "--all-features",
        "--",
        f"--test-threads={test_threads}",
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", action="append", dest="packages", default=[])
    parser.add_argument("--python", action="store_true", help="run the complete Python script-test tier")
    parser.add_argument("--all", action="store_true", help="deliberately run the full Rust workspace tier")
    args = parser.parse_args(argv)

    changed_paths = _changed_paths()
    inferred, workspace_change, python_change, fuzz_change = _infer(changed_paths)
    # Explicit package selection is the escape hatch for a deliberately small
    # local loop; automatic mode checks every changed package instead.
    packages = set(args.packages) if args.packages else inferred
    if args.packages and args.python:
        parser.error("--python and --package are separate fast-check scopes")
    if args.packages:
        # An explicit Rust scope must not expand because another unrelated
        # file is dirty in the working tree.
        python_change = False
        fuzz_change = False
        workspace_change = False
    elif args.python:
        # --python is a script-only gate, even in a mixed worktree.
        packages.clear()
        workspace_change = False
        fuzz_change = False
    if args.all:
        packages.clear()
        workspace_change = True

    if workspace_change and not args.all and not args.packages:
        print(
            "fast-check: manifest/toolchain changes require an explicit "
            "--package selection or --all; refusing an accidental workspace run",
            file=sys.stderr,
        )
        return 2

    rust_required = bool(packages) or args.all
    if rust_required:
        format_command = _format_command(packages, changed_paths, all_mode=args.all)
        format_failed = bool(format_command and _run(format_command, cwd=WORKSPACE))
        if args.all:
            test_command = _test_command(set(), all_mode=True)
            clippy_command = [
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ]
        else:
            package_args = sorted(packages)
            test_command = _test_command(packages, all_mode=False)
            clippy_command = [
                "cargo",
                "clippy",
                "--locked",
                *sum((["-p", package] for package in package_args), []),
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]
        if _run(test_command, cwd=WORKSPACE):
            return 1
        if _run(clippy_command, cwd=WORKSPACE):
            return 1
        if format_failed:
            print(
                "fast-check: formatting failed; focused tests and Clippy passed, "
                "but the repository baseline still needs rustfmt cleanup",
                file=sys.stderr,
            )
            return 1

    if args.python or python_change:
        if _run(
            [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_*.py"],
            cwd=WORKSPACE,
        ):
            return 1

    if fuzz_change:
        if _run([sys.executable, "fuzz/seed_corpus.py", "check"], cwd=WORKSPACE):
            return 1

    if not (rust_required or args.python or python_change or fuzz_change):
        print("fast-check: no Rust, Python, or fuzz changes detected; use --package or --python")
        return 2

    print("fast-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
