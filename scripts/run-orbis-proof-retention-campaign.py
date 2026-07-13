#!/usr/bin/env python3
"""Fail-closed orchestrator for the ten-case Orbis proof campaign.

The default mode is offline readiness validation. `--execute` owns the 108xx
topology, but only after an explicit destructive acknowledgement and a case
driver are supplied. Four semantic faults use the separate CORD-only
`origin-orbis-proof-campaign` binary; the production binary remains the topology
default. The driver is invoked once per manifest case as

  DRIVER --case CASE --context CONTEXT.json --output RESULT.json

Every result must carry all manifest assertions as booleans, non-empty trace
artifacts, and the manifest/topology hashes copied from the context. Missing
cases, stale results, missing assertions, `blocked`, and invalid isolation all
make the transaction-storage proof aggregate verdict non-passing. ``--resume``
reuses only prior passing results bound to those exact hashes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import shlex
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs/evidence/p1/storage-proof/proof-retention-cases.json"
OUTPUT = ROOT / "docs/evidence/p1/storage-proof/campaign/campaign-verdict.json"
TOPOLOGY = ROOT / "zombienet/proof-retention-isolated.toml"
RESULT_SCHEMA = ROOT / "docs/evidence/p1/storage-proof/proof-retention-result.schema.json"
GENERATED_SPECS = {
	"origin": ROOT / "docs/evidence/p1/storage-proof/campaign/generated/origin-proof-isolated.json",
	"orbis": ROOT / "docs/evidence/p1/storage-proof/campaign/generated/orbis-proof-isolated.json",
}
PROOF_CAMPAIGN_CASE_MODES = {
	"missing_retained_body_or_proof": "Missing",
	"late_proof": "Stale",
	"invalid_proof": "Invalid",
	"duplicate_proof": "Duplicate",
}


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def rpc_ready(port: int) -> bool:
	payload = json.dumps(
		{"jsonrpc": "2.0", "id": 1, "method": "chain_getBlockHash", "params": [0]}
	).encode()
	request = urllib.request.Request(
		f"http://127.0.0.1:{port}", payload, {"content-type": "application/json"}
	)
	try:
		response = json.load(urllib.request.urlopen(request, timeout=2))
		return bool(response.get("result"))
	except (OSError, urllib.error.URLError, json.JSONDecodeError):
		return False


def wait_for_ports(
	ports: list[int], timeout: int, process: subprocess.Popen[str] | None = None
) -> None:
	deadline = time.monotonic() + timeout
	while time.monotonic() < deadline:
		if process is not None and process.poll() is not None:
			raise RuntimeError(f"Zombienet exited before RPC readiness: {process.returncode}")
		if all(rpc_ready(port) for port in ports):
			return
		time.sleep(1)
	raise RuntimeError(f"RPC ports not ready within {timeout}s: {ports}")


def validate_manifest() -> tuple[dict[str, object], list[str]]:
	manifest = json.loads(MANIFEST.read_text())
	cases = manifest.get("cases", [])
	errors: list[str] = []
	ids = [case.get("id") for case in cases]
	if manifest.get("schema_version") != 2:
		errors.append(f"unsupported case manifest schema {manifest.get('schema_version')}")
	if len(cases) != 10:
		errors.append(f"expected 10 cases, found {len(cases)}")
	if len(set(ids)) != len(ids):
		errors.append("case IDs are not unique")
	for case in cases:
		if not case.get("required_assertions"):
			errors.append(f"{case.get('id')}: required_assertions is empty")
		if not case.get("mechanism"):
			errors.append(f"{case.get('id')}: mechanism is empty")
		if not case.get("fault_kind"):
			errors.append(f"{case.get('id')}: fault_kind is empty")
		if not case.get("driver_contract"):
			errors.append(f"{case.get('id')}: driver_contract is empty")
	return manifest, errors


def static_readiness(manifest: dict[str, object]) -> dict[str, object]:
	required = {
		"zombienet": shutil.which("zombienet"),
		"origin": ROOT / "target/release/origin",
		"origin_orbis": ROOT / "target/release/origin-orbis",
		"proof_campaign_origin_orbis": (
			ROOT / "target/proof-campaign/release/origin-orbis-proof-campaign"
		),
		"bootstrap": ROOT / "target/debug/examples/bootstrap_orbis_core",
		"signed_fault_tool": ROOT / "target/debug/examples/orbis_storage_proof_fault",
		"indexed_db_probe_tool": ROOT / "target/debug/examples/orbis_storage_index_fault",
		"canonical_case_driver": ROOT / "scripts/run-orbis-proof-retention-case.py",
		"result_schema": RESULT_SCHEMA,
		"prepare_specs": ROOT / "scripts/prepare-isolated-proof-specs.sh",
		"isolation_preflight": ROOT / "scripts/verify-proof-network-isolation.py",
	}
	artifacts = {
		name: bool(value and (not isinstance(value, Path) or value.exists()))
		for name, value in required.items()
	}
	blocked_cases = [
		case["id"]
		for case in manifest["cases"]  # type: ignore[index]
		if not case.get("local_mechanism_ready")
	]
	return {
		"artifacts": artifacts,
		"blocked_without_case_driver": blocked_cases,
		"status": "driver-required" if all(artifacts.values()) else "fail",
	}


def validate_proof_campaign_fault(case_id: str, fault: dict[str, object], mode: str) -> list[str]:
	"""Independently re-check node receipt semantics; do not trust driver booleans."""
	errors: list[str] = []
	target = fault.get("target_block")
	precondition = fault.get("canonical_precondition")
	receipt = fault.get("node_receipt")
	if not isinstance(receipt, dict):
		return [f"{case_id}: structured proof campaign node receipt is absent"]
	action = {
		"Missing": "provider-omitted",
		"Invalid": "provider-invalid",
		"Stale": "provider-stale",
		"Duplicate": "duplicate-second-push",
	}[mode]
	rejection = "FinalizationError" if mode == "Missing" else "BadMandatory"
	expected = {
		"schema_version": 1,
		"event": "proof-campaign-fault-attempt",
		"mode": mode,
		"chain_id": fault.get("chain_id"),
		"genesis_hash": fault.get("genesis_hash"),
		"target_block": target,
		"parent_block": target - 1 if isinstance(target, int) else None,
		"canonical_proof_present": True,
		"action": action,
		"rejection": rejection,
		"proposal_returned": False,
		"one_shot": True,
	}
	for field, value in expected.items():
		if receipt.get(field) != value:
			errors.append(f"{case_id}: node receipt {field} mismatch")
	if isinstance(precondition, dict):
		if receipt.get("parent_hash") != precondition.get("parent_hash"):
			errors.append(f"{case_id}: node receipt parent is not canonical")
		if receipt.get("canonical_proof_sha256") != precondition.get("proof_sha256"):
			errors.append(f"{case_id}: node receipt canonical proof hash drift")
	canonical = receipt.get("canonical_proof_sha256")
	injected = receipt.get("injected_proof_sha256")
	if not isinstance(canonical, str) or len(canonical) != 64:
		errors.append(f"{case_id}: canonical proof hash is malformed")
	if mode == "Missing" and injected is not None:
		errors.append(f"{case_id}: Missing receipt claims an injected proof")
	elif mode in ("Invalid", "Stale") and (
		not isinstance(injected, str) or len(injected) != 64 or injected == canonical
	):
		errors.append(f"{case_id}: {mode} proof is absent or unchanged")
	elif mode == "Duplicate" and injected != canonical:
		errors.append(f"{case_id}: Duplicate is not bound to the canonical proof")
	command = fault.get("armed_command")
	if not isinstance(command, list) or not command or not all(isinstance(item, str) for item in command):
		errors.append(f"{case_id}: exact campaign command is absent")
	else:
		para = command[: command.index("--")] if "--" in command else command
		if Path(para[0]).name != "origin-orbis-proof-campaign":
			errors.append(f"{case_id}: fault did not use the separate campaign binary")
		for option, value in (
			("--proof-campaign-mode", mode),
			("--proof-campaign-target-block", str(target)),
			("--proof-campaign-expected-chain-id", "orbis-proof-isolated"),
			("--proof-campaign-expected-genesis-hash", str(fault.get("genesis_hash"))),
		):
			positions = [index for index, item in enumerate(para) if item == option]
			if len(positions) != 1 or positions[0] + 1 >= len(para) or para[positions[0] + 1] != value:
				errors.append(f"{case_id}: exact campaign command lacks {option}={value}")
		if para.count("--unsafe-proof-campaign-acknowledge-disposable") != 1:
			errors.append(f"{case_id}: disposable campaign acknowledgement is absent/duplicated")
	return errors


def validate_case_result(
	case: dict[str, object],
	result: dict[str, object],
	*,
	manifest_sha256: str,
	topology_sha256: str,
	driver_sha256: str,
	result_schema_sha256: str,
	chain_specs_sha256: dict[str, str],
	binaries_sha256: dict[str, str],
	evidence_dir: Path,
) -> list[str]:
	errors = []
	if result.get("schema_version") != 3:
		errors.append(f"{case['id']}: unsupported result schema {result.get('schema_version')}")
	if result.get("case") != case["id"]:
		errors.append(f"case mismatch: expected {case['id']}, got {result.get('case')}")
	if result.get("status") != "pass":
		errors.append(f"{case['id']}: non-pass status {result.get('status')}")
	if result.get("manifest_sha256") != manifest_sha256:
		errors.append(f"{case['id']}: result is not bound to this case manifest")
	if result.get("topology_sha256") != topology_sha256:
		errors.append(f"{case['id']}: result is not bound to this isolated topology")
	if result.get("driver_sha256") != driver_sha256:
		errors.append(f"{case['id']}: result is not bound to this exact case driver")
	if result.get("result_schema_sha256") != result_schema_sha256:
		errors.append(f"{case['id']}: result is not bound to this evidence schema")
	if result.get("chain_specs_sha256") != chain_specs_sha256:
		errors.append(f"{case['id']}: result is not bound to the isolated chain specs")
	if result.get("binaries_sha256") != binaries_sha256:
		errors.append(f"{case['id']}: result is not bound to the exact campaign binaries")
	assertions = result.get("assertions", {})
	if not isinstance(assertions, dict):
		return errors + [f"{case['id']}: assertions must be an object"]
	for assertion in case["required_assertions"]:  # type: ignore[index]
		if assertions.get(assertion) is not True:
			errors.append(f"{case['id']}: assertion not true: {assertion}")

	signed = result.get("signed_receipts")
	if not isinstance(signed, list):
		errors.append(f"{case['id']}: signed_receipts must be a list")
	else:
		actions = set()
		for receipt in signed:
			if not isinstance(receipt, dict):
				errors.append(f"{case['id']}: signed receipt is not an object")
				continue
			actions.add(receipt.get("action"))
			for field in ("extrinsic_hash", "finalized_block_hash"):
				value = receipt.get(field)
				if not isinstance(value, str) or len(value) != 66 or not value.startswith("0x"):
					errors.append(f"{case['id']}: signed receipt lacks a 32-byte {field}")
			if not isinstance(receipt.get("signer"), str) or not receipt.get("signer"):
				errors.append(f"{case['id']}: signed receipt lacks signer")
			if receipt.get("status") != "finalized":
				errors.append(f"{case['id']}: signed receipt is not finalized")
		if not {"setup", "store"}.issubset(actions):
			errors.append(f"{case['id']}: finalized signed setup/store receipts are required")

	fault = result.get("fault")
	if not isinstance(fault, dict):
		errors.append(f"{case['id']}: fault receipt must be an object")
	else:
		if fault.get("kind") != case.get("fault_kind"):
			errors.append(
				f"{case['id']}: fault kind {fault.get('kind')} does not match "
				f"{case.get('fault_kind')}"
			)
		target = fault.get("target_block")
		observed = fault.get("observed_at_block")
		if not isinstance(target, int) or target <= 0:
			errors.append(f"{case['id']}: exact positive fault target block is absent")
		if not isinstance(observed, int) or observed <= 0:
			errors.append(f"{case['id']}: observed fault block is absent")
		if target != observed:
			errors.append(f"{case['id']}: fault did not occur at its exact target block")
		if fault.get("reverted") is not True:
			errors.append(f"{case['id']}: fault was not restored/reverted")
		mode = PROOF_CAMPAIGN_CASE_MODES.get(str(case["id"]))
		if mode is not None:
			if fault.get("mode") != mode:
				errors.append(f"{case['id']}: proof campaign mode is not {mode}")
			if fault.get("chain_id") != "orbis-proof-isolated":
				errors.append(f"{case['id']}: proof campaign chain allowlist is absent")
			genesis = fault.get("genesis_hash")
			if not isinstance(genesis, str) or len(genesis) != 66 or not genesis.startswith("0x"):
				errors.append(f"{case['id']}: exact proof campaign genesis hash is absent")
			precondition = fault.get("canonical_precondition")
			if not isinstance(precondition, dict) or precondition.get("proof_present") is not True:
				errors.append(f"{case['id']}: canonical Some(proof) precondition is absent")
			window = fault.get("fault_window")
			if not isinstance(window, dict) or not all(
				window.get(field) is True for field in ("target_not_imported", "target_not_finalized")
			):
				errors.append(f"{case['id']}: target no-import/finality evidence is absent")
			recovery = fault.get("recovery")
			if not isinstance(recovery, dict) or not recovery.get("target_block_hash"):
				errors.append(f"{case['id']}: canonical recovery evidence is absent")
			errors.extend(validate_proof_campaign_fault(str(case["id"]), fault, mode))

	cleanup = result.get("cleanup")
	if not isinstance(cleanup, dict) or cleanup.get("status") != "pass":
		errors.append(f"{case['id']}: cleanup receipt is absent or non-pass")
	elif not all(
		cleanup.get(field) is True
		for field in ("fault_state_restored", "temporary_processes_stopped")
	):
		errors.append(f"{case['id']}: cleanup did not restore state and stop temporary processes")

	artifacts = result.get("artifacts")
	required_kinds = {
		"signed-receipt",
		"rpc-trace",
		"node-log",
		"fault-receipt",
		"cleanup-receipt",
		"capacity-samples",
		"proof-campaign-node-receipts",
	}
	seen_kinds: set[str] = set()
	verified_artifacts: dict[str, Path] = {}
	if not isinstance(artifacts, list) or not artifacts:
		errors.append(f"{case['id']}: no hashed raw artifacts")
	else:
		root = evidence_dir.resolve()
		for artifact in artifacts:
			if not isinstance(artifact, dict):
				errors.append(f"{case['id']}: artifact entry is not an object")
				continue
			kind = artifact.get("kind")
			relative = artifact.get("path")
			expected_hash = artifact.get("sha256")
			if isinstance(kind, str):
				seen_kinds.add(kind)
			if not isinstance(relative, str) or not relative:
				errors.append(f"{case['id']}: artifact path is absent")
				continue
			path = (evidence_dir / relative).resolve()
			if path != root and root not in path.parents:
				errors.append(f"{case['id']}: artifact escapes evidence directory: {relative}")
				continue
			if not path.is_file() or path.stat().st_size <= 0:
				errors.append(f"{case['id']}: artifact is absent/empty: {relative}")
				continue
			if not isinstance(expected_hash, str) or sha256(path) != expected_hash:
				errors.append(f"{case['id']}: artifact hash mismatch: {relative}")
				continue
			if isinstance(kind, str):
				verified_artifacts[kind] = path
	missing_kinds = required_kinds - seen_kinds
	if missing_kinds:
		errors.append(f"{case['id']}: missing raw artifact kinds: {sorted(missing_kinds)}")

	# Bind structured claims back to the hashed raw receipts, not merely to true booleans.
	try:
		fault_raw = json.loads(verified_artifacts["fault-receipt"].read_text())
		cleanup_raw = json.loads(verified_artifacts["cleanup-receipt"].read_text())
		capacity_raw = json.loads(verified_artifacts["capacity-samples"].read_text())
		signed_raw = json.loads(verified_artifacts["signed-receipt"].read_text())
		campaign_raw = json.loads(
			verified_artifacts["proof-campaign-node-receipts"].read_text()
		)
		if fault_raw != fault:
			errors.append(f"{case['id']}: fault result differs from hashed fault receipt")
		if cleanup_raw != cleanup:
			errors.append(f"{case['id']}: cleanup result differs from hashed cleanup receipt")
		if capacity_raw.get("ac13_p1") != result.get("capacity_ac13_p1"):
			errors.append(f"{case['id']}: capacity result differs from hashed raw verdict")
		if not isinstance(signed_raw, list):
			raise ValueError("signed raw receipt is not a list")
		raw_by_label = {
			entry.get("label"): json.loads(entry["stdout"])
			for entry in signed_raw
			if isinstance(entry, dict)
			and isinstance(entry.get("label"), str)
			and isinstance(entry.get("stdout"), str)
		}
		for receipt in signed if isinstance(signed, list) else []:
			if not isinstance(receipt, dict) or receipt.get("action") not in ("setup", "store"):
				continue
			raw = raw_by_label.get(receipt["action"])
			if not isinstance(raw, dict) or (
				raw.get("status") != "accepted"
				or raw.get("extrinsic_hash") != receipt.get("extrinsic_hash")
				or raw.get("block_hash") != receipt.get("finalized_block_hash")
			):
				errors.append(
					f"{case['id']}: {receipt.get('action')} result differs from hashed signed receipt"
				)
		mode = PROOF_CAMPAIGN_CASE_MODES.get(str(case["id"]))
		if not isinstance(campaign_raw, list):
			errors.append(f"{case['id']}: hashed campaign receipt artifact is not a list")
		elif mode is not None:
			if len(campaign_raw) != 1 or campaign_raw[0] != fault.get("node_receipt"):
				errors.append(f"{case['id']}: node receipt differs from hashed campaign evidence")
		elif campaign_raw:
			errors.append(f"{case['id']}: non-campaign case contains a campaign node receipt")
		capacity_samples = capacity_raw.get("samples")
		if not isinstance(capacity_samples, list) or not capacity_samples:
			errors.append(f"{case['id']}: hashed raw capacity sample list is absent")
		else:
			for sample in capacity_samples:
				if not isinstance(sample, dict) or not isinstance(sample.get("block_number"), int):
					errors.append(f"{case['id']}: malformed raw capacity sample")
					continue
				raw = raw_by_label.get(f"measure-block-{sample['block_number']}")
				if not isinstance(raw, dict) or any(
					raw_value != sample.get(sample_field)
					for raw_value, sample_field in (
						(raw.get("block_hash"), "block_hash"),
						(raw.get("metadata_hash"), "metadata_hash"),
						(raw.get("block_weight_scale"), "block_weight_scale"),
						(raw.get("block_weights_constant_scale"), "block_weights_constant_scale"),
						(raw.get("block_length_constant_scale"), "block_length_constant_scale"),
						(raw.get("block_encoded_bytes"), "block_length_bytes"),
						(raw.get("max_block_length_bytes"), "block_length_limit_bytes"),
					)
				):
					errors.append(
						f"{case['id']}: block {sample.get('block_number')} sample differs from hashed metadata measurement"
					)
					continue
				total = raw.get("total_consumed")
				maximum = raw.get("max_block")
				if (
					not isinstance(total, dict)
					or not isinstance(maximum, dict)
					or not isinstance(sample.get("block_weight_ratio_operands"), dict)
					or not isinstance(sample.get("block_weight_by_class"), dict)
					or not isinstance(sample.get("block_length_ratio_operands"), dict)
					or not isinstance(sample.get("block_length_limits_by_class"), list)
					or not isinstance(sample.get("corrected_extrinsic_event_count"), int)
					or not isinstance(sample.get("extrinsic_count"), int)
					or (
					total.get("ref_time") != sample.get("block_weight_ref_time")
					or total.get("proof_size") != sample.get("block_weight_proof_size")
					or maximum.get("ref_time") != sample.get("block_weight_ref_time_limit")
					or maximum.get("proof_size") != sample.get("block_weight_proof_size_limit")
					or raw.get("total_max_block_ratio") != sample.get("block_weight_ratio_operands")
					or {name: raw.get(name) for name in ("normal", "operational", "mandatory")}
					!= sample.get("block_weight_by_class")
					or raw.get("block_length_ratio") != sample.get("block_length_ratio_operands")
					or raw.get("block_length_limits") != sample.get("block_length_limits_by_class")
					or raw.get("corrected_extrinsic_event_count")
					!= sample.get("corrected_extrinsic_event_count")
					or raw.get("extrinsic_count") != sample.get("extrinsic_count")
					)
				):
					errors.append(
						f"{case['id']}: block {sample['block_number']} weight sample differs from hashed metadata measurement"
					)
	except (KeyError, OSError, TypeError, ValueError, json.JSONDecodeError) as error:
		errors.append(f"{case['id']}: cannot bind hashed receipt contents: {error}")
	return errors


def terminate(process: subprocess.Popen[str]) -> None:
	if process.poll() is not None:
		return
	try:
		os.killpg(process.pid, signal.SIGINT)
	except ProcessLookupError:
		return
	try:
		process.wait(timeout=20)
	except subprocess.TimeoutExpired:
		os.killpg(process.pid, signal.SIGTERM)
		try:
			process.wait(timeout=10)
		except subprocess.TimeoutExpired:
			os.killpg(process.pid, signal.SIGKILL)
			process.wait()


def terminate_owned_processes(registry: Path) -> list[str]:
	"""Terminate only exact command-hash/PID/PGID tuples registered by the case driver."""
	errors: list[str] = []
	if not registry.exists():
		return errors
	try:
		rows = json.loads(registry.read_text())
	except (OSError, json.JSONDecodeError) as error:
		return [f"invalid owned-process registry: {error}"]
	if not isinstance(rows, list):
		return ["owned-process registry must be a list"]
	for row in rows:
		if not isinstance(row, dict) or row.get("active") is not True:
			continue
		pid = row.get("pid")
		pgid = row.get("pgid")
		command = row.get("command")
		if not isinstance(pid, int) or not isinstance(pgid, int) or pid <= 1 or pgid <= 1:
			errors.append(f"unsafe owned-process identity: {row}")
			continue
		if not isinstance(command, list) or not all(isinstance(value, str) for value in command):
			errors.append(f"invalid owned-process command for PID {pid}")
			continue
		expected = hashlib.sha256("\0".join(command).encode()).hexdigest()
		if row.get("command_sha256") != expected:
			errors.append(f"owned-process command ledger mismatch for PID {pid}")
			continue
		try:
			live = subprocess.check_output(
				["ps", "-ww", "-p", str(pid), "-o", "command="], text=True
			).strip()
		except subprocess.CalledProcessError:
			row["active"] = False
			continue
		if shlex.split(live) != command or os.getpgid(pid) != pgid:
			errors.append(f"PID/PGID/command reuse guard refused cleanup for PID {pid}")
			continue
		try:
			os.killpg(pgid, signal.SIGTERM)
			deadline = time.monotonic() + 20
			while time.monotonic() < deadline:
				try:
					os.kill(pid, 0)
				except ProcessLookupError:
					break
				time.sleep(0.25)
			else:
				os.killpg(pgid, signal.SIGKILL)
		except ProcessLookupError:
			pass
		row["active"] = False
	registry.write_text(json.dumps(rows, indent=2) + "\n")
	return errors


def topology_node_identity(port: int) -> dict[str, object]:
	"""Bind destructive case execution to the exact Zombienet-owned Orbis base path."""
	listeners = subprocess.run(
		["lsof", "-nP", "-t", f"-iTCP:{port}", "-sTCP:LISTEN"],
		text=True,
		capture_output=True,
		check=False,
	)
	pids = sorted({int(value) for value in listeners.stdout.splitlines() if value.isdigit()})
	if listeners.returncode or len(pids) != 1 or pids[0] <= 1:
		raise RuntimeError(f"expected one safe Zombienet listener on {port}, found {pids}")
	command_text = subprocess.check_output(
		["ps", "-ww", "-p", str(pids[0]), "-o", "command="], text=True
	).strip()
	command = shlex.split(command_text)
	if not command or "origin-orbis" not in Path(command[0]).name:
		raise RuntimeError(f"listener {port} is not origin-orbis: {command_text}")
	def required_option(option: str) -> str:
		for index, value in enumerate(command):
			if value == option and index + 1 < len(command):
				return command[index + 1]
			if value.startswith(option + "="):
				return value.split("=", 1)[1]
		raise RuntimeError(f"listener {port} lacks {option}")
	if int(required_option("--rpc-port")) != port:
		raise RuntimeError(f"listener {port} command has another RPC port")
	base_path = Path(required_option("--base-path")).resolve()
	if not base_path.is_dir():
		raise RuntimeError(f"listener {port} base path is absent: {base_path}")
	return {
		"initial_pid": pids[0],
		"base_path": str(base_path),
		"name": required_option("--name"),
		"command_sha256": hashlib.sha256("\0".join(command).encode()).hexdigest(),
	}


def aggregate_capacity(case_results: list[dict[str, object]], evidence: Path) -> dict[str, object]:
	samples: list[dict[str, object]] = []
	limits: set[int] = set()
	for result in case_results:
		artifacts = result.get("artifacts", [])
		entry = next(
			(
				value
				for value in artifacts
				if isinstance(value, dict) and value.get("kind") == "capacity-samples"
			),
			None,
		)
		if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
			return {"status": "fail", "reason": "a case lacks raw capacity samples"}
		path = (evidence / entry["path"]).resolve()
		root = evidence.resolve()
		if path != root and root not in path.parents:
			return {"status": "fail", "reason": "capacity artifact escapes evidence root"}
		payload = json.loads(path.read_text())
		case_samples = payload.get("samples")
		constants = payload.get("runtime_constants")
		if not isinstance(case_samples, list) or not isinstance(constants, dict):
			return {"status": "fail", "reason": "malformed capacity sample payload"}
		samples.extend(value for value in case_samples if isinstance(value, dict))
		limits.add(int(constants.get("max_block_transactions", 0)))
	if len(limits) != 1 or 0 in limits or not samples:
		return {"status": "fail", "reason": f"inconsistent/absent configured limits: {limits}"}
	limit = limits.pop()
	counts = sorted(int(sample["storage_tx_count"]) for sample in samples)
	p95 = counts[math.ceil(0.95 * len(counts)) - 1]
	headroom = 1.0 - p95 / limit
	required_limit = (5 * p95 + 3) // 4
	independent_fields = (
		"block_weight_ref_time",
		"block_weight_proof_size",
		"block_length_bytes",
		"database_bytes",
		"finality_lag_blocks",
	)
	independent_operands_present = all(
		sample.get(field) is not None for sample in samples for field in independent_fields
	)
	if not independent_operands_present:
		return {
			"status": "fail",
			"configured_storage_tx_limit": limit,
			"sample_count": len(samples),
			"p95_storage_tx_count": p95,
			"headroom_fraction": headroom,
			"required_limit_ceiling": required_limit,
			"checks": {"independent_operands_present": False},
			"p6_mixed_workload_slo_claimed": False,
		}
	ref_values = [int(sample["block_weight_ref_time"]) for sample in samples]
	proof_values = [int(sample["block_weight_proof_size"]) for sample in samples]
	length_values = [int(sample["block_length_bytes"]) for sample in samples]
	ref_limits = [int(sample["block_weight_ref_time_limit"]) for sample in samples]
	proof_limits = [int(sample["block_weight_proof_size_limit"]) for sample in samples]
	length_limits = [int(sample["block_length_limit_bytes"]) for sample in samples]
	resource_limits_stable = all(
		len({int(sample[field]) for sample in samples}) == 1
		for field in (
			"block_weight_ref_time_limit",
			"block_weight_proof_size_limit",
			"block_length_limit_bytes",
		)
	) and all(value > 0 for value in (*ref_limits, *proof_limits, *length_limits))
	metadata_hashes = {str(sample["metadata_hash"]) for sample in samples}
	resource_limits_stable = resource_limits_stable and (
		len(metadata_hashes) == 1
		and all(value.startswith("0x") and len(value) == 66 for value in metadata_hashes)
		and all(
			len({str(sample[field]) for sample in samples}) == 1
			and all(str(sample[field]).startswith("0x") and len(str(sample[field])) > 2 for sample in samples)
			for field in ("block_weights_constant_scale", "block_length_constant_scale")
		)
	)
	nearest = lambda values: sorted(values)[math.ceil(0.95 * len(values)) - 1]
	checks = {
		"headroom_at_least_20_percent": 5 * p95 <= 4 * limit,
		"limit_at_least_ceil_p95_over_80_percent": limit >= required_limit,
		"no_block_over_90_percent": all(10 * count <= 9 * limit for count in counts),
		"independent_operands_present": independent_operands_present,
		"runtime_resource_limits_stable": resource_limits_stable,
		"p95_ref_time_utilization_at_most_80_percent": 5 * nearest(ref_values)
		<= 4 * ref_limits[0],
		"p95_proof_size_utilization_at_most_80_percent": 5 * nearest(proof_values)
		<= 4 * proof_limits[0],
		"p95_block_length_utilization_at_most_80_percent": 5 * nearest(length_values)
		<= 4 * length_limits[0],
		"no_weight_or_length_sample_over_90_percent": all(
			10 * value <= 9 * limit
			for values, resource_limits in (
				(ref_values, ref_limits),
				(proof_values, proof_limits),
				(length_values, length_limits),
			)
			for value, limit in zip(values, resource_limits)
		),
		"database_and_finality_operands_valid": all(
			int(sample["database_bytes"]) > 0 and int(sample["finality_lag_blocks"]) >= 0
			for sample in samples
		),
	}
	return {
		"status": "pass" if all(checks.values()) else "fail",
		"configured_storage_tx_limit": limit,
		"sample_count": len(samples),
		"p95_storage_tx_count": p95,
		"headroom_fraction": headroom,
		"required_limit_ceiling": required_limit,
		"checks": checks,
		"p6_mixed_workload_slo_claimed": False,
	}


def execute(args: argparse.Namespace, manifest: dict[str, object]) -> dict[str, object]:
	if not args.allow_destructive_faults:
		raise RuntimeError("--execute requires --allow-destructive-faults")
	if not args.case_driver:
		raise RuntimeError(
			"--execute requires --case-driver; Missing/Invalid/Stale/Duplicate use node-side injection"
		)
	driver = shlex.split(args.case_driver)
	if not driver:
		raise RuntimeError("--case-driver cannot be empty")
	driver_executable = Path(driver[0]) if Path(driver[0]).is_file() else None
	if driver_executable is None:
		resolved = shutil.which(driver[0])
		driver_executable = Path(resolved) if resolved else None
	if driver_executable is None or not driver_executable.is_file():
		raise RuntimeError(f"case driver not found: {driver[0]}")
	driver_hash = sha256(driver_executable)
	canonical_driver = (ROOT / "scripts/run-orbis-proof-retention-case.py").resolve()
	if driver_executable.resolve() != canonical_driver:
		raise RuntimeError(f"refusing non-canonical case driver: {driver_executable}")
	binary_paths = {
		"origin": ROOT / "target/release/origin",
		"origin_orbis": ROOT / "target/release/origin-orbis",
		"proof_campaign_origin_orbis": (
			ROOT / "target/proof-campaign/release/origin-orbis-proof-campaign"
		),
		"bootstrap_orbis_core": ROOT / "target/debug/examples/bootstrap_orbis_core",
		"signed_fault_tool": ROOT / "target/debug/examples/orbis_storage_proof_fault",
		"indexed_db_probe_tool": ROOT / "target/debug/examples/orbis_storage_index_fault",
	}
	missing_binaries = [name for name, path in binary_paths.items() if not path.is_file()]
	if missing_binaries:
		raise RuntimeError(f"required campaign binaries are absent: {missing_binaries}")
	binary_hashes = {name: sha256(path) for name, path in binary_paths.items()}
	if any(rpc_ready(port) for port in range(10801, 10814)):
		raise RuntimeError("refusing to reuse occupied 10801..10813 ports")

	subprocess.run([str(ROOT / "scripts/prepare-isolated-proof-specs.sh")], cwd=ROOT, check=True)
	missing_specs = [name for name, path in GENERATED_SPECS.items() if not path.is_file()]
	if missing_specs:
		raise RuntimeError(f"isolated chain specs were not generated: {missing_specs}")
	chain_spec_hashes = {name: sha256(path) for name, path in GENERATED_SPECS.items()}
	evidence = args.evidence_dir
	if evidence.exists() and any(evidence.iterdir()) and not args.resume:
		raise RuntimeError(
			f"refusing to overwrite existing evidence directory without --resume: {evidence}"
		)
	evidence.mkdir(parents=True, exist_ok=True)
	run_suffix = f"-resume-{time.time_ns()}" if args.resume else ""
	owned_registry = evidence / "owned-processes.json"
	if not args.resume and owned_registry.exists():
		owned_registry.unlink()
	log_path = evidence / f"zombienet{run_suffix}.log"
	verdict: dict[str, object] = {}
	with log_path.open("w") as log:
		zombie = subprocess.Popen(
			["zombienet", "-p", "native", "spawn", str(TOPOLOGY)],
			cwd=ROOT,
			stdout=log,
			stderr=subprocess.STDOUT,
			text=True,
			start_new_session=True,
		)
		try:
			wait_for_ports([10801, 10810, 10811], args.startup_timeout, zombie)
			isolation_path = evidence / f"isolation{run_suffix}.json"
			subprocess.run(
				[
					str(ROOT / "scripts/verify-proof-network-isolation.py"),
					"--output",
					str(isolation_path),
				],
				cwd=ROOT,
				check=True,
			)
			isolation = json.loads(isolation_path.read_text())
			if isolation.get("status") != "pass":
				raise RuntimeError("isolation preflight returned a non-pass verdict")
			with (evidence / f"core-assignment{run_suffix}.out").open("x") as core_log:
				subprocess.run(
					[
						str(ROOT / "target/debug/examples/bootstrap_orbis_core"),
						"--endpoint",
						"ws://127.0.0.1:10801",
						"--first-core",
						"0",
						"--cores",
						"1",
					],
					cwd=ROOT,
					check=True,
					stdout=core_log,
					stderr=subprocess.STDOUT,
				)

			manifest_hash = sha256(MANIFEST)
			topology_hash = sha256(TOPOLOGY)
			result_schema_hash = sha256(RESULT_SCHEMA)
			owned_orbis_nodes = {
				"alice": topology_node_identity(10810),
				"bob": topology_node_identity(10811),
			}
			if len({value["base_path"] for value in owned_orbis_nodes.values()}) != 2:
				raise RuntimeError("Orbis collators unexpectedly share one base path")
			context = {
				"schema_version": 2,
				"repo": str(ROOT),
				"relay_rpc": "http://127.0.0.1:10801",
				"orbis_rpcs": ["http://127.0.0.1:10810", "http://127.0.0.1:10811"],
				"orbis_ws": ["ws://127.0.0.1:10810", "ws://127.0.0.1:10811"],
				"evidence_dir": str(evidence),
				"zombienet_log": str(log_path),
				"manifest_sha256": manifest_hash,
				"topology_sha256": topology_hash,
				"driver_sha256": driver_hash,
				"result_schema_sha256": result_schema_hash,
				"chain_specs_sha256": chain_spec_hashes,
				"binaries_sha256": binary_hashes,
				"owned_process_registry": str(owned_registry),
				"owned_orbis_nodes": owned_orbis_nodes,
			}
			context_path = evidence / f"context{run_suffix}.json"
			context_path.write_text(json.dumps(context, indent=2) + "\n", errors="strict")

			case_results = []
			errors = []
			for case in manifest["cases"]:  # type: ignore[index]
				result_path = evidence / f"{case['id']}.json"
				if args.resume and result_path.exists():
					try:
						prior = json.loads(result_path.read_text())
						prior_errors = validate_case_result(
							case,
							prior,
							manifest_sha256=manifest_hash,
							topology_sha256=topology_hash,
							driver_sha256=driver_hash,
							result_schema_sha256=result_schema_hash,
							chain_specs_sha256=chain_spec_hashes,
							binaries_sha256=binary_hashes,
							evidence_dir=evidence,
						)
						if not prior_errors:
							case_results.append(prior)
							continue
						raise RuntimeError(
							f"refusing to overwrite invalid prior case evidence {result_path}: {prior_errors}"
						)
					except (OSError, json.JSONDecodeError) as error:
						raise RuntimeError(
							f"refusing to overwrite unreadable prior case evidence {result_path}: {error}"
						) from error
				completed = subprocess.run(
					[
						*driver,
						"--case",
						case["id"],
						"--context",
						str(context_path),
						"--output",
						str(result_path),
					],
					cwd=ROOT,
					check=False,
				)
				if completed.returncode != 0 or not result_path.exists():
					errors.append(f"{case['id']}: driver exit {completed.returncode}")
					continue
				result = json.loads(result_path.read_text())
				errors.extend(
					validate_case_result(
						case,
						result,
						manifest_sha256=manifest_hash,
						topology_sha256=topology_hash,
						driver_sha256=driver_hash,
						result_schema_sha256=result_schema_hash,
						chain_specs_sha256=chain_spec_hashes,
						binaries_sha256=binary_hashes,
						evidence_dir=evidence,
					)
				)
				case_results.append(result)

			capacity = aggregate_capacity(case_results, evidence) if len(case_results) == 10 else {
				"status": "fail",
				"reason": "ten complete case results are required before AC13 P1 aggregation",
			}
			verdict = {
				"schema_version": 1,
				"status": (
					"pass"
					if not errors and len(case_results) == 10 and capacity["status"] == "pass"
					else "fail"
				),
				"isolation": str(isolation_path),
				"manifest_sha256": manifest_hash,
				"topology_sha256": topology_hash,
				"driver_sha256": driver_hash,
				"result_schema_sha256": result_schema_hash,
				"chain_specs_sha256": chain_spec_hashes,
				"binaries_sha256": binary_hashes,
				"cases": case_results,
				"capacity_ac13_p1": capacity,
				"errors": errors,
			}
		finally:
			terminate(zombie)
			cleanup_errors = terminate_owned_processes(owned_registry)
			occupied = [port for port in range(10801, 10814) if rpc_ready(port)]
			if occupied:
				cleanup_errors.append(f"listeners remain after cleanup: {occupied}")
			if cleanup_errors:
				verdict["status"] = "fail"
				verdict.setdefault("errors", []).extend(cleanup_errors)  # type: ignore[union-attr]
			verdict["global_cleanup"] = {
				"status": "pass" if not cleanup_errors else "fail",
				"errors": cleanup_errors,
				"owned_process_registry": str(owned_registry),
			}
	return verdict


def parse_args() -> argparse.Namespace:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--execute", action="store_true")
	parser.add_argument("--allow-destructive-faults", action="store_true")
	parser.add_argument("--case-driver")
	parser.add_argument(
		"--resume",
		action="store_true",
		help="reuse only prior passing case files bound to the same manifest and topology",
	)
	parser.add_argument("--startup-timeout", type=int, default=300)
	parser.add_argument(
		"--evidence-dir",
		type=Path,
		default=ROOT / "docs/evidence/p1/storage-proof/campaign/production-e2e",
	)
	parser.add_argument("--output", type=Path, default=OUTPUT)
	return parser.parse_args()


def main() -> int:
	args = parse_args()
	if args.output.exists():
		print(f"refusing to overwrite existing campaign output: {args.output}", file=sys.stderr)
		return 2
	manifest, errors = validate_manifest()
	readiness = static_readiness(manifest)
	if errors:
		verdict = {"schema_version": 1, "status": "fail", "errors": errors, "readiness": readiness}
	elif args.execute:
		try:
			verdict = execute(args, manifest)
		except (OSError, RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as error:
			verdict = {"schema_version": 1, "status": "fail", "errors": [str(error)], "readiness": readiness}
	else:
		verdict = {
			"schema_version": 1,
			"status": "blocked" if readiness["blocked_without_case_driver"] else "ready",
			"mode": "offline-readiness",
			"manifest": str(MANIFEST.relative_to(ROOT)),
			"case_count": len(manifest["cases"]),  # type: ignore[index]
			"readiness": readiness,
			"note": "No production E2E claim is made by offline readiness validation.",
		}
	args.output.parent.mkdir(parents=True, exist_ok=True)
	args.output.write_text(json.dumps(verdict, indent=2) + "\n")
	print(json.dumps(verdict, indent=2))
	return 0 if verdict["status"] in ("pass", "ready") else 2


if __name__ == "__main__":
	sys.exit(main())
