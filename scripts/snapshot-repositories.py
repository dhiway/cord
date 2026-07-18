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

"""Create a content-sensitive, read-only snapshot of Git repositories."""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import stat
import subprocess
import sys
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

from evidence_common import (
    CANONICALIZATION,
    atomic_write_json,
    canonical_bytes,
    sha256_bytes,
    sha256_file,
)


def git(root: Path, *argv: str, check: bool = True) -> bytes:
    environment = dict(os.environ)
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    result = subprocess.run(
        ["git", *argv], cwd=root, check=False, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, env=environment,
    )
    if check and result.returncode != 0:
        error = result.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"git {' '.join(argv)} failed in {root}: {error}")
    return result.stdout


def decode_paths(payload: bytes) -> list[str]:
    values = payload.split(b"\0")
    return [value.decode("utf-8", "strict") for value in values if value]


def file_record(root: Path, relative: str, mode: str | None = None) -> dict[str, Any]:
    path = root / relative
    if not os.path.lexists(path):
        return {"path": relative, "mode": mode or "missing", "length": 0, "sha256": None}
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode):
        payload = os.readlink(path).encode("utf-8")
        kind = "120000"
    elif stat.S_ISREG(metadata.st_mode):
        payload = path.read_bytes()
        kind = mode or f"{stat.S_IMODE(metadata.st_mode):06o}"
    elif stat.S_ISDIR(metadata.st_mode) and mode == "160000":
        payload = git(path, "rev-parse", "HEAD").strip()
        kind = mode
    else:
        payload = b""
        kind = mode or f"{stat.S_IMODE(metadata.st_mode):06o}"
    return {
        "path": relative,
        "mode": kind,
        "length": len(payload),
        "sha256": sha256_bytes(payload),
    }


def merkle_root(records: list[dict[str, Any]]) -> str:
    digest = bytearray()
    for item in sorted(records, key=lambda row: row["path"].encode("utf-8")):
        path = item["path"].encode("utf-8")
        mode = item["mode"].encode("ascii")
        length = int(item["length"]).to_bytes(8, "big")
        content = (item["sha256"] or "").encode("ascii")
        digest.extend(len(path).to_bytes(8, "big"))
        digest.extend(path)
        digest.extend(len(mode).to_bytes(4, "big"))
        digest.extend(mode)
        digest.extend(length)
        digest.extend(content)
    return sha256_bytes(bytes(digest))


def tracked_files(root: Path) -> tuple[list[dict[str, Any]], str]:
    stage_payload = git(root, "ls-files", "--stage", "-z")
    modes: dict[str, str] = {}
    index_entries: list[dict[str, str]] = []
    for raw in stage_payload.split(b"\0"):
        if not raw:
            continue
        prefix, encoded_path = raw.split(b"\t", 1)
        mode, object_id, stage = prefix.decode("ascii").split(" ")
        relative = encoded_path.decode("utf-8", "strict")
        index_entries.append(
            {"path": relative, "mode": mode, "object_id": object_id, "stage": stage}
        )
        if stage == "0":
            modes[relative] = mode
    records = [file_record(root, path, modes.get(path)) for path in decode_paths(git(root, "ls-files", "-z"))]
    index_hash = sha256_bytes(stage_payload)
    return records, index_hash


def submodules(root: Path) -> list[dict[str, Any]]:
    output = git(root, "submodule", "status", "--recursive", check=False)
    records = []
    for line in output.decode("utf-8", "strict").splitlines():
        if not line:
            continue
        marker = line[0]
        fields = line[1:].split(" ", 2)
        records.append(
            {
                "path": fields[1] if len(fields) > 1 else "",
                "sha": fields[0],
                "status": marker,
                "description": fields[2] if len(fields) > 2 else "",
            }
        )
    return sorted(records, key=lambda row: row["path"].encode("utf-8"))


