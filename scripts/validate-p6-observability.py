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

"""Validate executable P6 alert, redaction and developer-recovery contracts."""

from __future__ import annotations

import argparse
import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib


REQUIRED_LABELS = ["chain", "provider_id_hash", "operation", "result", "error_code"]
REQUIRED_METRICS = {
    "cord_storage_checkpoint_age_blocks",
    "cord_storage_replica_lag_chunks",
    "cord_storage_challenge_deadline_blocks",
    "cord_storage_integrity_failures_total",
    "cord_storage_auth_rejections_total",
    "cord_host_inflight_chunks",
    "cord_chain_finality_lag_blocks",
}
REQUIRED_ALERTS = {
    "P6_CHECKPOINT_STALE": ("cord_storage_checkpoint_age_blocks", "gt", 120, 0),
    "P6_REPLICA_LAG": ("cord_storage_replica_lag_chunks", "gt", 256, 0),
    "P6_CHALLENGE_DEADLINE": ("cord_storage_challenge_deadline_blocks", "lt", 5, 0),
    "P6_INTEGRITY_FAILURE": ("cord_storage_integrity_failures_total", "gt", 0, 300),
    "P6_AUTH_REJECTION_SPIKE": ("cord_storage_auth_rejections_total", "gt", 10, 60),
    "P6_FINALITY_LAG": ("cord_chain_finality_lag_blocks", "gt", 20, 0),
}
FORBIDDEN_LABELS = {
    "account", "account_id", "bearer", "bucket_id", "capability", "cid", "content",
    "grant_id", "nonce", "operation_id", "organization_id", "plaintext", "profile", "proof",
    "request_id", "secret", "subject", "token",
}
PROVIDER_REDACTED_COUNTS = {
    "installed_objects", "ready_objects", "quarantined_objects", "duties",
    "initiator_duties", "failover_duties", "promotion_pending_duties", "blocked_duties",
}
GUIDANCE_MARKERS = (
    "preserve the original operation id",
    "cancellation is terminal only after",
    "typed cancelled event",
    "continuity=false",
    "applications must re-enrol",
    "never record a cid",
)
PROVIDER_FAILURE_CODES = (
    "PROVIDER_CHECKPOINT_DUTY_INTAKE_FAILED",
    "PROVIDER_CHECKPOINT_LIFECYCLE_FAILED",
    "PROVIDER_CHECKPOINT_QUORUM_ACTION_FAILED",
    "PROVIDER_CHECKPOINT_QUORUM_JOIN_FAILED",
    "PROVIDER_CHECKPOINT_QUORUM_SELECTION_FAILED",
    "PROVIDER_CHECKPOINT_QUORUM_TICK_FAILED",
    "PROVIDER_MANIFEST_DELETION_FAILED",
    "PROVIDER_HTTP_CONNECTION_FAILED",
    "PROVIDER_REPLICATION_COORDINATOR_FAILED",
    "PROVIDER_REPLICATION_DISCOVERY_REJECTED",
    "PROVIDER_REPLICATION_INTENT_FAILED",
)
CHAIN_SECRET = re.compile(
    r"(?i)(?:deposit_event|Event::).{0,160}(?:subject_master_seed|subject_proof|profile_secret|"
    r"capability_token|bearer_token|resume_token|plaintext|encryption_key|private_key)"
)


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent, delete=False) as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
        temporary = Path(handle.name)
    os.replace(temporary, path)


def evaluate_alerts(contract: dict[str, Any], scenario: dict[str, Any]) -> list[str]:
    metrics = scenario.get("metrics", {})
    flags = scenario.get("flags", {})
    fired: list[str] = []
    for alert in contract.get("alerts", []):
        if alert.get("metric") not in metrics:
            continue
        value = metrics[alert["metric"]]
        operator = alert.get("operator")
        threshold = alert.get("threshold")
        triggered = operator == "gt" and value > threshold or operator == "lt" and value < threshold
        required_flag = alert.get("required_flag", {})
        if triggered and all(flags.get(key) == expected for key, expected in required_flag.items()):
            fired.append(alert["code"])
    return sorted(fired)


