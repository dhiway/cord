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

"""Compare repository snapshots without normalizing or cleaning any checkout."""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json, canonical_bytes, sha256_bytes


VOLATILE_SNAPSHOT_KEYS = {"captured_at", "snapshot_sha256", "manifest_path", "manifest_sha256"}
CORD_CHANGE_KEYS = {
	# CORD may be executed from an isolated detached worktree while the immutable
	# baseline is captured from the protected branch checkout. These describe
	# checkout topology, not CORD content; content remains bound below by the
	# changed-file contract and declared commit set.
	"path",
	"branch",
	"detached",
    "head",
    "head_tree",
    "index_sha256",
    "cached_diff_sha256",
    "worktree_diff_sha256",
    "raw_status_sha256",
    "raw_status_utf8",
    "tracked_count",
    "tracked_tree_root",
    "tracked_files",
    "untracked_count",
    "untracked_tree_root",
    "untracked_files",
    "submodules",
}


def load_snapshot(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    expected = value.get("snapshot_sha256")
    unsigned = dict(value)
    unsigned.pop("snapshot_sha256", None)
    actual = sha256_bytes(canonical_bytes(unsigned))
    if expected != actual:
        raise ValueError(f"snapshot hash mismatch: {path}")
    return value


def repository_map(snapshot: dict[str, Any]) -> dict[str, dict[str, Any]]:
    result = {row["name"]: row for row in snapshot.get("repositories", [])}
    if len(result) != len(snapshot.get("repositories", [])):
        raise ValueError("duplicate repository name in snapshot")
    return result


def record_map(repository: dict[str, Any], field: str) -> dict[str, dict[str, Any]]:
    return {record["path"]: record for record in repository.get(field, [])}


def changed_paths(before: dict[str, Any], after: dict[str, Any]) -> set[str]:
    paths: set[str] = set()
    for field in ("tracked_files", "untracked_files"):
        old = record_map(before, field)
        new = record_map(after, field)
        for path in old.keys() | new.keys():
            if old.get(path) != new.get(path):
                paths.add(path)
    old_modules = {row["path"]: row for row in before.get("submodules", [])}
    new_modules = {row["path"]: row for row in after.get("submodules", [])}
    for path in old_modules.keys() | new_modules.keys():
        if old_modules.get(path) != new_modules.get(path):
            paths.add(path)
    return paths


def allowed_contract(path: Path) -> tuple[list[str], set[str]]:
    contract = tomllib.loads(path.read_text(encoding="utf-8"))
    globs: list[str] = []
    commits: set[str] = set()
    for slice_entry in contract.get("slice", []):
        if slice_entry.get("enabled", False):
            globs.extend(slice_entry.get("path_globs", []))
            commits.update(slice_entry.get("commit_shas", []))
    return globs, commits


def validate_cord_history(
    baseline: dict[str, Any], current: dict[str, Any], allowed_globs: list[str], allowed_commits: set[str]
) -> list[dict[str, Any]]:
    """Require a declared, Satish-authored, path-scoped descendant history."""
    old_head, new_head = baseline.get("head"), current.get("head")
    checkout = current.get("path")
    if not all(isinstance(value, str) and value for value in (old_head, new_head, checkout)):
        return [{"kind": "cord-history-unavailable"}]
    ancestor = subprocess.run(["git", "-C", checkout, "merge-base", "--is-ancestor", old_head, new_head], check=False)
    if ancestor.returncode != 0:
        return [{"kind": "cord-history-non-ancestral", "before": old_head, "after": new_head}]
    log = subprocess.run(
        ["git", "-C", checkout, "log", "--format=%H%x00%an%x00%ae", f"{old_head}..{new_head}"],
        check=True, stdout=subprocess.PIPE, text=True,
    ).stdout.splitlines()
    violations: list[dict[str, Any]] = []
    commits = []
    for line in log:
        commit, author, email = line.split("\0")
        commits.append(commit)
        if (author, email) != ("Satish Mohan", "satish@dhiway.com"):
            violations.append({"kind": "cord-history-author", "commit": commit, "author": author, "email": email})
        paths = subprocess.run(
            ["git", "-C", checkout, "diff-tree", "--no-commit-id", "--name-only", "-r", commit],
            check=True, stdout=subprocess.PIPE, text=True,
        ).stdout.splitlines()
        undeclared = [path for path in paths if not any(fnmatch.fnmatchcase(path, pattern) for pattern in allowed_globs)]
        if undeclared:
            violations.append({"kind": "cord-history-paths", "commit": commit, "paths": undeclared})
    if new_head != old_head and allowed_commits and not any(anchor in commits for anchor in allowed_commits):
        violations.append({"kind": "cord-history-anchor-missing", "after": new_head})
    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--allow-cord", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    baseline = load_snapshot(args.baseline)
    current = load_snapshot(args.current)
    before = repository_map(baseline)
    after = repository_map(current)
    violations: list[dict[str, Any]] = []
    if before.keys() != after.keys():
        violations.append(
            {
                "kind": "repository-set",
                "before": sorted(before),
                "after": sorted(after),
            }
        )

    allowed_globs, allowed_commits = allowed_contract(args.allow_cord)
    external_delta_count = 0
    cord_undeclared_paths: list[str] = []
    for name in sorted(before.keys() & after.keys()):
        old = before[name]
        new = after[name]
        if old.get("role") != "cord":
            if old != new:
                external_delta_count += 1
                differing = sorted(key for key in old.keys() | new.keys() if old.get(key) != new.get(key))
                violations.append({"kind": "external-delta", "repository": name, "fields": differing})
            continue

        for key in sorted((old.keys() | new.keys()) - CORD_CHANGE_KEYS):
            if old.get(key) != new.get(key):
                violations.append({"kind": "cord-invariant", "field": key})
        for changed in sorted(changed_paths(old, new), key=lambda value: value.encode("utf-8")):
            if not any(fnmatch.fnmatchcase(changed, pattern) for pattern in allowed_globs):
                cord_undeclared_paths.append(changed)
        if old.get("head") != new.get("head"):
            violations.extend(validate_cord_history(old, new, allowed_globs, allowed_commits))

    if cord_undeclared_paths:
        violations.append({"kind": "cord-undeclared-paths", "paths": cord_undeclared_paths})
    result = {
        "schema_version": 1,
        "status": "pass" if not violations else "fail",
        "baseline_snapshot_sha256": baseline["snapshot_sha256"],
        "current_snapshot_sha256": current["snapshot_sha256"],
        "baseline_hash_valid": True,
        "current_hash_valid": True,
        "external_content_deltas": external_delta_count,
        "cord_undeclared_path_count": len(cord_undeclared_paths),
        "cord_changed_paths": sorted(
            changed_paths(before.get("cord", {}), after.get("cord", {})),
            key=lambda value: value.encode("utf-8"),
        ),
        "allowed_path_globs": allowed_globs,
        "violations": violations,
    }
    atomic_write_json(args.out, result)
    print(f"{result['status'].upper()} repository boundary: {len(violations)} violation(s)")
    return 0 if not violations else 1


if __name__ == "__main__":
    raise SystemExit(main())
