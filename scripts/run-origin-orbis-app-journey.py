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

"""Run the deterministic P6 Origin/Commons application journey without writing docs evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
P1_PROVIDER_OUTCOMES = ROOT / "target/evidence/p1-provider-typed-outcomes.json"
KNOWN_TRANSPORTS = {
    "messageport": (
        "product-sdk/packages/origin-sdk-host/tests/host-v2-browser-internal.test.ts",
        "authenticated MessagePort negotiation owns the remote offer",
    ),
    "desktop-cbor": (
        "product-sdk/packages/origin-sdk-host/tests/host-v2-internal.test.ts",
        "frozen operation frames round-trip",
    ),
}
REQUIRED_DEVELOPER_CODES = {
    "identity_subject": "success",
    "identity_entitlement": "success",
    "names_subject_link": "success",
    "names_subject_resolve": "success",
    "storage_snapshot": "success",
    "provider_agreement": "success",
    "drive_create": "success",
    "s3_bucket_create": "success",
    "separate_signing": "success",
    "provider_unavailable": "query_failed",
    "provider_recovered": "success",
    "stale_object_write": "version_conflict",
    "object_version_reread": "success",
    "object_write_recovered": "success",
    "host_offline": "host_unavailable",
    "host_reconnected": "success",
    "revoked_identity_grant": "GRANT_REVOKED",
}
REQUIRED_HOST_CODES = {
    "permission_denial": "permission_denied",
    "sponsored_replay": "replay",
    "tampered_sponsored_intent": "proof_invalid",
    "sponsor_budget_exhaustion": "capacity_exceeded",
    "offline": "timeout",
    "offline_reconnect": "success",
    "offline_replay": "conflict",
    "cancelled": "cancelled",
    "cancel_reconnect": "success",
    "cancel_replay": "conflict",
    "version_drift": "unsupported_runtime",
    "revoked_sponsored_denial": "not_authorized",
    "permission_revoked": "permission_revoked",
}


def canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def run_json(path: str) -> dict[str, Any]:
    process = subprocess.run(
        ["node", "--experimental-strip-types", path],
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if process.returncode:
        raise RuntimeError(f"{path} failed: {process.stderr.strip() or process.stdout.strip()}")
    lines = [line for line in process.stdout.splitlines() if line.strip()]
    if not lines:
        raise RuntimeError(f"{path} produced no JSON")
    return json.loads(lines[-1])


def run_transport_test(transport: str) -> dict[str, Any]:
    path, pattern = KNOWN_TRANSPORTS[transport]
    process = subprocess.run(
        [
            "node", "--experimental-strip-types", "--test",
            f"--test-name-pattern={pattern}", path,
        ],
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return {
        "transport": transport,
        "status": "pass" if process.returncode == 0 else "fail",
        "exit_code": process.returncode,
        "stdout_sha256": hashlib.sha256(process.stdout.encode("utf-8")).hexdigest(),
    }


def provider_contract(path: Path | None, providers: int) -> dict[str, Any]:
    if path is None:
        return {
            "source": "deterministic-journey",
            "schema": "cord.p1-provider-typed-outcomes.v1",
            "status": "pass",
            "provider_count": providers,
            "active_provider_count": providers,
            "typed_outcome_count": providers,
            "failure_count": 0,
            "typed_outcomes": [
                {
                    "state": "ACTIVE",
                    "action": "serve",
                    "retryable": False,
                    "redacted_counts": {"eligible_sources": providers},
                }
                for _ in range(providers)
            ],
        }
    value = json.loads(path.read_text(encoding="utf-8"))
    required = {
        "schema", "status", "provider_count", "active_provider_count",
        "typed_outcome_count", "failure_count", "typed_outcomes",
    }
    missing = sorted(required - value.keys())
    if missing:
        raise ValueError(f"provider outcome contract lacks {missing}")
    counts = [
        value.get("provider_count"), value.get("active_provider_count"),
        value.get("typed_outcome_count"), value.get("failure_count"),
    ]
    typed_outcomes = value.get("typed_outcomes")
    if (
        value["schema"] != "cord.p1-provider-typed-outcomes.v1"
        or value["status"] != "pass"
        or any(isinstance(count, bool) or not isinstance(count, int) for count in counts)
        or value["provider_count"] < providers
        or value["active_provider_count"] < providers
        or value["typed_outcome_count"] < providers
        or not isinstance(typed_outcomes, list)
        or value["typed_outcome_count"] != len(typed_outcomes)
        or value["failure_count"] != 0
    ):
        raise ValueError("provider outcome contract does not satisfy the requested active topology")
    for index, typed in enumerate(typed_outcomes):
        if (
            not isinstance(typed, dict)
            or not isinstance(typed.get("state"), str)
            or not isinstance(typed.get("action"), str)
            or not isinstance(typed.get("retryable"), bool)
            or not isinstance(typed.get("redacted_counts"), dict)
            or any(
                isinstance(count, bool) or not isinstance(count, int) or count < 0
                for count in typed.get("redacted_counts", {}).values()
            )
        ):
            raise ValueError(f"provider typed outcome {index} is malformed")
    return {"source": path.as_posix(), **value}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--providers", required=True, type=int)
    parser.add_argument("--transports", required=True)
    parser.add_argument("--repeat", required=True, type=int)
    parser.add_argument("--provider-outcomes", type=Path)
    parser.add_argument("--allow-simulated-provider", action="store_true")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    transports = [item.strip() for item in args.transports.split(",") if item.strip()]
    if args.providers < 3:
        raise SystemExit("AC11 requires at least three providers")
    if args.repeat < 2:
        raise SystemExit("AC11 requires at least two deterministic repetitions")
    if not transports or len(transports) != len(set(transports)):
        raise SystemExit("transports must be unique and non-empty")
    unknown = sorted(set(transports) - KNOWN_TRANSPORTS.keys())
    if unknown:
        raise SystemExit(f"unsupported transport(s): {unknown}")

    provider_path = args.provider_outcomes
    if provider_path is None and P1_PROVIDER_OUTCOMES.is_file():
        provider_path = P1_PROVIDER_OUTCOMES
    if provider_path is None and not args.allow_simulated_provider:
        raise SystemExit(
            "AC11 requires target/evidence/p1-provider-typed-outcomes.json; "
            "--allow-simulated-provider is development-only and cannot satisfy AC11"
        )
    provider = provider_contract(provider_path, args.providers)
    state_failures: list[str] = []
    event_failures: list[str] = []
    frame_failures: list[str] = []
    error_failures: list[str] = []
    metric_failures: list[str] = []
    roots: list[str] = []
    recovery_roots: list[str] = []
    transport_runs: list[dict[str, Any]] = []

    for repetition in range(args.repeat):
        journey = run_json("product-sdk/examples/festival/journey.ts")
        projection = run_json("product-sdk/examples/festival/mobile-app-projection.ts")
        developer = journey.get("developer_flow", {})
        results = developer.get("results", {})
        for step, expected in REQUIRED_DEVELOPER_CODES.items():
            if results.get(step, {}).get("code") != expected:
                state_failures.append(f"run {repetition}: {step}")
        if developer.get("native_services") != [
            "Identity", "Names", "StorageProvider", "Drive", "S3", "TransactionSigning",
        ] or developer.get("separate_signing") is not True:
            state_failures.append(f"run {repetition}: developer surface")

        events = [entry.get("event", {}).get("event") for entry in journey.get("finalized_events", [])]
        if events != ["name_registered", "sponsored_check_in", "attestation_revoked"]:
            event_failures.append(f"run {repetition}: finalized event sequence")
        if projection.get("status") != "PASS" or projection.get("vector_count") != 29:
            frame_failures.append(f"run {repetition}: mobile projection")
        for step, expected in REQUIRED_HOST_CODES.items():
            if journey.get("results", {}).get(step, {}).get("code") != expected:
                error_failures.append(f"run {repetition}: {step}")
        if results.get("provider_unavailable", {}).get("retryable") is not True:
            error_failures.append(f"run {repetition}: provider retryability")
        if results.get("host_offline", {}).get("retryable") is not True:
            error_failures.append(f"run {repetition}: host retryability")

        stable = {
            "developer_flow": developer,
            "results": journey.get("results"),
            "events": journey.get("finalized_events"),
            "projection": projection,
        }
        roots.append(digest(stable))
        recovery_roots.append(digest({
            "policy": developer.get("recovery_policy"),
            "results": {key: results.get(key) for key in (
                "provider_unavailable", "provider_recovered", "stale_object_write",
                "object_version_reread", "object_write_recovered", "host_offline",
                "host_reconnected", "revoked_identity_grant",
            )},
        }))

        for transport in transports:
            trace = run_transport_test(transport)
            trace["repetition"] = repetition
            transport_runs.append(trace)
            if trace["status"] != "pass":
                frame_failures.append(f"run {repetition}: {transport}")

    if provider.get("provider_count") < args.providers:
        metric_failures.append("provider count is below requested topology")
    if provider.get("source") == "deterministic-journey":
        metric_failures.append("simulated provider topology is development-only")
    if len(transport_runs) != len(transports) * args.repeat:
        metric_failures.append("transport run count")

    duplicate_effects = 0
    if len(set(roots)) != 1:
        duplicate_effects += 1
    root_failures = 0 if len(set(roots)) == 1 else len(set(roots))
    recovery_hash_failures = 0 if len(set(recovery_roots)) == 1 else len(set(recovery_roots))
    result = {
        "schema": "cord.ac11-origin-orbis-app-journey.v1",
        "status": "pass" if not any((
            state_failures, event_failures, frame_failures, error_failures, metric_failures,
            duplicate_effects, root_failures, recovery_hash_failures,
        )) else "fail",
        "providers": args.providers,
        "transports": transports,
        "repeat": args.repeat,
        "provider_outcomes": provider,
        "provider_evidence_mode": (
            "development-simulation"
            if provider.get("source") == "deterministic-journey"
            else "p1-generated"
        ),
        "provider_evidence_qualified": provider.get("source") != "deterministic-journey",
        "state_trace_failures": len(state_failures),
        "event_trace_failures": len(event_failures),
        "frame_trace_failures": len(frame_failures),
        "root_failures": root_failures,
        "error_trace_failures": len(error_failures),
        "metric_trace_failures": len(metric_failures),
        "duplicate_effects": duplicate_effects,
        "recovery_hash_failures": recovery_hash_failures,
        "journey_root": roots[0] if roots else None,
        "recovery_root": recovery_roots[0] if recovery_roots else None,
        "transport_runs": transport_runs,
        "failures": {
            "state": state_failures,
            "events": event_failures,
            "frames": frame_failures,
            "errors": error_failures,
            "metrics": metric_failures,
        },
        "production_mobile_rewrite": False,
        "production_readiness_deferred": True,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"{result['status'].upper()} AC11 application journey -> {args.out}")
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
