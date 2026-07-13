#!/usr/bin/env python3
"""Unit checks for the proof campaign's fail-closed result contract."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("run-orbis-proof-retention-campaign.py")
SPEC = importlib.util.spec_from_file_location("proof_campaign", SCRIPT)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)


class ProofCampaignContractTests(unittest.TestCase):
	def setUp(self):
		self.case = {
			"id": "late_proof",
			"fault_kind": "node-provider-stale",
			"required_assertions": ["stale_proof_rejected"],
		}

	def validate(self, result, evidence):
		return CAMPAIGN.validate_case_result(
			self.case,
			result,
			manifest_sha256="m" * 64,
			topology_sha256="t" * 64,
			driver_sha256="d" * 64,
			result_schema_sha256="s" * 64,
			chain_specs_sha256={"origin": "o" * 64, "orbis": "p" * 64},
			binaries_sha256={"origin-orbis": "b" * 64},
			evidence_dir=evidence,
		)

	def test_boolean_only_callback_cannot_claim_production_pass(self):
		with tempfile.TemporaryDirectory() as directory:
			result = {
				"schema_version": 3,
				"case": "late_proof",
				"status": "pass",
				"manifest_sha256": "m" * 64,
				"topology_sha256": "t" * 64,
				"assertions": {"stale_proof_rejected": True},
				"artifacts": ["claimed.log"],
			}
			errors = self.validate(result, Path(directory))
			self.assertTrue(any("signed_receipts" in error for error in errors))
			self.assertTrue(any("driver" in error for error in errors))
			self.assertTrue(any("cleanup" in error for error in errors))
			self.assertTrue(any("artifact" in error for error in errors))

	def test_exact_hashed_receipts_pass_and_tamper_fails(self):
		with tempfile.TemporaryDirectory() as directory:
			evidence = Path(directory)
			artifacts = []
			node_receipt = {
				"schema_version": 1,
				"event": "proof-campaign-fault-attempt",
				"mode": "Stale",
				"chain_id": "orbis-proof-isolated",
				"genesis_hash": "0x" + "aa" * 32,
				"target_block": 12,
				"parent_block": 11,
				"parent_hash": "0x" + "bb" * 32,
				"canonical_proof_present": True,
				"canonical_proof_sha256": "11" * 32,
				"injected_proof_sha256": "22" * 32,
				"action": "provider-stale",
				"rejection": "BadMandatory",
				"proposal_returned": False,
				"one_shot": True,
			}
			fault = {
				"kind": "node-provider-stale",
				"target_block": 12,
				"observed_at_block": 12,
				"reverted": True,
				"mode": "Stale",
				"chain_id": "orbis-proof-isolated",
				"genesis_hash": "0x" + "aa" * 32,
				"armed_command": [
					"/repo/target/proof-campaign/release/origin-orbis-proof-campaign",
					"--proof-campaign-mode", "Stale",
					"--proof-campaign-target-block", "12",
					"--proof-campaign-expected-chain-id", "orbis-proof-isolated",
					"--proof-campaign-expected-genesis-hash", "0x" + "aa" * 32,
					"--unsafe-proof-campaign-acknowledge-disposable",
				],
				"canonical_precondition": {
					"proof_present": True,
					"proof_sha256": "11" * 32,
					"parent_block": 11,
					"parent_hash": "0x" + "bb" * 32,
				},
				"node_receipt": node_receipt,
				"fault_window": {"target_not_imported": True, "target_not_finalized": True},
				"recovery": {
					"common_finalized_head": "0x" + "cc" * 32,
					"target_block_hash": "0x" + "dd" * 32,
					"finalized_at_least": 13,
				},
			}
			capacity_sample = {
				"block_number": 7,
				"block_hash": "0x" + "77" * 32,
				"metadata_hash": "0x" + "44" * 32,
				"block_weight_scale": "0x01",
				"block_weights_constant_scale": "0x02",
				"block_length_constant_scale": "0x03",
				"block_weight_ref_time": 10,
				"block_weight_proof_size": 5,
				"block_weight_ref_time_limit": 100,
				"block_weight_proof_size_limit": 50,
				"block_weight_ratio_operands": {
					"ref_time": {"numerator": 10, "denominator": 100, "fraction": 0.1},
					"proof_size": {"numerator": 5, "denominator": 50, "fraction": 0.1},
				},
				"block_weight_by_class": {
					"normal": {"consumed": "fixture"},
					"operational": {"consumed": "fixture"},
					"mandatory": {"consumed": "fixture"},
				},
				"corrected_extrinsic_event_count": 2,
				"extrinsic_count": 2,
				"block_length_bytes": 200,
				"block_length_limit_bytes": 1_000,
				"block_length_ratio_operands": {
					"numerator": 200, "denominator": 1_000, "fraction": 0.2,
				},
				"block_length_limits_by_class": [800, 1_000, 1_000],
			}
			for kind in (
				"signed-receipt",
				"rpc-trace",
				"node-log",
				"fault-receipt",
				"cleanup-receipt",
				"capacity-samples",
				"proof-campaign-node-receipts",
			):
				path = evidence / f"{kind}.json"
				payload = {"kind": kind}
				if kind == "signed-receipt":
					payload = [
						{
							"label": action,
							"stdout": json.dumps({
								"status": "accepted",
								"extrinsic_hash": "0x" + byte * 32,
								"block_hash": "0x" + "33" * 32,
							}),
						}
						for action, byte in (("setup", "11"), ("store", "22"))
					]
					payload.append({
						"label": "measure-block-7",
						"stdout": json.dumps({
							"block_hash": capacity_sample["block_hash"],
							"metadata_hash": capacity_sample["metadata_hash"],
							"block_weight_scale": capacity_sample["block_weight_scale"],
							"block_weights_constant_scale": capacity_sample["block_weights_constant_scale"],
							"block_length_constant_scale": capacity_sample["block_length_constant_scale"],
							"block_encoded_bytes": capacity_sample["block_length_bytes"],
							"max_block_length_bytes": capacity_sample["block_length_limit_bytes"],
							"total_consumed": {"ref_time": 10, "proof_size": 5},
							"max_block": {"ref_time": 100, "proof_size": 50},
							"total_max_block_ratio": capacity_sample["block_weight_ratio_operands"],
							"normal": capacity_sample["block_weight_by_class"]["normal"],
							"operational": capacity_sample["block_weight_by_class"]["operational"],
							"mandatory": capacity_sample["block_weight_by_class"]["mandatory"],
							"corrected_extrinsic_event_count": 2,
							"extrinsic_count": 2,
							"block_length_ratio": capacity_sample["block_length_ratio_operands"],
							"block_length_limits": capacity_sample["block_length_limits_by_class"],
						}),
					})
				elif kind == "fault-receipt":
					payload = fault
				elif kind == "cleanup-receipt":
					payload = {
						"status": "pass", "fault_state_restored": True,
						"temporary_processes_stopped": True,
					}
				elif kind == "capacity-samples":
					payload = {"ac13_p1": {"status": "fail"}, "samples": [capacity_sample]}
				elif kind == "proof-campaign-node-receipts":
					payload = [node_receipt]
				path.write_text(json.dumps(payload) + "\n")
				artifacts.append(
					{
						"kind": kind,
						"path": path.name,
						"sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
					}
				)
			result = {
				"schema_version": 3,
				"case": "late_proof",
				"status": "pass",
				"manifest_sha256": "m" * 64,
				"topology_sha256": "t" * 64,
				"driver_sha256": "d" * 64,
				"result_schema_sha256": "s" * 64,
				"chain_specs_sha256": {"origin": "o" * 64, "orbis": "p" * 64},
				"binaries_sha256": {"origin-orbis": "b" * 64},
				"assertions": {"stale_proof_rejected": True},
				"signed_receipts": [
					{
						"action": action,
						"status": "finalized",
						"extrinsic_hash": "0x" + ("11" if action == "setup" else "22") * 32,
						"finalized_block_hash": "0x" + "33" * 32,
						"signer": "//Alice",
					}
					for action in ("setup", "store")
				],
				"fault": fault,
				"cleanup": {
					"status": "pass",
					"fault_state_restored": True,
					"temporary_processes_stopped": True,
				},
				"capacity_ac13_p1": {"status": "fail"},
				"artifacts": artifacts,
			}
			self.assertEqual(self.validate(result, evidence), [])
			result["chain_specs_sha256"]["orbis"] = "x" * 64
			self.assertTrue(
				any("chain specs" in error for error in self.validate(result, evidence))
			)
			result["chain_specs_sha256"]["orbis"] = "p" * 64
			# Even internally consistent re-hashing cannot turn a target import into PASS.
			fault["fault_window"]["target_not_imported"] = False
			fault_path = evidence / "fault-receipt.json"
			fault_path.write_text(json.dumps(fault) + "\n")
			fault_artifact = next(
				artifact for artifact in artifacts if artifact["kind"] == "fault-receipt"
			)
			fault_artifact["sha256"] = hashlib.sha256(fault_path.read_bytes()).hexdigest()
			self.assertTrue(
				any("no-import" in error for error in self.validate(result, evidence))
			)
			fault["fault_window"]["target_not_imported"] = True
			fault_path.write_text(json.dumps(fault) + "\n")
			fault_artifact["sha256"] = hashlib.sha256(fault_path.read_bytes()).hexdigest()
			self.assertEqual(self.validate(result, evidence), [])
			(evidence / "rpc-trace.json").write_text("tampered\n")
			self.assertTrue(
				any("hash mismatch" in error for error in self.validate(result, evidence))
			)
			outside = evidence.parent / "proof-campaign-outside.json"
			outside.write_text("outside\n")
			try:
				rpc_artifact = next(
					artifact for artifact in artifacts if artifact["kind"] == "rpc-trace"
				)
				rpc_artifact["path"] = "../proof-campaign-outside.json"
				rpc_artifact["sha256"] = hashlib.sha256(outside.read_bytes()).hexdigest()
				self.assertTrue(
					any("escapes evidence" in error for error in self.validate(result, evidence))
				)
			finally:
				outside.unlink(missing_ok=True)

	def test_capacity_aggregate_reads_raw_operands_and_fails_closed(self):
		with tempfile.TemporaryDirectory() as directory:
			evidence = Path(directory)
			results = []
			for index in range(10):
				path = evidence / f"capacity-{index}.json"
				path.write_text(json.dumps({
					"runtime_constants": {"max_block_transactions": 128},
					"samples": [{
						"storage_tx_count": 16 + index,
						"block_weight_ref_time": 1,
						"block_weight_proof_size": 2,
						"block_length_bytes": 3,
						"block_weight_ref_time_limit": 100,
						"block_weight_proof_size_limit": 100,
						"block_length_limit_bytes": 100,
						"metadata_hash": "0x" + "44" * 32,
						"block_weights_constant_scale": "0x01",
						"block_length_constant_scale": "0x02",
						"database_bytes": 4,
						"finality_lag_blocks": 0,
					}],
				}) + "\n")
				results.append({"artifacts": [{
					"kind": "capacity-samples", "path": path.name
				}]})
			verdict = CAMPAIGN.aggregate_capacity(results, evidence)
			self.assertEqual(verdict["status"], "pass")
			self.assertFalse(verdict["p6_mixed_workload_slo_claimed"])
			payload = json.loads((evidence / "capacity-0.json").read_text())
			payload["samples"][0]["block_weight_proof_size"] = None
			(evidence / "capacity-0.json").write_text(json.dumps(payload) + "\n")
			verdict = CAMPAIGN.aggregate_capacity(results, evidence)
			self.assertEqual(verdict["status"], "fail")
			self.assertFalse(verdict["checks"]["independent_operands_present"])
			payload["samples"][0]["block_weight_proof_size"] = 2
			payload["samples"][0]["metadata_hash"] = "0x" + "55" * 32
			(evidence / "capacity-0.json").write_text(json.dumps(payload) + "\n")
			verdict = CAMPAIGN.aggregate_capacity(results, evidence)
			self.assertEqual(verdict["status"], "fail")
			self.assertFalse(verdict["checks"]["runtime_resource_limits_stable"])
			payload["samples"][0]["metadata_hash"] = "0x" + "44" * 32
			payload["samples"][0]["block_weight_ref_time"] = 95
			(evidence / "capacity-0.json").write_text(json.dumps(payload) + "\n")
			verdict = CAMPAIGN.aggregate_capacity(results, evidence)
			self.assertEqual(verdict["status"], "fail")
			self.assertFalse(verdict["checks"]["no_weight_or_length_sample_over_90_percent"])


if __name__ == "__main__":
	unittest.main()