def snapshot_repo(entry: dict[str, Any]) -> dict[str, Any]:
    root = Path(entry["path"])
    if not root.is_absolute():
        if root != Path("."):
            raise ValueError(f"only the current repository may use a relative path: {root}")
        root = Path.cwd().resolve()
    if not root.exists():
        raise FileNotFoundError(root)
    actual_root = Path(git(root, "rev-parse", "--show-toplevel").decode().strip()).resolve()
    if actual_root != root.resolve():
        raise ValueError(f"manifest root {root} resolves to Git root {actual_root}")

    cache_globs = list(entry.get("cache_artifact_globs", []))
    untracked = []
    for relative in decode_paths(git(root, "ls-files", "--others", "--exclude-standard", "-z")):
        record = file_record(root, relative)
        record["cache_artifact"] = any(fnmatch.fnmatchcase(relative, glob) for glob in cache_globs)
        untracked.append(record)

    tracked, index_hash = tracked_files(root)
    cached_diff = git(root, "diff", "--binary", "--cached", "--no-ext-diff")
    worktree_diff = git(root, "diff", "--binary", "--no-ext-diff")
    branch_result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "HEAD"],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    )
    head = git(root, "rev-parse", "HEAD").decode("ascii").strip()
    remotes = {}
    for remote in git(root, "remote").decode("utf-8").splitlines():
        remotes[remote] = git(
            root, "remote", "get-url", "--all", remote, check=False
        ).decode("utf-8", "strict").splitlines()
    raw_status = git(
        root,
        "status",
        "--porcelain=v2",
        "--branch",
        "-z",
        "--untracked-files=all",
    )
    return {
        "name": entry["name"],
        "role": entry["role"],
        "path": str(root.resolve()),
        "remote_urls": remotes,
        "branch": branch_result.stdout.decode("utf-8").strip() if branch_result.returncode == 0 else None,
        "detached": branch_result.returncode != 0,
        "head": head,
        "head_tree": git(root, "rev-parse", f"{head}^{{tree}}").decode("ascii").strip(),
        "index_tree": index_hash,
        "index_tree_algorithm": "sha256(git ls-files --stage -z)",
        "index_sha256": index_hash,
        "cached_diff_sha256": sha256_bytes(cached_diff),
        "worktree_diff_sha256": sha256_bytes(worktree_diff),
        "raw_status_sha256": sha256_bytes(raw_status),
        "raw_status_utf8": raw_status.decode("utf-8", "backslashreplace"),
        "tracked_count": len(tracked),
        "tracked_tree_root": merkle_root(tracked),
        "tracked_files": tracked,
        "untracked_count": len(untracked),
        "untracked_tree_root": merkle_root(untracked),
        "untracked_files": sorted(untracked, key=lambda row: row["path"].encode("utf-8")),
        "submodules": submodules(root),
        "cache_artifact_globs": cache_globs,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    manifest = tomllib.loads(args.manifest.read_text(encoding="utf-8"))
    if args.out.name == "repos-before.json" and args.out.exists():
        existing = json.loads(args.out.read_text(encoding="utf-8"))
        unsigned = dict(existing)
        expected = unsigned.pop("snapshot_sha256", None)
        if expected != sha256_bytes(canonical_bytes(unsigned)):
            raise SystemExit("existing immutable baseline has an invalid snapshot hash")
        if existing.get("manifest_sha256") != sha256_file(args.manifest):
            raise SystemExit("existing immutable baseline uses a different manifest")
        print(f"reuse immutable repository baseline -> {args.out}")
        return 0
    repositories = manifest.get("repository", [])
    names = [entry.get("name") for entry in repositories]
    paths = [entry.get("path") for entry in repositories]
    if not repositories or len(names) != len(set(names)) or len(paths) != len(set(paths)):
        raise SystemExit("manifest must contain repositories with unique names and paths")
    report = {
        "schema_version": 1,
        "canonicalization": CANONICALIZATION,
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "manifest_path": str(args.manifest),
        "manifest_sha256": sha256_file(args.manifest),
        "repositories": [snapshot_repo(entry) for entry in repositories],
    }
    report["snapshot_sha256"] = sha256_bytes(canonical_bytes(report))
    atomic_write_json(args.out, report)
    print(f"snapshot {len(repositories)} repositories -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
