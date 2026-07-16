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

"""Validate the frozen private host/provider v2 protocol kernel."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib

sys.dont_write_bytecode = True


def repository_root() -> Path:
    output = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True
    )
    return Path(output.strip())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def vector_hash_failures(vector: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    vector_id = str(vector.get("id", "unknown"))
    for name, encoded in vector.items():
        if not name.endswith("_cbor_hex") or not isinstance(encoded, str):
            continue
        digest_name = name.removesuffix("_cbor_hex") + "_sha256"
        if digest_name not in vector:
            continue
        try:
            observed = hashlib.sha256(bytes.fromhex(encoded)).hexdigest()
        except ValueError:
            failures.append(f"{vector_id}:{name} is not hexadecimal")
            continue
        if observed != vector[digest_name]:
            failures.append(f"{vector_id}:{digest_name} drifted")
    return failures


def rust_registry_hash(path: Path, pattern: str) -> str | None:
    source = path.read_text(encoding="utf-8")
    matched = re.search(pattern, source, re.DOTALL)
    return matched.group(1) if matched else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    root = repository_root()
    registry_path = args.registry if args.registry.is_absolute() else root / args.registry
    manifest = tomllib.loads(registry_path.read_text(encoding="utf-8"))
    artifacts = {row["role"]: row for row in manifest.get("artifact", [])}
    artifact_failures: dict[str, str] = {}
    for role, row in artifacts.items():
        path = root / row["path"]
        observed = sha256(path) if path.is_file() else None
        if observed != row["sha256"]:
            artifact_failures[role] = f"expected {row['sha256']}, observed {observed}"

    expected_roles = {
        "protocol", "registry", "operations", "errors", "projection", "host-vectors-json",
        "host-vectors-cbor", "protocol-vectors", "provider-registry", "provider-vectors",
        "outbox-state-machine", "outbox-vectors",
    }
    for missing in expected_roles.difference(artifacts):
        artifact_failures[missing] = "artifact declaration is missing"

    registry_sha = str(manifest.get("registry_sha256", ""))
    cddl_path = root / artifacts.get("registry", {}).get("path", "missing")
    host_vectors = load_json(root / artifacts["host-vectors-json"]["path"])
    generated_hash = rust_registry_hash(
        root / "origin-rs/tests/generated/origin_host_registry_v2.rs",
        r'REGISTRY_SHA256:&str="([0-9a-f]{64})"',
    )
    capability_source = (
        root / "origin/orbis/provider-node/src/capability.rs"
    ).read_text(encoding="utf-8")
    capability_match = re.search(
        r"NORMATIVE_REGISTRY_SHA256:\s*\[u8; 32\]\s*=\s*\[(.*?)\];",
        capability_source,
        re.DOTALL,
    )
    capability_hash = None
    if capability_match:
        octets = re.findall(r"0x([0-9a-fA-F]{2})", capability_match.group(1))
        if len(octets) == 32:
            capability_hash = "".join(value.lower() for value in octets)
    registry_hash_matches = (
        cddl_path.is_file()
        and sha256(cddl_path) == registry_sha
        and artifacts.get("registry", {}).get("sha256") == registry_sha
        and host_vectors.get("registry_sha256") == registry_sha
        and generated_hash == registry_sha
        and capability_hash == registry_sha
    )

    errors = load_json(root / artifacts["errors"]["path"])["errors"]
    error_code_failures: list[str] = []
    error_by_code: dict[int, dict[str, Any]] = {}
    error_by_name: dict[str, dict[str, Any]] = {}
    for row in errors:
        if row["code"] in error_by_code:
            error_code_failures.append(f"duplicate error code {row['code']}")
        if row["name"] in error_by_name:
            error_code_failures.append(f"duplicate error name {row['name']}")
        error_by_code[row["code"]] = row
        error_by_name[row["name"]] = row

    expectations = manifest["expectations"]
    for name, code in expectations["host_outbox_errors"].items():
        row = error_by_name.get(name)
        if row is None or row.get("code") != code:
            error_code_failures.append(f"{name} does not map to {code}")

    operations = load_json(root / artifacts["operations"]["path"])
    if operations.get("protocol") != manifest.get("protocol"):
        error_code_failures.append("operation registry protocol does not match manifest")
    for operation in operations.get("operations", []):
        for allowed in operation.get("allowed_errors", []):
            canonical = error_by_code.get(allowed["code"])
            if canonical is None or any(
                canonical.get(field) != allowed.get(field)
                for field in ("name", "retryable")
            ):
                error_code_failures.append(
                    f"operation {operation['code']} has a drifting error {allowed['code']}"
                )

    provider_vectors = load_json(root / artifacts["provider-vectors"]["path"])
    protocol_vectors = load_json(root / artifacts["protocol-vectors"]["path"])
    for vector in provider_vectors.get("vectors", []):
        for negative in vector.get("negative_vectors", []):
            code = negative.get("expected_error_code")
            canonical = error_by_code.get(code)
            if canonical is None or canonical.get("name") != negative.get("expected_error"):
                error_code_failures.append(f"{negative.get('id')} has a drifting error code")

    trace_failures: list[str] = [
        f"{role}: {reason}"
        for role, reason in artifact_failures.items()
        if not role.startswith("outbox-")
    ]
    boundary = manifest.get("boundary", {})
    if boundary != {
        "scope": "p2-private-protocol-kernel",
        "public_host_provider_routes": False,
        "browser_adapter_complete": False,
        "desktop_adapter_complete": False,
        "p3_transport_complete": False,
    }:
        trace_failures.append("manifest exceeds the private P2 protocol-kernel boundary")
    if provider_vectors.get("authority") != "origin-host-registry-v2.cddl":
        trace_failures.append("provider vectors do not name the normative CDDL authority")
    if provider_vectors.get("vector_authority") != "protocol-executable-v2.vectors.json":
        trace_failures.append("provider vectors do not name their executable vector authority")

    provider_by_id = {row["id"]: row for row in provider_vectors.get("vectors", [])}
    protocol_by_id = {row["id"]: row for row in protocol_vectors.get("vectors", [])}
    if set(provider_by_id) != set(expectations["provider_vector_ids"]):
        trace_failures.append("provider vector inventory drifted")
    for vector_id, provider in provider_by_id.items():
        protocol = protocol_by_id.get(vector_id)
        if protocol is None:
            trace_failures.append(f"protocol vector missing {vector_id}")
            continue
        for field in ("type", "canonical_sha256", "effect_count", "event_count"):
            if provider.get(field) != protocol.get(field):
                trace_failures.append(f"{vector_id}:{field} differs across vector authorities")
        trace_failures.extend(vector_hash_failures(provider))

    state_machine = load_json(root / artifacts["outbox-state-machine"]["path"])
    outbox_vectors = load_json(root / artifacts["outbox-vectors"]["path"])
    outbox_failures: list[str] = [
        f"{role}: {reason}"
        for role, reason in artifact_failures.items()
        if role.startswith("outbox-")
    ]
    if state_machine.get("states") != expectations["outbox_states"]:
        outbox_failures.append("outbox state inventory drifted")
    actual_transitions = [
        "|".join((row["from"], row["event"], row["to"], row["effect"]))
        for row in state_machine.get("transitions", [])
    ]
    if actual_transitions != expectations["outbox_transitions"]:
        outbox_failures.append("outbox transition law drifted")
    if state_machine.get("authority") != "host-provider-protocol-v2.md#durable-pre-send-outbox":
        outbox_failures.append("outbox state machine authority drifted")
    if outbox_vectors.get("version") != 2:
        outbox_failures.append("outbox vector version drifted")
    outbox_failures.extend(vector_hash_failures(outbox_vectors.get("base_vector", {})))
    for vector in outbox_vectors.get("crash_vectors", []):
        outbox_failures.extend(vector_hash_failures(vector))
    protocol_outbox = protocol_by_id.get("host-outbox-entry-v1", {})
    base_outbox = outbox_vectors.get("base_vector", {})
    for field in ("canonical_sha256", "effect_count", "event_count"):
        if protocol_outbox.get(field) != base_outbox.get(field):
            outbox_failures.append(f"host outbox base {field} differs from protocol authority")

    loss_boundary_failures: list[str] = []
    expected_loss = {row["id"]: row for row in manifest.get("loss_boundary", [])}
    actual_loss = {row["id"]: row for row in outbox_vectors.get("crash_vectors", [])}
    if len(expected_loss) != len(manifest.get("loss_boundary", [])):
        loss_boundary_failures.append("manifest has duplicate loss boundary IDs")
    if len(actual_loss) != len(outbox_vectors.get("crash_vectors", [])):
        loss_boundary_failures.append("outbox vectors have duplicate loss boundary IDs")
    if set(expected_loss) != set(actual_loss):
        loss_boundary_failures.append("loss boundary inventory drifted")
    for vector_id in set(expected_loss).intersection(actual_loss):
        for field in ("effect_count", "event_count"):
            if expected_loss[vector_id].get(field) != actual_loss[vector_id].get(field):
                loss_boundary_failures.append(f"{vector_id}:{field} drifted")

    duplicate_effects = 0
    for vector in outbox_vectors.get("crash_vectors", []):
        effect_count = vector.get("effect_count")
        if isinstance(effect_count, int):
            duplicate_effects += max(0, effect_count - 1)
        else:
            loss_boundary_failures.append(f"{vector.get('id')}:effect_count is not an integer")

    report = {
        "registry_hash_matches": registry_hash_matches,
        "error_code_failures": len(error_code_failures),
        "trace_failures": len(trace_failures),
        "loss_boundary_failures": len(loss_boundary_failures),
        "duplicate_effects": duplicate_effects,
        "outbox_failures": len(outbox_failures),
        "error_code_failure_details": error_code_failures,
        "trace_failure_details": trace_failures,
        "loss_boundary_failure_details": loss_boundary_failures,
        "outbox_failure_details": outbox_failures,
        "manifest_sha256": sha256(registry_path),
        "boundary": boundary,
    }
    output = args.out if args.out.is_absolute() else root / args.out
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    failed = (
        not registry_hash_matches
        or any(report[field] != 0 for field in (
            "error_code_failures", "trace_failures", "loss_boundary_failures",
            "duplicate_effects", "outbox_failures",
        ))
    )
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
