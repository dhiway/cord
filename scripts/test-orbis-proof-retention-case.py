#!/usr/bin/env python3
"""Offline unit/static tests for canonical proof-case driver logic only."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import sys
import subprocess


ROOT = Path(__file__).resolve().parents[1]


def load(name):
	path = Path(__file__).with_name(name)
	spec = importlib.util.spec_from_file_location(name.replace("-", "_"), path)
	module = importlib.util.module_from_spec(spec)
	sys.modules[spec.name] = module
	spec.loader.exec_module(module)
	return module


DRIVER = load("run-orbis-proof-retention-case.py")
CAMPAIGN = load("run-orbis-proof-retention-campaign.py")


BASE_COMMAND = [
	"target/release/origin-orbis",
	"--name",
	"proof-orbis-alice",
	"--node-key",
	"11" * 32,
	"--chain",
	"/tmp/orbis.json",
	"--base-path",
	"/tmp/alice/data",
	"--listen-addr",
	"/ip4/127.0.0.1/tcp/30333/ws",
	"--rpc-port",
	"10810",
	"--collator",
	"--force-authoring",
	"--authoring=slot-based",
	"--",
	"--base-path",
	"/tmp/alice/relay",
	"--chain",
	"/tmp/relay.json",
	"--port",
	"30444",
	"--rpc-port",
	"30445",
]


class DriverStaticTests(unittest.TestCase):
	def test_all_ten_cases_have_exact_fault_and_assertion_contracts(self):
		manifest = json.loads(
			(ROOT / "docs/evidence/p1/storage-proof/proof-retention-cases.json").read_text()
		)
		ids = tuple(case["id"] for case in manifest["cases"])
		self.assertEqual(ids, DRIVER.CASE_ORDER)
		self.assertEqual(set(ids), set(DRIVER.FAULT_KINDS))
		self.assertEqual(set(ids), set(DRIVER.REQUIRED_ASSERTIONS))
		self.assertTrue(all(case["local_mechanism_ready"] is False for case in manifest["cases"]))

	def test_dispatch_reaches_a_concrete_handler_for_every_case(self):
		class FakeRunner(DRIVER.CaseRunner):
			def __init__(self, case):
				self.case = case
				self.called = []

			def case_two_collator(self): self.called.append("two")
			def case_proof_campaign_fault(self, mode): self.called.append(f"campaign:{mode}")
			def case_fork_reorg(self): self.called.append("fork")
			def case_pruning(self): self.called.append("pruning")
			def case_restart(self): self.called.append("restart")
			def case_fresh_resync(self): self.called.append("resync")
			def case_disk_pressure(self): self.called.append("disk")

		for case in DRIVER.CASE_ORDER:
			runner = FakeRunner(case)
			runner.run_case()
			self.assertEqual(len(runner.called), 1, case)

	def test_exact_target_is_store_plus_runtime_retention(self):
		self.assertEqual(DRIVER.proof_target(41, 32), 73)
		for store, retention in ((0, 32), (41, 0), (-1, 2)):
			with self.assertRaises(DRIVER.CampaignError):
				DRIVER.proof_target(store, retention)

	def test_campaign_command_is_exact_and_canonical_cleanup_removes_fault(self):
		genesis = "0x" + "ab" * 32
		command = DRIVER.proof_campaign_command(
			BASE_COMMAND,
			Path("target/proof-campaign/release/origin-orbis-proof-campaign"),
			"Duplicate",
			73,
			genesis,
		)
		para, relay = DRIVER.split_command(command)
		self.assertEqual(para[0], "target/proof-campaign/release/origin-orbis-proof-campaign")
		self.assertEqual(DRIVER.option_value(para, "--proof-campaign-mode"), "Duplicate")
		self.assertEqual(DRIVER.option_value(para, "--proof-campaign-target-block"), "73")
		self.assertEqual(
			DRIVER.option_value(para, "--proof-campaign-expected-chain-id"),
			"orbis-proof-isolated",
		)
		self.assertEqual(
			DRIVER.option_value(para, "--proof-campaign-expected-genesis-hash"), genesis
		)
		self.assertIn("--unsafe-proof-campaign-acknowledge-disposable", para)
		self.assertTrue(relay)
		clean = DRIVER.canonical_command(command)
		self.assertNotIn("--unsafe-proof-campaign-acknowledge-disposable", clean)
		self.assertNotIn("--proof-campaign-mode", clean)
		self.assertNotIn("--proof-campaign-expected-genesis-hash", clean)

	def test_campaign_command_rejects_missing_guardrails(self):
		binary = Path("target/proof-campaign/release/origin-orbis-proof-campaign")
		for mode, target, genesis in (
			("late", 73, "0x" + "11" * 32),
			("Missing", 0, "0x" + "11" * 32),
			("Missing", 73, "0x1234"),
			("Missing", 73, "0x" + "zz" * 32),
		):
			with self.subTest(mode=mode, target=target, genesis=genesis), self.assertRaises(
				DRIVER.CampaignError
			):
				DRIVER.proof_campaign_command(BASE_COMMAND, binary, mode, target, genesis)

	def test_cli_refuses_to_overwrite_existing_result(self):
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory) / "case.json"
			output.write_text("original\n")
			completed = subprocess.run(
				[
					sys.executable,
					str(ROOT / "scripts/run-orbis-proof-retention-case.py"),
					"--case", "late_proof",
					"--context", str(Path(directory) / "absent-context.json"),
					"--output", str(output),
				],
				capture_output=True,
				text=True,
			)
			self.assertEqual(completed.returncode, 2)
			self.assertEqual(output.read_text(), "original\n")

	def test_partition_and_observer_commands_do_not_author_or_reuse_identity(self):
		partitioned = DRIVER.partition_command(BASE_COMMAND)
		self.assertIn("--reserved-only", DRIVER.split_command(partitioned)[0])
		observer = DRIVER.observer_command(
			BASE_COMMAND, Path("/repo/origin-orbis"), Path("/tmp/fresh"), "fresh", 10812, 30812
		)
		para, relay = DRIVER.split_command(observer)
		self.assertNotIn("--collator", para)
		self.assertNotIn("--force-authoring", para)
		self.assertEqual(DRIVER.option_value(para, "--rpc-port"), "10812")
		self.assertEqual(DRIVER.option_value(para, "--base-path"), "/tmp/fresh")
		self.assertEqual(DRIVER.option_value(relay, "--base-path"), "/tmp/fresh/relay")
		self.assertNotEqual(DRIVER.option_value(para, "--node-key"), "11" * 32)

	def test_scale_vector_count_and_capacity_math(self):
		self.assertEqual(DRIVER.compact_length("0x00"), 0)
		self.assertEqual(DRIVER.compact_length("0x0c"), 3)
		self.assertEqual(DRIVER.compact_length("0x1501"), 69)
		samples = [
			{
				"storage_tx_count": value,
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
			}
			for value in range(1, 21)
		]
		verdict = DRIVER.capacity_verdict(samples, 128)
		self.assertEqual(verdict["status"], "pass")
		samples[0]["block_weight_ref_time"] = None
		self.assertEqual(DRIVER.capacity_verdict(samples, 128)["status"], "fail")

	def test_compact_size_matches_rpc_extrinsic_vector_boundaries(self):
		self.assertEqual(DRIVER.compact_encoded_size(0), 1)
		self.assertEqual(DRIVER.compact_encoded_size(63), 1)
		self.assertEqual(DRIVER.compact_encoded_size(64), 2)
		self.assertEqual(DRIVER.compact_encoded_size((1 << 14) - 1), 2)
		self.assertEqual(DRIVER.compact_encoded_size(1 << 14), 4)

	def test_metadata_block_measurement_fixture_binds_hash_lengths_and_exact_ratios(self):
		block_hash = "0x" + "aa" * 32
		measure = {
			"status": "ok",
			"block_hash": block_hash,
			"spec_version": 29,
			"transaction_version": 8,
			"metadata_hash": "0x" + "bb" * 32,
			"extrinsics": [
				{"raw_bytes": 10, "scale_encoded_bytes": 11},
				{"raw_bytes": 20, "scale_encoded_bytes": 21},
			],
			"extrinsics_encoded_bytes": 33,
			"total_consumed": {"ref_time": 20, "proof_size": 10},
			"max_block": {"ref_time": 100, "proof_size": 50},
			"total_max_block_ratio": {
				"ref_time": {"numerator": 20, "denominator": 100, "fraction": 0.2},
				"proof_size": {"numerator": 10, "denominator": 50, "fraction": 0.2},
			},
			"block_encoded_bytes": 133,
			"max_block_length_bytes": 1_000,
			"block_length_limits": [800, 1_000, 1_000],
			"block_length_ratio": {
				"numerator": 133, "denominator": 1_000, "fraction": 0.133,
			},
			"header_encoded_bytes": 100,
			"normal": {
				"consumed": {"ref_time": 12, "proof_size": 5},
				"corrected_extrinsic_event_total": {"ref_time": 10, "proof_size": 4},
				"block_weight_minus_corrected_event_total": {"ref_time": 2, "proof_size": 1},
			},
			"operational": {
				"consumed": {"ref_time": 3, "proof_size": 2},
				"corrected_extrinsic_event_total": {"ref_time": 3, "proof_size": 2},
				"block_weight_minus_corrected_event_total": {"ref_time": 0, "proof_size": 0},
			},
			"mandatory": {
				"consumed": {"ref_time": 5, "proof_size": 3},
				"corrected_extrinsic_event_total": {"ref_time": 5, "proof_size": 3},
				"block_weight_minus_corrected_event_total": {"ref_time": 0, "proof_size": 0},
			},
			"corrected_extrinsic_event_count": 2,
			"extrinsic_count": 2,
			"block_weight_scale": "0x01",
			"block_weights_constant_scale": "0x02",
			"block_length_constant_scale": "0x03",
		}
		operands = DRIVER.validated_block_measurement(measure, block_hash, [10, 20])
		self.assertEqual(operands["block_extrinsics_encoded_bytes"], 33)
		self.assertEqual(operands["block_weight_ref_time"], 20)
		measure["total_max_block_ratio"]["ref_time"]["denominator"] = 99
		with self.assertRaisesRegex(DRIVER.CampaignError, "ratio operands drift"):
			DRIVER.validated_block_measurement(measure, block_hash, [10, 20])

	def test_live_node_discovery_validates_listener_command_base_and_runtime(self):
		class FakeRpc:
			def call(self, url, method, params=None):
				if method == "state_getRuntimeVersion":
					return {"specName": "orbis", "specVersion": 29, "transactionVersion": 8}
				if method == "chain_getBlockHash":
					return "0x" + "aa" * 32
				raise AssertionError(method)

		with tempfile.TemporaryDirectory() as directory:
			base = Path(directory)
			command = list(BASE_COMMAND)
			command = DRIVER.set_option(command, "--base-path", str(base))
			manager = DRIVER.ProcessManager(
				base, FakeRpc(), base / "owned.json", base, {"alice": base.resolve()}
			)
			with mock.patch.object(manager, "_listener_pid", return_value=1234), mock.patch.object(
				DRIVER.subprocess, "check_output", return_value=DRIVER.shlex.join(command)
			):
				node = manager.discover("alice", 10810)
			self.assertEqual(node.pid, 1234)
			self.assertEqual(node.base_path, base.resolve())
			self.assertEqual(node.genesis_hash, "0x" + "aa" * 32)
			manager.owned_bases["alice"] = base / "another-owned-base"
			with mock.patch.object(manager, "_listener_pid", return_value=1234), mock.patch.object(
				DRIVER.subprocess, "check_output", return_value=DRIVER.shlex.join(command)
			), self.assertRaisesRegex(DRIVER.CampaignError, "unowned base path"):
				manager.discover("alice", 10810)

	def test_cleanup_refuses_pid_command_reuse_without_signalling(self):
		with tempfile.TemporaryDirectory() as directory:
			registry = Path(directory) / "owned.json"
			command = ["/safe/node", "--rpc-port", "10812"]
			registry.write_text(json.dumps([{
				"pid": 1234,
				"pgid": 1234,
				"command": command,
				"command_sha256": DRIVER.hashlib.sha256("\0".join(command).encode()).hexdigest(),
				"active": True,
			}]))
			with mock.patch.object(CAMPAIGN.subprocess, "check_output", return_value="/other/node"), mock.patch.object(
				CAMPAIGN.os, "killpg"
			) as killpg:
				errors = CAMPAIGN.terminate_owned_processes(registry)
			self.assertTrue(any("reuse guard" in error for error in errors))
			killpg.assert_not_called()

	def campaign_receipt(self, mode="Invalid"):
		canonical = "11" * 32
		injected = None if mode == "Missing" else (canonical if mode == "Duplicate" else "22" * 32)
		return {
			"schema_version": 1,
			"event": "proof-campaign-fault-attempt",
			"mode": mode,
			"chain_id": "orbis-proof-isolated",
			"genesis_hash": "0x" + "aa" * 32,
			"target_block": 73,
			"parent_block": 72,
			"parent_hash": "0x" + "bb" * 32,
			"canonical_proof_present": True,
			"canonical_proof_sha256": canonical,
			"injected_proof_sha256": injected,
			"action": {
				"Missing": "provider-omitted",
				"Invalid": "provider-invalid",
				"Stale": "provider-stale",
				"Duplicate": "duplicate-second-push",
			}[mode],
			"rejection": "FinalizationError" if mode == "Missing" else "BadMandatory",
			"proposal_returned": False,
			"one_shot": True,
		}

	def test_structured_receipt_accepts_all_four_exact_modes(self):
		for mode in DRIVER.PROOF_CAMPAIGN_MODES:
			receipt = self.campaign_receipt(mode)
			line = "INFO " + DRIVER.PROOF_CAMPAIGN_RECEIPT_PREFIX + json.dumps(receipt)
			parsed = DRIVER.parse_proof_campaign_receipts(line)
			self.assertEqual(
				DRIVER.validated_proof_campaign_receipt(
					parsed, mode=mode, target=73, genesis_hash="0x" + "aa" * 32
				),
				receipt,
			)

	def test_structured_receipt_tamper_duplicate_and_prose_fail_closed(self):
		receipt = self.campaign_receipt()
		for field, value in (
			("chain_id", "orbis-local"),
			("target_block", 74),
			("canonical_proof_present", False),
			("proposal_returned", True),
			("injected_proof_sha256", receipt["canonical_proof_sha256"]),
		):
			bad = dict(receipt)
			bad[field] = value
			with self.subTest(field=field), self.assertRaises(DRIVER.CampaignError):
				DRIVER.validated_proof_campaign_receipt(
					[bad], mode="Invalid", target=73, genesis_hash="0x" + "aa" * 32
				)
		with self.assertRaises(DRIVER.CampaignError):
			DRIVER.validated_proof_campaign_receipt(
				[receipt, dict(receipt)],
				mode="Invalid",
				target=73,
				genesis_hash="0x" + "aa" * 32,
			)
		self.assertEqual(
			DRIVER.parse_proof_campaign_receipts("BadMandatory proof fault injected"), []
		)

	def test_dispatch_uses_campaign_binary_for_all_four_semantic_faults(self):
		source = (ROOT / "scripts/run-orbis-proof-retention-case.py").read_text()
		for mode in DRIVER.PROOF_CAMPAIGN_MODES:
			self.assertIn(f'case_proof_campaign_fault("{mode}")', source)
		self.assertNotIn('case_feature_fault("late")', source)
		self.assertNotIn('case_db_fault("corrupt")', source)
		self.assertNotIn("def case_db_fault", source)
		self.assertIn('action = "probe"', source)
		self.assertNotIn('self.index_probe("remove"', source)
		self.assertNotIn('self.index_probe("corrupt"', source)


if __name__ == "__main__":
	unittest.main()