def contract_failures(contract: dict[str, Any], runbook: str) -> list[str]:
    failures: list[str] = []
    policy = contract.get("cardinality_policy", {})
    allowed = policy.get("allowed_labels")
    forbidden = set(policy.get("forbidden_labels", []))
    if allowed != REQUIRED_LABELS:
        failures.append("allowed metric labels are not the exact bounded P6 set")
    if not FORBIDDEN_LABELS.issubset(forbidden) or forbidden.intersection(REQUIRED_LABELS):
        failures.append("forbidden metric label inventory is incomplete or contradictory")
    max_values = policy.get("max_values", {})
    if set(max_values) != set(REQUIRED_LABELS) or any(
        not isinstance(value, int) or value < 1 or value > 4096 for value in max_values.values()
    ):
        failures.append("label cardinality bounds are missing or invalid")

    metrics = contract.get("metrics", [])
    names = [metric.get("name") for metric in metrics]
    if set(names) != REQUIRED_METRICS or len(names) != len(set(names)):
        failures.append("required metric inventory is incomplete or duplicated")
    for metric in metrics:
        labels = metric.get("labels")
        if metric.get("kind") not in {"counter", "gauge"} or not isinstance(labels, list):
            failures.append(f"invalid metric shape: {metric.get('name')}")
        elif not set(labels).issubset(REQUIRED_LABELS) or forbidden.intersection(labels):
            failures.append(f"invalid metric labels: {metric.get('name')}")

    alerts = contract.get("alerts", [])
    codes = [alert.get("code") for alert in alerts]
    if set(codes) != set(REQUIRED_ALERTS) or len(codes) != len(set(codes)):
        failures.append("required alert inventory is incomplete or duplicated")
    for alert in alerts:
        code = alert.get("code")
        expected = REQUIRED_ALERTS.get(code)
        actual = (
            alert.get("metric"), alert.get("operator"), alert.get("threshold"),
            alert.get("window_seconds"),
        )
        if expected != actual:
            failures.append(f"alert threshold drift: {code}")
        if alert.get("severity") not in {"warning", "critical"} or not alert.get("action"):
            failures.append(f"alert is not typed and actionable: {code}")
        anchor = str(alert.get("runbook", "")).split("#", 1)[-1]
        if f"## {anchor.replace('-', ' ')}" not in runbook.lower():
            failures.append(f"alert has no runbook action: {code}")
    challenge = next((alert for alert in alerts if alert.get("code") == "P6_CHALLENGE_DEADLINE"), {})
    if challenge.get("required_flag") != {"proof_present": False}:
        failures.append("challenge alert does not require an absent proof")
    return failures


