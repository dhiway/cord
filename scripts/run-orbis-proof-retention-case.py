#!/usr/bin/env python3
"""Canonical destructive case driver for the isolated Orbis proof-retention campaign.

This program is intentionally unusable outside the orchestrator-owned 108xx topology. It discovers
and validates the live node listeners, records their exact commands/base paths, runs signed setup
and store transactions, schedules one manifest fault at the derived retention boundary, captures
raw RPC/process/node evidence, and restores every reversible fault in ``finally``. Missing,
Invalid, Stale, and Duplicate use only the separate CORD campaign binary and require a structured
canonical-Some/rejection receipt; database mutation and prose markers cannot satisfy those cases.

The driver never converts fixture data or an unparsed claim into a production pass. Unit tests may
inject fake process/RPC fixtures into the pure helpers, but the CLI always uses live RPC, lsof, ps,
and the exact binaries hashed into the orchestrator context.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import shlex
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any, Callable


CASE_ORDER = (
	"two_collator_proof_production",
	"missing_retained_body_or_proof",
	"late_proof",
	"invalid_proof",
	"duplicate_proof",
	"fork_reorg_retention_boundary",
	"database_pruning_boundary",
	"collator_restart_retained_state",
	"fresh_node_resync",
	"disk_pressure_failure_recovery",
)
FAULT_KINDS = {
	"two_collator_proof_production": "none",
	"missing_retained_body_or_proof": "node-provider-missing",
	"late_proof": "node-provider-stale",
	"invalid_proof": "node-provider-invalid",
	"duplicate_proof": "node-proposer-duplicate",
	"fork_reorg_retention_boundary": "network-partition-heal",
	"database_pruning_boundary": "retention-prune-observation",
	"collator_restart_retained_state": "persistent-process-restart",
	"fresh_node_resync": "fresh-observer-resync",
	"disk_pressure_failure_recovery": "disk-limit-recovery",
}
REQUIRED_ASSERTIONS = {
	"two_collator_proof_production": (
		"store_finalized",
		"proof_boundary_finalized",
		"both_collators_same_finalized_hash",
		"no_proof_inherent_error",
	),
	"missing_retained_body_or_proof": (
		"canonical_proof_precondition_some",
		"missing_proof_injected_by_provider",
		"authoring_failed_closed",
		"target_block_not_imported_during_fault",
		"restored_node_recovers",
	),
	"late_proof": (
		"canonical_proof_precondition_some",
		"stale_proof_injected_by_provider",
		"stale_proof_rejected",
		"target_block_not_imported_during_fault",
		"canonical_state_unchanged",
	),
	"invalid_proof": (
		"canonical_proof_precondition_some",
		"invalid_proof_injected_by_provider",
		"invalid_proof_rejected",
		"target_block_not_imported_during_fault",
		"restored_node_recovers",
	),
	"duplicate_proof": (
		"canonical_proof_precondition_some",
		"duplicate_second_push_attempted",
		"second_rejected_as_bad_mandatory",
		"proposal_not_finalized",
		"target_block_not_imported_during_fault",
		"canonical_state_unchanged",
	),
	"fork_reorg_retention_boundary": (
		"competing_forks_observed",
		"reorg_observed",
		"finalized_branch_proof_valid",
		"orphan_state_not_canonical",
	),
	"database_pruning_boundary": (
		"runtime_retention_entry_pruned",
		"database_reference_released",
		"later_blocks_finalize",
		"no_premature_prune",
	),
	"collator_restart_retained_state": (
		"restart_from_same_database",
		"retained_body_available",
		"proof_boundary_finalized",
		"peer_rejoined",
	),
	"fresh_node_resync": (
		"fresh_base_path",
		"indexed_body_synced",
		"best_hash_matches",
		"finalized_hash_matches",
	),
	"disk_pressure_failure_recovery": (
		"disk_fault_observed",
		"process_failed_nonzero",
		"database_reopens",
		"node_resyncs_after_recovery",
	),
}
PROOF_ERROR_MARKERS = (
	"badmandatory",
	"invalidproof",
	"missingstatedata",
	"inherent mutator",
)

PROOF_CAMPAIGN_MODES = ("Missing", "Invalid", "Stale", "Duplicate")
PROOF_CAMPAIGN_RECEIPT_PREFIX = "ORIGIN_ORBIS_PROOF_CAMPAIGN_RECEIPT "
PROOF_CAMPAIGN_CHAIN_ID = "orbis-proof-isolated"


class CampaignError(RuntimeError):
	pass


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
	path.parent.mkdir(parents=True, exist_ok=True)
	path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def parse_hex_u32(value: str) -> int:
	return int(value, 16)


def scale_u32(value: int) -> str:
	if not 0 <= value <= 0xFFFFFFFF:
		raise CampaignError(f"u32 out of range: {value}")
	return "0x" + value.to_bytes(4, "little").hex()


def compact_length(encoded: str) -> int:
	data = bytes.fromhex(encoded.removeprefix("0x"))
	if not data:
		raise CampaignError("empty SCALE vector")
	first = data[0]
	mode = first & 3
	if mode == 0:
		return first >> 2
	if mode == 1:
		if len(data) < 2:
			raise CampaignError("truncated SCALE compact")
		return int.from_bytes(data[:2], "little") >> 2
	if mode == 2:
		if len(data) < 4:
			raise CampaignError("truncated SCALE compact")
		return int.from_bytes(data[:4], "little") >> 2
	length = (first >> 2) + 4
	if len(data) < 1 + length:
		raise CampaignError("truncated big SCALE compact")
	return int.from_bytes(data[1 : 1 + length], "little")


def compact_encoded_size(value: int) -> int:
	if value < 0:
		raise CampaignError("SCALE compact value cannot be negative")
	if value < 1 << 6:
		return 1
	if value < 1 << 14:
		return 2
	if value < 1 << 30:
		return 4
	return 1 + max(4, (value.bit_length() + 7) // 8)


def nearest_rank_p95(values: list[int]) -> int:
	if not values:
		raise CampaignError("cannot calculate p95 without samples")
	return sorted(values)[math.ceil(0.95 * len(values)) - 1]


def proof_target(finalized_store_block: int, retention: int) -> int:
	if finalized_store_block <= 0 or retention <= 0:
		raise CampaignError("proof target requires positive store block and retention")
	return finalized_store_block + retention


def capacity_verdict(samples: list[dict[str, object]], configured_limit: int) -> dict[str, object]:
	counts = [int(sample["storage_tx_count"]) for sample in samples]
	p95 = nearest_rank_p95(counts)
	headroom = 1.0 - p95 / configured_limit
	required_limit = (5 * p95 + 3) // 4
	independent = all(
		sample.get(field) is not None
		for sample in samples
		for field in (
			"block_weight_ref_time",
			"block_weight_proof_size",
			"block_length_bytes",
			"database_bytes",
			"finality_lag_blocks",
		)
	)
	base_checks = {
		"headroom_at_least_20_percent": 5 * p95 <= 4 * configured_limit,
		"configured_limit_covers_p95_over_80_percent": configured_limit >= required_limit,
		"no_block_over_90_percent": all(10 * count <= 9 * configured_limit for count in counts),
		"independent_operands_present": independent,
	}
	if not independent:
		return {
			"status": "fail",
			"configured_limit": configured_limit,
			"sample_count": len(samples),
			"p95_storage_tx_count": p95,
			"headroom_fraction": headroom,
			"required_limit_ceiling": required_limit,
			"checks": base_checks,
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
	checks = {
		**base_checks,
		"runtime_resource_limits_stable": resource_limits_stable,
		"p95_ref_time_utilization_at_most_80_percent": 5 * nearest_rank_p95(ref_values)
		<= 4 * ref_limits[0],
		"p95_proof_size_utilization_at_most_80_percent": 5 * nearest_rank_p95(proof_values)
		<= 4 * proof_limits[0],
		"p95_block_length_utilization_at_most_80_percent": 5
		* nearest_rank_p95(length_values)
		<= 4 * length_limits[0],
		"no_weight_or_length_sample_over_90_percent": all(
			10 * value <= 9 * limit
			for values, limits in (
				(ref_values, ref_limits),
				(proof_values, proof_limits),
				(length_values, length_limits),
			)
			for value, limit in zip(values, limits)
		),
		"database_and_finality_operands_valid": all(
			int(sample["database_bytes"]) > 0 and int(sample["finality_lag_blocks"]) >= 0
			for sample in samples
		),
	}
	return {
		"status": "pass" if all(checks.values()) else "fail",
		"configured_limit": configured_limit,
		"sample_count": len(samples),
		"p95_storage_tx_count": p95,
		"headroom_fraction": headroom,
		"required_limit_ceiling": required_limit,
		"checks": checks,
	}


def validated_block_measurement(
	measure: dict[str, object], block_hash: str, rpc_extrinsic_lengths: list[int]
) -> dict[str, object]:
	if measure.get("status") != "ok" or str(measure.get("block_hash", "")).lower() != block_hash.lower():
		raise CampaignError(f"metadata block measurement failed at {block_hash}: {measure}")
	if (measure.get("spec_version"), measure.get("transaction_version")) != (29, 8):
		raise CampaignError(f"metadata block measurement runtime drift: {measure}")
	helper_extrinsics = measure.get("extrinsics")
	if not isinstance(helper_extrinsics, list) or [
		int(value["raw_bytes"])
		for value in helper_extrinsics
		if isinstance(value, dict)
	] != rpc_extrinsic_lengths:
		raise CampaignError(f"chain_getBlock/helper extrinsic length mismatch at {block_hash}")
	rpc_extrinsics_encoded = compact_encoded_size(len(rpc_extrinsic_lengths)) + sum(
		compact_encoded_size(value) + value for value in rpc_extrinsic_lengths
	)
	if int(measure["extrinsics_encoded_bytes"]) != rpc_extrinsics_encoded:
		raise CampaignError(f"chain_getBlock/helper encoded body length mismatch at {block_hash}")
	if int(measure["header_encoded_bytes"]) + rpc_extrinsics_encoded != int(
		measure["block_encoded_bytes"]
	):
		raise CampaignError(f"helper header/body block length mismatch at {block_hash}")
	total = measure.get("total_consumed")
	maximum = measure.get("max_block")
	weight_ratio = measure.get("total_max_block_ratio")
	length_ratio = measure.get("block_length_ratio")
	if not all(isinstance(value, dict) for value in (total, maximum, weight_ratio, length_ratio)):
		raise CampaignError(f"metadata resource operands are malformed at {block_hash}")
	ref_time = int(total["ref_time"])
	proof_size = int(total["proof_size"])
	ref_limit = int(maximum["ref_time"])
	proof_limit = int(maximum["proof_size"])
	length = int(measure["block_encoded_bytes"])
	length_limit = int(measure["max_block_length_bytes"])
	length_limits_by_class = measure.get("block_length_limits")
	if (
		not isinstance(length_limits_by_class, list)
		or len(length_limits_by_class) != 3
		or max(int(value) for value in length_limits_by_class) != length_limit
	):
		raise CampaignError(f"per-class BlockLength limits drift at {block_hash}")
	for label, ratio, numerator, denominator in (
		("ref_time", weight_ratio.get("ref_time"), ref_time, ref_limit),
		("proof_size", weight_ratio.get("proof_size"), proof_size, proof_limit),
		("block_length", length_ratio, length, length_limit),
	):
		if not isinstance(ratio, dict) or (
			int(ratio.get("numerator", -1)) != numerator
			or int(ratio.get("denominator", -1)) != denominator
			or denominator <= 0
		):
			raise CampaignError(f"metadata {label} ratio operands drift at {block_hash}")
	metadata_hash = measure.get("metadata_hash")
	if not isinstance(metadata_hash, str) or len(metadata_hash) != 66 or not metadata_hash.startswith("0x"):
		raise CampaignError(f"metadata hash is malformed at {block_hash}")
	classes = {name: measure.get(name) for name in ("normal", "operational", "mandatory")}
	if not all(isinstance(value, dict) for value in classes.values()):
		raise CampaignError(f"per-class weight measurement is malformed at {block_hash}")
	class_totals = {"ref_time": 0, "proof_size": 0}
	for name, value in classes.items():
		consumed = value.get("consumed")
		events = value.get("corrected_extrinsic_event_total")
		overhead = value.get("block_weight_minus_corrected_event_total")
		if not all(isinstance(part, dict) for part in (consumed, events, overhead)):
			raise CampaignError(f"{name} metadata weight components are malformed at {block_hash}")
		for component in ("ref_time", "proof_size"):
			consumed_value = int(consumed[component])
			if int(events[component]) + int(overhead[component]) != consumed_value:
				raise CampaignError(f"{name} metadata weight components do not sum at {block_hash}")
			class_totals[component] += consumed_value
	if class_totals != {"ref_time": ref_time, "proof_size": proof_size}:
		raise CampaignError(f"per-class weights do not sum to total at {block_hash}")
	if int(measure.get("corrected_extrinsic_event_count", -1)) != len(rpc_extrinsic_lengths) or int(
		measure.get("extrinsic_count", -1)
	) != len(rpc_extrinsic_lengths):
		raise CampaignError(f"corrected event/extrinsic count drift at {block_hash}")
	for field in (
		"block_weight_scale",
		"block_weights_constant_scale",
		"block_length_constant_scale",
	):
		encoded = measure.get(field)
		if not isinstance(encoded, str) or not encoded.startswith("0x") or len(encoded) <= 2:
			raise CampaignError(f"{field} is absent at {block_hash}")
		try:
			bytes.fromhex(encoded[2:])
		except ValueError as error:
			raise CampaignError(f"{field} is not hex at {block_hash}") from error
	return {
		"metadata_hash": metadata_hash,
		"block_weight_scale": measure["block_weight_scale"],
		"block_weights_constant_scale": measure["block_weights_constant_scale"],
		"block_length_constant_scale": measure["block_length_constant_scale"],
		"block_weight_by_class": classes,
		"block_weight_ref_time": ref_time,
		"block_weight_proof_size": proof_size,
		"block_weight_ref_time_limit": ref_limit,
		"block_weight_proof_size_limit": proof_limit,
		"block_weight_ref_time_ratio": ref_time / ref_limit,
		"block_weight_proof_size_ratio": proof_size / proof_limit,
		"block_weight_ratio_operands": weight_ratio,
		"corrected_extrinsic_event_count": int(measure["corrected_extrinsic_event_count"]),
		"extrinsic_count": int(measure["extrinsic_count"]),
		"header_encoded_bytes": int(measure["header_encoded_bytes"]),
		"block_extrinsics_encoded_bytes": rpc_extrinsics_encoded,
		"block_length_bytes": length,
		"block_length_limit_bytes": length_limit,
		"block_length_ratio": length / length_limit,
		"block_length_ratio_operands": length_ratio,
		"block_length_limits_by_class": [int(value) for value in length_limits_by_class],
	}


def split_command(command: list[str]) -> tuple[list[str], list[str]]:
	if "--" not in command:
		return list(command), []
	index = command.index("--")
	return command[:index], command[index + 1 :]


def option_value(command: list[str], option: str) -> str:
	for index, item in enumerate(command):
		if item == option and index + 1 < len(command):
			return command[index + 1]
		if item.startswith(option + "="):
			return item.split("=", 1)[1]
	raise CampaignError(f"command lacks required option {option}: {shlex.join(command)}")


def remove_option(command: list[str], option: str, *, takes_value: bool = True) -> list[str]:
	result: list[str] = []
	index = 0
	while index < len(command):
		item = command[index]
		if item == option:
			index += 2 if takes_value and index + 1 < len(command) else 1
			continue
		if item.startswith(option + "="):
			index += 1
			continue
		result.append(item)
		index += 1
	return result


def set_option(command: list[str], option: str, value: str) -> list[str]:
	return [*remove_option(command, option), option, value]


def canonical_command(command: list[str]) -> list[str]:
	para, relay = split_command(command)
	for option in (
		"--proof-campaign-mode",
		"--proof-campaign-target-block",
		"--proof-campaign-expected-chain-id",
		"--proof-campaign-expected-genesis-hash",
	):
		para = remove_option(para, option)
	para = remove_option(
		para, "--unsafe-proof-campaign-acknowledge-disposable", takes_value=False
	)
	para = remove_option(para, "--reserved-only", takes_value=False)
	return [*para, "--", *relay] if relay else para


def checked_genesis_hash(value: str) -> str:
	if len(value) != 66 or not value.startswith("0x"):
		raise CampaignError("proof campaign requires an exact 32-byte genesis hash")
	try:
		bytes.fromhex(value[2:])
	except ValueError as error:
		raise CampaignError("proof campaign genesis hash is not hexadecimal") from error
	return value.lower()


def proof_campaign_command(
	command: list[str], binary: Path, mode: str, target: int, expected_genesis_hash: str
) -> list[str]:
	if mode not in PROOF_CAMPAIGN_MODES:
		raise CampaignError(f"unsupported proof campaign mode: {mode}")
	if target <= 0:
		raise CampaignError("proof campaign target block must be positive")
	expected_genesis_hash = checked_genesis_hash(expected_genesis_hash)
	para, relay = split_command(canonical_command(command))
	para[0] = str(binary)
	para.extend(
		[
			"--proof-campaign-mode",
			mode,
			"--proof-campaign-target-block",
			str(target),
			"--proof-campaign-expected-chain-id",
			PROOF_CAMPAIGN_CHAIN_ID,
			"--proof-campaign-expected-genesis-hash",
			expected_genesis_hash,
			"--unsafe-proof-campaign-acknowledge-disposable",
		]
	)
	return [*para, "--", *relay] if relay else para


def _checked_sha256(value: object, field: str) -> str:
	if not isinstance(value, str) or len(value) != 64:
		raise CampaignError(f"proof campaign receipt lacks 32-byte {field}")
	try:
		bytes.fromhex(value)
	except ValueError as error:
		raise CampaignError(f"proof campaign receipt has non-hex {field}") from error
	return value.lower()


def parse_proof_campaign_receipts(text: str) -> list[dict[str, object]]:
	"""Extract only explicitly prefixed node receipts; prose log markers are not evidence."""
	receipts: list[dict[str, object]] = []
	for line in text.splitlines():
		index = line.find(PROOF_CAMPAIGN_RECEIPT_PREFIX)
		if index < 0:
			continue
		payload = line[index + len(PROOF_CAMPAIGN_RECEIPT_PREFIX) :].strip()
		try:
			value = json.loads(payload)
		except json.JSONDecodeError as error:
			raise CampaignError("proof campaign node receipt is malformed JSON") from error
		if not isinstance(value, dict):
			raise CampaignError("proof campaign node receipt must be a JSON object")
		receipts.append(value)
	return receipts


def validated_proof_campaign_receipt(
	receipts: list[dict[str, object]], *, mode: str, target: int, genesis_hash: str
) -> dict[str, object]:
	"""Validate the one-shot, exact-chain node evidence contract for one fault attempt."""
	if len(receipts) != 1:
		raise CampaignError(
			f"proof campaign requires exactly one node receipt, observed {len(receipts)}"
		)
	receipt = receipts[0]
	expected_action = {
		"Missing": "provider-omitted",
		"Invalid": "provider-invalid",
		"Stale": "provider-stale",
		"Duplicate": "duplicate-second-push",
	}[mode]
	expected_rejection = "FinalizationError" if mode == "Missing" else "BadMandatory"
	expected = {
		"schema_version": 1,
		"event": "proof-campaign-fault-attempt",
		"mode": mode,
		"chain_id": PROOF_CAMPAIGN_CHAIN_ID,
		"genesis_hash": checked_genesis_hash(genesis_hash),
		"target_block": target,
		"canonical_proof_present": True,
		"action": expected_action,
		"rejection": expected_rejection,
		"proposal_returned": False,
		"one_shot": True,
	}
	for field, value in expected.items():
		actual = receipt.get(field)
		if field == "genesis_hash" and isinstance(actual, str):
			actual = actual.lower()
		if actual != value:
			raise CampaignError(
				f"proof campaign receipt {field} mismatch: expected {value!r}, got {actual!r}"
			)
	if not isinstance(receipt.get("parent_block"), int) or receipt["parent_block"] != target - 1:
		raise CampaignError("proof campaign receipt is not bound to the target parent block")
	parent_hash = receipt.get("parent_hash")
	if not isinstance(parent_hash, str):
		raise CampaignError("proof campaign receipt lacks parent hash")
	checked_genesis_hash(parent_hash)
	canonical_hash = _checked_sha256(receipt.get("canonical_proof_sha256"), "canonical proof hash")
	injected = receipt.get("injected_proof_sha256")
	if mode == "Missing":
		if injected is not None:
			raise CampaignError("Missing receipt must not claim an injected proof hash")
	elif mode in ("Invalid", "Stale"):
		if _checked_sha256(injected, "injected proof hash") == canonical_hash:
			raise CampaignError(f"{mode} receipt did not change the canonical proof")
	else:
		if _checked_sha256(injected, "injected proof hash") != canonical_hash:
			raise CampaignError("Duplicate receipt is not bound to the canonical proof")
	return receipt


def partition_command(command: list[str]) -> list[str]:
	para, relay = split_command(canonical_command(command))
	para = remove_option(para, "--bootnodes")
	if "--reserved-only" not in para:
		para.append("--reserved-only")
	return [*para, "--", *relay] if relay else para


def observer_command(
	command: list[str], binary: Path, base_path: Path, name: str, rpc_port: int, p2p_port: int
) -> list[str]:
	para, relay = split_command(canonical_command(command))
	para[0] = str(binary)
	for flag in ("--collator", "--force-authoring", "--reserved-only"):
		para = remove_option(para, flag, takes_value=False)
	para = remove_option(para, "--authoring")
	para = set_option(para, "--name", name)
	para = set_option(para, "--base-path", str(base_path))
	para = set_option(para, "--rpc-port", str(rpc_port))
	para = set_option(para, "--listen-addr", f"/ip4/127.0.0.1/tcp/{p2p_port}/ws")
	para = set_option(para, "--node-key", hashlib.blake2s(name.encode()).hexdigest())
	para = set_option(para, "--prometheus-port", str(rpc_port + 100))
	if relay:
		relay = set_option(relay, "--base-path", str(base_path / "relay"))
		relay = set_option(relay, "--port", str(p2p_port + 1))
		relay = set_option(relay, "--rpc-port", str(rpc_port + 101))
		relay = set_option(relay, "--prometheus-port", str(rpc_port + 102))
	return [*para, "--", *relay] if relay else para


class RpcTrace:
	def __init__(self, path: Path):
		self.path = path
		self.records: list[dict[str, object]] = []
		self.next_id = 1

	def call(self, url: str, method: str, params: list[object] | None = None) -> object:
		request_id = self.next_id
		self.next_id += 1
		payload = {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params or []}
		record: dict[str, object] = {
			"time_ns": time.time_ns(),
			"url": url,
			"request": payload,
		}
		try:
			request = urllib.request.Request(
				url,
				data=json.dumps(payload).encode(),
				headers={"content-type": "application/json"},
			)
			response = json.load(urllib.request.urlopen(request, timeout=10))
			record["response"] = response
			if "error" in response:
				raise CampaignError(f"{method} on {url}: {response['error']}")
			return response["result"]
		except Exception as error:
			record["exception"] = repr(error)
			raise
		finally:
			self.records.append(record)
			write_json(self.path, self.records)


@dataclass
class Node:
	role: str
	port: int
	pid: int
	command: list[str]
	base_path: Path
	name: str
	genesis_hash: str


class ProcessManager:
	def __init__(
		self,
		repo: Path,
		rpc: RpcTrace,
		registry: Path,
		case_dir: Path,
		owned_bases: dict[str, Path] | None = None,
	):
		self.repo = repo
		self.rpc = rpc
		self.registry = registry
		self.case_dir = case_dir
		self.started: list[subprocess.Popen[bytes]] = []
		self.logs: list[Path] = []
		self.owned_bases = owned_bases or {}

	def _listener_pid(self, port: int) -> int:
		completed = subprocess.run(
			["lsof", "-nP", "-t", f"-iTCP:{port}", "-sTCP:LISTEN"],
			text=True,
			capture_output=True,
			check=False,
		)
		pids = sorted({int(line) for line in completed.stdout.splitlines() if line.strip().isdigit()})
		if completed.returncode or len(pids) != 1 or pids[0] <= 1:
			raise CampaignError(f"expected one safe listener PID on {port}, found {pids}")
		return pids[0]

	def discover(self, role: str, port: int) -> Node:
		pid = self._listener_pid(port)
		command_text = subprocess.check_output(
			["ps", "-ww", "-p", str(pid), "-o", "command="], text=True
		).strip()
		command = shlex.split(command_text)
		if not command or "origin-orbis" not in Path(command[0]).name:
			raise CampaignError(f"PID {pid} is not origin-orbis: {command_text}")
		if int(option_value(command, "--rpc-port")) != port:
			raise CampaignError(f"PID {pid} command/RPC mismatch on {port}")
		base_path = Path(option_value(command, "--base-path")).resolve()
		if not base_path.is_dir():
			raise CampaignError(f"node base path is absent: {base_path}")
		expected_base = self.owned_bases.get(role)
		if expected_base is not None and base_path != expected_base:
			raise CampaignError(
				f"refusing unowned base path for {role} on {port}: {base_path} != {expected_base}"
			)
		name = option_value(command, "--name")
		url = f"http://127.0.0.1:{port}"
		version = self.rpc.call(url, "state_getRuntimeVersion")
		if not isinstance(version, dict) or (
			version.get("specName"), version.get("specVersion"), version.get("transactionVersion")
		) != ("orbis", 29, 8):
			raise CampaignError(f"unexpected Orbis runtime on {port}: {version}")
		genesis = self.rpc.call(url, "chain_getBlockHash", [0])
		return Node(role, port, pid, command, base_path, name, str(genesis))

	def stop(self, node: Node, timeout: int = 30) -> None:
		try:
			os.kill(node.pid, signal.SIGTERM)
		except ProcessLookupError:
			self.mark_stopped(node.pid)
			return
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			completed = subprocess.run(
				["lsof", "-nP", "-t", f"-iTCP:{node.port}", "-sTCP:LISTEN"],
				text=True,
				capture_output=True,
				check=False,
			)
			listeners = {int(value) for value in completed.stdout.splitlines() if value.isdigit()}
			if node.pid not in listeners:
				self.mark_stopped(node.pid)
				return
			time.sleep(0.25)
		os.kill(node.pid, signal.SIGKILL)
		deadline = time.monotonic() + 5
		while time.monotonic() < deadline:
			completed = subprocess.run(
				["lsof", "-nP", "-t", f"-iTCP:{node.port}", "-sTCP:LISTEN"],
				text=True,
				capture_output=True,
				check=False,
			)
			if str(node.pid) not in completed.stdout.splitlines():
				self.mark_stopped(node.pid)
				return
			time.sleep(0.1)
		raise CampaignError(f"node PID {node.pid} did not stop")

	def _registry_rows(self) -> list[dict[str, object]]:
		if not self.registry.exists():
			return []
		try:
			rows = json.loads(self.registry.read_text())
		except json.JSONDecodeError as error:
			raise CampaignError(f"invalid owned-process registry: {error}") from error
		if not isinstance(rows, list):
			raise CampaignError("owned-process registry must be a list")
		return rows

	def _register(self, process: subprocess.Popen[bytes], command: list[str], kind: str, log: Path) -> None:
		try:
			observed = shlex.split(
				subprocess.check_output(
					["ps", "-ww", "-p", str(process.pid), "-o", "command="], text=True
				).strip()
			)
		except subprocess.CalledProcessError:
			observed = command
		rows = self._registry_rows()
		# ``start_new_session=True`` makes the child its own process-group leader. Using the
		# immutable launch PID also records fast-failing disk-pressure children that may have
		# exited before this ledger write; the global cleanup's ps/command guard handles them.
		pgid = process.pid
		rows.append(
			{
				"pid": process.pid,
				"pgid": pgid,
				"kind": kind,
				"command": observed,
				"command_sha256": hashlib.sha256("\0".join(observed).encode()).hexdigest(),
				"log": str(log),
				"active": True,
			}
		)
		write_json(self.registry, rows)

	def mark_stopped(self, pid: int) -> None:
		rows = self._registry_rows()
		for row in rows:
			if row.get("pid") == pid:
				row["active"] = False
		write_json(self.registry, rows)

	def start(
		self,
		command: list[str],
		role: str,
		port: int,
		*,
		kind: str = "topology-replacement",
		preexec_fn: Callable[[], None] | None = None,
		expect_ready: bool = True,
		timeout: int = 90,
	) -> tuple[subprocess.Popen[bytes], Path]:
		log = self.case_dir / f"{role}-{time.time_ns()}.log"
		with log.open("wb") as output:
			process = subprocess.Popen(
				command,
				cwd=self.repo,
				stdin=subprocess.DEVNULL,
				stdout=output,
				stderr=subprocess.STDOUT,
				start_new_session=True,
				preexec_fn=preexec_fn,
			)
		self.started.append(process)
		self.logs.append(log)
		self._register(process, command, kind, log)
		if expect_ready:
			self.wait_ready(port, timeout)
			live = self.discover(role, port)
			if live.pid != process.pid:
				raise CampaignError(
					f"started PID {process.pid}, but listener {port} belongs to {live.pid}"
				)
		return process, log

	def wait_ready(self, port: int, timeout: int) -> None:
		url = f"http://127.0.0.1:{port}"
		deadline = time.monotonic() + timeout
		last: Exception | None = None
		while time.monotonic() < deadline:
			try:
				version = self.rpc.call(url, "state_getRuntimeVersion")
				if isinstance(version, dict) and version.get("specName") == "orbis":
					return
			except Exception as error:
				last = error
			time.sleep(1)
		raise CampaignError(f"Orbis RPC {port} not ready: {last}")

	def terminate_process(self, process: subprocess.Popen[bytes], timeout: int = 20) -> int:
		if process.poll() is not None:
			self.mark_stopped(process.pid)
			return int(process.returncode or 0)
		try:
			os.killpg(os.getpgid(process.pid), signal.SIGTERM)
		except ProcessLookupError:
			pass
		try:
			code = process.wait(timeout=timeout)
		except subprocess.TimeoutExpired:
			os.killpg(os.getpgid(process.pid), signal.SIGKILL)
			code = process.wait(timeout=5)
		self.mark_stopped(process.pid)
		return code


class CaseRunner:
	def __init__(self, case: str, context: dict[str, object], output: Path):
		if case not in CASE_ORDER:
			raise CampaignError(f"unknown proof case: {case}")
		if context.get("schema_version") != 2:
			raise CampaignError(f"unsupported campaign context schema: {context.get('schema_version')}")
		self.case = case
		self.context = context
		self.output = output
		self.repo = Path(str(context["repo"])).resolve()
		self.evidence = Path(str(context["evidence_dir"])).resolve()
		self.zombienet_log = Path(str(context["zombienet_log"])).resolve()
		if self.zombienet_log.parent != self.evidence:
			raise CampaignError("Zombienet log must be directly inside the evidence directory")
		if self.output.resolve().parent != self.evidence:
			raise CampaignError("result output must be directly inside the campaign evidence dir")
		self.case_dir = self.evidence / case
		if self.case_dir.exists() and any(self.case_dir.iterdir()):
			raise CampaignError(f"refusing to overwrite existing case artifacts: {self.case_dir}")
		self.case_dir.mkdir(parents=True, exist_ok=True)
		self.rpc_path = self.case_dir / "rpc-trace.json"
		self.rpc = RpcTrace(self.rpc_path)
		self.registry = Path(str(context["owned_process_registry"])).resolve()
		if self.registry.parent != self.evidence:
			raise CampaignError("owned process registry must be inside evidence dir")
		owned_nodes = context.get("owned_orbis_nodes")
		if not isinstance(owned_nodes, dict) or set(owned_nodes) != {"alice", "bob"}:
			raise CampaignError("context lacks exact Zombienet-owned Orbis node identities")
		owned_bases: dict[str, Path] = {}
		self.owned_names: dict[str, str] = {}
		for role, value in owned_nodes.items():
			if not isinstance(value, dict):
				raise CampaignError(f"invalid owned node identity for {role}")
			base = Path(str(value.get("base_path", ""))).resolve()
			name = value.get("name")
			if not base.is_dir() or not isinstance(name, str) or not name:
				raise CampaignError(f"invalid owned base/name for {role}")
			owned_bases[str(role)] = base
			self.owned_names[str(role)] = name
		if len(set(owned_bases.values())) != 2:
			raise CampaignError("owned collator base paths must be distinct")
		self.processes = ProcessManager(
			self.repo, self.rpc, self.registry, self.case_dir, owned_bases
		)
		self.orbis_urls = [str(value) for value in context["orbis_rpcs"]]  # type: ignore[index]
		self.binary_hashes = context["binaries_sha256"]
		self.signed_path = self.case_dir / "signed-receipts.json"
		self.fault_path = self.case_dir / "fault-receipt.json"
		self.campaign_receipt_path = self.case_dir / "proof-campaign-node-receipts.json"
		self.cleanup_path = self.case_dir / "cleanup-receipt.json"
		self.capacity_path = self.case_dir / "capacity-samples.json"
		self.node_log_path = self.case_dir / "node-logs.raw"
		self.signed_raw: list[dict[str, object]] = []
		self.signed_receipts: list[dict[str, object]] = []
		self.assertions = {name: False for name in REQUIRED_ASSERTIONS[case]}
		self.fault: dict[str, object] = {
			"kind": FAULT_KINDS[case],
			"target_block": 0,
			"observed_at_block": 0,
			"reverted": False,
		}
		self.cleanup = {
			"status": "fail",
			"fault_state_restored": False,
			"temporary_processes_stopped": False,
			"errors": [],
		}
		self.original_commands: dict[str, list[str]] = {}
		self.original_bases: dict[str, Path] = {}
		self.nodes: dict[str, Node] = {}
		self.temp_dirs: list[Path] = []
		self.case_logs: list[Path] = []
		self.error: str | None = None
		self.retention = 0
		self.store_block = 0
		self.store_hash = ""
		self.limit = 0
		self.inspect: dict[str, object] = {}
		self.campaign_receipts: list[dict[str, object]] = []

	def binary(self, name: str) -> Path:
		paths = {
			"origin_orbis": self.repo / "target/release/origin-orbis",
			"proof_campaign_origin_orbis": (
				self.repo / "target/proof-campaign/release/origin-orbis-proof-campaign"
			),
			"signed_fault_tool": self.repo / "target/debug/examples/orbis_storage_proof_fault",
			"indexed_db_probe_tool": self.repo / "target/debug/examples/orbis_storage_index_fault",
		}
		path = paths[name]
		expected = self.binary_hashes.get(name) if isinstance(self.binary_hashes, dict) else None
		if not path.is_file() or sha256(path) != expected:
			raise CampaignError(f"binary absent or hash mismatch: {name}")
		return path

	def height(self, url: str, *, finalized: bool = False) -> tuple[int, str]:
		block_hash = str(self.rpc.call(url, "chain_getFinalizedHead")) if finalized else None
		header = self.rpc.call(url, "chain_getHeader", [block_hash] if block_hash else [])
		if not isinstance(header, dict):
			raise CampaignError(f"invalid header from {url}: {header}")
		return parse_hex_u32(str(header["number"])), block_hash or str(header.get("hash", ""))

	def block_number(self, url: str, block_hash: str) -> int:
		header = self.rpc.call(url, "chain_getHeader", [block_hash])
		if not isinstance(header, dict):
			raise CampaignError(f"block header is absent for {block_hash}")
		return parse_hex_u32(str(header["number"]))

	def canonical_hash(self, url: str, number: int) -> str | None:
		value = self.rpc.call(url, "chain_getBlockHash", [number])
		return None if value is None else str(value)

	def wait_finalized(self, number: int, timeout: int = 240) -> str:
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			height, _ = self.height(self.orbis_urls[0], finalized=True)
			if height >= number:
				block_hash = self.canonical_hash(self.orbis_urls[0], number)
				if block_hash:
					return block_hash
			time.sleep(1)
		raise CampaignError(f"Orbis did not finalize block {number} before {timeout}s")

	def wait_both_same_finalized(self, minimum: int, timeout: int = 240) -> str:
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			heads = [str(self.rpc.call(url, "chain_getFinalizedHead")) for url in self.orbis_urls]
			heights = [self.block_number(url, head) for url, head in zip(self.orbis_urls, heads)]
			if len(set(heads)) == 1 and min(heights) >= minimum:
				return heads[0]
			time.sleep(1)
		raise CampaignError(f"collators did not agree on finalized head >= {minimum}")

	def indexed_count(self, block: int, *, at: str | None = None, url: str | None = None) -> int:
		params: list[object] = ["TransactionStorageApi_indexed_transactions", scale_u32(block)]
		if at:
			params.append(at)
		encoded = self.rpc.call(url or self.orbis_urls[0], "state_call", params)
		return compact_length(str(encoded))

	def retention_period(self) -> int:
		encoded = str(
			self.rpc.call(
				self.orbis_urls[0], "state_call", ["TransactionStorageApi_retention_period", "0x"]
			)
		)
		data = bytes.fromhex(encoded.removeprefix("0x"))
		if len(data) != 4:
			raise CampaignError(f"retention API returned {len(data)} bytes")
		return int.from_bytes(data, "little")

	def run_tool(self, action: list[str], label: str) -> dict[str, object]:
		command = [
			str(self.binary("signed_fault_tool")),
			"--endpoint",
			self.orbis_urls[0].replace("http://", "ws://"),
			"--seed",
			"//Alice",
			"--allow-dev-faults",
			*action,
		]
		completed = subprocess.run(command, cwd=self.repo, text=True, capture_output=True, check=False)
		record = {
			"label": label,
			"command": command,
			"returncode": completed.returncode,
			"stdout": completed.stdout,
			"stderr": completed.stderr,
		}
		self.signed_raw.append(record)
		write_json(self.signed_path, self.signed_raw)
		if completed.returncode:
			raise CampaignError(f"signed tool {label} failed: {completed.stderr.strip()}")
		try:
			value = json.loads(completed.stdout)
		except json.JSONDecodeError as error:
			raise CampaignError(f"signed tool {label} returned invalid JSON") from error
		if not isinstance(value, dict):
			raise CampaignError(f"signed tool {label} returned non-object")
		return value

	def finalized_receipt(self, action: str, value: dict[str, object]) -> dict[str, object]:
		if value.get("status") != "accepted":
			raise CampaignError(f"signed {action} was not accepted: {value}")
		block_hash = str(value.get("block_hash", ""))
		extrinsic_hash = str(value.get("extrinsic_hash", ""))
		if len(block_hash) != 66 or len(extrinsic_hash) != 66:
			raise CampaignError(f"signed {action} omitted 32-byte hashes")
		number = self.block_number(self.orbis_urls[0], block_hash)
		finalized, _ = self.height(self.orbis_urls[0], finalized=True)
		if finalized < number or self.canonical_hash(self.orbis_urls[0], number) != block_hash:
			raise CampaignError(f"signed {action} block is not canonical/finalized")
		receipt = {
			"action": action,
			"status": "finalized",
			"extrinsic_hash": extrinsic_hash,
			"finalized_block_hash": block_hash,
			"block_number": number,
			"signer": "//Alice",
			"events": value.get("events", []),
		}
		self.signed_receipts.append(receipt)
		return receipt

	def signed_setup_and_store(self) -> None:
		inspect = self.run_tool(["inspect"], "inspect-runtime-capacity")
		if inspect.get("status") != "ok":
			raise CampaignError(f"runtime capacity inspection failed: {inspect}")
		self.inspect = inspect
		self.limit = int(inspect["max_block_transactions"])
		if self.limit <= 0:
			raise CampaignError("runtime MaxBlockTransactions must be positive")
		setup = self.run_tool(
			["setup", "--retention", "32", "--transactions", "128", "--bytes", "16777216"],
			"setup",
		)
		self.finalized_receipt("setup", setup)
		index = CASE_ORDER.index(self.case)
		warm_fill = 0x20 + index
		main_fill = 0x60 + index
		warm = self.run_tool(
			["store", "--bytes", "2048", "--fill", str(warm_fill)], "warmup-store"
		)
		self.finalized_receipt("warmup-store", warm)
		store = self.run_tool(
			["store", "--bytes", "2048", "--fill", str(main_fill)], "store"
		)
		receipt = self.finalized_receipt("store", store)
		self.store_block = int(receipt["block_number"])
		self.store_hash = str(receipt["finalized_block_hash"])
		self.retention = self.retention_period()
		if self.retention != 32:
			raise CampaignError(f"signed setup did not establish retention 32: {self.retention}")
		if self.indexed_count(self.store_block, at=self.store_hash) < 1:
			raise CampaignError("finalized store block has no indexed transaction metadata")
		self.fault["target_block"] = proof_target(self.store_block, self.retention)
		# This is only an orchestration preflight. The campaign binary must independently
		# materialize a canonical Some(proof) and record its hash before injecting any fault.
		self.fault["indexed_metadata_precondition"] = {
			"store_block": self.store_block,
			"indexed_transaction_count": self.indexed_count(self.store_block, at=self.store_hash),
		}

	def discover_topology(self) -> None:
		for role, port in (("alice", 10810), ("bob", 10811)):
			node = self.processes.discover(role, port)
			if node.name != self.owned_names[role]:
				raise CampaignError(f"unexpected Zombienet node name for {role}: {node.name}")
			if Path(node.command[0]).resolve() != self.binary("origin_orbis").resolve():
				raise CampaignError(f"unexpected initial Orbis executable for {role}")
			self.nodes[role] = node
			self.original_commands[role] = canonical_command(node.command)
			self.original_bases[role] = node.base_path
		if self.nodes["alice"].genesis_hash != self.nodes["bob"].genesis_hash:
			raise CampaignError("collators do not share genesis")

	def stop_both_before_target(self, margin: int = 3) -> None:
		target = int(self.fault["target_block"])
		deadline = time.monotonic() + 180
		while time.monotonic() < deadline:
			best, _ = self.height(self.orbis_urls[0])
			if best >= target:
				raise CampaignError(f"chain reached target {target} before fault was armed")
			if best >= target - margin:
				break
			time.sleep(0.5)
		else:
			raise CampaignError("chain did not approach fault target")
		for role in ("alice", "bob"):
			self.nodes[role] = self.processes.discover(role, self.nodes[role].port)
		for role in ("alice", "bob"):
			self.processes.stop(self.nodes[role])

	def start_role(self, role: str, command: list[str] | None = None) -> Node:
		base = command or self.original_commands[role]
		process, log = self.processes.start(base, role, self.nodes[role].port)
		self.case_logs.append(log)
		node = self.processes.discover(role, self.nodes[role].port)
		if node.pid != process.pid or node.genesis_hash != self.nodes[role].genesis_hash:
			raise CampaignError(f"{role} restart identity/genesis mismatch")
		self.nodes[role] = node
		return node

	def stop_live_role(self, role: str) -> None:
		try:
			node = self.processes.discover(role, self.nodes[role].port)
		except Exception:
			return
		self.processes.stop(node)
		self.processes.mark_stopped(node.pid)

	def restore_default_collators(self) -> None:
		for role in ("alice", "bob"):
			self.stop_live_role(role)
		for role in ("alice", "bob"):
			self.start_role(role, self.original_commands[role])
		self.wait_both_same_finalized(max(1, int(self.fault.get("target_block", 1))), timeout=240)

	def db_path(self, role: str) -> Path:
		base = self.original_bases[role]
		matches = [path.resolve() for path in base.glob("chains/*/db/full") if path.is_dir()]
		if len(matches) != 1 or base not in matches[0].parents:
			raise CampaignError(f"expected one owned RocksDB under {base}, found {matches}")
		return matches[0]

	def index_probe(self, backup: Path, *, database: Path | None = None) -> dict[str, object]:
		action = "probe"
		command = [
			str(self.binary("indexed_db_probe_tool")),
			"--database",
			str(database or self.db_path("alice")),
			"--allow-dev-faults",
			"--action",
			action,
			"--bytes",
			"2048",
			"--fill",
			str(0x60 + CASE_ORDER.index(self.case)),
			"--backup",
			str(backup),
		]
		completed = subprocess.run(command, cwd=self.repo, text=True, capture_output=True, check=False)
		record = {
			"command": command,
			"returncode": completed.returncode,
			"stdout": completed.stdout,
			"stderr": completed.stderr,
		}
		if completed.returncode:
			raise CampaignError(f"indexed DB {action} failed: {completed.stderr.strip()}")
		try:
			record["output"] = json.loads(completed.stdout)
		except json.JSONDecodeError as error:
			raise CampaignError(f"indexed DB {action} returned invalid JSON") from error
		return record

	def stall_observation(self, target: int, seconds: int = 24) -> dict[str, object]:
		before_best, _ = self.height(self.orbis_urls[0])
		before_finalized, before_head = self.height(self.orbis_urls[0], finalized=True)
		time.sleep(seconds)
		after_best, _ = self.height(self.orbis_urls[0])
		after_finalized, after_head = self.height(self.orbis_urls[0], finalized=True)
		return {
			"before_best": before_best,
			"before_finalized": before_finalized,
			"before_finalized_hash": before_head,
			"after_best": after_best,
			"after_finalized": after_finalized,
			"after_finalized_hash": after_head,
			"target": target,
			"target_not_finalized": after_finalized < target,
			"target_not_imported": self.canonical_hash(self.orbis_urls[0], target) is None,
		}

	def collected_log_text(self) -> str:
		sources = [self.zombienet_log, *self.case_logs, *self.processes.logs]
		seen: set[Path] = set()
		with self.node_log_path.open("wb") as output:
			for source in sources:
				source = source.resolve()
				if source in seen or not source.is_file():
					continue
				seen.add(source)
				output.write(f"\n===== {source} =====\n".encode())
				output.write(source.read_bytes())
		if self.node_log_path.stat().st_size == 0:
			raise CampaignError("no raw node log was captured")
		return self.node_log_path.read_text(errors="replace").lower()

	def case_two_collator(self) -> None:
		target = int(self.fault["target_block"])
		boundary_hash = self.wait_finalized(target + 2)
		common = self.wait_both_same_finalized(target + 2)
		logs = self.collected_log_text()
		self.fault.update(observed_at_block=target, reverted=True, boundary_hash=boundary_hash)
		self.assertions.update(
			store_finalized=True,
			proof_boundary_finalized=bool(boundary_hash),
			both_collators_same_finalized_hash=bool(common),
			no_proof_inherent_error=not any(marker in logs for marker in PROOF_ERROR_MARKERS),
		)

	def case_proof_campaign_fault(self, mode: str) -> None:
		target = int(self.fault["target_block"])
		self.stop_both_before_target()
		genesis_hash = self.nodes["alice"].genesis_hash
		command = proof_campaign_command(
			self.original_commands["alice"],
			self.binary("proof_campaign_origin_orbis"),
			mode,
			target,
			genesis_hash,
		)
		self.start_role("alice", command)
		stall = self.stall_observation(target)
		parent_hash = self.canonical_hash(self.orbis_urls[0], target - 1)
		if parent_hash is None:
			raise CampaignError("fault target parent is not canonical after the attempt")
		self.collected_log_text()
		raw_logs = self.node_log_path.read_text(errors="replace")
		self.campaign_receipts = parse_proof_campaign_receipts(raw_logs)
		receipt = validated_proof_campaign_receipt(
			self.campaign_receipts, mode=mode, target=target, genesis_hash=genesis_hash
		)
		if str(receipt["parent_hash"]).lower() != parent_hash.lower():
			raise CampaignError("proof campaign receipt parent hash is not canonical")
		write_json(self.campaign_receipt_path, self.campaign_receipts)
		self.stop_live_role("alice")
		self.start_role("alice")
		self.start_role("bob")
		recovered = self.wait_both_same_finalized(target + 1)
		recovered_target_hash = self.canonical_hash(self.orbis_urls[0], target)
		if recovered_target_hash is None:
			raise CampaignError("canonical authoring did not recover the exact target block")
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			mode=mode,
			chain_id=PROOF_CAMPAIGN_CHAIN_ID,
			genesis_hash=genesis_hash,
			armed_command=command,
			canonical_precondition={
				"proof_present": receipt["canonical_proof_present"],
				"proof_sha256": receipt["canonical_proof_sha256"],
				"parent_block": target - 1,
				"parent_hash": parent_hash,
			},
			node_receipt=receipt,
			fault_window=stall,
			recovery={
				"common_finalized_head": recovered,
				"target_block_hash": recovered_target_hash,
				"finalized_at_least": target + 1,
			},
		)
		target_state_unchanged = stall["target_not_imported"] and stall["target_not_finalized"]
		common = {
			"canonical_proof_precondition_some": receipt["canonical_proof_present"] is True,
			"target_block_not_imported_during_fault": stall["target_not_imported"],
		}
		if mode == "Missing":
			self.assertions.update(
				**common,
				missing_proof_injected_by_provider=receipt["action"] == "provider-omitted",
				authoring_failed_closed=(
					receipt["rejection"] == "FinalizationError" and stall["target_not_finalized"]
				),
				restored_node_recovers=bool(recovered_target_hash),
			)
		elif mode == "Invalid":
			self.assertions.update(
				**common,
				invalid_proof_injected_by_provider=receipt["action"] == "provider-invalid",
				invalid_proof_rejected=(
					receipt["rejection"] == "BadMandatory" and stall["target_not_finalized"]
				),
				restored_node_recovers=bool(recovered_target_hash),
			)
		elif mode == "Stale":
			self.assertions.update(
				**common,
				stale_proof_injected_by_provider=receipt["action"] == "provider-stale",
				stale_proof_rejected=(
					receipt["rejection"] == "BadMandatory" and stall["target_not_finalized"]
				),
				canonical_state_unchanged=target_state_unchanged and bool(recovered_target_hash),
			)
		else:
			self.assertions.update(
				**common,
				duplicate_second_push_attempted=receipt["action"] == "duplicate-second-push",
				second_rejected_as_bad_mandatory=receipt["rejection"] == "BadMandatory",
				proposal_not_finalized=stall["target_not_finalized"],
				canonical_state_unchanged=target_state_unchanged and bool(recovered_target_hash),
			)

	def case_fork_reorg(self) -> None:
		target = int(self.fault["target_block"])
		self.stop_both_before_target(margin=5)
		for role in ("alice", "bob"):
			self.start_role(role, partition_command(self.original_commands[role]))
		deadline = time.monotonic() + 90
		competing: dict[str, object] | None = None
		while time.monotonic() < deadline:
			best = [self.height(url)[0] for url in self.orbis_urls]
			height = min(best)
			if height > 0:
				hashes = [self.canonical_hash(url, height) for url in self.orbis_urls]
				if hashes[0] and hashes[1] and hashes[0] != hashes[1]:
					competing = {"height": height, "hashes": hashes, "best": best}
					break
			time.sleep(1)
		if not competing:
			raise CampaignError("partition produced no observable competing Orbis forks")
		orphan_candidates = list(competing["hashes"])  # type: ignore[arg-type]
		self.stop_live_role("alice")
		self.stop_live_role("bob")
		self.start_role("alice")
		self.start_role("bob")
		final_head = self.wait_both_same_finalized(target + 1)
		canonical = self.canonical_hash(self.orbis_urls[0], int(competing["height"]))
		orphans = [value for value in orphan_candidates if value != canonical]
		logs = self.collected_log_text()
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			competing=competing,
			canonical_hash=canonical,
			orphan_hashes=orphans,
			final_head=final_head,
		)
		self.assertions.update(
			competing_forks_observed=len(set(orphan_candidates)) == 2,
			reorg_observed=bool(orphans) and any(
				marker in logs for marker in ("reorg", "retracted", "enact", "fork")
			),
			finalized_branch_proof_valid=(
				bool(final_head) and not any(marker in logs for marker in PROOF_ERROR_MARKERS)
			),
			orphan_state_not_canonical=bool(orphans),
		)

	def case_pruning(self) -> None:
		target = int(self.fault["target_block"])
		pre_target_hash = self.wait_finalized(target - 1)
		before = self.indexed_count(self.store_block, at=pre_target_hash)
		post_hash = self.wait_finalized(target + 2)
		after = self.indexed_count(self.store_block, at=post_hash)
		later = self.wait_both_same_finalized(target + 3)
		for role in ("alice", "bob"):
			self.stop_live_role(role)
		probe = self.index_probe(self.case_dir / "unused-probe.backup")
		self.start_role("alice")
		self.start_role("bob")
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			before_count=before,
			after_count=after,
			post_hash=post_hash,
		)
		self.assertions.update(
			runtime_retention_entry_pruned=after == 0,
			database_reference_released=(
				after == 0 and probe["output"].get("present") is False  # type: ignore[union-attr]
			),
			later_blocks_finalize=bool(later),
			no_premature_prune=before > 0,
		)

	def case_restart(self) -> None:
		target = int(self.fault["target_block"])
		self.wait_finalized(target - 6)
		old = self.processes.discover("bob", 10811)
		old_base = old.base_path
		self.processes.stop(old)
		bob_databases = [path.resolve() for path in old_base.glob("chains/*/db/full") if path.is_dir()]
		if len(bob_databases) != 1:
			raise CampaignError(f"expected one Bob database, found {bob_databases}")
		probe = self.index_probe(
			self.case_dir / "unused-restart-probe.backup", database=bob_databases[0]
		)
		new = self.start_role("bob")
		peers = self.rpc.call(self.orbis_urls[1], "system_peers")
		retained = self.indexed_count(self.store_block, url=self.orbis_urls[1])
		boundary = self.wait_both_same_finalized(target + 1)
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			old_pid=old.pid,
			new_pid=new.pid,
			base_path=str(new.base_path),
		)
		self.assertions.update(
			restart_from_same_database=new.pid != old.pid and new.base_path == old_base,
			retained_body_available=(
				retained > 0 and probe["output"].get("present") is True  # type: ignore[union-attr]
			),
			proof_boundary_finalized=bool(boundary),
			peer_rejoined=isinstance(peers, list) and len(peers) >= 1,
		)

	def launch_observer(
		self, label: str, *, file_limit: int | None = None
	) -> tuple[subprocess.Popen[bytes], Path, int, bool]:
		base = Path(tempfile.mkdtemp(prefix=f"orbis-proof-{label}-"))
		self.temp_dirs.append(base)
		initially_empty = not any(base.iterdir())
		port = 10812
		command = observer_command(
			self.original_commands["alice"],
			self.binary("origin_orbis"),
			base,
			f"proof-{label}-{self.case}",
			port,
			30812,
		)
		preexec = None
		if file_limit is not None:
			def limit_files() -> None:
				resource.setrlimit(resource.RLIMIT_FSIZE, (file_limit, file_limit))
			preexec = limit_files
		process, log = self.processes.start(
			command,
			label,
			port,
			kind="temporary-observer",
			preexec_fn=preexec,
			expect_ready=file_limit is None,
			timeout=180,
		)
		self.case_logs.append(log)
		return process, base, port, initially_empty

	def wait_observer_sync(self, port: int, timeout: int = 300) -> tuple[str, str]:
		url = f"http://127.0.0.1:{port}"
		deadline = time.monotonic() + timeout
		while time.monotonic() < deadline:
			best = str(self.rpc.call(url, "chain_getBlockHash", []))
			finalized = str(self.rpc.call(url, "chain_getFinalizedHead"))
			canonical_best = str(self.rpc.call(self.orbis_urls[0], "chain_getBlockHash", []))
			canonical_finalized = str(self.rpc.call(self.orbis_urls[0], "chain_getFinalizedHead"))
			if best == canonical_best and finalized == canonical_finalized:
				return best, finalized
			time.sleep(2)
		raise CampaignError("fresh observer did not fully sync")

	def case_fresh_resync(self) -> None:
		target = int(self.fault["target_block"])
		process, base, port, fresh_before = self.launch_observer("fresh")
		best, finalized = self.wait_observer_sync(port)
		indexed = self.indexed_count(self.store_block, url=f"http://127.0.0.1:{port}")
		code = self.processes.terminate_process(process)
		fresh_databases = [path.resolve() for path in base.glob("chains/*/db/full") if path.is_dir()]
		if len(fresh_databases) != 1:
			raise CampaignError(f"expected one fresh observer database, found {fresh_databases}")
		probe = self.index_probe(
			self.case_dir / "unused-fresh-probe.backup", database=fresh_databases[0]
		)
		boundary = self.wait_both_same_finalized(target + 1)
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			observer_pid=process.pid,
			exit_code=code,
		)
		self.assertions.update(
			fresh_base_path=fresh_before,
			indexed_body_synced=(
				indexed > 0 and probe["output"].get("present") is True  # type: ignore[union-attr]
			),
			best_hash_matches=bool(best),
			finalized_hash_matches=bool(finalized) and bool(boundary),
		)

	def case_disk_pressure(self) -> None:
		target = int(self.fault["target_block"])
		self.wait_finalized(target + 1)
		process, base, port, _ = self.launch_observer("disk-limited", file_limit=1_048_576)
		try:
			code = process.wait(timeout=90)
		except subprocess.TimeoutExpired:
			self.processes.terminate_process(process)
			raise CampaignError("disk-limited observer did not fail under RLIMIT_FSIZE")
		self.processes.mark_stopped(process.pid)
		disk_log = self.case_logs[-1].read_text(errors="replace").lower()
		command = observer_command(
			self.original_commands["alice"],
			self.binary("origin_orbis"),
			base,
			f"proof-disk-recovery-{self.case}",
			port,
			30812,
		)
		recovered, recovery_log = self.processes.start(
			command, "disk-recovery", port, kind="temporary-observer", timeout=180
		)
		self.case_logs.append(recovery_log)
		best, finalized = self.wait_observer_sync(port)
		recovery_code = self.processes.terminate_process(recovered)
		self.fault.update(
			observed_at_block=target,
			reverted=True,
			limited_pid=process.pid,
			limited_exit_code=code,
			recovery_pid=recovered.pid,
			recovery_exit_code=recovery_code,
		)
		self.assertions.update(
			disk_fault_observed=any(
				marker in disk_log for marker in ("file too large", "disk", "rocksdb", "io error")
			),
			process_failed_nonzero=code != 0,
			database_reopens=bool(best),
			node_resyncs_after_recovery=bool(finalized),
		)

	def database_bytes(self) -> int:
		total = 0
		for base in self.original_bases.values():
			for path in base.rglob("*"):
				try:
					if path.is_file():
						total += path.stat().st_size
				except FileNotFoundError:
					pass
		return total

	def capacity_samples(self, count: int = 16) -> list[dict[str, object]]:
		finalized, _ = self.height(self.orbis_urls[0], finalized=True)
		start = max(1, finalized - count + 1)
		samples = []
		for number in range(start, finalized + 1):
			block_hash = self.canonical_hash(self.orbis_urls[0], number)
			if not block_hash:
				continue
			block = self.rpc.call(self.orbis_urls[0], "chain_getBlock", [block_hash])
			extrinsics = block.get("block", {}).get("extrinsics", []) if isinstance(block, dict) else []
			if not isinstance(extrinsics, list) or not all(isinstance(item, str) for item in extrinsics):
				raise CampaignError(f"chain_getBlock returned malformed extrinsics at {block_hash}")
			rpc_extrinsic_lengths = [
				len(bytes.fromhex(item.removeprefix("0x"))) for item in extrinsics
			]
			version = self.rpc.call(
				self.orbis_urls[0], "state_getRuntimeVersion", [block_hash]
			)
			if not isinstance(version, dict) or (
				version.get("specName"),
				version.get("specVersion"),
				version.get("transactionVersion"),
			) != ("orbis", 29, 8):
				raise CampaignError(f"runtime version drift at {block_hash}: {version}")
			measure = self.run_tool(
				["measure-block", "--block-hash", block_hash], f"measure-block-{number}"
			)
			operands = validated_block_measurement(measure, block_hash, rpc_extrinsic_lengths)
			best, _ = self.height(self.orbis_urls[0])
			current_finalized, _ = self.height(self.orbis_urls[0], finalized=True)
			samples.append(
				{
					"block_number": number,
					"block_hash": block_hash,
					"storage_tx_count": self.indexed_count(number, at=block_hash),
					**operands,
					"database_bytes": self.database_bytes(),
					"finality_lag_blocks": best - current_finalized,
				}
			)
		return samples

	def run_case(self) -> None:
		dispatch = {
			"two_collator_proof_production": self.case_two_collator,
			"missing_retained_body_or_proof": lambda: self.case_proof_campaign_fault("Missing"),
			"late_proof": lambda: self.case_proof_campaign_fault("Stale"),
			"invalid_proof": lambda: self.case_proof_campaign_fault("Invalid"),
			"duplicate_proof": lambda: self.case_proof_campaign_fault("Duplicate"),
			"fork_reorg_retention_boundary": self.case_fork_reorg,
			"database_pruning_boundary": self.case_pruning,
			"collator_restart_retained_state": self.case_restart,
			"fresh_node_resync": self.case_fresh_resync,
			"disk_pressure_failure_recovery": self.case_disk_pressure,
		}
		dispatch[self.case]()

	def cleanup_all(self) -> None:
		errors: list[str] = []
		try:
			if self.original_commands:
				self.restore_default_collators()
		except Exception as error:
			errors.append(f"restore topology: {error}")
		for process in list(self.processes.started):
			# Topology replacements remain registered for orchestrator teardown; only observers
			# must be stopped by the case itself.
			rows = self.processes._registry_rows()
			row = next((value for value in rows if value.get("pid") == process.pid), {})
			if row.get("kind") == "temporary-observer" and process.poll() is None:
				try:
					self.processes.terminate_process(process)
				except Exception as error:
					errors.append(f"terminate observer {process.pid}: {error}")
		for directory in self.temp_dirs:
			try:
				shutil.rmtree(directory)
			except FileNotFoundError:
				pass
			except Exception as error:
				errors.append(f"remove {directory}: {error}")
		self.fault["reverted"] = not errors
		self.cleanup = {
			"status": "pass" if not errors else "fail",
			"fault_state_restored": not errors,
			"temporary_processes_stopped": not errors,
			"errors": errors,
		}
		write_json(self.cleanup_path, self.cleanup)

	def artifacts(self) -> list[dict[str, str]]:
		self.collected_log_text()
		if not self.campaign_receipt_path.exists():
			write_json(self.campaign_receipt_path, self.campaign_receipts)
		paths = {
			"signed-receipt": self.signed_path,
			"rpc-trace": self.rpc_path,
			"node-log": self.node_log_path,
			"fault-receipt": self.fault_path,
			"proof-campaign-node-receipts": self.campaign_receipt_path,
			"cleanup-receipt": self.cleanup_path,
			"capacity-samples": self.capacity_path,
		}
		artifacts = []
		for kind, path in paths.items():
			if not path.is_file() or path.stat().st_size <= 0:
				raise CampaignError(f"required raw artifact absent/empty: {path}")
			artifacts.append(
				{
					"kind": kind,
					"path": path.relative_to(self.evidence).as_posix(),
					"sha256": sha256(path),
				}
			)
		return artifacts

	def execute(self) -> int:
		try:
			self.discover_topology()
			self.signed_setup_and_store()
			self.run_case()
		except Exception as error:
			self.error = f"{type(error).__name__}: {error}"
		finally:
			self.cleanup_all()
		try:
			samples = self.capacity_samples()
		except Exception as error:
			samples = []
			self.error = self.error or f"capacity sampling: {error}"
		capacity = capacity_verdict(samples, self.limit) if samples and self.limit else {
			"status": "fail",
			"reason": "capacity samples or runtime-configured limit are absent",
		}
		write_json(
			self.capacity_path,
			{
				"runtime_constants": self.inspect,
				"samples": samples,
				"ac13_p1": capacity,
				"p6_mixed_workload_slo_claimed": False,
			},
		)
		write_json(self.fault_path, self.fault)
		write_json(self.campaign_receipt_path, self.campaign_receipts)
		write_json(self.signed_path, self.signed_raw)
		write_json(self.rpc_path, self.rpc.records)
		try:
			artifacts = self.artifacts()
		except Exception as error:
			artifacts = []
			self.error = self.error or f"artifact capture: {error}"
		passed = (
			self.error is None
			and all(self.assertions.values())
			and self.cleanup["status"] == "pass"
			and bool(artifacts)
		)
		result = {
			"schema_version": 3,
			"case": self.case,
			"status": "pass" if passed else "fail",
			"manifest_sha256": self.context["manifest_sha256"],
			"topology_sha256": self.context["topology_sha256"],
			"driver_sha256": self.context["driver_sha256"],
			"result_schema_sha256": self.context["result_schema_sha256"],
			"chain_specs_sha256": self.context["chain_specs_sha256"],
			"binaries_sha256": self.context["binaries_sha256"],
			"signed_receipts": self.signed_receipts,
			"fault": self.fault,
			"assertions": self.assertions,
			"cleanup": self.cleanup,
			"capacity_ac13_p1": capacity,
			"artifacts": artifacts,
			"error": self.error,
		}
		write_json(self.output, result)
		return 0


def parse_args() -> argparse.Namespace:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument("--case", required=True, choices=CASE_ORDER)
	parser.add_argument("--context", type=Path, required=True)
	parser.add_argument("--output", type=Path, required=True)
	return parser.parse_args()


def main() -> int:
	args = parse_args()
	if args.output.exists():
		print(f"refusing to overwrite existing result: {args.output}", file=sys.stderr)
		return 2
	try:
		context = json.loads(args.context.read_text())
		runner = CaseRunner(args.case, context, args.output)
		return runner.execute()
	except Exception as error:
		write_json(
			args.output,
			{
				"schema_version": 3,
				"case": args.case,
				"status": "fail",
				"error": f"{type(error).__name__}: {error}",
			},
		)
		return 0


if __name__ == "__main__":
	raise SystemExit(main())
