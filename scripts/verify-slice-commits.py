#!/usr/bin/env python3
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Verify the accepted CORD slice commit identities and declared paths."""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import re
import subprocess
import sys
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

SHA = re.compile(r"^[0-9a-f]{40}$")


def command(root: Path, *argv: str) -> str:
    return subprocess.check_output(argv, cwd=root, text=True).strip()


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as target:
            json.dump(value, target, indent=2, sort_keys=True)
            target.write("\n")
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def commit_paths(root: Path, sha: str) -> list[str]:
    raw = subprocess.check_output(
        ["git", "diff-tree", "--no-commit-id", "--name-only", "-r", sha], cwd=root
    )
    return [row.decode("utf-8", "strict") for row in raw.splitlines()]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--slices", required=True, type=Path)
    parser.add_argument("--author", required=True)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    root = Path(command(Path.cwd(), "git", "rev-parse", "--show-toplevel"))
    contract = tomllib.loads((root / args.slices).read_text(encoding="utf-8"))
    failures: list[str] = []
    enabled_slices = [entry for entry in contract.get("slice", []) if entry.get("enabled", False)]
    allowed_globs = [glob for entry in enabled_slices for glob in entry.get("path_globs", [])]
    accepted: list[dict[str, Any]] = []
    seen: set[str] = set()

    for slice_entry in enabled_slices:
        identifier = slice_entry.get("id")
        globs = slice_entry.get("path_globs")
        commits = slice_entry.get("commit_shas")
        if not isinstance(identifier, str) or not isinstance(globs, list) or not isinstance(commits, list):
            failures.append("enabled slice has invalid identity, paths, or commit list")
            continue
        if not commits:
            failures.append(f"enabled slice {identifier} has no immutable accepted commit")
            continue
        for sha in commits:
            if not isinstance(sha, str) or not SHA.fullmatch(sha):
                failures.append(f"{identifier} has non-immutable commit SHA: {sha!r}")
                continue
            if sha in seen:
                failures.append(f"accepted commit {sha} is assigned more than once")
                continue
            seen.add(sha)
            try:
                identity = command(root, "git", "show", "-s", "--format=%an <%ae>%n%cn <%ce>", sha).splitlines()
                ancestor = subprocess.run(
                    ["git", "merge-base", "--is-ancestor", sha, "HEAD"], cwd=root, check=False
                ).returncode == 0
                paths = commit_paths(root, sha)
            except subprocess.CalledProcessError:
                failures.append(f"{identifier} references unavailable commit {sha}")
                continue
            if len(identity) != 2 or identity[0] != args.author or identity[1] != args.author:
                failures.append(f"{identifier} commit {sha} author/committer is not {args.author}")
            if not ancestor:
                failures.append(f"{identifier} commit {sha} is not reachable from HEAD")
            # A commit can span adjacent accepted slices (notably the initial
            # contract freeze).  It is therefore constrained by the frozen
            # *union* of enabled CORD write paths, exactly as AC12 is.
            undeclared = [
                path for path in paths if not any(fnmatch.fnmatchcase(path, glob) for glob in allowed_globs)
            ]
            if undeclared:
                failures.append(f"{identifier} commit {sha} changes undeclared paths: {undeclared}")
            accepted.append({"slice": identifier, "sha": sha, "paths": paths, "reachable": ancestor})

    report: dict[str, Any] = {
        "schema_version": 1,
        "header_failures": 0,
        "identity_failures": len([failure for failure in failures if "author/committer" in failure]),
        "recorded_gate_failures": len(failures),
        "undeclared_paths": sum(1 for failure in failures if "undeclared paths" in failure),
        "accepted_commit_count": len(accepted),
        "accepted_commits": accepted,
        "failures": failures,
    }
    output = args.out if args.out.is_absolute() else root / args.out
    atomic_json(output, report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
