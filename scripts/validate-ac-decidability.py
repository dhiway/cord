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

"""Decide whether every acceptance criterion has executable evidence semantics."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json, canonical_bytes, sha256_bytes, sha256_file


OPERATORS = {"eq", "zero", "gte", "lte", "subset", "sha256_eq", "matches_schema"}
SHELL_PROGRAMS = {"sh", "bash", "zsh", "fish", "dash", "cmd", "powershell", "pwsh"}
PHASE_OUTPUTS = {
    "P0": "../.omx/evidence/origin-orbis-web3-storage/p0.json",
    "P1": "../.omx/evidence/origin-orbis-web3-storage/p1.json",
    "P2": "../.omx/evidence/origin-orbis-web3-storage/p2.json",
    "P3": "../.omx/evidence/origin-orbis-web3-storage/p3.json",
    "P4_OBJECT": "../.omx/evidence/origin-orbis-web3-storage/p4-object.json",
    "P4_PROVIDER": "../.omx/evidence/origin-orbis-web3-storage/p4-provider.json",
    "P4_DRIVE": "../.omx/evidence/origin-orbis-web3-storage/p4-drive.json",
    "P4_S3": "../.omx/evidence/origin-orbis-web3-storage/p4-s3.json",
    "P4_RUNTIME": "../.omx/evidence/origin-orbis-web3-storage/p4-runtime.json",
    "P5": "../.omx/evidence/origin-orbis-web3-storage/p5.json",
    "P6": "../.omx/evidence/origin-orbis-web3-storage/p6.json",
    "P7": "../.omx/evidence/origin-orbis-web3-storage/p7.json",
}


def hash_path(path: Path) -> str:
    if path.is_file():
        return sha256_file(path)
    if not path.is_dir():
        raise FileNotFoundError(path)
    result = __import__("subprocess").run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=path,
        check=False,
        stdout=__import__("subprocess").PIPE,
        stderr=__import__("subprocess").DEVNULL,
    )
    records = []
    if result.returncode == 0:
        for encoded in result.stdout.split(b"\0"):
            if not encoded:
                continue
            relative = encoded.decode("utf-8", "strict")
            absolute = path / relative
            if absolute.is_file():
                records.append({"path": relative, "sha256": sha256_file(absolute)})
    if not records:
        for absolute in sorted(candidate for candidate in path.rglob("*") if candidate.is_file()):
            relative = absolute.relative_to(path).as_posix()
            records.append({"path": relative, "sha256": sha256_file(absolute)})
    records.sort(key=lambda record: record["path"])
    return sha256_bytes(canonical_bytes(records))


def validate_gate(
    gate: dict[str, Any], root: Path, policy: dict[str, Any] | None = None,
    gates_by_id: dict[str, dict[str, Any]] | None = None,
) -> list[str]:
    failures: list[str] = []
    policy = policy or {}
    gates_by_id = gates_by_id or {}
    gate_id = gate.get("id", "<missing>")
    clauses = gate.get("clauses", [])
    assertions = gate.get("assertion", [])
    covered = [assertion.get("clause") for assertion in assertions]
    if not clauses or len(clauses) != len(set(clauses)):
        failures.append("clauses must be unique and non-empty")
    if set(clauses) != set(covered) or len(covered) != len(set(covered)):
        failures.append("each clause must have exactly one typed assertion")
    for assertion in assertions:
        if assertion.get("operator") not in OPERATORS:
            failures.append(f"invalid assertion operator for {assertion.get('clause')}")
        if not isinstance(assertion.get("json_pointer"), str) or "source" not in assertion:
            failures.append(f"assertion lacks source/pointer for {assertion.get('clause')}")
    commands = gate.get("command", [])
    if not commands:
        failures.append("no report-producing command")
    gate_input_paths = {item.get("path") for item in gate.get("input", [])}
    prior_outputs: set[str] = set()
    for command in commands:
        argv = command.get("argv")
        if not isinstance(argv, list) or not argv or not all(isinstance(item, str) and item for item in argv):
            failures.append("argv is not a literal non-empty string array")
        elif Path(argv[0]).name in SHELL_PROGRAMS:
            failures.append("shell-mediated command is forbidden")
        elif argv[0] not in policy.get("allowed_executables", []):
            failures.append("command executable is not explicitly approved")
        elif Path(argv[0]).name.startswith("python") and (
            len(argv) < 2 or argv[1] not in policy.get("allowed_python_scripts", [])
        ):
            failures.append("Python entry point is not explicitly approved")
        outputs = command.get("outputs")
        read_inputs = command.get("inputs")
        allowed_writes = command.get("allowed_writes")
        allowed_reads = command.get("allowed_reads", [])
        if not isinstance(outputs, list):
            failures.append("command outputs are not explicitly declared")
        elif isinstance(argv, list) and "--out" in argv:
            declared = argv[argv.index("--out") + 1] if argv.index("--out") + 1 < len(argv) else None
            if declared not in outputs:
                failures.append("--out path is absent from declared outputs")
        if not isinstance(read_inputs, list):
            failures.append("command read inputs are not explicitly declared")
        else:
            for value in read_inputs:
                if value not in {"@registry", "@schema", "@repository_snapshot"} and value not in gate_input_paths and value not in prior_outputs:
                    failures.append(f"command read is not frozen or generated: {value}")
            if isinstance(argv, list):
                inferred = {
                    "@registry" if value == "docs/specs/evidence-gates-v1.toml"
                    else "@schema" if value == "docs/specs/evidence-report-v1.schema.json"
                    else value for value in argv[1:]
                    if (root / value).exists() and value not in (outputs or [])
                }
                if not inferred.issubset(set(read_inputs)):
                    failures.append(f"argv read inputs are undeclared: {sorted(inferred - set(read_inputs))}")
        if not isinstance(allowed_writes, list):
            failures.append("command allowed cache writes are not explicitly declared")
        elif any(pattern in {"target/**", "**"} for pattern in allowed_writes):
            failures.append("command uses a blanket write allowlist")
        if not isinstance(allowed_reads, list):
            failures.append("command allowed reads are not explicitly enumerable")
        elif any(pattern in {"target/**", "**"} for pattern in allowed_reads):
            failures.append("command uses a blanket read allowlist")
        if any(
            "${" in item or re.fullmatch(r"<[A-Za-z0-9_|-]+>", item) is not None
            for item in (argv or [])
        ):
            failures.append("argv contains a placeholder")
        prior_outputs.update(outputs or [])
    inputs = gate.get("input", [])
    if not inputs:
        failures.append("no frozen input hash")
    for item in inputs:
        expected = item.get("sha256", "")
        path = root / item.get("path", "")
        if expected == "record" and item.get("input_class") == "generated":
            if not item.get("producer_gate") or not item.get("producer_output"):
                failures.append(f"generated input lacks ancestry: {item.get('path')}")
            producer = gates_by_id.get(item.get("producer_gate"))
            if producer is not None:
                output = item.get("producer_output")
                produced = {
                    value for command in producer.get("command", [])
                    for value in command.get("outputs", [])
                }
                artifacts = {row.get("path") for row in producer.get("artifact", [])}
                if output != item.get("path") or output not in produced or output not in artifacts:
                    failures.append(f"generated input producer relation is invalid: {item.get('path')}")
            continue
        if expected == "record" and item.get("input_class") == "canonical_release":
            attestation = item.get("attestation")
            if not isinstance(attestation, str) or not attestation:
                failures.append(f"canonical release input lacks SHA256SUMS attestation: {item.get('path')}")
            elif not (root / attestation).is_file():
                failures.append(f"canonical release attestation is missing: {attestation}")
            continue
        if re.fullmatch(r"[0-9a-f]{64}", expected) is None:
            failures.append(f"input hash is not frozen: {item.get('path')}")
        elif not path.exists() or hash_path(path) != expected:
            failures.append(f"input hash is stale or missing: {item.get('path')}")
    artifacts = gate.get("artifact", [])
    if not artifacts:
        failures.append("no declared artifact")
    for artifact in artifacts:
        if artifact.get("sha256") not in ("record",) and re.fullmatch(
            r"[0-9a-f]{64}", artifact.get("sha256", "")
        ) is None:
            failures.append(f"artifact lacks hash policy: {artifact.get('path')}")
        if "schema" not in artifact:
            failures.append(f"artifact lacks schema declaration: {artifact.get('path')}")
    declared_output_paths = {
        output for command in commands for output in command.get("outputs", [])
    }
    undeclared_artifacts = {
        artifact.get("path") for artifact in artifacts
        if artifact.get("path") not in declared_output_paths
    }
    if undeclared_artifacts:
        failures.append(f"artifact is not bound to a producing command: {sorted(undeclared_artifacts)}")
    if gate.get("decidability") not in ("mechanical", "blocked"):
        failures.append("decidability must be mechanical or blocked")
    if gate.get("enforce_write_set") is not True or not isinstance(gate.get("write_paths"), list):
        failures.append("gate has no enforced explicit write set")
    elif any(pattern in {"target/**", "**"} for pattern in gate.get("write_paths", [])):
        failures.append("gate uses a blanket write allowlist")
    return [f"{gate_id}: {failure}" for failure in failures]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--registry", required=True, type=Path)
    parser.add_argument("--schema", required=True, type=Path)
    parser.add_argument("--minimum-mechanical", required=True, type=int)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    root = Path.cwd().resolve()
    registry = tomllib.loads(args.registry.read_text(encoding="utf-8"))
    policy = registry.get("execution_policy", {})
    gates = registry.get("gate", [])
    identifiers = [gate.get("id") for gate in gates]
    failures: list[str] = []
    ac_ids = {f"AC{number}" for number in range(1, 15)}
    ac_gates = [gate for gate in gates if gate.get("id") in ac_ids]
    if len(ac_gates) != 14 or {gate.get("id") for gate in ac_gates} != ac_ids:
        failures.append("registry must contain exactly one entry for each AC1..AC14")
    if len(identifiers) != len(set(identifiers)):
        failures.append("gate IDs are not unique")
    outputs = [gate.get("output") for gate in ac_gates]
    expected_outputs = {
        f"../.omx/evidence/origin-orbis-web3-storage/ac{number}.json" for number in range(1, 15)
    }
    if len(outputs) != len(set(outputs)) or set(outputs) != expected_outputs:
        failures.append("AC outputs are not the 14 unique exact report paths")
    for gate_id, output in PHASE_OUTPUTS.items():
        rows = [gate for gate in gates if gate.get("id") == gate_id]
        if len(rows) != 1 or rows[0].get("output") != output:
            failures.append(f"missing exact phase pair: {gate_id} -> {output}")
    gates_by_id = {gate.get("id"): gate for gate in gates}
    for gate in gates:
        failures.extend(validate_gate(gate, root, policy, gates_by_id))

    schema = json.loads(args.schema.read_text(encoding="utf-8"))
    required = set(schema.get("required", []))
    report_fields = {
        "schema_version", "gate_id", "status", "cord_head", "started_at", "finished_at",
        "registry_sha256", "input_hashes", "input_ancestry", "toolchains", "commands", "artifacts",
        "assertions", "blockers", "report_sha256",
        "write_set",
    }
    if not report_fields.issubset(required):
        failures.append("EvidenceReportV1 schema does not cover every required report field")
    schema_operators = set(
        schema.get("properties", {}).get("assertions", {}).get("items", {}).get("properties", {}).get("operator", {}).get("enum", [])
    )
    if schema_operators != OPERATORS:
        failures.append("EvidenceReportV1 assertion operators do not match the runner")
    schema_ac_clauses = schema.get("x-cord-ac-required-assertions", {})
    registry_ac_clauses = {gate["id"]: gate.get("clauses", []) for gate in ac_gates}
    if schema_ac_clauses != registry_ac_clauses:
        failures.append("EvidenceReportV1 AC assertion coverage does not match the registry")
    runner = root / "scripts/run-evidence-gate.py"
    if not runner.is_file():
        failures.append("report-producing runner is missing")

    structural_failures = {failure.split(":", 1)[0] for failure in failures if failure.startswith("AC")}
    mechanical = sum(
        1
        for gate in ac_gates
        if gate.get("decidability") == "mechanical" and gate.get("id") not in structural_failures
    )
    blocked_rows = sorted(
        gate["id"] for gate in ac_gates
        if gate.get("decidability") != "mechanical" or gate.get("id") in structural_failures
    )
    passed = mechanical >= args.minimum_mechanical and not [
        failure for failure in failures if not failure.startswith(tuple(f"AC{number}:" for number in range(1, 15)))
    ]
    report = {
        "schema_version": 1,
        "status": "pass" if passed else "blocked",
        "minimum_mechanical": args.minimum_mechanical,
        "mechanical": mechanical,
        "total": 14,
        "blocked_rows": blocked_rows,
        "registry_sha256": sha256_file(args.registry),
        "schema_sha256": sha256_file(args.schema),
        "runner_sha256": sha256_file(runner) if runner.is_file() else None,
        "failures": failures,
    }
    report["report_sha256"] = sha256_bytes(canonical_bytes(report))
    atomic_write_json(args.out, report)
    print(f"{report['status'].upper()} AC decidability: {mechanical}/14 mechanical")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
