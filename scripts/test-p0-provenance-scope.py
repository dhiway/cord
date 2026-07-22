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

"""Hostile local-fixture tests for the P0 repository-scope guard."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PINNED = "cc190ea83c590b6a14a6b9771ab02c81618dc118"
EMPTY = hashlib.sha256(b"").hexdigest()
FIELDS = [
    "family", "repository", "commit", "literal_raw_path", "class", "tree_hash",
    "scan_hash", "spdx", "license_path", "license_sha256", "security_disposition",
    "legal_disposition", "sdk_compatibility_disposition", "maintenance_owner", "phase",
    "zero_code_graph_proof", "cord_destination_path", "external_repo_identity",
    "external_head", "external_status_sha256", "external_modified",
]


def run(*command: str, cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, text=True, capture_output=True, check=check)


def git(repo: Path, *args: str) -> str:
    return run("git", "-C", str(repo), *args).stdout.strip()


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def entry(repo: Path, workspace: Path, locator: str) -> dict:
    status = subprocess.check_output(
        ["git", "-C", str(repo), "status", "--porcelain=v1", "-z", "--untracked-files=all"]
    )
    remote = run("git", "-C", str(repo), "remote", "get-url", "origin", check=False)
    return {
        "locator": locator,
        "repository": remote.stdout.strip() if remote.returncode == 0 else "no-origin-remote",
        "head": git(repo, "rev-parse", "HEAD"),
        "tree_hash": git(repo, "rev-parse", "HEAD^{tree}"),
        "status_sha256": hashlib.sha256(status).hexdigest(),
        "modified": bool(status),
    }


def write_fixture(workspace: Path, source_workspace: Path) -> tuple[Path, Path, Path]:
    (workspace / "raw").mkdir(parents=True)
    cache = workspace / ".omx/cache/reference"
    cache.mkdir(parents=True)
    run("git", "init", "-q", str(cache))
    run("git", "-C", str(cache), "config", "user.email", "fixture@example.invalid")
    run("git", "-C", str(cache), "config", "user.name", "fixture")
    (cache / "README.md").write_text("fixture\n")
    run("git", "-C", str(cache), "add", "README.md")
    run("git", "-C", str(cache), "commit", "-q", "-m", "fixture")
    sdk_source = source_workspace / "sdk"
    run("git", "clone", "-q", "--shared", "--no-checkout", str(sdk_source), str(workspace / "sdk"))
    sdk = workspace / "sdk"
    run("git", "-C", str(sdk), "checkout", "-q", "--detach", PINNED)
    cache_entry = entry(cache, workspace, "workspace:.omx/cache/reference")
    sdk_entry = entry(sdk, workspace, "workspace:sdk")
    report = {
        "schema_version": 1,
        "status": "pass",
        "cord_repository": {"path": ".", "head": "fixture", "branch": "fixture"},
        "workspace_input_identity": workspace.name,
        "external_repository_count": 2,
        "external_modified": 0,
        "reference_only_destination": "not-applicable-reference-only",
        "cord_destinations": ["Cargo.lock", "Cargo.toml"],
        "repositories": sorted([cache_entry, sdk_entry], key=lambda item: item["locator"]),
        "status_command": "git status --porcelain=v1 -z --untracked-files=all",
        "empty_status_sha256": EMPTY,
    }
    report_path = workspace / "report.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    rows = [{
        "family": "fixture-reference", "repository": "fixture", "commit": cache_entry["head"],
        "literal_raw_path": "raw/fixture.md", "class": "reference-only",
        "tree_hash": cache_entry["tree_hash"], "scan_hash": "0" * 64, "spdx": "not-applicable",
        "license_path": "not-present", "license_sha256": "not-present",
        "security_disposition": "fixture", "legal_disposition": "fixture",
        "sdk_compatibility_disposition": "fixture", "maintenance_owner": "fixture", "phase": "P0",
        "zero_code_graph_proof": "no source code enters CORD dependency/build graph",
        "cord_destination_path": "not-applicable-reference-only",
        "external_repo_identity": cache_entry["repository"], "external_head": cache_entry["head"],
        "external_status_sha256": EMPTY, "external_modified": "0",
    }]
    for relative in ("Cargo.toml", "Cargo.lock"):
        rows.append({
            "family": "dhiway-sdk-current-dependency", "repository": "https://github.com/dhiway/sdk.git",
            "commit": PINNED, "literal_raw_path": relative, "class": "unchanged-upstream",
            "tree_hash": git(sdk, "rev-parse", f"{PINNED}^{{tree}}"), "scan_hash": sha(ROOT / relative),
            "spdx": "Apache-2.0", "license_path": "substrate/LICENSE-APACHE2",
            "license_sha256": "0" * 64, "security_disposition": "fixture",
            "legal_disposition": "fixture", "sdk_compatibility_disposition": "fixture",
            "maintenance_owner": "fixture", "phase": "P0",
            "zero_code_graph_proof": "not-applicable-code-adoption", "cord_destination_path": relative,
            "external_repo_identity": "https://github.com/dhiway/sdk.git", "external_head": PINNED,
            "external_status_sha256": EMPTY, "external_modified": "0",
        })
    ledger_path = workspace / "ledger.csv"
    with ledger_path.open("w", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=FIELDS)
        writer.writeheader()
        writer.writerows(rows)
    return ledger_path, report_path, cache


def validate(workspace: Path, ledger: Path, report: Path) -> subprocess.CompletedProcess[str]:
    return run(
        sys.executable, str(ROOT / "scripts/validate-p0-provenance.py"),
        "--workspace-root", str(workspace), "--scope-only",
        "--ledger", str(ledger), "--repository-report", str(report), check=False,
    )


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source-workspace", type=Path, default=ROOT.parent,
        help="workspace containing a local sdk checkout with the pinned commit",
    )
    args = parser.parse_args()
    source_workspace = args.source_workspace.expanduser().resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="p0-provenance-fixture-") as temporary:
        workspace = Path(temporary)
        ledger, report, cache = write_fixture(workspace, source_workspace)
        baseline = validate(workspace, ledger, report)
        require(baseline.returncode == 0, baseline.stdout + baseline.stderr)

        original_ledger = ledger.read_text()
        rows = list(csv.DictReader(original_ledger.splitlines()))
        rows[1]["cord_destination_path"] = "../outside"
        with ledger.open("w", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=FIELDS)
            writer.writeheader(); writer.writerows(rows)
        escaped = validate(workspace, ledger, report)
        require(escaped.returncode == 1 and "unsafe CORD destination" in escaped.stdout, escaped.stdout)
        ledger.write_text(original_ledger)

        original_report = report.read_text()
        value = json.loads(original_report)
        value["repositories"][0]["head"] = "0" * 40
        report.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        head_tamper = validate(workspace, ledger, report)
        require(head_tamper.returncode == 1 and "inventory drift" in head_tamper.stdout, head_tamper.stdout)
        report.write_text(original_report)

        value = json.loads(original_report)
        value["repositories"][0]["status_sha256"] = "f" * 64
        report.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        status_tamper = validate(workspace, ledger, report)
        require(status_tamper.returncode == 1 and "inventory drift" in status_tamper.stdout, status_tamper.stdout)
        report.write_text(original_report)

        protected = [
            ROOT / "docs/evidence/source-ledger.csv",
            *sorted((ROOT / "docs/evidence/p0-contract-native").glob("*.json")),
        ]
        before = {path: sha(path) for path in protected}
        (cache / "UNTRACKED").write_text("hostile\n")
        dirty = validate(workspace, ledger, report)
        require(dirty.returncode == 1 and "external checkout modified" in dirty.stdout, dirty.stdout)
        generation = run(
            sys.executable, str(ROOT / "scripts/generate-p0-provenance.py"),
            "--workspace-root", str(workspace), check=False,
        )
        require(generation.returncode != 0, "dirty generator unexpectedly passed")
        require("external repositories are modified" in generation.stdout + generation.stderr,
                generation.stdout + generation.stderr)
        after = {path: sha(path) for path in protected}
        require(before == after, "dirty preflight changed P0 evidence")

    print("PASS: baseline plus destination, head, status and untracked/atomic hostile cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
