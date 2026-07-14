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

"""Validate P0 provenance and the CORD-only repository-scope guard."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
EV = ROOT / "docs/evidence/p0-contract-native"
PINNED_SDK = "cc190ea83c590b6a14a6b9771ab02c81618dc118"
EMPTY_STATUS_SHA256 = hashlib.sha256(b"").hexdigest()
REFERENCE_DESTINATION = "not-applicable-reference-only"
SIBLING_REFERENCE_REPOS = (
    "sdk", "runtimes", "runtimes-paseo", "polkadot-bulletin-chain",
    "individuality-community",
)
REQUIRED_LEDGER_FIELDS = {
    "cord_destination_path", "external_repo_identity", "external_head",
    "external_status_sha256", "external_modified",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace-root", type=Path, default=ROOT.parent)
    parser.add_argument("--scope-only", action="store_true")
    parser.add_argument("--ledger", type=Path, default=ROOT / "docs/evidence/source-ledger.csv")
    parser.add_argument(
        "--repository-report", type=Path, default=EV / "repository-scope.report.json"
    )
    parser.add_argument("--structural", action="store_true")
    return parser.parse_args()


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def git_bytes(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def git(repo: Path, *args: str) -> str:
    return git_bytes(repo, *args).decode().strip()


def normalize_remote(remote: str) -> str:
    remote = remote.strip()
    match = re.fullmatch(r"git@github\.com:([^/]+)/(.+?)(?:\.git)?", remote)
    if match:
        return f"https://github.com/{match.group(1)}/{match.group(2)}.git"
    if remote.startswith("https://github.com/"):
        return remote if remote.endswith(".git") else remote + ".git"
    return remote


def resolve_workspace(path: Path, errors: list[str]) -> Path | None:
    try:
        workspace = path.expanduser().resolve(strict=True)
    except (FileNotFoundError, OSError) as error:
        errors.append(f"workspace root cannot be resolved: {error}")
        return None
    for required in (workspace / "raw", workspace / ".omx/cache"):
        if not required.is_dir():
            errors.append(f"workspace input is missing:{required}")
    if workspace == ROOT.resolve():
        errors.append("workspace root equals the CORD output repository")
    return workspace


def checkout_key(repo: Path) -> tuple[str, str]:
    top = Path(git(repo, "rev-parse", "--show-toplevel")).resolve(strict=True)
    git_dir = Path(git(repo, "rev-parse", "--git-dir"))
    if not git_dir.is_absolute():
        git_dir = top / git_dir
    return str(top), str(git_dir.resolve(strict=True))


def discover(workspace: Path, errors: list[str]) -> list[dict[str, Any]]:
    candidates = sorted(
        path for path in (workspace / ".omx/cache").iterdir()
        if path.is_dir() and (path / ".git").exists()
    )
    candidates.extend(
        workspace / name for name in SIBLING_REFERENCE_REPOS
        if (workspace / name / ".git").exists()
    )
    seen: set[tuple[str, str]] = set()
    entries = []
    for candidate in candidates:
        try:
            key = checkout_key(candidate)
            if key in seen:
                continue
            seen.add(key)
            relative = candidate.resolve().relative_to(workspace).as_posix()
            status = git_bytes(
                candidate, "status", "--porcelain=v1", "-z", "--untracked-files=all"
            )
            try:
                remote = normalize_remote(git(candidate, "remote", "get-url", "origin"))
            except subprocess.CalledProcessError:
                remote = "no-origin-remote"
            entries.append({
                "locator": "workspace:" + relative,
                "repository": remote,
                "head": git(candidate, "rev-parse", "HEAD"),
                "tree_hash": git(candidate, "rev-parse", "HEAD^{tree}"),
                "status_sha256": sha_bytes(status),
                "modified": bool(status),
                "path": candidate.resolve(),
            })
        except (subprocess.CalledProcessError, OSError, ValueError) as error:
            errors.append(f"cannot audit external checkout:{candidate}:{error}")
    if not entries:
        errors.append("no external Git checkouts discovered")
    return sorted(entries, key=lambda entry: entry["locator"])


def validate_destination(row: dict[str, str], errors: list[str]) -> None:
    family = row.get("family", "unknown")
    destination = row.get("cord_destination_path", "")
    if row.get("class") == "reference-only":
        if destination != REFERENCE_DESTINATION:
            errors.append(f"reference-only row has a CORD destination:{family}:{destination}")
        if row.get("external_modified") != "0":
            errors.append(f"reference-only external_modified is not zero:{family}")
        return
    if row.get("class") != "unchanged-upstream":
        errors.append(f"unexpected class:{family}")
        return
    relative = Path(destination)
    if not destination or relative.is_absolute() or ".." in relative.parts:
        errors.append(f"unsafe CORD destination:{family}:{destination}")
        return
    try:
        resolved = (ROOT / relative).resolve(strict=True)
        resolved.relative_to(ROOT.resolve(strict=True))
    except (FileNotFoundError, OSError, ValueError):
        errors.append(f"CORD destination escapes or is missing:{family}:{destination}")
        return
    if not resolved.is_file():
        errors.append(f"CORD destination is not a file:{family}:{destination}")
    elif sha(resolved) != row.get("scan_hash"):
        errors.append(f"CORD destination hash drift:{family}:{destination}")


def validate_scope(
    workspace: Path, rows: list[dict[str, str]], report: dict[str, Any], errors: list[str]
) -> list[dict[str, Any]]:
    entries = discover(workspace, errors)
    live = [{key: value for key, value in entry.items() if key != "path"} for entry in entries]
    if report.get("schema_version") != 1:
        errors.append("repository scope schema_version must be 1")
    if report.get("status") != "pass":
        errors.append("repository scope status is not pass")
    if report.get("external_modified") != 0:
        errors.append("repository scope external_modified is not zero")
    if report.get("external_repository_count") != len(entries):
        errors.append("repository scope external repository count drift")
    if report.get("empty_status_sha256") != EMPTY_STATUS_SHA256:
        errors.append("repository scope empty status digest drift")
    if report.get("reference_only_destination") != REFERENCE_DESTINATION:
        errors.append("repository scope reference-only destination drift")
    if report.get("cord_destinations") != ["Cargo.lock", "Cargo.toml"]:
        errors.append("repository scope CORD destinations drift")
    if report.get("repositories") != live:
        errors.append("repository scope live checkout inventory drift")
    for entry in entries:
        if entry["modified"] or entry["status_sha256"] != EMPTY_STATUS_SHA256:
            errors.append(f"external checkout modified:{entry['locator']}")
    sdk = next((entry for entry in entries if entry["locator"] == "workspace:sdk"), None)
    if sdk is None:
        errors.append("audited workspace:sdk checkout missing")
    else:
        try:
            git(sdk["path"], "cat-file", "-e", f"{PINNED_SDK}^{{commit}}")
        except subprocess.CalledProcessError:
            errors.append("pinned SDK commit is absent from workspace:sdk")
    ledger_fields = set(rows[0]) if rows else set()
    missing_fields = sorted(REQUIRED_LEDGER_FIELDS - ledger_fields)
    if missing_fields:
        errors.append(f"source ledger missing scope columns:{missing_fields}")
    for row in rows:
        validate_destination(row, errors)
        family = row.get("family", "unknown")
        if row.get("external_modified") != "0":
            errors.append(f"ledger external_modified is not zero:{family}")
        status = row.get("external_status_sha256")
        if status not in {EMPTY_STATUS_SHA256, "not-present-raw-hash-only"}:
            errors.append(f"ledger external status is not clean:{family}")
        if row.get("class") == "unchanged-upstream":
            if row.get("commit") != PINNED_SDK or row.get("external_head") != PINNED_SDK:
                errors.append("wrong current SDK commit")
            if row.get("external_repo_identity") != "https://github.com/dhiway/sdk.git":
                errors.append("wrong current SDK external identity")
            if sdk and status != sdk["status_sha256"]:
                errors.append("current SDK status digest does not match workspace:sdk")
    return entries


def main() -> int:
    args = parse_args()
    errors: list[str] = []
    workspace = resolve_workspace(args.workspace_root, errors)
    try:
        rows = list(csv.DictReader(args.ledger.open(newline="")))
    except Exception as error:
        errors.append(f"cannot read source ledger:{error}")
        rows = []
    try:
        report = json.loads(args.repository_report.read_text())
    except Exception as error:
        errors.append(f"cannot read repository scope report:{error}")
        report = {}
    entries: list[dict[str, Any]] = []
    if workspace is not None:
        entries = validate_scope(workspace, rows, report, errors)
    if args.scope_only:
        result = {
            "status": "fail" if errors else "pass",
            "source_rows": len(rows),
            "external_repositories": len(entries),
            "external_modified": sum(1 for entry in entries if entry["modified"]),
            "errors": errors,
        }
        print(json.dumps(result, sort_keys=True))
        return 1 if errors else 0

    approval_path = EV / "architect-semantic-disposition-approval.json"
    try:
        approval_hash = sha(approval_path)
    except OSError as error:
        errors.append(f"semantic approval missing:{error}")
        approval_hash = ""
    for row in rows:
        family = row.get("family", "unknown")
        if not re.fullmatch(r"[0-9a-f]{40}", row.get("commit", "")):
            errors.append(f"bad commit:{family}")
        if row.get("class") == "unchanged-upstream":
            if not re.fullmatch(r"[0-9a-f]{64}", row.get("license_sha256", "")):
                errors.append(f"bad adoption license:{family}")
        elif row.get("class") == "reference-only":
            license_hash = row.get("license_sha256", "")
            if license_hash != "not-present" and not re.fullmatch(r"[0-9a-f]{64}", license_hash):
                errors.append(f"bad reference license:{family}")
            if "no source code" not in row.get("zero_code_graph_proof", "").lower():
                errors.append(f"no zero-code proof:{family}")
    try:
        scope = json.loads((EV / "source-scope.report.json").read_text())
        if scope.get("status") not in {"pass", "blocked"} or scope.get("missing_families") or scope.get("raw_documents") != 89:
            errors.append("source family scope incomplete")
        if scope.get("snapshot_mismatches") and scope.get("status") == "pass":
            errors.append("false source-scope pass with snapshot mismatch")
    except Exception as error:
        errors.append(f"cannot validate source scope:{error}")
        scope = {"covered_families": [], "snapshot_mismatches": [], "status": "fail"}
    try:
        guide = json.loads((EV / "guidance-audit.json").read_text())
        if guide.get("status") != "pass" or guide.get("actual_count") != 59:
            errors.append("guidance count/mapping failed")
    except Exception as error:
        errors.append(f"cannot validate guidance:{error}")
        guide = {"actual_count": 0}
    try:
        index = json.loads((EV / "evidence-index.json").read_text())
        for artifact_name in (
            "guidance-audit.json", "source-scope.report.json", "reference-sbom-inputs.json"
        ):
            reference = json.loads((EV / artifact_name).read_text()).get("approval_manifest", {})
            if reference.get("sha256") != approval_hash:
                errors.append(f"semantic approval reference mismatch:{artifact_name}")
        if index.get("semantic_approval_manifest", {}).get("sha256") != approval_hash:
            errors.append("semantic approval index reference mismatch")
        required_report = "docs/evidence/p0-contract-native/repository-scope.report.json"
        if required_report not in index.get("artifacts", {}):
            errors.append("repository scope report absent from semantic index")
        for relative, expected in index.get("artifacts", {}).items():
            path = ROOT / relative
            if not path.is_file() or sha(path) != expected:
                errors.append(f"index hash mismatch:{relative}")
    except Exception as error:
        errors.append(f"cannot validate semantic evidence index:{error}")
    blocked = not errors and scope.get("status") == "blocked" and not args.structural
    result = {
        "status": "fail" if errors else "blocked" if blocked else "pass",
        "source_rows": len(rows),
        "source_families": len(scope.get("covered_families", [])),
        "guidance_files": guide.get("actual_count", 0),
        "external_repositories": len(entries),
        "external_modified": sum(1 for entry in entries if entry["modified"]),
        "snapshot_mismatches": len(scope.get("snapshot_mismatches", [])),
        "errors": errors,
    }
    print(json.dumps(result, sort_keys=True))
    return 1 if errors else 2 if blocked else 0


if __name__ == "__main__":
    raise SystemExit(main())
