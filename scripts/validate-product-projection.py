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

"""Validate the unified Identity projection and conditional internal runtime disposition."""

from __future__ import annotations

import argparse
import json
import os
import re
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 and earlier.
    import tomli as tomllib
from pathlib import Path
from typing import Any, Iterable


IDENTITY_OPERATIONS = (
    "identity.account",
    "identity.profile.read",
    "identity.profile.disclose",
    "identity.humanity.status",
    "identity.humanity.prove",
    "identity.subject.derive",
    "identity.entitlements.read",
)
SIGNING_OPERATION = "transaction.sign"
TEXT_SUFFIXES = {".json", ".md", ".toml", ".ts", ".tsx", ".rs"}
OLD_TAXONOMY = re.compile(
    r"(?i)(?<![A-Za-z0-9])(?:people[-_ ]?lite(?:auth)?|personhood|resources)(?![A-Za-z0-9])"
)
INTERNAL_ROUTE = re.compile(
    r"(?i)(?:attest[-_.]?lite|attestation[-_.]?allowance|people[-_ ]?lite(?:auth)?|resources[-_.]?allocate)"
)
SHIM_PATTERNS = (
    re.compile(r'concat!\(\s*"People"\s*,\s*"Lite"\s*\)'),
    re.compile(r'"People"\s*\+\s*"Lite"'),
    re.compile(r"\[\s*[\"']People[\"']\s*,\s*[\"']Lite[\"']\s*\]\.join"),
    re.compile(r"(?i)(?:metadata|descriptor).{0,40}(?:filter|replace).{0,80}people[-_ ]?lite"),
)


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent, delete=False) as handle:
        handle.write(payload)
        temporary = Path(handle.name)
    os.replace(temporary, path)


def text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def source_files(root: Path, starts: Iterable[str]) -> list[Path]:
    files: list[Path] = []
    for start in starts:
        path = root / start
        if path.is_file():
            files.append(path)
        elif path.is_dir():
            files.extend(candidate for candidate in path.rglob("*") if candidate.is_file())
    return sorted({path for path in files if path.suffix in TEXT_SUFFIXES})


def public_product_files(root: Path) -> list[Path]:
    files = source_files(
        root,
        (
            "product-sdk/src",
            "product-sdk/packages",
            "product-sdk/examples",
            "docs/Developer.md",
            "docs/cord-features.md",
            "docs/architecture/domains",
        ),
    )
    excluded_parts = {"node_modules", "dist", "internal"}
    excluded_files = {
        root / "product-sdk/packages/descriptors/generated/orbis-descriptor.json",
    }
    return [
        path for path in files
        if path not in excluded_files and not excluded_parts.intersection(path.relative_to(root).parts)
    ]


def line_findings(root: Path, files: Iterable[Path], pattern: re.Pattern[str]) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for path in files:
        for number, line in enumerate(text(path).splitlines(), 1):
            for match in pattern.finditer(line):
                findings.append({
                    "path": path.relative_to(root).as_posix(),
                    "line": number,
                    "value": match.group(0),
                })
    return findings