def scenario_failures(contract: dict[str, Any], scenarios: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    rows = scenarios.get("scenarios", [])
    identifiers = [row.get("id") for row in rows]
    if len(rows) < 8 or len(identifiers) != len(set(identifiers)):
        failures.append("scenario inventory is incomplete or duplicated")
    observed: list[str] = []
    for row in rows:
        actual = evaluate_alerts(contract, row)
        expected = sorted(row.get("expected_alerts", []))
        if actual != expected:
            failures.append(f"scenario alert mismatch: {row.get('id')}")
        observed.extend(actual)
    if set(observed) != set(REQUIRED_ALERTS):
        failures.append("synthetic scenarios do not fire every required alert")
    return failures


def source_failures(root: Path) -> tuple[list[str], list[str]]:
    typed: list[str] = []
    redaction: list[str] = []
    provider = root / "origin/orbis/provider-node/src"
    observability = provider / "observability.rs"
    source = observability.read_text(encoding="utf-8")
    for marker in (*PROVIDER_FAILURE_CODES, "provider_failure code={} action={}"):
        if marker not in source:
            typed.append(f"typed provider failure marker missing: {marker}")
    if "pub(crate) fn emit_failure(code: ProviderFailureCode)" not in source:
        typed.append("provider failure emitter accepts unbounded input")
    for path in provider.rglob("*.rs"):
        value = path.read_text(encoding="utf-8", errors="replace")
        if path != observability and "eprintln!" in value:
            redaction.append(f"untyped provider stderr: {path.relative_to(root).as_posix()}")
    event_roots = (root / "origin/orbis/runtime/src", root / "origin/orbis/pallets")
    for event_root in event_roots:
        for path in event_root.rglob("*.rs"):
            for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                if CHAIN_SECRET.search(line):
                    redaction.append(
                        f"chain event secret marker: {path.relative_to(root).as_posix()}:{number}"
                    )
    return typed, redaction


def provider_outcome_failures(outcomes: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    rows = outcomes.get("typed_outcomes", [])
    if (
        outcomes.get("schema") != "cord.p1-provider-typed-outcomes.v1"
        or outcomes.get("status") != "pass"
    ):
        failures.append("P1 provider outcome schema/status is invalid")
    for field in ("provider_count", "active_provider_count", "typed_outcome_count", "failure_count"):
        if not isinstance(outcomes.get(field), int) or outcomes[field] < 0:
            failures.append(f"P1 provider outcome count is invalid: {field}")
    if (
        not isinstance(rows, list)
        or outcomes.get("provider_count") != 3
        or outcomes.get("active_provider_count") != 3
        or outcomes.get("typed_outcome_count") != len(rows)
        or len(rows) < 3
        or outcomes.get("failure_count") != 0
    ):
        failures.append("P1 provider outcomes do not prove three active providers without failures")
        return failures
    for index, row in enumerate(rows):
        state = row.get("state")
        action = row.get("action")
        phase = row.get("phase")
        provider = row.get("provider")
        retryable = row.get("retryable")
        byte_plane_ready = row.get("byte_plane_ready")
        counts = row.get("redacted_counts")
        if (
            not isinstance(state, str)
            or not re.fullmatch(r"[A-Z][A-Z0-9_]{2,63}", state)
            or not isinstance(action, str)
            or not re.fullmatch(r"[a-z][a-z0-9_-]{2,63}", action)
            or not isinstance(phase, str)
            or not re.fullmatch(r"[a-z][a-z0-9_-]{2,31}", phase)
            or not isinstance(provider, str)
            or not re.fullmatch(r"provider-[1-9][0-9]{0,3}", provider)
            or not isinstance(retryable, bool)
            or not isinstance(byte_plane_ready, bool)
            or not isinstance(counts, dict)
            or set(counts) != PROVIDER_REDACTED_COUNTS
            or any(
                key.lower() in FORBIDDEN_LABELS
                or not isinstance(value, int)
                or value < 0
                for key, value in counts.items()
            )
        ):
            failures.append(f"P1 provider typed outcome is unbounded or unredacted: {index}")
    return failures


def evidence_claim_failures(root: Path, registry: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    ac11 = next((gate for gate in registry.get("gate", []) if gate.get("id") == "AC11"), {})
    commands = ac11.get("command", [])
    producer = root / "scripts/run-origin-orbis-app-journey.py"
    blocked_truthful = (
        ac11.get("decidability") == "blocked"
        and not producer.exists()
        and any(
            any("run-origin-orbis-app-journey.py" in argument for argument in command.get("argv", []))
            for command in commands
        )
    )
    mechanical_truthful = ac11.get("decidability") == "mechanical" and producer.is_file()
    if not (blocked_truthful or mechanical_truthful):
        failures.append("AC11 producer/decidability claim is inconsistent")
    legacy = (root / "scripts/validate-p6-journeys.py").read_text(encoding="utf-8")
    if '"p6_acceptance": False' not in legacy or any(
        any("validate-p6-journeys.py" in argument for argument in command.get("argv", []))
        for command in commands
    ):
        failures.append("legacy docs-evidence generator was promoted as AC11 acceptance")
    return failures


def report(
    root: Path,
    contract_path: Path,
    scenarios_path: Path,
    registry_path: Path,
    provider_outcomes_path: Path,
) -> dict[str, Any]:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    scenarios = json.loads(scenarios_path.read_text(encoding="utf-8"))
    registry = tomllib.loads(registry_path.read_text(encoding="utf-8"))
    provider_outcomes = json.loads(provider_outcomes_path.read_text(encoding="utf-8"))
    runbook = (root / "docs/operations/origin-orbis-p6-incidents.md").read_text(encoding="utf-8")
    guide = (root / "docs/sdk/failure-recovery.md").read_text(encoding="utf-8")
    contract_errors = contract_failures(contract, runbook)
    scenarios_errors = scenario_failures(contract, scenarios)
    typed_errors, redaction_errors = source_failures(root)
    guidance_errors = [marker for marker in GUIDANCE_MARKERS if marker not in guide.lower()]
    claim_errors = evidence_claim_failures(root, registry)
    provider_errors = provider_outcome_failures(provider_outcomes)
    status = "pass" if not any(
        (
            contract_errors, scenarios_errors, typed_errors, redaction_errors, guidance_errors,
            claim_errors, provider_errors,
        )
    ) else "blocked"
    return {
        "schema_version": 1,
        "status": status,
        "metric_count": len(contract.get("metrics", [])),
        "alert_count": len(contract.get("alerts", [])),
        "scenario_count": len(scenarios.get("scenarios", [])),
        "metric_contract_failures": len(contract_errors),
        "alert_scenario_failures": len(scenarios_errors),
        "typed_log_failures": len(typed_errors),
        "redaction_findings": len(redaction_errors),
        "developer_guidance_failures": len(guidance_errors),
        "evidence_claim_failures": len(claim_errors),
        "provider_outcome_failures": len(provider_errors),
        "failures": contract_errors + scenarios_errors + typed_errors + redaction_errors
        + guidance_errors + claim_errors + provider_errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, required=True)
    parser.add_argument("--scenarios", type=Path, required=True)
    parser.add_argument("--registry", type=Path, default=Path("docs/specs/evidence-gates-v1.toml"))
    parser.add_argument("--provider-outcomes", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--root", type=Path, default=Path("."))
    args = parser.parse_args()
    root = args.root.resolve()
    contract = args.contract if args.contract.is_absolute() else root / args.contract
    scenarios = args.scenarios if args.scenarios.is_absolute() else root / args.scenarios
    registry = args.registry if args.registry.is_absolute() else root / args.registry
    provider_outcomes = (
        args.provider_outcomes
        if args.provider_outcomes.is_absolute()
        else root / args.provider_outcomes
    )
    result = report(root, contract, scenarios, registry, provider_outcomes)
    atomic_json(args.out, result)
    print(f"{result['status'].upper()} P6 observability")
    return 0 if result["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
