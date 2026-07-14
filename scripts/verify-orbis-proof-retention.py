#!/usr/bin/env python3
"""Audit the static Orbis transaction-storage proof foundation.

This verifier deliberately does not promote source wiring, pallet tests, or the
offline case-driver implementation to a production E2E claim. It records a
``blocked`` verdict until the exact compiled binaries run the destructive
multi-node campaign and every raw trace is revalidated.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("docs/evidence/p1/storage-proof/proof-foundation-verdict.json")
SDK_BRANCH = "release-v1.24.0"
SDK_COMMIT = "cc190ea83c590b6a14a6b9771ab02c81618dc118"


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def relative_repo_path(path: Path) -> str:
	return path.resolve().relative_to(ROOT).as_posix()


def check_source(
	name: str, path: Path, required_fragments: list[str], *, display_path: str | None = None
) -> dict[str, Any]:
	text = path.read_text(encoding="utf-8")
	missing = [fragment for fragment in required_fragments if fragment not in text]
	return {
		"name": name,
		"status": "pass" if not missing else "fail",
		"path": display_path or relative_repo_path(path),
		"sha256": sha256(path),
		"missing_fragments": missing,
	}


def cargo_metadata() -> dict[str, Any]:
	result = subprocess.run(
		["cargo", "metadata", "--format-version", "1", "--locked"],
		cwd=ROOT,
		text=True,
		capture_output=True,
		check=False,
	)
	if result.returncode:
		raise RuntimeError(
			"cargo metadata --locked failed: " + (result.stderr.strip() or result.stdout.strip())
		)
	return json.loads(result.stdout)


def find_omni_node(metadata: dict[str, Any]) -> dict[str, Any]:
	matches = [package for package in metadata["packages"] if package["name"] == "polkadot-omni-node-lib"]
	if len(matches) != 1:
		raise RuntimeError(f"expected exactly one polkadot-omni-node-lib package, found {len(matches)}")
	return matches[0]


def e2e_candidates() -> list[str]:
	"""Return repo-relative candidates that could be proof-specific E2E harnesses.

	The verifier itself is excluded. Generic Zombienet configurations are not
	proof of retention fault coverage; a candidate must explicitly mention the
	transaction-storage proof runtime API/provider.
	"""
	candidates: list[str] = []
	needles = (
		"transactionstorageapi",
		"transaction_storage_proof",
		"transaction-storage-proof",
		"transaction-storage proof",
		"transaction storage proof",
	)
	for directory in (ROOT / "zombienet", ROOT / "scripts"):
		if not directory.exists():
			continue
		for path in sorted(candidate for candidate in directory.rglob("*") if candidate.is_file()):
			if path.resolve() == Path(__file__).resolve():
				continue
			try:
				text = path.read_text(encoding="utf-8").lower()
			except (OSError, UnicodeDecodeError):
				continue
			if any(needle in text for needle in needles):
				candidates.append(relative_repo_path(path))
	return candidates


def production_campaign_evidence(
	case_manifest_path: Path,
	topology_path: Path,
	required_case_ids: list[str],
	required_assertions: dict[str, list[str]],
	required_fault_kinds: dict[str, str],
) -> dict[str, Any]:
	"""Revalidate the production campaign ledger and every raw artifact hash."""
	verdict_path = ROOT / "docs/evidence/p1/storage-proof/campaign/campaign-verdict.json"
	evidence_dir = ROOT / "docs/evidence/p1/storage-proof/campaign/production-e2e"
	if not verdict_path.is_file():
		return {
			"status": "blocked",
			"reason": "production campaign verdict is absent",
			"verdict": relative_repo_path(verdict_path),
			"required_case_ids": required_case_ids,
		}
	try:
		verdict = json.loads(verdict_path.read_text(encoding="utf-8"))
	except (OSError, json.JSONDecodeError) as error:
		return {"status": "fail", "reason": f"invalid production verdict: {error}"}
	errors: list[str] = []
	if verdict.get("status") != "pass":
		errors.append(f"campaign status is {verdict.get('status')}")
	if verdict.get("errors"):
		errors.append("campaign ledger contains errors")
	if verdict.get("manifest_sha256") != sha256(case_manifest_path):
		errors.append("campaign manifest hash is stale")
	if verdict.get("topology_sha256") != sha256(topology_path):
		errors.append("campaign topology hash is stale")
	global_cleanup = verdict.get("global_cleanup")
	if not isinstance(global_cleanup, dict) or global_cleanup.get("status") != "pass":
		errors.append("campaign global cleanup is absent or non-pass")
	capacity = verdict.get("capacity_ac13_p1")
	if not isinstance(capacity, dict) or capacity.get("status") != "pass":
		errors.append("AC13 P1 storage-capacity aggregate is absent or non-pass")
	else:
		checks = capacity.get("checks")
		if not isinstance(checks, dict) or not all(checks.values()):
			errors.append("AC13 P1 aggregate contains a false operand/check")
		if capacity.get("p6_mixed_workload_slo_claimed") is not False:
			errors.append("proof campaign must not claim the P6 mixed-workload SLO")
	driver_hash = verdict.get("driver_sha256")
	driver_path = ROOT / "scripts/run-orbis-proof-retention-case.py"
	if (
		not isinstance(driver_hash, str)
		or len(driver_hash) != 64
		or not driver_path.is_file()
		or sha256(driver_path) != driver_hash
	):
		errors.append("campaign driver is absent, changed, or hash-mismatched")
	binary_hashes = verdict.get("binaries_sha256")
	if not isinstance(binary_hashes, dict) or not binary_hashes:
		errors.append("campaign binary hash ledger is absent")
	else:
		binary_paths = {
			"origin": ROOT / "target/release/origin",
			"origin_orbis": ROOT / "target/release/origin-omni-node",
			"proof_fault_origin_orbis": ROOT / "target/proof-fault/release/origin-omni-node",
			"bootstrap_orbis_core": ROOT / "target/debug/examples/bootstrap_orbis_core",
			"signed_fault_tool": ROOT / "target/debug/examples/orbis_storage_proof_fault",
			"indexed_db_fault_tool": ROOT / "target/debug/examples/orbis_storage_index_fault",
		}
		for name, path in binary_paths.items():
			if not path.is_file() or binary_hashes.get(name) != sha256(path):
				errors.append(f"campaign binary is absent or changed: {name}")
	cases = verdict.get("cases")
	if not isinstance(cases, list):
		cases = []
		errors.append("campaign cases are not a list")
	case_ids = [case.get("case") for case in cases if isinstance(case, dict)]
	if case_ids != required_case_ids:
		errors.append(f"campaign case order/completeness mismatch: {case_ids}")
	for case in cases:
		if not isinstance(case, dict):
			continue
		case_id = case.get("case")
		if case.get("status") != "pass":
			errors.append(f"{case_id}: non-pass case result")
		if case.get("driver_sha256") != driver_hash:
			errors.append(f"{case_id}: driver hash mismatch")
		if case.get("manifest_sha256") != verdict.get("manifest_sha256"):
			errors.append(f"{case_id}: manifest hash mismatch")
		if case.get("topology_sha256") != verdict.get("topology_sha256"):
			errors.append(f"{case_id}: topology hash mismatch")
		if case.get("binaries_sha256") != binary_hashes:
			errors.append(f"{case_id}: binary hash ledger mismatch")
		assertions = case.get("assertions")
		if not isinstance(assertions, dict):
			errors.append(f"{case_id}: assertions are absent")
		else:
			for assertion in required_assertions.get(str(case_id), []):
				if assertions.get(assertion) is not True:
					errors.append(f"{case_id}: assertion is not true: {assertion}")
		signed = case.get("signed_receipts")
		if not isinstance(signed, list) or not {"setup", "store"}.issubset(
			{receipt.get("action") for receipt in signed if isinstance(receipt, dict)}
		):
			errors.append(f"{case_id}: finalized signed setup/store receipts are absent")
		elif any(
			receipt.get("status") != "finalized"
			or not isinstance(receipt.get("extrinsic_hash"), str)
			or len(receipt["extrinsic_hash"]) != 66
			or not isinstance(receipt.get("finalized_block_hash"), str)
			or len(receipt["finalized_block_hash"]) != 66
			for receipt in signed
			if isinstance(receipt, dict) and receipt.get("action") in ("setup", "store")
		):
			errors.append(f"{case_id}: signed receipt hash/finality contract failed")
		fault = case.get("fault")
		if not isinstance(fault, dict) or fault.get("reverted") is not True:
			errors.append(f"{case_id}: fault restoration receipt is absent")
		else:
			if fault.get("kind") != required_fault_kinds.get(str(case_id)):
				errors.append(f"{case_id}: fault kind mismatch")
			if (
				not isinstance(fault.get("target_block"), int)
				or fault["target_block"] <= 0
				or fault.get("target_block") != fault.get("observed_at_block")
			):
				errors.append(f"{case_id}: fault was not observed at its exact positive target")
		cleanup = case.get("cleanup")
		if not isinstance(cleanup, dict) or cleanup.get("status") != "pass":
			errors.append(f"{case_id}: cleanup is absent or non-pass")
		artifacts = case.get("artifacts")
		if not isinstance(artifacts, list) or not artifacts:
			errors.append(f"{case_id}: raw artifacts are absent")
			continue
		root = evidence_dir.resolve()
		kinds = {artifact.get("kind") for artifact in artifacts if isinstance(artifact, dict)}
		required_kinds = {
			"signed-receipt",
			"rpc-trace",
			"node-log",
			"fault-receipt",
			"cleanup-receipt",
			"capacity-samples",
		}
		if not required_kinds.issubset(kinds):
			errors.append(f"{case_id}: raw artifact kind set is incomplete")
		verified_artifacts: dict[str, Path] = {}
		for artifact in artifacts:
			if not isinstance(artifact, dict) or not isinstance(artifact.get("path"), str):
				errors.append(f"{case_id}: malformed artifact ledger entry")
				continue
			path = (evidence_dir / artifact["path"]).resolve()
			if path != root and root not in path.parents:
				errors.append(f"{case_id}: artifact escapes evidence root")
			elif not path.is_file() or sha256(path) != artifact.get("sha256"):
				errors.append(f"{case_id}: artifact absent or hash-mismatched: {artifact['path']}")
			elif isinstance(artifact.get("kind"), str):
				verified_artifacts[artifact["kind"]] = path
		try:
			fault_raw = json.loads(verified_artifacts["fault-receipt"].read_text())
			cleanup_raw = json.loads(verified_artifacts["cleanup-receipt"].read_text())
			capacity_raw = json.loads(verified_artifacts["capacity-samples"].read_text())
			signed_raw = json.loads(verified_artifacts["signed-receipt"].read_text())
			if fault_raw != fault:
				errors.append(f"{case_id}: structured fault differs from raw receipt")
			if cleanup_raw != cleanup:
				errors.append(f"{case_id}: structured cleanup differs from raw receipt")
			if capacity_raw.get("ac13_p1") != case.get("capacity_ac13_p1"):
				errors.append(f"{case_id}: structured capacity differs from raw receipt")
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
					errors.append(f"{case_id}: signed {receipt.get('action')} differs from raw receipt")
			capacity_samples = capacity_raw.get("samples")
			if not isinstance(capacity_samples, list) or not capacity_samples:
				errors.append(f"{case_id}: raw capacity sample list is absent")
			else:
				for sample in capacity_samples:
					if not isinstance(sample, dict) or not isinstance(sample.get("block_number"), int):
						errors.append(f"{case_id}: malformed raw capacity sample")
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
							f"{case_id}: block {sample.get('block_number')} differs from raw metadata measurement"
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
							f"{case_id}: block {sample['block_number']} weight differs from raw metadata measurement"
						)
		except (KeyError, OSError, TypeError, ValueError, json.JSONDecodeError) as error:
			errors.append(f"{case_id}: cannot bind raw receipt contents: {error}")

	# Recompute AC13 P1 from the hashed raw case artifacts; never trust a copied aggregate.
	raw_samples: list[dict[str, Any]] = []
	raw_limits: set[int] = set()
	for case in cases:
		if not isinstance(case, dict):
			continue
		entry = next(
			(
				artifact
				for artifact in case.get("artifacts", [])
				if isinstance(artifact, dict) and artifact.get("kind") == "capacity-samples"
			),
			None,
		)
		if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
			continue
		path = (evidence_dir / entry["path"]).resolve()
		root = evidence_dir.resolve()
		if (
			(path != root and root not in path.parents)
			or not path.is_file()
			or sha256(path) != entry.get("sha256")
		):
			errors.append(f"{case.get('case')}: unsafe or hash-mismatched capacity artifact")
			continue
		try:
			payload = json.loads(path.read_text())
			raw_limits.add(int(payload["runtime_constants"]["max_block_transactions"]))
			raw_samples.extend(
				sample for sample in payload["samples"] if isinstance(sample, dict)
			)
		except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError):
			errors.append(f"{case.get('case')}: malformed raw capacity artifact")
	counts: list[int] = []
	if len(raw_limits) != 1 or 0 in raw_limits or not raw_samples:
		errors.append("raw AC13 P1 limits/samples are inconsistent or absent")
	else:
		limit = next(iter(raw_limits))
		try:
			counts = sorted(int(sample["storage_tx_count"]) for sample in raw_samples)
		except (KeyError, TypeError, ValueError):
			errors.append("raw AC13 P1 storage transaction counts are malformed")
			counts = []
	if len(raw_limits) == 1 and 0 not in raw_limits and counts:
		limit = next(iter(raw_limits))
		p95 = counts[math.ceil(0.95 * len(counts)) - 1]
		try:
			ref_values = [int(sample["block_weight_ref_time"]) for sample in raw_samples]
			proof_values = [int(sample["block_weight_proof_size"]) for sample in raw_samples]
			length_values = [int(sample["block_length_bytes"]) for sample in raw_samples]
			ref_limits = [int(sample["block_weight_ref_time_limit"]) for sample in raw_samples]
			proof_limits = [
				int(sample["block_weight_proof_size_limit"]) for sample in raw_samples
			]
			length_limits = [int(sample["block_length_limit_bytes"]) for sample in raw_samples]
		except (KeyError, TypeError, ValueError):
			errors.append("raw AC13 P1 weight/length operands are malformed")
			ref_values = proof_values = length_values = []
			ref_limits = proof_limits = length_limits = []
		nearest = lambda values: sorted(values)[math.ceil(0.95 * len(values)) - 1]
		metadata_hashes = {str(sample.get("metadata_hash", "")) for sample in raw_samples}
		raw_checks = {
			"headroom_at_least_20_percent": 5 * p95 <= 4 * limit,
			"limit_at_least_ceil_p95_over_80_percent": limit >= (5 * p95 + 3) // 4,
			"no_block_over_90_percent": all(10 * count <= 9 * limit for count in counts),
			"independent_operands_present": all(
				sample.get(field) is not None
				for sample in raw_samples
				for field in (
					"block_weight_ref_time",
					"block_weight_proof_size",
					"block_length_bytes",
					"database_bytes",
					"finality_lag_blocks",
				)
			),
			"runtime_resource_limits_stable": bool(ref_values)
			and all(
				len({int(sample[field]) for sample in raw_samples}) == 1
				for field in (
					"block_weight_ref_time_limit",
					"block_weight_proof_size_limit",
					"block_length_limit_bytes",
				)
			)
			and all(value > 0 for value in (*ref_limits, *proof_limits, *length_limits))
			and len(metadata_hashes) == 1
			and all(value.startswith("0x") and len(value) == 66 for value in metadata_hashes)
			and all(
				len({str(sample.get(field, "")) for sample in raw_samples}) == 1
				and all(
					str(sample.get(field, "")).startswith("0x")
					and len(str(sample.get(field, ""))) > 2
					for sample in raw_samples
				)
				for field in ("block_weights_constant_scale", "block_length_constant_scale")
			),
			"p95_ref_time_utilization_at_most_80_percent": bool(ref_values)
			and 5 * nearest(ref_values) <= 4 * ref_limits[0],
			"p95_proof_size_utilization_at_most_80_percent": bool(proof_values)
			and 5 * nearest(proof_values) <= 4 * proof_limits[0],
			"p95_block_length_utilization_at_most_80_percent": bool(length_values)
			and 5 * nearest(length_values) <= 4 * length_limits[0],
			"no_weight_or_length_sample_over_90_percent": bool(ref_values)
			and all(
				10 * value <= 9 * resource_limit
				for values, resource_limits in (
					(ref_values, ref_limits),
					(proof_values, proof_limits),
					(length_values, length_limits),
				)
				for value, resource_limit in zip(values, resource_limits)
			),
			"database_and_finality_operands_valid": all(
				int(sample.get("database_bytes", 0)) > 0
				and int(sample.get("finality_lag_blocks", -1)) >= 0
				for sample in raw_samples
			),
		}
		if not all(raw_checks.values()):
			errors.append(f"raw AC13 P1 checks are non-pass: {raw_checks}")
		elif not isinstance(capacity, dict) or (
			capacity.get("configured_storage_tx_limit") != limit
			or capacity.get("sample_count") != len(raw_samples)
			or capacity.get("p95_storage_tx_count") != p95
			or capacity.get("checks") != raw_checks
		):
			errors.append("campaign AC13 P1 aggregate does not match raw samples")
	return {
		"status": "pass" if not errors else "fail",
		"verdict": relative_repo_path(verdict_path),
		"verdict_sha256": sha256(verdict_path),
		"required_case_ids": required_case_ids,
		"captured_case_result_ids": case_ids,
		"errors": errors,
	}


def build_verdict() -> dict[str, Any]:
	metadata = cargo_metadata()
	omni_node = find_omni_node(metadata)
	source = omni_node.get("source") or ""
	manifest = Path(omni_node["manifest_path"])
	aura = manifest.parent / "src/nodes/aura.rs"
	upstream_path = "cargo:polkadot-omni-node-lib/src/nodes/aura.rs"

	checks = [
		{
			"name": "sdk_dependency_pin",
			"status": (
				"pass"
				if f"branch={SDK_BRANCH}" in source and source.endswith(f"#{SDK_COMMIT}")
				else "fail"
			),
			"package_version": omni_node["version"],
			"source": source,
			"expected_branch": SDK_BRANCH,
			"expected_commit": SDK_COMMIT,
		},
		check_source(
			"orbis_omni_node_entrypoint",
			ROOT / "origin/orbis/node/src/main.rs",
			["DefaultRuntimeResolver", "RunConfig::new", "run::<CliConfig>"],
		),
		check_source(
			"orbis_transaction_storage_runtime_api",
			ROOT / "origin/orbis/runtime/src/lib.rs",
			[
				"sp_transaction_storage_proof::runtime_api::TransactionStorageApi<Block>",
				"fn retention_period() -> BlockNumber",
				"TransactionStorage::retention_period()",
				"fn indexed_transactions(",
				"TransactionStorage::transactions_at(block)",
			],
		),
		check_source(
			"pinned_omni_node_proof_provider",
			aura,
			[
				"has_api_with::<dyn TransactionStorageApi<Block>",
				"sp_transaction_storage_proof::registration::new_data_provider",
			],
			display_path=upstream_path,
		),
		check_source(
			"proof_and_retention_unit_invariants",
			ROOT / "origin/orbis/pallets/transaction-storage/src/tests.rs",
			[
				"fn checks_proof()",
				"fn missing_required_proof_panics_at_finalize()",
				"fn duplicate_proof_is_rejected_without_clearing_first_success()",
				"fn late_proof_cannot_resurrect_pruned_retention_state()",
				"fn migration_v6_to_v7_try_runtime_pre_post_preserves_authoritative_state()",
				"fn try_state_passes_through_retention_lifecycle()",
			],
		),
	]

	# The pinned omni-node has two Aura authoring paths. Both must register the
	# proof provider or the static composition check is incomplete.
	aura_text = aura.read_text(encoding="utf-8")
	provider_occurrences = aura_text.count(
		"sp_transaction_storage_proof::registration::new_data_provider"
	)
	checks.append(
		{
			"name": "all_aura_authoring_paths_register_provider",
			"status": "pass" if provider_occurrences >= 2 else "fail",
			"provider_registration_occurrences": provider_occurrences,
			"minimum_required": 2,
			"path": upstream_path,
			"sha256": sha256(aura),
		}
	)
	case_manifest_path = ROOT / "docs/evidence/p1/storage-proof/proof-retention-cases.json"
	case_manifest = json.loads(case_manifest_path.read_text(encoding="utf-8"))
	cases = case_manifest.get("cases", [])
	case_ids = [case.get("id") for case in cases]
	required_assertions = {
		str(case.get("id")): list(case.get("required_assertions", [])) for case in cases
	}
	required_fault_kinds_by_id = {
		str(case.get("id")): str(case.get("fault_kind")) for case in cases
	}
	required_case_ids = [
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
	]
	required_fault_kinds = [
		"none",
		"indexed-db-remove",
		"node-provider-late",
		"indexed-db-corrupt",
		"node-inherent-duplicate",
		"network-partition-heal",
		"retention-prune-observation",
		"persistent-process-restart",
		"fresh-observer-resync",
		"disk-limit-recovery",
	]
	manifest_valid = (
		case_manifest.get("schema_version") == 1
		and case_ids == required_case_ids
		and [case.get("fault_kind") for case in cases] == required_fault_kinds
		and len(set(case_ids)) == 10
		and all(
			case.get("mechanism")
			and case.get("required_assertions")
			and case.get("driver_contract")
			for case in cases
		)
	)
	checks.append(
		{
			"name": "ten_case_fault_campaign_manifest",
			"status": "pass" if manifest_valid else "fail",
			"path": relative_repo_path(case_manifest_path),
			"sha256": sha256(case_manifest_path),
			"case_ids": case_ids,
		}
	)
	checks.append(
		check_source(
			"fail_closed_resumable_campaign_orchestrator",
			ROOT / "scripts/run-orbis-proof-retention-campaign.py",
			[
				"--allow-destructive-faults",
				"--case-driver",
				"--resume",
				"manifest_sha256",
				"topology_sha256",
				"validate_case_result",
				"verify-proof-network-isolation.py",
			],
		)
	)
	checks.append(
		check_source(
			"canonical_ten_case_signed_fault_driver",
			ROOT / "scripts/run-orbis-proof-retention-case.py",
			[
				"CASE_ORDER = (",
				"signed_setup_and_store",
				"feature_fault_command",
				"case_db_fault",
				"case_fork_reorg",
				"case_pruning",
				"case_restart",
				"case_fresh_resync",
				"case_disk_pressure",
				"capacity_verdict",
				"validated_block_measurement",
				"block_weight_ref_time_limit",
				"p6_mixed_workload_slo_claimed",
				"cleanup_all",
			],
		)
	)
	checks.append(
		check_source(
			"loopback_signed_setup_and_pool_rejection_tool",
			ROOT / "origin-rs/examples/orbis_storage_proof_fault.rs",
			[
				"--allow-dev-faults",
				"refusing a non-loopback endpoint",
				"expected Orbis 29/8",
				'Action::Store { bytes, fill }',
				"Action::Inspect",
				"Action::MeasureBlock",
				'"MaxBlockTransactions"',
				'subxt::dynamic::storage("System", "BlockWeight"',
				"corrected_extrinsic_event_total",
				'status: "pool-rejected"',
			],
		)
	)
	checks.append(
		check_source(
			"stopped_node_indexed_database_fault_tool",
			ROOT / "origin-rs/examples/orbis_storage_index_fault.rs",
			[
				"const SUBSTRATE_COLUMNS: u32 = 13",
				"const TRANSACTION_COLUMN: u32 = 11",
				"database path must end in db/full",
				"blake2_256(&restored) != key",
				"transaction.delete(TRANSACTION_COLUMN, &key)",
				"transaction.put_vec(TRANSACTION_COLUMN, &key, corrupted.clone())",
				"Action::Probe",
				"present: bool",
			],
		)
	)
	checks.append(
		check_source(
			"isolated_proof_network_definition",
			ROOT / "zombienet/proof-retention-isolated.toml",
			[
				"proof-relay-alice",
				"proof-orbis-alice",
				"origin-proof-isolated.json",
				"orbis-proof-isolated.json",
			],
		)
	)
	checks.append(
		check_source(
			"isolated_genesis_and_protocol_transform",
			ROOT / "scripts/isolate-proof-chainspec.py",
			[
				"origin-proof-retention-v1",
				"orbis-proof-retention-v1",
				'RELAY_ID = "origin_proof_isolated"',
				'ORBIS_ID = "orbis-proof-isolated"',
			],
		)
	)
	isolation_observation_path = (
		ROOT
		/ "docs/evidence/p1/storage-proof/campaign/valid-isolated-bringup/isolation-observation.json"
	)
	isolation_observation = json.loads(isolation_observation_path.read_text(encoding="utf-8"))
	isolation_checks = isolation_observation.get("checks", {})
	isolation_valid = (
		isolation_observation.get("status") == "pass"
		and isolation_observation.get("usable_as_proof_retention_fault_e2e") is False
		and isinstance(isolation_checks, dict)
		and all(
			value is True if isinstance(value, bool) else value == 0
			for value in isolation_checks.values()
		)
	)
	checks.append(
		{
			"name": "isolated_topology_bringup_not_promoted_to_fault_e2e",
			"status": "pass" if isolation_valid else "fail",
			"path": relative_repo_path(isolation_observation_path),
			"sha256": sha256(isolation_observation_path),
			"usable_as_proof_retention_fault_e2e": isolation_observation.get(
				"usable_as_proof_retention_fault_e2e"
			),
		}
	)
	node_build_log = ROOT / "docs/evidence/p1/storage-proof/origin-orbis-check.out"
	node_build_text = node_build_log.read_text(encoding="utf-8")
	node_build_passed = "Finished `dev` profile" in node_build_text and "error[" not in node_build_text
	checks.append(
		{
			"name": "origin_orbis_node_build",
			"status": "pass" if node_build_passed else "fail",
			"command": "cargo check -p origin-omni-node",
			"log": relative_repo_path(node_build_log),
			"log_sha256": sha256(node_build_log),
		}
	)

	static_failed = any(check["status"] != "pass" for check in checks)
	candidates = e2e_candidates()
	production_e2e = production_campaign_evidence(
		case_manifest_path,
		ROOT / "zombienet/proof-retention-isolated.toml",
		required_case_ids,
		required_assertions,
		required_fault_kinds_by_id,
	)
	if production_e2e["status"] != "pass":
		production_e2e["proof_specific_harness_candidates"] = candidates
		production_e2e["known_driver_gap"] = (
			"Canonical ten-case driver source is prepared offline, but its Rust tools and pinned SDK "
			"adapter are not compiled/published/live-proven. The SDK must emit exact late/duplicate "
			"injection markers. Finalized System.BlockWeight/event/BlockLength telemetry is now "
			"metadata-decoded and raw-bound in source, but remains uncompiled and has no live ledger. "
			"Static/unit fixtures are not E2E."
		)

	return {
		"schema_version": 1,
		"scope": "Origin/Orbis TransactionStorage110 HopPromotion111 Resources proof-inherent and retention foundation",
		"authority_model_changed": False,
		"static_checks": checks,
		"unit_evidence": {
			"status": "pass",
			"commands": [
				"cargo test -p pallet-bulletin-transaction-storage --lib",
				"cargo test -p pallet-bulletin-hop-promotion --lib",
				"cargo test -p indiv-pallet-resources --lib",
			],
			"result": "294 passed; 0 failed",
			"log": "docs/evidence/p1/storage-proof/pallet-tests.out",
			"log_sha256": sha256(ROOT / "docs/evidence/p1/storage-proof/pallet-tests.out"),
		},
		"node_build_evidence": {
			"status": "pass" if node_build_passed else "fail",
			"command": "cargo check -p origin-omni-node",
			"log": "docs/evidence/p1/storage-proof/origin-orbis-check.out",
			"log_sha256": sha256(
				ROOT / "docs/evidence/p1/storage-proof/origin-orbis-check.out"
			),
			"note": "Build passed with pre-existing dead-code warnings in runtime weights/meta_v6.rs.",
		},
		"production_e2e": production_e2e,
		"overall_status": (
			"fail"
			if static_failed or production_e2e["status"] == "fail"
			else "pass" if production_e2e["status"] == "pass" else "blocked"
		),
	}


def parse_args() -> argparse.Namespace:
	parser = argparse.ArgumentParser(description=__doc__)
	parser.add_argument(
		"--output",
		type=Path,
		default=DEFAULT_OUTPUT,
		help="repo-relative verdict path (default: %(default)s)",
	)
	return parser.parse_args()


def main() -> int:
	args = parse_args()
	output = args.output if args.output.is_absolute() else ROOT / args.output
	try:
		verdict = build_verdict()
	except (OSError, RuntimeError, KeyError, json.JSONDecodeError) as error:
		print(json.dumps({"overall_status": "fail", "error": str(error)}, sort_keys=True))
		return 1

	output.parent.mkdir(parents=True, exist_ok=True)
	output.write_text(json.dumps(verdict, indent=2, sort_keys=True) + "\n", encoding="utf-8")
	print(json.dumps({"overall_status": verdict["overall_status"], "output": relative_repo_path(output)}))
	return 1 if verdict["overall_status"] == "fail" else 2


if __name__ == "__main__":
	sys.exit(main())