def public_rust_findings(root: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    checks = {
        "origin-rs/src/product_sdk/domains/mod.rs": (
            re.compile(r"(?m)^pub mod identity_personhood;"),
            re.compile(r"(?m)^pub mod people_lite;"),
        ),
        "origin-rs/src/product_sdk/mod.rs": (
            re.compile(r"(?m)^.*pub use.*prepare_identity_personhood_command"),
        ),
        "origin-rs/src/product_sdk/route_registry.rs": (
            re.compile(r"(?m)^\s*IdentityPersonhood(?:Query|Command)\("),
            re.compile(r"attest_lite_person"),
        ),
        "origin-rs/src/product_sdk/sponsored_intent.rs": (
            re.compile(r"(?m)^\s*IdentityPersonhood\("),
        ),
        "origin-rs/src/product_sdk/transport.rs": (
            re.compile(r"(?m)^\s*pub\s+(?:async\s+)?fn\s+(?:read|submit|prepare)_identity_personhood"),
        ),
    }
    for relative, patterns in checks.items():
        value = text(root / relative)
        for pattern in patterns:
            for match in pattern.finditer(value):
                findings.append({"path": relative, "offset": match.start(), "value": match.group(0)})
    return findings


def metadata_shims(root: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    files = source_files(root, ("origin-rs/src", "origin/orbis", "product-sdk/src", "product-sdk/packages"))
    for path in files:
        if path == root / "scripts/validate-product-projection.py":
            continue
        value = text(path)
        for pattern in SHIM_PATTERNS:
            for match in pattern.finditer(value):
                findings.append({
                    "path": path.relative_to(root).as_posix(),
                    "offset": match.start(),
                    "value": match.group(0),
                })
    return findings


def validate_disposition(root: Path, ledger_path: Path) -> tuple[list[str], dict[str, Any]]:
    ledger = tomllib.loads(text(ledger_path))
    errors: list[str] = []
    required = {
        "schema_version", "component", "decision", "status", "adr_path", "adr_heading",
        "public_capabilities", "separate_signing", "metadata_visibility", "metadata_filter_shim",
    }
    missing = sorted(required - ledger.keys())
    if missing:
        errors.append(f"ledger missing fields: {', '.join(missing)}")
    if ledger.get("schema_version") != 1 or ledger.get("decision") != "internal-retain":
        errors.append("ledger must select schema v1 internal-retain")
    if ledger.get("status") != "ratified" or ledger.get("metadata_visibility") != "operator-only":
        errors.append("ledger is not a ratified operator-only disposition")
    if ledger.get("metadata_filter_shim") is not False:
        errors.append("metadata filter shim must be false")
    if tuple(ledger.get("public_capabilities", ())) != (
        "identity.humanity.status", "identity.humanity.prove"
    ) or ledger.get("separate_signing") != SIGNING_OPERATION:
        errors.append("ledger public facade is not the unified Humanity/signing contract")

    adr_path = root / str(ledger.get("adr_path", ""))
    if not adr_path.is_file():
        errors.append("accepted disposition ADR is missing")
    else:
        adr = text(adr_path)
        if not re.search(r"(?ms)^## Status\s+Accepted\.", adr):
            errors.append("disposition ADR is not Accepted")
        heading = str(ledger.get("adr_heading", ""))
        if f"### {heading}" not in adr:
            errors.append("disposition ADR heading does not match ledger")

    invariants = ledger.get("invariant", [])
    identifiers = [row.get("id") for row in invariants]
    if len(invariants) < 7 or len(identifiers) != len(set(identifiers)):
        errors.append("invariant ledger is incomplete or has duplicate IDs")
    distinct = 0
    reproduced = 0
    for row in invariants:
        fields = {
            "id", "name", "classification", "disposition", "source_path", "source_markers",
            "counterpart_paths", "counterpart_markers", "tests", "rationale",
        }
        absent = sorted(fields - row.keys())
        if absent:
            errors.append(f"{row.get('id', '<missing>')}: missing {', '.join(absent)}")
            continue
        classification = row["classification"]
        disposition = row["disposition"]
        if classification == "distinct" and disposition == "internal-retain":
            distinct += 1
        elif classification == "shared" and disposition == "reproduced-dependency":
            reproduced += 1
        else:
            errors.append(f"{row['id']}: invalid classification/disposition pair")
        evidence = root / row["source_path"]
        if not evidence.is_file():
            errors.append(f"{row['id']}: source path missing")
        else:
            value = text(evidence)
            for marker in row["source_markers"]:
                if marker not in value:
                    errors.append(f"{row['id']}: source marker missing: {marker}")
        counterparts = row["counterpart_paths"]
        counterpart_markers = row["counterpart_markers"]
        combined = "\n".join(text(root / path) for path in counterparts if (root / path).is_file())
        if len([path for path in counterparts if (root / path).is_file()]) != len(counterparts):
            errors.append(f"{row['id']}: counterpart path missing")
        for marker in counterpart_markers:
            if marker not in combined:
                errors.append(f"{row['id']}: counterpart marker missing: {marker}")
        for test in row["tests"]:
            if not (root / test).is_file():
                errors.append(f"{row['id']}: test path missing: {test}")
    if distinct == 0 or reproduced == 0:
        errors.append("ledger must prove both distinct and shared semantics")

    surfaces = ledger.get("internal_surface", [])
    surface_ids = [row.get("id") for row in surfaces]
    if len(surfaces) < 5 or len(surface_ids) != len(set(surface_ids)):
        errors.append("internal surface inventory is incomplete or has duplicate IDs")
    for row in surfaces:
        if row.get("visibility") != "internal" or row.get("projects_to") != []:
            errors.append(f"{row.get('id', '<missing>')}: internal surface projects publicly")
            continue
        path = root / str(row.get("path", ""))
        if not path.is_file() or str(row.get("marker", "")) not in text(path):
            errors.append(f"{row.get('id', '<missing>')}: internal surface marker missing")

    summary = {
        "decision": ledger.get("decision"),
        "distinct_invariant_count": distinct,
        "reproduced_dependency_count": reproduced,
        "internal_surface_count": len(surfaces),
    }
    return errors, summary


def identity_report(root: Path) -> dict[str, Any]:
    ts = text(root / "product-sdk/packages/origin-sdk-identity/src/v2.ts")
    ts_index = text(root / "product-sdk/packages/origin-sdk-identity/src/index.ts")
    rust = text(root / "origin-rs/src/product_sdk/domains/identity_v2.rs")
    operations = [operation for operation in IDENTITY_OPERATIONS if operation in ts and operation in rust]
    exact_public_export = 'export * from "./v2.ts"' in ts_index
    rust_public_v2 = (
        "pub mod identity_v2;" in text(root / "origin-rs/src/product_sdk/domains/mod.rs")
        and "pub enum IdentityV2Operation" in rust
    )

    vectors = json.loads(text(root / "docs/specs/identity-v2.vectors.json"))
    vector_failures = 0 if vectors.get("version") == 2 and operations == list(IDENTITY_OPERATIONS) else 1
    errors = json.loads(text(root / "docs/specs/origin-host-registry-v2.errors.json"))
    encoded = json.dumps(errors)
    expected_codes = {
        "IDENTITY_CHALLENGE_REPLAY": 401,
        "IDENTITY_RECOVERY_ENTROPY_FAILED": 408,
        "IDENTITY_RECOVERY_INSTALL_FAILED": 409,
        "IDENTITY_OLD_INCARNATION": 410,
        "IDENTITY_RETIRED_SET_FULL": 411,
    }
    error_code_failures = sum(
        not (name in encoded and f'"code": {code}' in encoded) for name, code in expected_codes.items()
    )

    authority = text(root / "origin-rs/src/product_sdk/host_v2/identity_authority.rs")
    recovery_markers = (
        "RecoveryInstallV2", "RecoveryReceiptV2", "IDENTITY_RECOVERY_ENTROPY_FAILED",
        "IDENTITY_RECOVERY_INSTALL_FAILED", "IDENTITY_RETIRED_SET_FULL", "16_384",
    )
    recovery_failures = sum(marker not in authority for marker in recovery_markers)
    same_store_replay_failures = 0 if all(
        marker in authority for marker in ("consumed_challenges", "persist", "IDENTITY_CHALLENGE_REPLAY")
    ) else 1
    non_live_continuity_true = 0 if (
        "continuity: false" in authority and "epoch: 0" in authority
    ) else 1

    leakage_findings = 0
    provider_files = source_files(root, ("origin/orbis/provider-node/src",))
    secret_pattern = re.compile(r"(?i)(?:subject_master_seed|subject_proof|profile_secret).{0,40}(?:log!|tracing|metric|label)")
    leakage_findings += len(line_findings(root, provider_files, secret_pattern))

    return {
        "schema_version": 1,
        "status": "pass" if (
            len(operations) == 7 and exact_public_export and rust_public_v2 and vector_failures == 0
            and error_code_failures == 0 and recovery_failures == 0
            and same_store_replay_failures == 0 and non_live_continuity_true == 0
            and leakage_findings == 0
        ) else "blocked",
        "identity_operation_count": len(operations),
        "identity_operations": operations,
        "separate_signing": SIGNING_OPERATION in ts and SIGNING_OPERATION in rust,
        "public_v2_export": exact_public_export,
        "rust_v2_public": rust_public_v2,
        "vector_failures": vector_failures,
        "error_code_failures": error_code_failures,
        "same_store_replay_failures": same_store_replay_failures,
        "recovery_failures": recovery_failures,
        "non_live_continuity_true": non_live_continuity_true,
        "leakage_findings": leakage_findings,
    }


def disposition_report(root: Path, ledger_path: Path) -> dict[str, Any]:
    ledger_errors, summary = validate_disposition(root, ledger_path)
    public_files = public_product_files(root)
    taxonomy = line_findings(root, public_files, OLD_TAXONOMY) + public_rust_findings(root)
    projected = line_findings(root, public_files, INTERNAL_ROUTE) + public_rust_findings(root)
    shims = metadata_shims(root)
    branch_valid = not ledger_errors
    status = "pass" if branch_valid and not taxonomy and not projected and not shims else "blocked"
    return {
        "schema_version": 1,
        "status": status,
        "public_old_taxonomy_count": len(taxonomy),
        "internal_projection_count": len(projected),
        "metadata_filter_shim_count": len(shims),
        "disposition_branch_valid": branch_valid,
        "disposition_errors": ledger_errors,
        "public_taxonomy_findings": taxonomy,
        "internal_projection_findings": projected,
        "metadata_filter_shim_findings": shims,
        **summary,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("identity", "people-lite-disposition"), required=True)
    parser.add_argument("--ledger", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    root = args.root.resolve()
    if args.mode == "identity":
        report = identity_report(root)
    else:
        if args.ledger is None:
            parser.error("--ledger is required for people-lite-disposition")
        ledger = args.ledger if args.ledger.is_absolute() else root / args.ledger
        report = disposition_report(root, ledger)
    atomic_json(args.out, report)
    print(f"{report['status'].upper()} product projection: mode={args.mode}")
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
