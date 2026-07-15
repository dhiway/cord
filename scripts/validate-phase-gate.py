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

"""Materialize current phase assertions from independently hashed evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib

sys.dont_write_bytecode = True

from evidence_common import (  # noqa: E402
    atomic_write_json,
    canonical_bytes,
    report_hash,
    sha256_bytes,
    sha256_file,
)


DEPENDENCIES = {
    "P1": ["AC2", "AC3", "AC4"],
    "P2": ["AC2", "AC3", "AC5", "AC7"],
    "P3": ["AC7", "AC8"],
    "P4_OBJECT": ["AC2", "AC7", "AC10"],
    "P4_PROVIDER": ["AC3", "AC4", "AC5", "AC7", "AC10"],
    "P4_DRIVE": ["AC6", "AC10"],
    "P4_S3": ["AC6", "AC10"],
    "P4_RUNTIME": ["AC1", "AC10"],
    "P5": ["AC8", "AC9", "AC10"],
    "P6": ["AC11"],
    "P7": [f"AC{number}" for number in range(1, 15)],
}


def load_json(path: Path, failures: list[str]) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exception:
        failures.append(f"cannot read {path}: {exception}")
        return {}


def p0(root: Path) -> dict[str, Any]:
    failures: list[str] = []
    evidence = root / "../.omx/evidence/origin-orbis-web3-storage"
    baseline = load_json(evidence / "repos-before.json", failures)
    unsigned = dict(baseline)
    baseline_hash = unsigned.pop("snapshot_sha256", None)
    baseline_hash_valid = bool(baseline_hash) and baseline_hash == sha256_bytes(canonical_bytes(unsigned))
    if not baseline_hash_valid:
        failures.append("repository baseline hash is invalid")
    decidability = load_json(evidence / "ac-decidability-v1.json", failures)
    decidability_unsigned = dict(decidability)
    decidability_expected = decidability_unsigned.pop("report_sha256", None)
    if not decidability_expected or decidability_expected != sha256_bytes(canonical_bytes(decidability_unsigned)):
        failures.append("AC decidability report hash is invalid")
    inventory = load_json(root / "target/evidence/ac1-assertions.json", failures)
    contract_failures: list[str] = []
    required_contracts = [
        "storage-content-v1.md", "storage-checkpoints-v2.md", "storage-control-v1.md",
        "drive-filesystem-v1.md", "s3-v1.md", "storage-encryption-v1.md",
        "provider-organization-sla-v1.md", "identity-v2.md", "host-provider-protocol-v2.md",
        "origin-host-registry-v2.cddl", "origin-host-registry-v2.schema.json",
        "origin-host-registry-v2.operations.json", "origin-host-registry-v2.errors.json",
        "origin-host-registry-v2.vectors.json", "origin-host-registry-v2.vectors.cbor",
        "protocol-executable-v2.vectors.json", "checkpoint-v2.vectors.json",
        "provider-protocol-v1.registry.json", "provider-protocol-v1.vectors.json",
        "host-outbox-v1.vectors.json", "identity-v2.vectors.json", "drive-s3-v1.vectors.json",
        "storage-bounds-v1.toml", "storage-v1.vectors.json",
        "host-outbox-v1.state-machine.json",
        "identity-recovery-v2.state-machine.json", "generated/host-outbox-v1.md",
        "generated/identity-recovery-v2.md", "p0-open-decisions.toml",
        "p0-ratification-v1.toml",
    ]
    for relative in required_contracts:
        if not (root / "docs/specs" / relative).is_file():
            contract_failures.append(f"missing normative contract: {relative}")
    try:
        operations = json.loads((root / "docs/specs/origin-host-registry-v2.operations.json").read_text())
        errors = json.loads((root / "docs/specs/origin-host-registry-v2.errors.json").read_text())
        vectors = json.loads((root / "docs/specs/origin-host-registry-v2.vectors.json").read_text())
        projection = json.loads((root / "docs/specs/origin-host-registry-v2.schema.json").read_text())
        cddl = (root / "docs/specs/origin-host-registry-v2.cddl").read_text(encoding="utf-8")
        operation_count = len(operations["operations"])
        error_count = len(errors["errors"])
        vector_count = len(vectors["vectors"])
        cddl_count = len(re.findall(r"(?m)^([A-Za-z][A-Za-z0-9_-]*)\s*=", cddl))
        projection_count = len(projection.get("$defs", {}))
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as exception:
        operation_count = error_count = vector_count = cddl_count = projection_count = 0
        contract_failures.append(f"cannot validate normative registries: {exception}")
    for generated, source in (
        ("generated/host-outbox-v1.md", "host-outbox-v1.state-machine.json"),
        ("generated/identity-recovery-v2.md", "identity-recovery-v2.state-machine.json"),
    ):
        try:
            expected_hash = hashlib.sha256((root / "docs/specs" / source).read_bytes()).hexdigest()
            rendered = (root / "docs/specs" / generated).read_text(encoding="utf-8")
            if expected_hash not in rendered:
                contract_failures.append(f"generated hash drift: {generated}")
        except OSError as exception:
            contract_failures.append(f"cannot validate generated artifact {generated}: {exception}")
    try:
        decisions = tomllib.loads((root / "docs/specs/p0-open-decisions.toml").read_text(encoding="utf-8"))
        unresolved = [
            row.get("id") for row in decisions.get("decision", [])
            if not str(row.get("status", "")).startswith("resolved")
            or row.get("implementation_may_start") is not True
        ]
        if unresolved:
            contract_failures.append(f"unresolved P0 decisions: {unresolved}")
    except (OSError, tomllib.TOMLDecodeError) as exception:
        contract_failures.append(f"cannot validate P0 decisions: {exception}")
    adr_text = (root / "docs/adr/0020-web3-storage-component-disposition-policy.md").read_text(
        encoding="utf-8"
    )
    adr_accepted = re.search(r"(?m)^Accepted\.", adr_text) is not None
    if not adr_accepted:
        failures.append("ADR 0020 has not been independently ratified as Accepted")
    try:
        ratification = tomllib.loads(
            (root / "docs/specs/p0-ratification-v1.toml").read_text(encoding="utf-8")
        )
        ledger_path = root / ratification["ledger_path"]
        inventory_path = root / ratification["inventory_path"]
        ratification_valid = (
            ratification.get("status") == "accepted"
            and ratification.get("p0_gate_status") == "pass"
            and isinstance(ratification.get("architect_approval"), str)
            and ratification["architect_approval"].startswith("P0-ARCH-")
            and "BLOCK" not in ratification["architect_approval"]
            and isinstance(ratification.get("critic_approval"), str)
            and ratification["critic_approval"].startswith("P0-CRITIC-")
            and "BLOCK" not in ratification["critic_approval"]
            and bool(ratification.get("accepted_at"))
            and hashlib.sha256(ledger_path.read_bytes()).hexdigest() == ratification.get("ledger_sha256")
            and hashlib.sha256(inventory_path.read_bytes()).hexdigest() == ratification.get("inventory_sha256")
        )
        if not ratification_valid:
            failures.append("P0 ledger/inventory has not received frozen architect and critic ratification")
    except (OSError, KeyError, tomllib.TOMLDecodeError) as exception:
        ratification_valid = False
        failures.append(f"cannot validate P0 ratification: {exception}")
    deletion = load_json(root / "target/evidence/p0-deletion-dag.json", failures)
    deletion_dag_failures = sum(
        int(deletion.get(key, 1))
        for key in (
            "dag_missing_fields", "dag_duplicate_ids", "dag_duplicate_orders", "dag_invalid_items",
            "dag_invalid_edges", "dag_cycle_count",
            "absent_symbol_count", "duplicate_surface_count", "unmapped_surface_count",
            "declaration_only_surface_count",
            "unmapped_finding_count", "duplicate_owner_count",
        )
    )
    if deletion_dag_failures:
        failures.append(f"deletion DAG has {deletion_dag_failures} structural failure(s)")
    ledger = load_json(root / "docs/specs/web3-storage-capability-ledger-v1.json", failures)
    authorization_boundary_valid = (
        ledger.get("p1_authorized") is False
        and ledger.get("feature_complete") is False
        and ledger.get("production_ready") is False
        and "may begin" not in str(ledger.get("authorization_scope", "")).lower()
    )
    if not authorization_boundary_valid:
        failures.append("capability ledger contains a competing P1 authorization")
    semantic_reports = [
        root / "target/p0-origin-host-rust-conformance.json",
        root / "target/p0-origin-host-cross-contract-report.json",
    ]
    semantic_conformance_failures = 0
    semantic_report_hashes = {}
    reports = {path.name: load_json(path, failures) for path in semantic_reports}
    for path in semantic_reports:
        semantic_report_hashes[str(path.relative_to(root))] = (
            sha256_file(path) if path.is_file() else None
        )
    empty_coverage_fields = (
        "missing_operations", "missing_errors", "missing_types", "extra_types", "missing_vectors",
    )
    semantic_empty_fields = ("unmatched_productions", "uncovered_constraints")
    decoder_counters = (
        "canonical_accepted", "canonical_rejected", "schema_rejected",
        "crypto_verified", "negative_state_assertions",
    )
    rust_report = reports["p0-origin-host-rust-conformance.json"]
    if (
        rust_report.get("status") != "pass"
        or any(rust_report.get(field) != [] for field in empty_coverage_fields)
        or any(rust_report.get(field) != [] for field in semantic_empty_fields)
        or any(not isinstance(rust_report.get(field), int) or rust_report[field] < 1 for field in decoder_counters)
        or any(
            not isinstance(rust_report.get(field), int) or rust_report[field] < 1
            for field in (
                "semantic_vector_accepted", "semantic_vector_rejected",
                "outbox_drift_rejections",
            )
        )
    ):
        semantic_conformance_failures += 1
    cross_report = reports["p0-origin-host-cross-contract-report.json"]
    cross_coverage = cross_report.get("coverage", {})
    typescript = cross_report.get("typescript", {})
    rust = cross_report.get("rust", {})
    authority_paths = {
        "cddl_sha256": "docs/specs/origin-host-registry-v2.cddl",
        "json_projection_sha256": "docs/specs/origin-host-registry-v2.schema.json",
        "operations_sha256": "docs/specs/origin-host-registry-v2.operations.json",
        "errors_sha256": "docs/specs/origin-host-registry-v2.errors.json",
        "host_vectors_sha256": "docs/specs/origin-host-registry-v2.vectors.cbor",
        "protocol_vectors_sha256": "docs/specs/protocol-executable-v2.vectors.json",
        "checkpoint_vectors_sha256": "docs/specs/checkpoint-v2.vectors.json",
        "provider_vectors_sha256": "docs/specs/provider-protocol-v1.vectors.json",
        "outbox_vectors_sha256": "docs/specs/host-outbox-v1.vectors.json",
        "identity_vectors_sha256": "docs/specs/identity-v2.vectors.json",
        "drive_s3_vectors_sha256": "docs/specs/drive-s3-v1.vectors.json",
    }
    authority = cross_report.get("authority", {})
    authority_valid = all(
        authority.get(field) == sha256_file(root / path)
        for field, path in authority_paths.items()
        if (root / path).is_file()
    ) and all((root / path).is_file() for path in authority_paths.values())
    if (
        cross_report.get("status") != "pass"
        or any(cross_coverage.get(field) != [] for field in empty_coverage_fields)
        or any(cross_coverage.get(field) != [] for field in semantic_empty_fields)
        or typescript.get("status") != "pass"
        or rust.get("status") != "pass"
        or any(typescript.get(field) != [] for field in semantic_empty_fields)
        or any(rust.get(field) != [] for field in semantic_empty_fields)
        or any(not isinstance(typescript.get(field), int) or typescript[field] < 1 for field in decoder_counters)
        or any(not isinstance(rust.get(field), int) or rust[field] < 1 for field in decoder_counters)
        or any(
            not isinstance(cross_coverage.get(field), int) or cross_coverage[field] < 1
            for field in (
                "semantic_vector_accepted", "semantic_vector_rejected",
                "outbox_drift_rejections",
            )
        )
        or any(
            not isinstance(rust.get(field), int) or rust[field] < 1
            for field in (
                "semantic_vector_accepted", "semantic_vector_rejected",
                "outbox_drift_rejections",
            )
        )
        or not isinstance(cross_coverage.get("semantic_coverage_sha256"), str)
        or cross_coverage.get("semantic_coverage_sha256") != typescript.get("semantic_coverage_sha256")
        or cross_coverage.get("semantic_coverage_sha256") != rust.get("semantic_coverage_sha256")
        or authority.get("semantic_schema_sha256") != typescript.get("semantic_schema_sha256")
        or authority.get("semantic_schema_sha256") != rust.get("semantic_schema_sha256")
        or not authority_valid
    ):
        semantic_conformance_failures += 1
    if semantic_conformance_failures:
        failures.append(
            f"semantic/reference conformance has {semantic_conformance_failures} failing report(s)"
        )
    failures.extend(contract_failures)
    mechanical = int(decidability.get("mechanical", 0))
    blocked = int(inventory.get("blocked", -1))
    if decidability.get("status") != "pass" or mechanical < 13:
        failures.append("AC decidability is below the P0 threshold")
    if blocked != 0:
        failures.append(f"inventory has {blocked} blocked-conflict row(s)")
    if inventory.get("unknown", -1) != 0 or inventory.get("unmapped", -1) != 0:
        failures.append("inventory has unknown or unmapped artifacts")
    return {
        "schema_version": 1,
        "gate_id": "P0",
        "baseline_hash_valid": baseline_hash_valid,
        "decidability_status": decidability.get("status"),
        "mechanical": mechanical,
        "repo_sha": inventory.get("repo_sha"),
        "census_root": inventory.get("census_root"),
        "census_count": inventory.get("census_count"),
        "blocked": blocked,
        "unknown": inventory.get("unknown"),
        "unmapped": inventory.get("unmapped"),
        "normative_contract_count": len(required_contracts),
        "normative_contract_failures": len(contract_failures),
        "operation_count": operation_count,
        "error_count": error_count,
        "vector_count": vector_count,
        "cddl_production_count": cddl_count,
        "projection_definition_count": projection_count,
        "adr_accepted": adr_accepted,
        "ratification_valid": ratification_valid,
        "deletion_dag_failures": deletion_dag_failures,
        "authorization_boundary_valid": authorization_boundary_valid,
        "semantic_conformance_failures": semantic_conformance_failures,
        "semantic_report_hashes": semantic_report_hashes,
        "failures": len(failures),
        "failure_details": failures,
    }


def aggregate(root: Path, gate: str) -> dict[str, Any]:
    failures: list[str] = []
    evidence = (root / "../.omx/evidence/origin-orbis-web3-storage").resolve()
    statuses = {}
    report_hashes = {}
    for dependency in DEPENDENCIES[gate]:
        report = load_json(evidence / f"{dependency.lower()}.json", failures)
        statuses[dependency] = report.get("status")
        report_hashes[dependency] = report.get("report_sha256")
        if report.get("report_sha256") != report_hash(report):
            failures.append(f"{dependency} report hash is invalid")
        if report.get("status") != "pass":
            failures.append(f"{dependency} is not pass")
    return {
        "schema_version": 1,
        "gate_id": gate,
        "dependency_statuses": statuses,
        "dependency_report_hashes": report_hashes,
        "failures": len(failures),
        "failure_details": failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gate", required=True, choices=["P0", *DEPENDENCIES])
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    root = Path.cwd().resolve()
    report = p0(root) if args.gate == "P0" else aggregate(root, args.gate)
    report["status"] = "pass" if report["failures"] == 0 else "blocked"
    atomic_write_json(args.out, report)
    print(f"{report['status'].upper()} {args.gate} assertions")
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
