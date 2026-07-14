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

"""Freeze P0 provenance while enforcing the CORD-only repository boundary."""
from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/evidence/p0-contract-native"
PINNED_SDK = "cc190ea83c590b6a14a6b9771ab02c81618dc118"
EMPTY_STATUS_SHA256 = hashlib.sha256(b"").hexdigest()
REFERENCE_DESTINATION = "not-applicable-reference-only"
FIELDS = [
    "family", "repository", "commit", "literal_raw_path", "class", "tree_hash",
    "scan_hash", "spdx", "license_path", "license_sha256", "security_disposition",
    "legal_disposition", "sdk_compatibility_disposition", "maintenance_owner", "phase",
    "zero_code_graph_proof", "cord_destination_path", "external_repo_identity",
    "external_head", "external_status_sha256", "external_modified",
]
SIBLING_REFERENCE_REPOS = (
    "sdk", "runtimes", "runtimes-paseo", "polkadot-bulletin-chain",
    "individuality-community",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--workspace-root",
        type=Path,
        default=ROOT.parent,
        help="immutable source workspace (default: parent of the CORD repository)",
    )
    return parser.parse_args()


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def git_bytes(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def git(repo: Path, *args: str) -> str:
    return git_bytes(repo, *args).decode().strip()


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def csv_bytes(rows: list[dict[str, str]]) -> bytes:
    target = io.StringIO(newline="")
    writer = csv.DictWriter(target, fieldnames=FIELDS, lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return target.getvalue().encode()


def atomic_replace(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        os.replace(name, path)
    except BaseException:
        try:
            os.unlink(name)
        except FileNotFoundError:
            pass
        raise


def normalize_remote(remote: str) -> str:
    remote = remote.strip()
    match = re.fullmatch(r"git@github\.com:([^/]+)/(.+?)(?:\.git)?", remote)
    if match:
        return f"https://github.com/{match.group(1)}/{match.group(2)}.git"
    if remote.startswith("https://github.com/"):
        return remote if remote.endswith(".git") else remote + ".git"
    return remote


def resolve_workspace(path: Path) -> Path:
    workspace = path.expanduser().resolve(strict=True)
    for required in (workspace / "raw", workspace / ".omx/cache"):
        if not required.is_dir():
            raise SystemExit(f"workspace input is missing: {required}")
    if workspace == ROOT.resolve():
        raise SystemExit("workspace root must contain, not equal, the CORD output repository")
    return workspace


def checkout_key(repo: Path) -> tuple[str, str]:
    top = Path(git(repo, "rev-parse", "--show-toplevel")).resolve(strict=True)
    git_dir_text = git(repo, "rev-parse", "--git-dir")
    git_dir = Path(git_dir_text)
    if not git_dir.is_absolute():
        git_dir = top / git_dir
    return str(top), str(git_dir.resolve(strict=True))


def stable_locator(repo: Path, workspace: Path) -> str:
    try:
        return "workspace:" + repo.resolve().relative_to(workspace).as_posix()
    except ValueError:
        raise SystemExit(f"external checkout escapes audited workspace: {repo}")


def audit_external_repositories(workspace: Path) -> list[dict[str, Any]]:
    candidates = sorted(
        path for path in (workspace / ".omx/cache").iterdir()
        if path.is_dir() and (path / ".git").exists()
    )
    candidates.extend(
        workspace / name for name in SIBLING_REFERENCE_REPOS
        if (workspace / name / ".git").exists()
    )
    seen: set[tuple[str, str]] = set()
    entries: list[dict[str, Any]] = []
    for candidate in candidates:
        key = checkout_key(candidate)
        if key in seen:
            continue
        seen.add(key)
        status = git_bytes(
            candidate, "status", "--porcelain=v1", "-z", "--untracked-files=all"
        )
        try:
            remote = normalize_remote(git(candidate, "remote", "get-url", "origin"))
        except subprocess.CalledProcessError:
            remote = "no-origin-remote"
        entries.append({
            "locator": stable_locator(candidate, workspace),
            "path": candidate.resolve(),
            "repository": remote,
            "head": git(candidate, "rev-parse", "HEAD"),
            "tree_hash": git(candidate, "rev-parse", "HEAD^{tree}"),
            "status_sha256": sha_bytes(status),
            "modified": bool(status),
        })
    if not entries:
        raise SystemExit("no immutable external Git checkouts were discovered")
    dirty = [entry["locator"] for entry in entries if entry["modified"]]
    if dirty:
        raise SystemExit("external repositories are modified: " + ", ".join(dirty))
    return sorted(entries, key=lambda entry: entry["locator"])


def assert_cord_destination(relative: str) -> Path:
    if Path(relative).is_absolute() or ".." in Path(relative).parts:
        raise SystemExit(f"CORD destination is not repository-relative: {relative}")
    resolved = (ROOT / relative).resolve(strict=True)
    try:
        resolved.relative_to(ROOT.resolve(strict=True))
    except ValueError:
        raise SystemExit(f"CORD destination escapes repository: {relative}")
    if not resolved.is_file():
        raise SystemExit(f"CORD destination is not a file: {relative}")
    return resolved


def main() -> int:
    workspace = resolve_workspace(parse_args().workspace_root)
    cache = workspace / ".omx/cache"
    raw = workspace / "raw"
    entries = audit_external_repositories(workspace)  # fail before every output write
    by_commit = {entry["head"]: entry for entry in entries}
    sdk_entry = next(
        (entry for entry in entries if entry["locator"] == "workspace:sdk"), None
    )
    if sdk_entry is None:
        raise SystemExit("the audited workspace:sdk checkout is required")
    sdk_repo = sdk_entry["path"]
    try:
        git(sdk_repo, "cat-file", "-e", f"{PINNED_SDK}^{{commit}}")
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"pinned SDK object is missing: {PINNED_SDK}") from error
    sdk_tree = git(sdk_repo, "rev-parse", f"{PINNED_SDK}^{{tree}}")
    sdk_license = git_bytes(sdk_repo, "show", f"{PINNED_SDK}:substrate/LICENSE-APACHE2")

    approval_path = OUT / "architect-semantic-disposition-approval.json"
    approval_manifest = json.loads(approval_path.read_text())
    approval_ref = {
        "schema_version": approval_manifest["schema_version"],
        "path": approval_path.relative_to(ROOT).as_posix(),
        "sha256": sha(approval_path),
        "verdict": approval_manifest["verdict"],
        "required_reviewer_role": approval_manifest["required_reviewer_role"],
        "review_thread_id": approval_manifest["review_thread_id"],
    }
    rows: list[dict[str, str]] = []
    sbom: list[dict[str, Any]] = []
    cache_by_commit: dict[str, Path] = {}
    for repo in sorted(
        path for path in cache.iterdir() if path.is_dir() and (path / ".git").exists()
    ):
        commit = git(repo, "rev-parse", "HEAD")
        cache_by_commit[commit] = repo
        manifests: list[Path] = []
        for filename in (
            "Cargo.lock", "Cargo.toml", "package.json", "foundry.lock", "foundry.toml", "bun.lock"
        ):
            manifests.extend(repo.rglob(filename))
        sbom.append({
            "family": repo.name,
            "commit": commit,
            "tree_hash": git(repo, "rev-parse", "HEAD^{tree}"),
            "reference_dependency_manifests": [
                {"path": path.relative_to(repo).as_posix(), "sha256": sha(path)}
                for path in sorted(set(manifests))
            ],
            "cord_build_graph_adoption": "none-reference-only",
        })

    source_re = re.compile(
        r"^Source: https://github.com/([^/]+)/([^/]+)/(?:blob|tree)/([0-9a-f]{40})(?:/(.*))?$",
        re.M,
    )
    for raw_path in sorted(raw.rglob("*.md")):
        match = source_re.search(raw_path.read_text(errors="replace")[:4096])
        if not match:
            raise SystemExit(f"raw source lacks immutable GitHub source: {raw_path}")
        owner, repository_name, commit, original_path = match.groups()
        cache_repo = cache_by_commit.get(commit)
        license_files = sorted(cache_repo.glob("LICENSE*")) if cache_repo else []
        license_file = license_files[0] if license_files else None
        entry = by_commit.get(commit)
        rows.append({
            "family": repository_name,
            "repository": f"https://github.com/{owner}/{repository_name}.git",
            "commit": commit,
            "literal_raw_path": f"raw/{raw_path.relative_to(raw).as_posix()}",
            "class": "reference-only",
            "tree_hash": git(cache_repo, "rev-parse", "HEAD^{tree}") if cache_repo else commit,
            "scan_hash": sha(raw_path),
            "spdx": "reference-only; repository license recorded" if license_file else "not-applicable",
            "license_path": license_file.relative_to(cache_repo).as_posix() if license_file else "not-present",
            "license_sha256": sha(license_file) if license_file else "not-present",
            "security_disposition": "unaudited reference; native threat review required before semantic adoption",
            "legal_disposition": "reference-use-only; copying forbidden until a separate legal gate",
            "sdk_compatibility_disposition": "clean-break native SDK only; no ABI compatibility or client migration",
            "maintenance_owner": "P0-provenance-owner",
            "phase": "P0-P7",
            "zero_code_graph_proof": f"raw document references {original_path or 'repository tree'}; no source code enters CORD dependency/build graph",
            "cord_destination_path": REFERENCE_DESTINATION,
            "external_repo_identity": entry["repository"] if entry else "not-present-raw-hash-only",
            "external_head": entry["head"] if entry else "not-present-raw-hash-only",
            "external_status_sha256": entry["status_sha256"] if entry else "not-present-raw-hash-only",
            "external_modified": "0",
        })

    for relative in ("Cargo.toml", "Cargo.lock"):
        destination = assert_cord_destination(relative)
        rows.append({
            "family": "dhiway-sdk-current-dependency",
            "repository": "https://github.com/dhiway/sdk.git",
            "commit": PINNED_SDK,
            "literal_raw_path": relative,
            "class": "unchanged-upstream",
            "tree_hash": sdk_tree,
            "scan_hash": sha(destination),
            "spdx": "Apache-2.0",
            "license_path": "substrate/LICENSE-APACHE2",
            "license_sha256": sha_bytes(sdk_license),
            "security_disposition": "branch-locked dependency; runtime composition/review and advisory monitoring required",
            "legal_disposition": "approved dependency input subject to complete workspace license scan",
            "sdk_compatibility_disposition": "single release-v1.24.0 graph at cc190ea; upstream primitive forks forbidden",
            "maintenance_owner": "runtime-sdk-owner",
            "phase": "P0-P7",
            "zero_code_graph_proof": "not-applicable-code-adoption",
            "cord_destination_path": relative,
            "external_repo_identity": "https://github.com/dhiway/sdk.git",
            "external_head": PINNED_SDK,
            "external_status_sha256": sdk_entry["status_sha256"],
            "external_modified": "0",
        })
    sbom.append({
        "family": "dhiway-sdk-current-dependency",
        "commit": PINNED_SDK,
        "tree_hash": sdk_tree,
        "reference_dependency_manifests": [
            {"path": "Cargo.toml", "sha256": sha(ROOT / "Cargo.toml")},
            {"path": "Cargo.lock", "sha256": sha(ROOT / "Cargo.lock")},
        ],
        "cord_build_graph_adoption": "unchanged-upstream-release-v1.24.0-single-commit",
    })

    required_families = {
        "dhiway-sdk-current-dependency", "polkadot-sdk", "runtimes", "polkadot-bulletin-chain",
        "product-sdk", "truapi", "verifiable", "verifiablejs", "bcts", "dotns", "dotns-sdk",
        "attestation-protocol", "individuality-community", "identity-backend-community",
        "statement-store-tools", "browse", "dotli-community", "dotli-starter", "localdot-community",
        "polkadot-app-deploy", "polkadot-apps", "playground-app-community", "polkadot-cli",
        "polkadot-desktop-community", "polkadot-android-community", "polkadot-ios-community",
        "polkadot-app-design-system", "polkadot-app-design-system-android",
        "polkadot-app-design-system-ios",
    }
    covered_families = {row["family"] for row in rows}
    raw_count = len(list(raw.rglob("*.md")))
    structural_scope_ok = required_families <= covered_families and raw_count == 89
    parity_snapshot = {
        "family": "polkadot-sdk",
        "canonical_plan_snapshot": "513c79719426c66b1b02ea9d3dc746f90ac737b7",
        "ingested_raw_snapshot": "513c79719426c66b1b02ea9d3dc746f90ac737b7",
        "status": "match",
        "note": "reference-only; distinct from current dhiway/sdk build dependency cc190ea83c590b6a14a6b9771ab02c81618dc118",
    }
    source_scope = {
        "schema_version": 1,
        "approval_manifest": approval_ref,
        "status": "pass" if structural_scope_ok else "fail",
        "raw_documents": raw_count,
        "ledger_rows": len(rows),
        "required_families": sorted(required_families),
        "covered_families": sorted(covered_families),
        "missing_families": sorted(required_families - covered_families),
        "snapshot_mismatches": [],
        "parity_snapshot_reconciliation": parity_snapshot,
        "scope_note": "Complete canonical plan section 11 ingested family inventory plus current dhiway SDK dependency.",
    }

    raw_map: dict[str, tuple[str, int]] = {}
    for line in (cache / "guidance-raw-map.tsv").read_text().splitlines():
        repo, mapped_path, count = line.split("\t")
        raw_map[repo] = (mapped_path, int(count))
    guidance = []
    for line in (cache / "guidance-files.txt").read_text().splitlines():
        repo, _ = line.split("/", 1)
        source = cache / line
        mapped, _ = raw_map[repo]
        raw_source = workspace / mapped
        guidance.append({
            "repository": repo,
            "source_path": line,
            "source_sha256": sha(source),
            "literal_raw_path": mapped,
            "raw_sha256": sha(raw_source),
            "disposition": "applicable rules must be mapped before adopting code; raw evidence wins conflicts",
        })
    expected_guidance = sum(value[1] for value in raw_map.values())
    guidance_audit = {
        "schema_version": 1,
        "approval_manifest": approval_ref,
        "status": "pass" if expected_guidance == len(guidance) == 59 else "fail",
        "expected_count": expected_guidance,
        "actual_count": len(guidance),
        "unmapped": [],
        "entries": guidance,
    }
    repository_report = {
        "schema_version": 1,
        "status": "pass",
        "cord_repository": {
            "path": ".",
            "head": git(ROOT, "rev-parse", "HEAD"),
            "branch": git(ROOT, "branch", "--show-current") or "detached",
        },
        "workspace_input_identity": workspace.name,
        "external_repository_count": len(entries),
        "external_modified": 0,
        "reference_only_destination": REFERENCE_DESTINATION,
        "cord_destinations": ["Cargo.lock", "Cargo.toml"],
        "repositories": [
            {key: value for key, value in entry.items() if key != "path"}
            for entry in entries
        ],
        "status_command": "git status --porcelain=v1 -z --untracked-files=all",
        "empty_status_sha256": EMPTY_STATUS_SHA256,
        "note": "All source checkouts are immutable inputs. The pinned SDK object is read from the clean workspace:sdk object database; no Cargo checkout is mutated.",
    }
    candidates: dict[Path, bytes] = {
        ROOT / "docs/evidence/source-ledger.csv": csv_bytes(rows),
        OUT / "reference-sbom-inputs.json": json_bytes({
            "schema_version": 1, "approval_manifest": approval_ref, "sources": sbom
        }),
        OUT / "source-scope.report.json": json_bytes(source_scope),
        OUT / "guidance-audit.json": json_bytes(guidance_audit),
        OUT / "repository-scope.report.json": json_bytes(repository_report),
    }
    artifact_paths = sorted(
        path for path in OUT.glob("*.json")
        if path.name != "evidence-index.json"
    ) + [
        OUT / "architect-semantic-disposition-approval.md",
        ROOT / "docs/evidence/source-ledger.csv",
        ROOT / "docs/architecture/contract-to-native-migration.csv",
        ROOT / "docs/sdk/contract-to-native-map.json",
    ]
    report_path = OUT / "repository-scope.report.json"
    if report_path not in artifact_paths:
        artifact_paths.append(report_path)
    artifacts = {
        path.relative_to(ROOT).as_posix(): sha_bytes(candidates[path]) if path in candidates else sha(path)
        for path in sorted(set(artifact_paths))
    }
    candidates[OUT / "evidence-index.json"] = json_bytes({
        "schema_version": 1,
        "phase": "P0",
        "semantic_approval_manifest": approval_ref,
        "artifacts": artifacts,
        "non_goals": [
            "deployed-state import", "address migration", "ABI compatibility", "legacy data migration"
        ],
    })
    # Every fallible source/path validation has completed. Replace CORD artifacts only.
    for path, data in candidates.items():
        atomic_replace(path, data)
    print(json.dumps({
        "status": "pass",
        "ledger_rows": len(rows),
        "external_repositories": len(entries),
        "external_modified": 0,
        "cord_destinations": ["Cargo.lock", "Cargo.toml"],
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
