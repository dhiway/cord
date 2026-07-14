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

"""Fail-closed AC7/AC8/AC10/P1-AC13 canonical evidence materializer.

The input ledger binds every source, executable, runtime identity, topology and raw result by
SHA-256. Invalid, missing, prepared, blocked or failed inputs produce no canonical PASS artifact.
The P1 index is a second-stage output and is written only when an independent review binds all four
materialized artifact hashes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
INPUT_SCHEMA = "cord.p1-materializer-inputs.v1"
REVIEW_SCHEMA = "cord.p1-independent-review.v1"
OUTPUTS = {
    "AC7": "control-scenarios.json",
    "AC8": "broker-e2e.json",
    "AC10": "orbis-elastic-smoke.json",
    "AC13-P1": "storage-resource-headroom.json",
}
REQUIRED_SOURCE_ROLES = {
    "workspace_lock",
    "origin_runtime",
    "orbis_runtime",
    "control_runner",
    "control_scenarios",
    "smoke_runner",
    "proof_campaign_runner",
    "proof_case_driver",
    "proof_verifier",
}
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class InvalidEvidence(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise InvalidEvidence(f"cannot read JSON {path}: {error}") from error


def resolve_path(value: Any, base: Path, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise InvalidEvidence(f"{label} path is absent")
    path = Path(value)
    return (path if path.is_absolute() else base / path).resolve()


def require_hash(value: Any, label: str) -> str:
    if not isinstance(value, str) or not HEX64.fullmatch(value):
        raise InvalidEvidence(f"{label} SHA-256 is malformed")
    return value


def validate_file_binding(binding: Any, base: Path, label: str) -> dict[str, Any]:
    if not isinstance(binding, dict):
        raise InvalidEvidence(f"{label} binding is absent")
    path = resolve_path(binding.get("path"), base, label)
    expected = require_hash(binding.get("sha256"), label)
    if not path.is_file():
        raise InvalidEvidence(f"{label} file is absent: {path}")
    actual = sha256(path)
    if actual != expected:
        raise InvalidEvidence(f"{label} hash mismatch: {actual} != {expected}")
    return {"path": str(path), "sha256": actual, "bytes": path.stat().st_size}


def current_head() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, capture_output=True, check=False
    )
    if completed.returncode:
        raise InvalidEvidence("cannot resolve current Git HEAD")
    return completed.stdout.strip()


def validate_source(source: Any, base: Path) -> dict[str, Any]:
    if not isinstance(source, dict):
        raise InvalidEvidence("source binding is absent")
    commit = source.get("commit")
    if commit != current_head():
        raise InvalidEvidence(f"source commit {commit!r} does not match current HEAD")
    files = source.get("files")
    if not isinstance(files, list) or not files:
        raise InvalidEvidence("source file hash ledger is absent")
    validated = []
    for index, item in enumerate(files):
        validated_item = validate_file_binding(item, base, f"source.files[{index}]")
        role = item.get("role") if isinstance(item, dict) else None
        if not isinstance(role, str) or not role:
            raise InvalidEvidence(f"source.files[{index}] role is absent")
        validated_item["role"] = role
        validated.append(validated_item)
    paths = [item["path"] for item in validated]
    if len(paths) != len(set(paths)):
        raise InvalidEvidence("source file ledger contains duplicate paths")
    roles = [item["role"] for item in validated]
    if len(roles) != len(set(roles)) or set(roles) != REQUIRED_SOURCE_ROLES:
        raise InvalidEvidence(f"source role ledger mismatch: {set(roles)} != {REQUIRED_SOURCE_ROLES}")
    return {"commit": commit, "files": validated}


def validate_bindings(bindings: Any, base: Path, label: str) -> dict[str, dict[str, Any]]:
    if not isinstance(bindings, dict) or not bindings:
        raise InvalidEvidence(f"{label} bindings are absent")
    return {name: validate_file_binding(value, base, f"{label}.{name}") for name, value in sorted(bindings.items())}


def safe_child(root: Path, relative: Any, label: str) -> Path:
    if not isinstance(relative, str) or not relative:
        raise InvalidEvidence(f"{label} path is malformed")
    path = (root / relative).resolve()
    if path == root or root not in path.parents:
        raise InvalidEvidence(f"{label} escapes its evidence root: {relative}")
    return path


def validate_control_raw(config: Any, base: Path, source: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    if not isinstance(config, dict):
        raise InvalidEvidence("AC7/AC8 configuration is absent")
    raw_binding = validate_file_binding(config.get("raw"), base, "ac7_ac8.raw")
    scenario_binding = validate_file_binding(config.get("scenario_manifest"), base, "ac7_ac8.scenario_manifest")
    candidate_binding = validate_file_binding(config.get("candidate_manifest"), base, "ac7_ac8.candidate_manifest")
    topology = validate_file_binding(config.get("topology"), base, "ac7_ac8.topology")
    binaries = validate_bindings(config.get("binaries"), base, "ac7_ac8.binaries")
    required_binaries = {"origin", "orbis", "driver", "origin_upgrade_wasm", "orbis_upgrade_wasm"}
    if set(binaries) != required_binaries:
        raise InvalidEvidence(f"AC7/AC8 binary ledger mismatch: {set(binaries)} != {required_binaries}")
    runtime = config.get("runtime")
    if not isinstance(runtime, dict):
        raise InvalidEvidence("AC7/AC8 runtime identity binding is absent")
    expected_runtime = {
        "origin": {"spec_name": "origin", "current_spec_version": 9901, "candidate_spec_version": 9902},
        "orbis": {"spec_name": "orbis", "current_spec_version": 29, "candidate_spec_version": 30, "transaction_version": 8},
    }
    if runtime != expected_runtime:
        raise InvalidEvidence(f"AC7/AC8 runtime identity binding mismatch: {runtime}")
    candidate_manifest = read_json(Path(candidate_binding["path"]))
    if (
        not isinstance(candidate_manifest, dict)
        or candidate_manifest.get("schema") != "cord.p1-upgrade-candidates.v1"
        or candidate_manifest.get("status") != "pass"
        or candidate_manifest.get("authoritative") is not True
        or candidate_manifest.get("production_versions_unchanged") is not True
        or candidate_manifest.get("source", {}).get("commit") != source["commit"]
    ):
        raise InvalidEvidence("AC7/AC8 upgrade-candidate manifest is absent, non-authoritative, or stale")
    candidates = candidate_manifest.get("candidates")
    if not isinstance(candidates, dict) or set(candidates) != {"origin", "orbis-fast"}:
        raise InvalidEvidence("AC7/AC8 upgrade-candidate set is incomplete")
    candidate_expectations = {
        "origin": (binaries["origin_upgrade_wasm"]["sha256"], "origin", 9901, 9902, {"p1-upgrade-candidate"}),
        "orbis-fast": (binaries["orbis_upgrade_wasm"]["sha256"], "orbis", 29, 30, {"fast-runtime", "p1-upgrade-candidate"}),
    }
    for name, (wasm_hash, spec_name, current_spec, candidate_spec, features) in candidate_expectations.items():
        candidate = candidates.get(name)
        if (
            not isinstance(candidate, dict)
            or candidate.get("sha256") != wasm_hash
            or candidate.get("spec_name") != spec_name
            or candidate.get("current_spec_version") != current_spec
            or candidate.get("candidate_spec_version") != candidate_spec
            or candidate.get("higher_spec") is not True
            or not features.issubset(set(candidate.get("features", [])))
        ):
            raise InvalidEvidence(f"AC7/AC8 upgrade candidate binding mismatch: {name}")
    raw_path = Path(raw_binding["path"])
    evidence_root = raw_path.parent.resolve()
    raw = read_json(raw_path)
    if not isinstance(raw, dict) or raw.get("schema") != "cord.p1-control-broker-evidence.v1":
        raise InvalidEvidence("unsupported AC7/AC8 raw schema")
    if raw.get("status") != "pass" or raw.get("prepared_only") is not False:
        raise InvalidEvidence(f"AC7/AC8 raw status is not a live pass: {raw.get('status')}")
    if raw.get("campaign_id") != "origin-orbis-p1-control-broker-v1":
        raise InvalidEvidence("AC7/AC8 campaign identity mismatch")
    inputs = raw.get("inputs")
    if not isinstance(inputs, dict):
        raise InvalidEvidence("AC7/AC8 raw inputs are absent")
    expected_input_hashes = {
        "origin_binary_sha256": binaries["origin"]["sha256"],
        "orbis_binary_sha256": binaries["orbis"]["sha256"],
        "driver_sha256": binaries["driver"]["sha256"],
        "origin_upgrade_wasm_sha256": binaries["origin_upgrade_wasm"]["sha256"],
        "orbis_upgrade_wasm_sha256": binaries["orbis_upgrade_wasm"]["sha256"],
        "topology_sha256": topology["sha256"],
        "scenario_manifest_sha256": scenario_binding["sha256"],
    }
    for key, expected in expected_input_hashes.items():
        if inputs.get(key) != expected:
            raise InvalidEvidence(f"AC7/AC8 input hash mismatch for {key}")
    preflight = raw.get("preflight")
    if not isinstance(preflight, dict) or preflight.get("status") != "pass":
        raise InvalidEvidence("AC7/AC8 contamination preflight is absent or non-pass")
    if preflight.get("topology_sha256") != topology["sha256"]:
        raise InvalidEvidence("AC7/AC8 preflight topology hash mismatch")
    versions = preflight.get("versions")
    if not isinstance(versions, dict) or len(versions) != 8:
        raise InvalidEvidence("AC7/AC8 runtime-version ledger must contain eight nodes")
    spec_names = [value.get("specName") for value in versions.values() if isinstance(value, dict)]
    if spec_names.count("origin") != 6 or spec_names.count("orbis") != 2:
        raise InvalidEvidence(f"AC7/AC8 runtime identities are invalid: {spec_names}")
    if any(
        value.get("specVersion") != (9901 if value.get("specName") == "origin" else 29)
        for value in versions.values()
        if isinstance(value, dict)
    ):
        raise InvalidEvidence("AC7/AC8 preflight runtime spec versions are invalid")

    ledger = raw.get("raw_hashes")
    if not isinstance(ledger, list) or not ledger:
        raise InvalidEvidence("AC7/AC8 raw hash ledger is absent")
    seen: set[str] = set()
    validated_raw: list[dict[str, Any]] = []
    for index, entry in enumerate(ledger):
        if not isinstance(entry, dict):
            raise InvalidEvidence(f"AC7/AC8 raw hash entry {index} is malformed")
        relative = entry.get("path")
        if relative in seen:
            raise InvalidEvidence(f"AC7/AC8 raw hash path is duplicated: {relative}")
        seen.add(relative)
        path = safe_child(evidence_root, relative, f"AC7/AC8 raw[{index}]")
        expected = require_hash(entry.get("sha256"), f"AC7/AC8 raw[{index}]")
        if not path.is_file() or sha256(path) != expected or path.stat().st_size != entry.get("bytes"):
            raise InvalidEvidence(f"AC7/AC8 raw artifact mismatch: {relative}")
        validated_raw.append({"path": relative, "sha256": expected, "bytes": entry["bytes"]})
    side_ledger = evidence_root / "raw-hashes.json"
    if not side_ledger.is_file() or read_json(side_ledger) != ledger:
        raise InvalidEvidence("AC7/AC8 side raw-hashes ledger is absent or differs")

    scenarios = read_json(Path(scenario_binding["path"]))
    if not isinstance(scenarios, dict) or scenarios.get("schema") != "cord.p1-control-broker-scenarios.v1":
        raise InvalidEvidence("unsupported AC7/AC8 scenario manifest")
    phases = scenarios.get("phases")
    cases = raw.get("cases")
    if not isinstance(phases, dict) or not isinstance(cases, dict):
        raise InvalidEvidence("AC7/AC8 phase/case ledger is absent")
    if set(cases) != {"control", "broker-pre-restart", "full-restart-runner", "broker-post-restart"}:
        raise InvalidEvidence("AC7/AC8 raw phase set is incomplete")
    required_fields = ("input_hashes", "output_hashes", "finalized_blocks", "events", "assertions")
    captured: dict[str, list[str]] = {}
    for phase in ("control", "broker-pre-restart", "broker-post-restart"):
        expected_ids = [entry.get("id") for entry in phases.get(phase, []) if isinstance(entry, dict)]
        phase_result = cases.get(phase)
        records = phase_result.get("cases") if isinstance(phase_result, dict) else None
        if not expected_ids or not isinstance(records, dict) or list(records) != expected_ids:
            raise InvalidEvidence(f"AC7/AC8 case order/completeness mismatch for {phase}")
        for case_id, record in records.items():
            if not isinstance(record, dict) or record.get("status") != "pass":
                raise InvalidEvidence(f"AC7/AC8 case is non-pass: {case_id}")
            if any(not record.get(field) for field in required_fields):
                raise InvalidEvidence(f"AC7/AC8 case lacks hash/finality evidence: {case_id}")
            assertions = record.get("assertions")
            if isinstance(assertions, dict) and not all(value is True for value in assertions.values()):
                raise InvalidEvidence(f"AC7/AC8 case has a false assertion: {case_id}")
        captured[phase] = expected_ids
    restart = cases.get("full-restart-runner")
    if not isinstance(restart, dict) or restart.get("status") != "pass":
        raise InvalidEvidence("AC8 full restart is absent or non-pass")

    common = {
        "source": source,
        "raw": raw_binding,
        "raw_artifacts": validated_raw,
        "scenario_manifest": scenario_binding,
        "candidate_manifest": candidate_binding,
        "topology": topology,
        "binaries": binaries,
        "runtime": runtime,
    }
    ac7 = canonical("AC7", common, {"cases": captured["control"], "all_cases_finalized_and_hash_bound": True})
    ac8 = canonical(
        "AC8",
        common,
        {
            "cases": captured["broker-pre-restart"] + captured["broker-post-restart"],
            "full_restart": True,
            "all_cases_finalized_and_hash_bound": True,
        },
    )
    return ac7, ac8


def validate_ac10(config: Any, base: Path, source: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(config, dict):
        raise InvalidEvidence("AC10 configuration is absent")
    raw_binding = validate_file_binding(config.get("raw"), base, "ac10.raw")
    topology = validate_file_binding(config.get("topology"), base, "ac10.topology")
    binaries = validate_bindings(config.get("binaries"), base, "ac10.binaries")
    if set(binaries) != {"origin", "orbis"}:
        raise InvalidEvidence("AC10 requires exactly Origin and Orbis binary bindings")
    runtime = config.get("runtime")
    if not isinstance(runtime, dict):
        raise InvalidEvidence("AC10 runtime binding is absent")
    origin_runtime = runtime.get("origin")
    orbis_runtime = runtime.get("orbis")
    if not isinstance(origin_runtime, dict) or not isinstance(orbis_runtime, dict):
        raise InvalidEvidence("AC10 runtime identities are absent")
    raw = read_json(Path(raw_binding["path"]))
    if not isinstance(raw, dict) or raw.get("status") not in ("ok", "pass"):
        raise InvalidEvidence(f"AC10 raw status is not pass/ok: {raw.get('status') if isinstance(raw, dict) else None}")
    if raw.get("relay_spec_version") != origin_runtime.get("spec_version") or origin_runtime.get("spec_name") != "origin":
        raise InvalidEvidence("AC10 Foundation runtime identity mismatch")
    if raw.get("orbis_spec_version") != orbis_runtime.get("spec_version") or orbis_runtime.get("spec_name") != "orbis":
        raise InvalidEvidence("AC10 Commons runtime identity mismatch")
    if orbis_runtime.get("transaction_version") != 8:
        raise InvalidEvidence("AC10 Orbis transaction version must be 8")
    before, after = raw.get("before"), raw.get("after")
    if not isinstance(before, dict) or not isinstance(after, dict):
        raise InvalidEvidence("AC10 progress snapshots are absent")
    progress = all(
        isinstance(before.get(f"{chain}_{kind}"), int)
        and isinstance(after.get(f"{chain}_{kind}"), int)
        and after[f"{chain}_{kind}"] > before[f"{chain}_{kind}"]
        for chain in ("relay", "orbis")
        for kind in ("best", "finalized")
    )
    cores = raw.get("claim_queue_cores")
    ratio = raw.get("observed_finalized_block_ratio", raw.get("observed_block_ratio"))
    measurement = raw.get("measurement_seconds")
    assertions = {
        "para_id_is_1006": raw.get("para_id") == 1006,
        "target_block_rate_is_3": raw.get("target_block_rate") == 3,
        "relay_parent_offset_is_1": raw.get("relay_parent_offset") == 1,
        "unincluded_segment_capacity_is_12": config.get("unincluded_segment_capacity") == 12,
        "best_and_finalized_progress": progress,
        "three_distinct_claim_queue_cores": isinstance(cores, list) and len(set(cores)) >= 3,
        "finalized_ratio_at_least_2_4": isinstance(ratio, (int, float)) and ratio >= 2.4,
        "measurement_at_least_60_seconds": isinstance(measurement, int) and measurement >= 60,
    }
    if not all(assertions.values()):
        raise InvalidEvidence(f"AC10 assertions are non-pass: {assertions}")
    return canonical(
        "AC10",
        {"source": source, "raw": raw_binding, "topology": topology, "binaries": binaries, "runtime": runtime},
        assertions,
    )


def p95(values: list[int]) -> int:
    return sorted(values)[math.ceil(0.95 * len(values)) - 1]


def recompute_capacity(samples: list[dict[str, Any]], limit: int) -> dict[str, Any]:
    if not samples or limit <= 0:
        raise InvalidEvidence("AC13 raw samples/limit are absent")
    fields = (
        "storage_tx_count", "block_weight_ref_time", "block_weight_proof_size", "block_length_bytes",
        "block_weight_ref_time_limit", "block_weight_proof_size_limit", "block_length_limit_bytes",
        "database_bytes", "finality_lag_blocks", "metadata_hash", "block_weights_constant_scale",
        "block_length_constant_scale",
    )
    if any(not isinstance(sample, dict) or any(sample.get(field) is None for field in fields) for sample in samples):
        raise InvalidEvidence("AC13 raw sample operands are incomplete")
    numeric = lambda field: [int(sample[field]) for sample in samples]
    counts = numeric("storage_tx_count")
    ref, proof, length = numeric("block_weight_ref_time"), numeric("block_weight_proof_size"), numeric("block_length_bytes")
    ref_limits = numeric("block_weight_ref_time_limit")
    proof_limits = numeric("block_weight_proof_size_limit")
    length_limits = numeric("block_length_limit_bytes")
    storage_p95 = p95(counts)
    stable = all(len(set(values)) == 1 and values[0] > 0 for values in (ref_limits, proof_limits, length_limits))
    stable = stable and all(
        len({str(sample[field]) for sample in samples}) == 1
        for field in ("metadata_hash", "block_weights_constant_scale", "block_length_constant_scale")
    )
    checks = {
        "headroom_at_least_20_percent": 5 * storage_p95 <= 4 * limit,
        "limit_at_least_ceil_p95_over_80_percent": limit >= (5 * storage_p95 + 3) // 4,
        "no_block_over_90_percent": all(10 * value <= 9 * limit for value in counts),
        "independent_operands_present": True,
        "runtime_resource_limits_stable": stable,
        "p95_ref_time_utilization_at_most_80_percent": 5 * p95(ref) <= 4 * ref_limits[0],
        "p95_proof_size_utilization_at_most_80_percent": 5 * p95(proof) <= 4 * proof_limits[0],
        "p95_block_length_utilization_at_most_80_percent": 5 * p95(length) <= 4 * length_limits[0],
        "no_weight_or_length_sample_over_90_percent": all(
            10 * value <= 9 * resource_limit
            for values, limits in ((ref, ref_limits), (proof, proof_limits), (length, length_limits))
            for value, resource_limit in zip(values, limits)
        ),
        "database_and_finality_operands_valid": all(
            int(sample["database_bytes"]) > 0 and int(sample["finality_lag_blocks"]) >= 0 for sample in samples
        ),
    }
    return {
        "configured_storage_tx_limit": limit,
        "sample_count": len(samples),
        "p95_storage_tx_count": storage_p95,
        "headroom_fraction": 1.0 - storage_p95 / limit,
        "required_limit_ceiling": (5 * storage_p95 + 3) // 4,
        "checks": checks,
        "p6_mixed_workload_slo_claimed": False,
    }


def validate_ac13(config: Any, base: Path, source: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(config, dict):
        raise InvalidEvidence("AC13 configuration is absent")
    raw_binding = validate_file_binding(config.get("raw"), base, "ac13.raw")
    proof_binding = validate_file_binding(config.get("proof_verdict"), base, "ac13.proof_verdict")
    topology = validate_file_binding(config.get("topology"), base, "ac13.topology")
    driver = validate_file_binding(config.get("driver"), base, "ac13.driver")
    binaries = validate_bindings(config.get("binaries"), base, "ac13.binaries")
    evidence_root = resolve_path(config.get("evidence_root"), base, "ac13.evidence_root")
    if not evidence_root.is_dir():
        raise InvalidEvidence("AC13 evidence root is absent")
    runtime = config.get("runtime")
    if not isinstance(runtime, dict) or not isinstance(runtime.get("orbis"), dict):
        raise InvalidEvidence("AC13 Commons runtime binding is absent")
    metadata_hash = runtime["orbis"].get("metadata_hash")
    if not isinstance(metadata_hash, str) or not re.fullmatch(r"0x[0-9a-fA-F]{64}", metadata_hash):
        raise InvalidEvidence("AC13 Orbis metadata hash is malformed")
    verdict = read_json(Path(raw_binding["path"]))
    if not isinstance(verdict, dict) or verdict.get("schema_version") != 1 or verdict.get("status") != "pass":
        raise InvalidEvidence("AC13 production campaign verdict is absent or non-pass")
    if verdict.get("errors") not in ([], None):
        raise InvalidEvidence("AC13 production campaign contains errors")
    if verdict.get("topology_sha256") != topology["sha256"] or verdict.get("driver_sha256") != driver["sha256"]:
        raise InvalidEvidence("AC13 topology/driver hash mismatch")
    expected_binaries = {name: binding["sha256"] for name, binding in binaries.items()}
    if verdict.get("binaries_sha256") != expected_binaries:
        raise InvalidEvidence("AC13 binary hash ledger mismatch")
    cleanup = verdict.get("global_cleanup")
    if not isinstance(cleanup, dict) or cleanup.get("status") != "pass":
        raise InvalidEvidence("AC13 global cleanup is absent or non-pass")
    cases = verdict.get("cases")
    if not isinstance(cases, list) or len(cases) != 10:
        raise InvalidEvidence("AC13 requires exactly ten case results")
    samples: list[dict[str, Any]] = []
    limits: set[int] = set()
    artifact_hashes: list[dict[str, Any]] = []
    case_ids: list[str] = []
    for case in cases:
        if not isinstance(case, dict) or case.get("status") != "pass":
            raise InvalidEvidence("AC13 contains a non-pass case")
        case_id = case.get("case")
        if not isinstance(case_id, str) or case_id in case_ids:
            raise InvalidEvidence("AC13 case identity is absent or duplicated")
        case_ids.append(case_id)
        if case.get("manifest_sha256") != verdict.get("manifest_sha256") or case.get("topology_sha256") != topology["sha256"]:
            raise InvalidEvidence(f"AC13 case source/topology mismatch: {case_id}")
        if case.get("driver_sha256") != driver["sha256"] or case.get("binaries_sha256") != expected_binaries:
            raise InvalidEvidence(f"AC13 case driver/binary mismatch: {case_id}")
        artifacts = case.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            raise InvalidEvidence(f"AC13 raw artifact ledger is absent: {case_id}")
        capacity_path: Path | None = None
        for artifact in artifacts:
            if not isinstance(artifact, dict):
                raise InvalidEvidence(f"AC13 malformed artifact: {case_id}")
            path = safe_child(evidence_root, artifact.get("path"), f"AC13 {case_id}")
            expected = require_hash(artifact.get("sha256"), f"AC13 {case_id}")
            if not path.is_file() or sha256(path) != expected:
                raise InvalidEvidence(f"AC13 artifact hash mismatch: {case_id}/{artifact.get('path')}")
            artifact_hashes.append({"case": case_id, "kind": artifact.get("kind"), "path": artifact.get("path"), "sha256": expected})
            if artifact.get("kind") == "capacity-samples":
                if capacity_path is not None:
                    raise InvalidEvidence(f"AC13 duplicate capacity artifact: {case_id}")
                capacity_path = path
        if capacity_path is None:
            raise InvalidEvidence(f"AC13 capacity artifact is absent: {case_id}")
        payload = read_json(capacity_path)
        if not isinstance(payload, dict) or not isinstance(payload.get("samples"), list) or not isinstance(payload.get("runtime_constants"), dict):
            raise InvalidEvidence(f"AC13 capacity payload is malformed: {case_id}")
        limits.add(int(payload["runtime_constants"].get("max_block_transactions", 0)))
        samples.extend(payload["samples"])
    if len(limits) != 1 or 0 in limits:
        raise InvalidEvidence(f"AC13 configured storage limits are inconsistent: {limits}")
    if {str(sample.get("metadata_hash")) for sample in samples} != {metadata_hash}:
        raise InvalidEvidence("AC13 sample metadata hash differs from runtime binding")
    recomputed = recompute_capacity(samples, limits.pop())
    aggregate = verdict.get("capacity_ac13_p1")
    if not isinstance(aggregate, dict) or aggregate.get("status") != "pass":
        raise InvalidEvidence("AC13 aggregate is absent or non-pass")
    for key, value in recomputed.items():
        if aggregate.get(key) != value:
            raise InvalidEvidence(f"AC13 aggregate differs from raw recomputation: {key}")
    if not all(recomputed["checks"].values()):
        raise InvalidEvidence(f"AC13 raw checks are non-pass: {recomputed['checks']}")
    proof = read_json(Path(proof_binding["path"]))
    if not isinstance(proof, dict):
        raise InvalidEvidence("AC13 proof verifier root is malformed")
    production = proof.get("production_e2e")
    if proof.get("overall_status") != "pass" or not isinstance(production, dict) or production.get("status") != "pass":
        raise InvalidEvidence("AC13 proof verifier is absent or non-pass")
    if production.get("verdict_sha256") != raw_binding["sha256"]:
        raise InvalidEvidence("AC13 proof verifier does not bind the campaign verdict")
    return canonical(
        "AC13-P1",
        {
            "source": source, "raw": raw_binding, "proof_verdict": proof_binding, "topology": topology,
            "driver": driver, "binaries": binaries, "runtime": runtime, "raw_artifacts": artifact_hashes,
        },
        recomputed,
    )


def canonical(criterion: str, inputs: dict[str, Any], assertions: Any) -> dict[str, Any]:
    return {
        "schema": "cord.p1-canonical-verdict.v1",
        "criterion": criterion,
        "status": "pass",
        "verdict": "PASS",
        "pass": True,
        "inputs": inputs,
        "assertions": assertions,
        "performance_claim": False,
        "production_hardware_claim": False,
    }


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def validate_review(review_binding: Any, base: Path, artifacts: dict[str, dict[str, Any]], source: dict[str, Any]) -> dict[str, Any]:
    binding = validate_file_binding(review_binding, base, "review")
    review = read_json(Path(binding["path"]))
    expected = {OUTPUTS[key]: value["sha256"] for key, value in artifacts.items()}
    if not isinstance(review, dict) or review.get("schema") != REVIEW_SCHEMA:
        raise InvalidEvidence("independent review schema is invalid")
    if review.get("status") != "pass" or review.get("independent") is not True:
        raise InvalidEvidence("independent review is absent or non-pass")
    if not isinstance(review.get("reviewer"), str) or not review["reviewer"].strip():
        raise InvalidEvidence("independent reviewer identity is absent")
    if review.get("source_commit") != source["commit"] or review.get("artifacts") != expected:
        raise InvalidEvidence("independent review does not bind source and all materialized artifacts")
    return {**binding, "reviewer": review["reviewer"]}


def atomically_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--review", type=Path, help="hash binding for an independent review JSON")
    args = parser.parse_args()
    input_path = args.inputs.resolve()
    base = input_path.parent
    try:
        inputs = read_json(input_path)
        if not isinstance(inputs, dict) or inputs.get("schema") != INPUT_SCHEMA:
            raise InvalidEvidence("unsupported P1 materializer input schema")
        source = validate_source(inputs.get("source"), base)
        ac7, ac8 = validate_control_raw(inputs.get("ac7_ac8"), base, source)
        ac10 = validate_ac10(inputs.get("ac10"), base, source)
        ac13 = validate_ac13(inputs.get("ac13"), base, source)
        values = {"AC7": ac7, "AC8": ac8, "AC10": ac10, "AC13-P1": ac13}
        serialized = {key: (json.dumps(value, indent=2, sort_keys=True) + "\n").encode() for key, value in values.items()}
        artifacts = {
            key: {"path": OUTPUTS[key], "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
            for key, data in serialized.items()
        }
        review = None
        if args.review:
            review = validate_review(
                {"path": str(args.review.resolve()), "sha256": sha256(args.review.resolve())}, base, artifacts, source
            )
        output = args.output_dir.resolve()
        output.mkdir(parents=True, exist_ok=True)
        for key, data in serialized.items():
            atomically_write(output / OUTPUTS[key], data)
        if review is None:
            print(json.dumps({"status": "prepared", "reason": "independent review required before index", "artifacts": artifacts}, sort_keys=True))
            return 2
        index = {
            "schema": "cord.p1-verification-index.v1",
            "phase": "p1",
            "status": "pass",
            "pass": True,
            "source": source,
            "criteria": artifacts,
            "independent_review": review,
            "excluded": {"AC11": "not materialized by this prerequisite lane", "AC13-P6": "P6 dependency"},
        }
        atomically_write(output / "index.json", (json.dumps(index, indent=2, sort_keys=True) + "\n").encode())
        print(json.dumps({"status": "pass", "index": str(output / "index.json"), "sha256": sha256(output / "index.json")}, sort_keys=True))
        return 0
    except (InvalidEvidence, OSError, TypeError, ValueError) as error:
        print(f"P1 materialization failed closed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
