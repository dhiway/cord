#!/usr/bin/env python3
"""Validate P5 host/operator bootstrap and the M7/M9 native-cutover boundary."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/evidence/verification/p5"
RAW_EVIDENCE = EVIDENCE / "raw"
APP_MANIFEST = ROOT / "product-sdk/examples/festival/reference-app.manifest.json"
OPERATOR_MANIFEST = ROOT / "docs/operations/origin-orbis-native-bootstrap.json"
MONITORING_MANIFEST = ROOT / "docs/operations/origin-orbis-monitoring.json"
INCIDENT_RUNBOOK = ROOT / "docs/operations/origin-orbis-native-incidents.md"
ALLOWLIST = ROOT / "docs/evidence/p0-contract-native/native-cutover-allowlist.json"
ALLOWLIST_GENERATOR = ROOT / "scripts/generate-contract-native-census.py"
DESCRIPTOR = ROOT / "product-sdk/packages/descriptors/generated/orbis-descriptor.json"
PROVIDER_MAIN = ROOT / "origin/orbis/provider-node/src/main.rs"
PROVIDER_OUTBOX_MAIN = ROOT / "origin/orbis/provider-node/src/bin/origin-orbis-provider-outbox.rs"
RAW_LOGS: dict[str, list[str]] = {}

MIGRATED_DOMAINS = ("attestation", "identity", "personhood", "individuality", "dotns", "storage")
FORBIDDEN_ALLOWLIST_REASONS = (
    "backward-compatibility", "legacy-data", "future-migration", "speculative-reuse"
)
CONTRACT_DEPENDENCIES = re.compile(
    r"^(ethers|web3|viem|hardhat|foundry|forge|solc|alloy|@openzeppelin/|@polkadot/api-contract|pallet-revive-(?:eth-rpc|fixtures|proc-macro|uapi))",
    re.IGNORECASE,
)
CLEAN_BREAK_FORBIDDEN_SYMBOLS = {
    "P0 compatibility methods retained": "retained P0 compatibility method block",
    "OriginHubConfig": "deleted OriginHubConfig alias",
    "OriginHubExtrinsicParams": "deleted OriginHubExtrinsicParams alias",
    "build_origin_hub_params": "deleted Origin Hub parameter builder",
}
CLEAN_BREAK_FORBIDDEN_SCOPES = {
    "identity:read", "attestation:read", "dotns:resolve", "storage:read",
    "content:fetch", "assets:balance", "transaction:submit",
}


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def repository_files() -> list[Path]:
    paths: list[Path] = []
    excluded_directories = {
        ".git", ".omx", ".idea", ".vscode", "target", "node_modules", "__pycache__",
    }
    for directory, child_directories, filenames in os.walk(ROOT):
        child_directories[:] = sorted(
            name for name in child_directories if name not in excluded_directories
        )
        base = Path(directory)
        paths.extend(base / filename for filename in filenames if filename != ".DS_Store")
    return sorted(path for path in paths if path.is_file())


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def report(path: str, payload: dict) -> None:
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    (EVIDENCE / path).write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_raw_logs_and_index(report_names: list[str], command_mode: str) -> None:
    RAW_EVIDENCE.mkdir(parents=True, exist_ok=True)
    expected_logs = (
        "host-conformance", "operator-executable", "m9-cleanup", "revive-retained-evidence"
    )
    for name in expected_logs:
        content = "\n".join(RAW_LOGS.get(name, ["## not executed\n"])).rstrip() + "\n"
        (RAW_EVIDENCE / f"{name}.log").write_text(content)
    artifact_paths = [EVIDENCE / name for name in report_names]
    artifact_paths.extend(sorted(RAW_EVIDENCE.glob("*.log")))
    index = {
        "schema": "cord.p5-native-launch-evidence-index.v1",
        "command": "python3 scripts/validate-p5-native-launch.py"
        + (" --static-only" if command_mode == "static-only" else ""),
        "mode": command_mode,
        "artifacts": [
            {
                "path": relative(path),
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
            for path in artifact_paths
        ],
    }
    (EVIDENCE / "index.json").write_text(json.dumps(index, indent=2, sort_keys=True) + "\n")


def run(command: list[str]) -> tuple[int, str]:
    process = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT)
    return process.returncode, process.stdout


def record_log(name: str, heading: str, output: str) -> None:
    RAW_LOGS.setdefault(name, []).append(f"## {heading}\n{output.rstrip()}\n")


def validate_feature_completeness(execute: bool) -> tuple[dict, str, list[str]]:
    command = [
        sys.executable,
        "scripts/validate-origin-orbis-feature-completeness.py",
        "--write",
    ]
    code, output = recorded_run("m9-cleanup", command)
    errors: list[str] = []
    if code:
        errors.append("current Origin/Orbis feature-completeness validation failed")
    path = EVIDENCE / "feature-completeness.report.json"
    if not path.is_file():
        errors.append("feature-completeness report was not written")
        return {
            "status": "fail",
            "feature_complete": False,
            "production_ready": False,
            "errors": errors,
        }, "fail", errors
    result = read_json(path)
    if result.get("status") != "pass" or result.get("feature_complete") is not True:
        errors.append("native feature boundary is not complete")
    if result.get("production_ready") is not False:
        errors.append("feature report improperly claims production readiness")
    benchmark_status = "not-executed"
    if execute:
        benchmark_status = "pass"
        commands = [
            ["cargo", "test", "-p", "pallet-coretime-control", "--features",
             "runtime-benchmarks", "--locked"],
            ["cargo", "test", "-p", "pallet-orbis-token", "--features",
             "runtime-benchmarks", "--locked"],
            ["cargo", "test", "-p", "pallet-orbis-feeless", "--features",
             "runtime-benchmarks", "--locked"],
            ["env", "SKIP_WASM_BUILD=1", "cargo", "check", "-p", "origin-foundation-runtime",
             "--features", "runtime-benchmarks", "--locked"],
            ["env", "SKIP_PALLET_REVIVE_FIXTURES=1", "cargo", "check", "-p",
             "origin-commons-runtime", "--features", "runtime-benchmarks", "--locked"],
        ]
        for build_command in commands:
            build_code, _ = recorded_run("m9-cleanup", build_command, timeout=900)
            if build_code:
                benchmark_status = "fail"
                errors.append(f"feature benchmark command failed: {shlex.join(build_command)}")
    return result, benchmark_status, errors


def recorded_run(name: str, command: list[str], *, env: dict[str, str] | None = None,
                 timeout: int | None = None) -> tuple[int, str]:
    process = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
    )
    record_log(name, f"$ {shlex.join(command)} [exit={process.returncode}]", process.stdout)
    return process.returncode, process.stdout


def parse_evidence_command(command: str) -> tuple[list[str], dict[str, str]]:
    tokens = shlex.split(command)
    environment = os.environ.copy()
    while tokens and re.fullmatch(r"[A-Z_][A-Z0-9_]*=.*", tokens[0]):
        key, value = tokens.pop(0).split("=", 1)
        environment[key] = value
    if not tokens:
        raise ValueError("evidence command contains only environment assignments")
    return tokens, environment


def required_provider_cli_flags() -> set[str]:
    return required_cli_flags(PROVIDER_MAIN)


def required_cli_flags(source_path: Path) -> set[str]:
    source = source_path.read_text()
    required: set[str] = set()
    for match in re.finditer(
        r"#\[arg\((?P<options>[^)]*)\)\]\s*(?P<field>[a-z][a-z0-9_]*):\s*(?P<type>[^,\n]+)",
        source,
    ):
        options, field_type = match.group("options"), match.group("type").strip()
        if ("long" not in options or "default_value" in options
                or field_type.startswith("Option<") or field_type == "bool"):
            continue
        required.add("--" + match.group("field").replace("_", "-"))
    if not required:
        raise AssertionError(f"CLI parser yielded no required flags: {relative(source_path)}")
    return required


def http_json(url: str, bearer: str | None = None) -> dict:
    request = urllib.request.Request(url)
    if bearer is not None:
        request.add_header("Authorization", f"Bearer {bearer}")
    with urllib.request.urlopen(request, timeout=2) as response:
        assert response.status == 200, f"{url}: HTTP {response.status}"
        return json.loads(response.read())


def validate_operator_executables() -> tuple[dict, list[str]]:
    errors: list[str] = []
    results: dict = {
        "build_check": "fail",
        "provider_cli": "fail",
        "outbox_cli": "fail",
        "local_provider_readiness": "fail",
        "clean_shutdown": "fail",
        "live_chain_registration": "deferred-to-p6-live-chain",
    }
    for command in (
        ["cargo", "check", "-p", "origin-orbis-provider", "--bins", "--locked"],
        ["cargo", "build", "-p", "origin-orbis-provider", "--bins", "--locked"],
    ):
        code, _ = recorded_run("operator-executable", command)
        if code:
            errors.append(f"operator executable command failed: {shlex.join(command)}")
            return results, errors
    results["build_check"] = "pass"

    provider_binary = ROOT / "target/debug/origin-orbis-provider"
    outbox_binary = ROOT / "target/debug/origin-orbis-provider-outbox"
    binaries = ((provider_binary, PROVIDER_MAIN, "provider_cli"),
                (outbox_binary, PROVIDER_OUTBOX_MAIN, "outbox_cli"))
    for binary, source, result_key in binaries:
        if not binary.is_file():
            errors.append(f"built operator binary is missing: {relative(binary)}")
            continue
        code, output = recorded_run("operator-executable", [str(binary), "--help"])
        flags = required_cli_flags(source)
        if code or any(flag not in output for flag in flags):
            errors.append(f"operator binary help does not expose required CLI contract: {binary.name}")
        else:
            results[result_key] = "pass"

    if errors:
        return results, errors

    with tempfile.TemporaryDirectory(prefix="cord-p5-provider-") as temporary:
        temporary_path = Path(temporary)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        bearer = "p5-local-readiness-token-0000000000000000"
        endpoint = f"http://127.0.0.1:{port}"
        environment = os.environ.copy()
        environment.update({
            "ORBIS_PROVIDER_BEARER_TOKEN": bearer,
            "ORBIS_PROVIDER_SERVICE_SURI": "//Alice",
        })
        command = [
            str(provider_binary),
            "--orbis-rpc", "http://127.0.0.1:9",
            "--provider", "0x" + "45" * 32,
            "--public-endpoint", endpoint,
            "--data-path", str(temporary_path / "data"),
            "--capacity-bytes", "1048576",
            "--listen", f"127.0.0.1:{port}",
            "--checkpoint-seconds", "3600",
        ]
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        responses: dict[str, dict] = {}
        try:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    break
                try:
                    responses["health"] = http_json(endpoint + "/health")
                    break
                except (OSError, urllib.error.URLError, json.JSONDecodeError):
                    time.sleep(0.1)
            if responses.get("health", {}).get("status") != "ok":
                errors.append("temporary provider did not become locally healthy")
            else:
                responses["info"] = http_json(endpoint + "/info")
                responses["stats"] = http_json(endpoint + "/stats")
                responses["replica_sync_status"] = http_json(
                    endpoint + "/replica/sync_status", bearer
                )
                if responses["info"].get("profile", {}).get("provider") != "0x" + "45" * 32:
                    errors.append("temporary provider /info identity mismatch")
                if responses["replica_sync_status"].get("status") != "ready":
                    errors.append("temporary provider replica readiness mismatch")
                results["local_provider_readiness"] = "pass"
            record_log("operator-executable", "local provider HTTP readiness", json.dumps(
                responses, indent=2, sort_keys=True
            ))
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
            try:
                output, _ = process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                output, _ = process.communicate()
                errors.append("temporary provider did not stop after SIGINT")
            record_log(
                "operator-executable",
                f"$ {shlex.join(command)} [exit={process.returncode}; secrets supplied by environment]",
                output or "",
            )
            if process.returncode == 0:
                results["clean_shutdown"] = "pass"
            else:
                errors.append(f"temporary provider exited with {process.returncode}")

        outbox_environment = os.environ.copy()
        outbox_environment["ORBIS_PROVIDER_ACCOUNT_SURI"] = "//Alice"
        outbox_command = [
            str(outbox_binary), "--once", "--orbis-rpc", "ws://127.0.0.1:9",
            "--outbox", str(temporary_path / "data/provider-submissions-v3.jsonl"),
        ]
        try:
            code, output = recorded_run(
                "operator-executable", outbox_command, env=outbox_environment, timeout=15
            )
            if code == 0 or not output.strip():
                errors.append("outbox live-chain boundary did not fail closed without Orbis RPC")
            else:
                results["outbox_without_live_chain"] = "expected-fail-closed"
        except subprocess.TimeoutExpired:
            errors.append("outbox did not fail promptly when live Orbis RPC was unavailable")

    return results, errors


def validate_host_and_operator(execute: bool) -> tuple[dict, dict, list[str]]:
    errors: list[str] = []
    app = read_json(APP_MANIFEST)
    descriptor = read_json(DESCRIPTOR)
    operator = read_json(OPERATOR_MANIFEST)
    monitoring = read_json(MONITORING_MANIFEST)
    methods = {
        (method["capability"], method["method"]): method["finality"]
        for method in descriptor["nativeHostContract"]["methods"]
    }

    if app.get("schema") != "cord.reference-app.v1":
        errors.append("reference app schema mismatch")
    if not app.get("owner"):
        errors.append("reference app owner is missing")
    permissions = app.get("host_permissions", [])
    permission_keys = [(item.get("capability"), item.get("method")) for item in permissions]
    if len(permission_keys) != len(set(permission_keys)) or not permission_keys:
        errors.append("reference app permissions must be non-empty and unique")
    for item in permissions:
        key = (item.get("capability"), item.get("method"))
        if methods.get(key) != item.get("finality"):
            errors.append(f"reference app permission is not descriptor-bound: {key}")
    cutover = app.get("native_cutover", {})
    for field in ("migrated_domain_contract_calls", "deployment_addresses", "generated_contract_bindings"):
        if cutover.get(field) != []:
            errors.append(f"reference app {field} must be empty")
    for field in ("raw_runtime_encoding", "pallet_or_call_indices"):
        if cutover.get(field) is not False:
            errors.append(f"reference app {field} must be false")
    consent = app.get("consent", {})
    if not all(consent.get(field) is True for field in
               ("explicit", "expiring", "revocable", "single_use_nonce")):
        errors.append("reference app consent contract is incomplete")
    transport = app.get("transport", {})
    if not all(transport.get(field) is True for field in
               ("finalized_reads_only", "submit_and_finalize_only", "cancellation_required",
                "runtime_identity_fail_closed")):
        errors.append("reference app transport contract is incomplete")

    network = operator.get("network", {})
    if network != {
        "origin_spec_version": 9901,
        "orbis_spec_version": 29,
        "orbis_transaction_version": 8,
        "orbis_para_id": 1006,
        "product_network_binding": "product-sdk/packages/descriptors/generated/orbis-network-binding.ts",
    }:
        errors.append("operator bootstrap network binding mismatch")
    clean_break = operator.get("clean_break", {})
    if not clean_break or any(value is not False for value in clean_break.values()):
        errors.append("operator bootstrap contains a compatibility or migration path")
    required_steps = {
        "verify-runtime-binding", "register-dotns-registrar", "register-attestation-schema",
        "authorize-storage-account", "register-provider", "start-provider-service",
        "start-provider-finality-consumer", "prove-provider-readiness",
    }
    steps = operator.get("steps", [])
    if {step.get("id") for step in steps} != required_steps:
        errors.append("operator bootstrap step inventory mismatch")
    for step in steps:
        if not all(step.get(field) for field in ("id", "actor", "kind", "surface", "success")):
            errors.append(f"operator step is incomplete: {step.get('id')}")
        if step.get("surface") == "native-sdk":
            key = (step.get("capability"), step.get("method"))
            expected = "finalized" if step.get("kind") == "finalized-read" else step.get("kind")
            if methods.get(key) != expected:
                errors.append(f"operator native method is not descriptor-bound: {key}")
        command = step.get("command", "")
        if command and ("${" not in command or re.search(r"(?i)(//Alice|secret|seed phrase)", command)):
            errors.append(f"operator command must use secret-free placeholders: {step.get('id')}")
    provider_step = next((step for step in steps if step.get("id") == "start-provider-service"), {})
    provider_command = provider_step.get("command", "")
    try:
        provider_tokens = shlex.split(provider_command)
    except ValueError as error:
        errors.append(f"operator provider command is not shell-parseable: {error}")
        provider_tokens = []
    provider_flags = {token for token in provider_tokens if token.startswith("--")}
    required_provider_flags = required_provider_cli_flags()
    if not provider_tokens or provider_tokens[0] != "origin-orbis-provider":
        errors.append("operator provider command uses the wrong executable")
    missing_provider_flags = sorted(required_provider_flags - provider_flags)
    if missing_provider_flags:
        errors.append(f"operator provider command misses required CLI flags: {missing_provider_flags}")
    for flag in sorted(required_provider_flags & provider_flags):
        position = provider_tokens.index(flag)
        if position + 1 >= len(provider_tokens) or not provider_tokens[position + 1].startswith("${"):
            errors.append(f"operator provider command has no placeholder value for required CLI flag: {flag}")
    outbox_step = next(
        (step for step in steps if step.get("id") == "start-provider-finality-consumer"), {}
    )
    outbox_command = outbox_step.get("command", "")
    try:
        outbox_tokens = shlex.split(outbox_command)
    except ValueError as error:
        errors.append(f"operator outbox command is not shell-parseable: {error}")
        outbox_tokens = []
    outbox_flags = {token for token in outbox_tokens if token.startswith("--")}
    required_outbox_flags = required_cli_flags(PROVIDER_OUTBOX_MAIN)
    if not outbox_tokens or outbox_tokens[0] != "origin-orbis-provider-outbox":
        errors.append("operator outbox command uses the wrong executable")
    missing_outbox_flags = sorted(required_outbox_flags - outbox_flags)
    if missing_outbox_flags:
        errors.append(f"operator outbox command misses required CLI flags: {missing_outbox_flags}")
    for flag in sorted(required_outbox_flags & outbox_flags):
        position = outbox_tokens.index(flag)
        if position + 1 >= len(outbox_tokens) or not outbox_tokens[position + 1].startswith("${"):
            errors.append(f"operator outbox command has no placeholder value for required CLI flag: {flag}")

    alerts = monitoring.get("alerts", [])
    required_alerts = {
        "relay-finality-lag", "orbis-finality-lag", "provider-unhealthy",
        "provider-proof-deadline", "provider-finality-backlog", "provider-capacity",
        "cid-integrity", "host-permission-denials", "runtime-binding-drift",
        "native-revocation-failure",
    }
    if {alert.get("id") for alert in alerts} != required_alerts:
        errors.append("monitoring alert inventory mismatch")
    runbook = INCIDENT_RUNBOOK.read_text().lower()
    for alert in alerts:
        if not all(alert.get(field) for field in
                   ("id", "source", "signal", "condition", "severity", "runbook")):
            errors.append(f"monitoring alert is incomplete: {alert.get('id')}")
            continue
        anchor = alert["runbook"].split("#", 1)[-1]
        heading = "## " + anchor.replace("-", " ")
        if heading not in runbook:
            errors.append(f"monitoring alert has no runbook heading: {alert['id']}")
    cardinality = monitoring.get("cardinality_policy", {})
    if not cardinality.get("forbidden_labels") or "request_id" not in cardinality["forbidden_labels"]:
        errors.append("monitoring cardinality policy is incomplete")

    bootstrap_result: dict = {"status": "not-executed"}
    operator_execution: dict = {"status": "not-executed"}
    command_results = []
    if execute:
        code, output = recorded_run("host-conformance", ["npm", "--prefix", "product-sdk", "run", "validate:reference-app"])
        command_results.append({"command": "npm --prefix product-sdk run validate:reference-app",
                                "exit_code": code, "output_sha256": hashlib.sha256(output.encode()).hexdigest()})
        if code:
            errors.append("reference application bootstrap command failed")
        else:
            try:
                bootstrap_result = next(json.loads(line) for line in reversed(output.splitlines())
                                        if line.startswith("{"))
                if bootstrap_result.get("status") != "pass":
                    errors.append("reference application bootstrap did not pass")
                bootstrap_checks = bootstrap_result.get("checks", {})
                for required_check in (
                    "scoped_permission", "host_owned_active_consent", "missing_consent_rejected",
                    "consent_revocation_rejected", "expiry_rejected", "revoke_before_sign",
                    "cancellation_propagated", "migrated_domain_native_only",
                ):
                    if bootstrap_checks.get(required_check) is not True:
                        errors.append(f"reference application bootstrap missed {required_check}")
                routes = bootstrap_checks.get("caller_transport_routes", {})
                if routes.get("finalized_reads", 0) < 1 or routes.get("submissions", 0) < 1:
                    errors.append("reference application caller transport routes did not execute")
            except (StopIteration, json.JSONDecodeError):
                errors.append("reference application bootstrap emitted no machine verdict")
        code, output = recorded_run("host-conformance", ["npm", "--prefix", "product-sdk", "run", "test:host"])
        command_results.append({"command": "npm --prefix product-sdk run test:host",
                                "exit_code": code, "output_sha256": hashlib.sha256(output.encode()).hexdigest()})
        if code:
            errors.append("host conformance tests failed")
        operator_execution, executable_errors = validate_operator_executables()
        errors.extend(f"operator executable: {error}" for error in executable_errors)

    operator_errors = [error for error in errors if error.startswith(("operator", "monitoring"))]
    host_errors = [error for error in errors if error not in operator_errors]
    host_report = {
        "schema": "cord.host-conformance-report.v1",
        "criterion": "AC23",
        "status": "pass" if not host_errors else "fail",
        "inputs": {
            "reference_app_manifest_sha256": sha256(APP_MANIFEST),
            "descriptor_sha256": sha256(DESCRIPTOR),
        },
        "checks": {
            "scoped_permissions": not any("permission" in error for error in host_errors),
            "host_owned_active_expiring_consent": not any("consent" in error for error in host_errors),
            "revoke_cancel_and_caller_transport": bootstrap_result.get("status") == "pass" if execute else "static-only",
            "native_only_reference_app": not any("reference app" in error and "schema" not in error for error in host_errors),
        },
        "commands": command_results,
        "errors": host_errors,
    }
    operator_report = {
        "schema": "cord.operator-bootstrap-report.v1",
        "status": (
            "local-executable-pass-live-chain-deferred" if execute
            else "static-contract-pass-execution-not-run"
        ) if not operator_errors else "fail",
        "checks": {
            "registration_and_provider_steps": len(steps),
            "monitoring_alerts": len(alerts),
            "incident_runbooks": len(alerts) - sum("runbook" in error for error in operator_errors),
            "clean_break": not any("compatibility or migration" in error for error in operator_errors),
            "provider_required_cli_flags": sorted(required_provider_flags),
            "outbox_required_cli_flags": sorted(required_outbox_flags),
            "execution": operator_execution,
        },
        "inputs": {
            "bootstrap_manifest_sha256": sha256(OPERATOR_MANIFEST),
            "monitoring_manifest_sha256": sha256(MONITORING_MANIFEST),
            "incident_runbook_sha256": sha256(INCIDENT_RUNBOOK),
            "provider_main_sha256": sha256(PROVIDER_MAIN),
            "provider_outbox_main_sha256": sha256(PROVIDER_OUTBOX_MAIN),
        },
        "errors": operator_errors,
        "live_chain_scope": {
            "status": "deferred-to-p6-live-chain",
            "not_claimed": [
                "provider registration finality", "provider_by_id finalized match",
                "provider heartbeat finality", "outbox finalized receipt journal",
            ],
        },
    }
    return host_report, operator_report, errors


def validate_cutover(execute: bool) -> tuple[dict, dict, list[str]]:
    errors: list[str] = []
    before = ALLOWLIST.read_bytes() if ALLOWLIST.is_file() else b""
    generator_mode = "--native-cutover-only" if execute else "--check-native-cutover"
    generator_code, generator_output = recorded_run(
        "m9-cleanup", [sys.executable, str(ALLOWLIST_GENERATOR), generator_mode], timeout=30
    )
    after = ALLOWLIST.read_bytes() if ALLOWLIST.is_file() else b""
    generator_reproducibility = {
        "command": f"python3 scripts/generate-contract-native-census.py {generator_mode}",
        "exit_code": generator_code,
        "before_sha256": hashlib.sha256(before).hexdigest(),
        "after_sha256": hashlib.sha256(after).hexdigest(),
        "byte_identical": before == after,
        "status": "pass" if generator_code == 0 and before == after else "fail",
    }
    if generator_code:
        errors.append("native cutover allowlist generator/check failed")
    if before != after:
        errors.append("native cutover allowlist generator is not reproducible (generated bytes differ)")
    files = repository_files()
    allowlist = read_json(ALLOWLIST)
    if allowlist.get("schema_version") != 2:
        errors.append("Revive allowlist must use strict schema version 2")
    if tuple(allowlist.get("migrated_domains", [])) != MIGRATED_DOMAINS:
        errors.append("Revive allowlist migrated-domain inventory mismatch")
    allowed_artifacts: set[str] = set()
    retained_test_commands: dict[str, dict] = {}
    for entry in allowlist.get("entries", []):
        entry_id = entry.get("id", "<missing>")
        if not all(entry.get(field) for field in
                   ("id", "artifacts", "artifact_evidence", "owner", "unrelated_live_use",
                    "dependency_path", "test_evidence", "exclusions")):
            errors.append(f"Revive allowlist entry is incomplete: {entry_id}")
            continue
        justification = f"{entry['unrelated_live_use']} {entry['dependency_path']}".lower()
        if any(reason in justification for reason in FORBIDDEN_ALLOWLIST_REASONS):
            errors.append(f"Revive allowlist has forbidden justification: {entry_id}")
        for artifact in entry["artifacts"]:
            if artifact in allowed_artifacts:
                errors.append(f"Revive allowlist artifact has multiple owners: {artifact}")
            allowed_artifacts.add(artifact)
            if not (ROOT / artifact).is_file():
                errors.append(f"Revive allowlist artifact is missing: {artifact}")
        evidence_by_artifact = {
            item.get("artifact"): item for item in entry.get("artifact_evidence", [])
            if isinstance(item, dict)
        }
        if set(evidence_by_artifact) != set(entry["artifacts"]):
            errors.append(f"Revive allowlist artifact evidence is not one-to-one: {entry_id}")
        for artifact, artifact_evidence in evidence_by_artifact.items():
            evidence_path = ROOT / artifact_evidence.get("evidence_path", "")
            symbols = artifact_evidence.get("symbols", [])
            if not evidence_path.is_file() or not symbols:
                errors.append(f"Revive allowlist live-use evidence is incomplete: {entry_id}:{artifact}")
            elif not all(symbol in evidence_path.read_text(errors="replace") for symbol in symbols):
                errors.append(f"Revive allowlist live-use symbol is missing: {entry_id}:{artifact}")
        evidence = entry["test_evidence"]
        evidence_path = ROOT / evidence.get("path", "")
        if not evidence_path.is_file() or not evidence.get("command") or not evidence.get("symbols"):
            errors.append(f"Revive allowlist test evidence is incomplete: {entry_id}")
        elif not all(symbol in evidence_path.read_text(errors="replace") for symbol in evidence["symbols"]):
            errors.append(f"Revive allowlist test symbol is missing: {entry_id}")
        else:
            retained_test_commands.setdefault(evidence["command"], {
                "entries": [], "path": evidence["path"], "status": "not-executed",
            })["entries"].append(entry_id)

    if execute:
        for command, evidence in retained_test_commands.items():
            try:
                tokens, environment = parse_evidence_command(command)
                code, output = recorded_run(
                    "revive-retained-evidence", tokens, env=environment, timeout=300
                )
                evidence["exit_code"] = code
                evidence["output_sha256"] = hashlib.sha256(output.encode()).hexdigest()
                evidence["status"] = "pass" if code == 0 else "fail"
                if code:
                    errors.append(f"retained Revive test evidence command failed: {command}")
            except (ValueError, subprocess.TimeoutExpired) as error:
                evidence["status"] = "fail"
                errors.append(f"retained Revive test evidence did not execute: {command}: {error}")

    contract_artifacts: list[str] = []
    archive_or_generated_contract_artifacts: list[str] = []
    for path in files:
        rel = relative(path)
        suffix = path.suffix.lower()
        if suffix in {".sol", ".abi"}:
            contract_artifacts.append(rel)
        elif suffix in {".bin", ".bytecode"} and path.parent.name == "build":
            contract_artifacts.append(rel)
        elif suffix in {".zip", ".tar", ".tgz", ".gz"} and re.search(
            r"(?i)(contract|solidity|abi|bytecode|deployment|binding)", rel
        ):
            archive_or_generated_contract_artifacts.append(rel)
        elif suffix == ".json" and re.search(
            r"(?i)/(?:build|generated|artifacts?)/.*(?:abi|bytecode|contract|deployment).*\.json$",
            rel,
        ):
            archive_or_generated_contract_artifacts.append(rel)
    unowned_contract_artifacts = sorted(set(contract_artifacts) - allowed_artifacts)
    unowned_archive_artifacts = sorted(
        set(archive_or_generated_contract_artifacts) - allowed_artifacts
    )
    errors.extend(f"unowned callable contract artifact: {path}" for path in unowned_contract_artifacts)
    errors.extend(f"unowned archived/generated contract artifact: {path}" for path in unowned_archive_artifacts)

    migrated_contract_paths = []
    deployment_paths = []
    contract_config_paths = []
    migrated_contract_content = []
    for path in files:
        rel = relative(path)
        lowered = rel.lower()
        suffix = path.suffix.lower()
        if rel.startswith(("docs/evidence/", "docs/architecture/", "docs/adr/")):
            continue
        has_domain = any(domain in lowered for domain in MIGRATED_DOMAINS)
        if has_domain and re.search(r"/(contracts?|abis?|bindings?|deployments?|addresses?)(/|$)", lowered):
            migrated_contract_paths.append(rel)
        if has_domain and re.search(r"(deploy(?:ment)?|contract)[-_]?(address|proxy)|create3", lowered):
            deployment_paths.append(rel)
        if not rel.startswith(("docs/evidence/", "docs/architecture/", "docs/adr/")):
            is_build_or_config = (
                rel.startswith(".github/")
                or path.name in {"Dockerfile", "docker-compose.yml", "docker-compose.yaml", "package.json"}
                or suffix in {".toml", ".yml", ".yaml"}
            )
            if is_build_or_config and rel not in allowed_artifacts:
                content = path.read_text(errors="replace")
                if re.search(
                    r"(?i)(hardhat(?:\.config|\s+(?:compile|test|run))|forge\s+(?:build|test|script|install)"
                    r"|foundry\.toml|solc\s+--|cargo-contract|deployproxy|contract[_-]?address)",
                    content,
                ):
                    contract_config_paths.append(rel)
            if path.suffix.lower() in {".rs", ".ts", ".tsx", ".js", ".json", ".toml", ".md", ".yml", ".yaml", ".sh"} \
                    and not rel.startswith(("scripts/", "product-sdk/tests/", "origin-rs/tests/")) \
                    and rel not in {
                        "docs/sdk/contract-to-native-map.json",
                        "product-sdk/examples/festival/bootstrap.ts",
                        "product-sdk/examples/festival/reference-app.manifest.json",
                    }:
                content = path.read_text(errors="replace")
                if re.search(
                    r"(?is)(?:attestation|identity|personhood|individuality|dotns|storage).{0,120}"
                    r"(?:contract[_-]?(?:address|proxy)|deployment[_-]?address|generated[_-]?(?:abi|binding)|revive[_-]?(?:call|contract))"
                    r"|(?:contract[_-]?(?:address|proxy)|deployment[_-]?address|generated[_-]?(?:abi|binding)|revive[_-]?(?:call|contract)).{0,120}"
                    r"(?:attestation|identity|personhood|individuality|dotns|storage)",
                    content,
                ):
                    migrated_contract_content.append(rel)
    errors.extend(f"migrated-domain contract path: {path}" for path in migrated_contract_paths)
    errors.extend(f"migrated-domain deployment path: {path}" for path in deployment_paths)
    errors.extend(f"contract-era build/CI/config path: {path}" for path in sorted(set(contract_config_paths)))
    errors.extend(f"migrated-domain contract content: {path}" for path in sorted(set(migrated_contract_content)))

    dependency_survivors = []
    manifest_files = [path for path in files if path.name in {"Cargo.toml", "package.json"}]
    for manifest_path in manifest_files:
        rel = relative(manifest_path)
        if manifest_path.name == "Cargo.toml":
            for match in re.finditer(r"^([A-Za-z0-9_.@/-]+)\s*=", manifest_path.read_text(), re.MULTILINE):
                name = match.group(1)
                if CONTRACT_DEPENDENCIES.match(name):
                    dependency_survivors.append(f"{rel}:{name}")
        else:
            package = read_json(manifest_path)
            for section in ("dependencies", "devDependencies", "optionalDependencies", "peerDependencies"):
                for name in package.get(section, {}):
                    if CONTRACT_DEPENDENCIES.match(name):
                        dependency_survivors.append(f"{rel}:{name}")
    errors.extend(f"stale contract dependency: {item}" for item in dependency_survivors)

    dead_product_paths = []
    for rel in (
        "origin-rs/origin-hub.json", "origin-rs/origin-hub.scale",
        "origin-rs/examples/metadata_dump.rs",
    ):
        if (ROOT / rel).exists():
            dead_product_paths.append(rel)
    product_scan_prefixes = (
        "docs/", "origin-rs/", "product-sdk/", "origin/", "scripts/", ".github/",
        "android/", "ios/", "mobile/",
    )
    immutable_evidence_prefixes = ("docs/evidence/", "docs/architecture/", "docs/adr/")
    stale_symbol_paths: list[str] = []
    stale_scope_paths: list[str] = []
    for path in files:
        rel = relative(path)
        if not rel.startswith(product_scan_prefixes) or rel.startswith(immutable_evidence_prefixes):
            continue
        if rel == "scripts/validate-p5-native-launch.py":
            continue
        if path.suffix.lower() not in {".rs", ".ts", ".tsx", ".js", ".json", ".toml", ".md", ".yml", ".yaml", ".sh"}:
            continue
        content = path.read_text(errors="replace")
        for symbol, purpose in CLEAN_BREAK_FORBIDDEN_SYMBOLS.items():
            if symbol in content:
                stale_symbol_paths.append(f"{rel}:{symbol} ({purpose})")
        if rel.startswith(("origin-rs/", "product-sdk/", "docs/sdk/")):
            for scope in CLEAN_BREAK_FORBIDDEN_SCOPES:
                if re.search(
                    rf"(?<![A-Za-z0-9_]){re.escape(scope)}(?![A-Za-z0-9_])",
                    content,
                ):
                    stale_scope_paths.append(f"{rel}:{scope}")
    forbidden_scope_pairs = {tuple(scope.split(":", 1)) for scope in CLEAN_BREAK_FORBIDDEN_SCOPES}
    descriptor_contract = read_json(DESCRIPTOR)
    for method in descriptor_contract.get("nativeHostContract", {}).get("methods", []):
        pair = (method.get("capability"), method.get("method"))
        if pair in forbidden_scope_pairs:
            stale_scope_paths.append(f"{relative(DESCRIPTOR)}:{pair[0]}:{pair[1]}")
    host_schema_path = ROOT / "docs/sdk/host/host-request.schema.json"
    host_schema = read_json(host_schema_path)
    for branch in host_schema.get("oneOf", []):
        properties = branch.get("properties", {})
        pair = (
            properties.get("capability", {}).get("const"),
            properties.get("method", {}).get("const"),
        )
        if pair in forbidden_scope_pairs:
            stale_scope_paths.append(f"{relative(host_schema_path)}:{pair[0]}:{pair[1]}")
    dead_product_paths.extend(stale_symbol_paths)
    errors.extend(f"dead product path: {item}" for item in dead_product_paths)
    errors.extend(f"deprecated compatibility scope: {item}" for item in stale_scope_paths)

    runtime_sources = "\n".join(
        path.read_text(errors="replace")
        for path in files
        if relative(path).startswith("origin/orbis/runtime/src/") and path.suffix == ".rs"
    )
    migrated_revive_calls = re.findall(
        r"(?im)^.*(?:attestation|identity|personhood|individuality|dotns|storage).*(?:RuntimeCall::Revive|pallet_revive::Call|eth_substrate_call).*$",
        runtime_sources,
    )
    app = read_json(APP_MANIFEST)
    descriptor = read_json(DESCRIPTOR)
    app_migrated_calls = app.get("native_cutover", {}).get("migrated_domain_contract_calls", ["missing"])
    revive_descriptor_methods = [
        method for method in descriptor["nativeHostContract"]["methods"]
        if method["capability"].lower() == "revive" or "revive" in method["method"].lower()
    ]
    genesis_source = (ROOT / "origin/orbis/runtime/src/genesis_config_presets.rs").read_text()
    genesis_deployments = re.findall(r"(?i)(instantiate|upload_code|contract_address|deployment_address)", genesis_source)
    if migrated_revive_calls:
        errors.append("migrated-domain Revive runtime calls remain")
    if app_migrated_calls != []:
        errors.append("reference application exposes migrated-domain contract calls")
    if revive_descriptor_methods:
        errors.append("product descriptor exposes Revive methods")
    if genesis_deployments:
        errors.append("Orbis genesis contains contract deployment behavior")

    deprecated_facades = sorted(set(stale_symbol_paths + stale_scope_paths))
    inventory_by_surface = {
        "docs": sum(relative(path).startswith("docs/") for path in files),
        "rust": sum(path.suffix == ".rs" for path in files),
        "typescript_javascript": sum(path.suffix.lower() in {".ts", ".tsx", ".js"} for path in files),
        "solidity_abi": sum(path.suffix.lower() in {".sol", ".abi"} for path in files),
        "mobile": sum(relative(path).startswith(("android/", "ios/", "mobile/")) for path in files),
        "cargo_package_manifests": len(manifest_files),
        "build_ci_config_generated": sum(
            any(part in {"build", "generated", "config", ".github", "ci"} for part in path.parts)
            or path.suffix.lower() in {".toml", ".yml", ".yaml"}
            for path in files
        ),
    }
    cleanup = {
        "schema": "cord.native-cutover-cleanup-report.v1",
        "criterion": "M9",
        "status": "pass" if not errors else "fail",
        "inventory": {
            "repository_files_scanned": len(files),
            "generator_reproducibility": generator_reproducibility,
            "retained_revive_entries": len(allowlist.get("entries", [])),
            "retained_contract_artifacts": sorted(set(contract_artifacts) & allowed_artifacts),
            "archived_or_generated_contract_artifacts": archive_or_generated_contract_artifacts,
            "contract_build_ci_config_paths": sorted(set(contract_config_paths)),
            "migrated_contract_content_paths": sorted(set(migrated_contract_content)),
            "retained_revive_test_evidence": retained_test_commands,
            "surfaces": inventory_by_surface,
        },
        "unowned_survivors": (
            len(unowned_contract_artifacts) + len(unowned_archive_artifacts)
            + len(migrated_contract_paths) + len(deployment_paths)
            + len(set(contract_config_paths)) + len(set(migrated_contract_content))
            + len(dependency_survivors) + len(deprecated_facades)
        ),
        "migrated_domain_callable_contracts": len(unowned_contract_artifacts) + len(migrated_revive_calls),
        "deprecated_facades": len(deprecated_facades),
        "dead_product_paths": len(dead_product_paths),
        "allowlist_sha256": sha256(ALLOWLIST),
        "errors": errors,
    }
    revive_error_markers = (
        "unowned callable contract artifact", "migrated-domain contract path",
        "migrated-domain deployment path", "migrated-domain revive runtime calls",
        "reference application exposes migrated-domain contract calls",
        "product descriptor exposes revive methods", "orbis genesis contains contract deployment",
    )
    revive_errors = [error for error in errors if any(marker in error.lower() for marker in revive_error_markers)]
    revive = {
        "schema": "cord.revive-boundary-report.v1",
        "criterion": "M7",
        "status": "pass" if not revive_errors else "fail",
        "checks": {
            "migrated_domain_deployments": len(genesis_deployments),
            "migrated_domain_abi_surfaces": len(unowned_contract_artifacts),
            "migrated_domain_runtime_calls": len(migrated_revive_calls),
            "migrated_domain_sdk_or_app_methods": len(revive_descriptor_methods) + len(app_migrated_calls),
            "strict_retained_revive_allowlist_entries": len(allowlist.get("entries", [])),
            "retained_revive_test_evidence": retained_test_commands,
        },
        "errors": revive_errors,
    }
    return revive, cleanup, errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--static-only", action="store_true",
                        help="skip the focused Product SDK executable conformance commands")
    args = parser.parse_args()
    feature, feature_benchmarks, feature_errors = validate_feature_completeness(
        not args.static_only
    )
    host, operator, host_errors = validate_host_and_operator(not args.static_only)
    revive, cleanup, cutover_errors = validate_cutover(not args.static_only)
    record_log("m9-cleanup", "M9 machine inventory", json.dumps(cleanup, indent=2, sort_keys=True))
    report("host-conformance.json", host)
    report("operator-bootstrap.report.json", operator)
    report("revive-boundary.report.json", revive)
    report("native-cutover-cleanup.report.json", cleanup)
    summary = {
        "schema": "cord.p5-native-launch-validation.v1",
        "status": "pass" if not feature_errors and not host_errors and not cutover_errors else "fail",
        "criteria": {"AC23": host["status"], "M7": revive["status"], "M9": cleanup["status"],
                     "feature_completeness": feature["status"],
                     "feature_benchmarks": feature_benchmarks,
                     "operator_bootstrap": operator["status"]},
        "reports": ["feature-completeness.report.json", "host-conformance.json", "operator-bootstrap.report.json",
                    "revive-boundary.report.json", "native-cutover-cleanup.report.json"],
        "errors": feature_errors + host_errors + cutover_errors,
    }
    report("native-launch-verdict.json", summary)
    write_raw_logs_and_index(
        [
            "feature-completeness.report.json", "host-conformance.json", "operator-bootstrap.report.json",
            "revive-boundary.report.json", "native-cutover-cleanup.report.json",
            "native-launch-verdict.json",
        ],
        "static-only" if args.static_only else "executable",
    )
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if summary["status"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
