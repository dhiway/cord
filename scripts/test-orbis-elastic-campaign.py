#!/usr/bin/env python3
import importlib.util
import json
import math
import tempfile
import unittest
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
	"elastic_campaign", ROOT / "scripts/run-orbis-elastic-campaign.py"
)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)
SAMPLE_DIRECTORY = tempfile.TemporaryDirectory()


def synthetic_runs(one=100.0, three=300.0, lag=5):
	runs = []
	for cores, repetition in CAMPAIGN.SCHEDULE:
		throughput = one if cores == 1 else three
		topology_sha256 = ("1" if cores == 1 else "3") * 64
		samples = []
		for elapsed in range(11):
			samples.append({
				"elapsed_seconds": float(elapsed),
				"phase": "measurement",
				"cores": cores,
				"repetition": repetition,
				"relay_best": elapsed,
				"relay_finalized": elapsed,
				"orbis_best": elapsed + lag,
				"orbis_finalized": elapsed,
				"orbis_finality_lag_blocks": lag,
				"state_sha256": "a" * 64,
				"workload_command_sha256": "b" * 64,
				"generated_topology_sha256": topology_sha256,
			})
		samples.append({**samples[-1], "elapsed_seconds": 11.0, "phase": "drain", "orbis_best": 11 + lag + 20, "orbis_finalized": 11, "orbis_finality_lag_blocks": lag + 20})
		path = Path(SAMPLE_DIRECTORY.name) / f"{cores}-{repetition}-{lag}.jsonl"
		path.write_text("".join(json.dumps(sample, sort_keys=True) + "\n" for sample in samples))
		runs.append({
			"cores": cores,
			"repetition": repetition,
			"finalized_successful_calls": int(throughput * 10),
			"measurement_seconds": 10,
			"finalized_throughput": throughput,
			"measurement_max_finality_lag_blocks": lag,
			"drain_max_finality_lag_blocks": lag + 20,
			"lag_samples_file": str(path),
			"lag_samples_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
			"claim_queue_cores": list(range(cores)),
			"runtime_contract": {
				"velocity": 3,
				"relay_parent_offset": 1,
				"unincluded_segment_capacity": 12,
			},
			"state_sha256": "a" * 64,
			"workload_command_sha256": "b" * 64,
			"generated_topology_sha256": topology_sha256,
			"network_identity": {
				"expected_peer_count": {"relay": 7, "orbis": 1},
				"observed_peer_count": {"relay": 7, "orbis": 1},
				"relay_genesis_hash": "0x" + ("1" if cores == 1 else "3") * 64,
				"orbis_genesis_hash": "0x" + "6" * 64,
			},
		})
	return runs


class ElasticCampaignTests(unittest.TestCase):
	def test_counterbalanced_schedule_is_exactly_five_by_core_count(self):
		self.assertEqual([core for core, _ in CAMPAIGN.SCHEDULE].count(1), 5)
		self.assertEqual([core for core, _ in CAMPAIGN.SCHEDULE].count(3), 5)
		self.assertEqual(CAMPAIGN.SCHEDULE, [
			(1, 1), (3, 1), (3, 2), (1, 2), (1, 3),
			(3, 3), (3, 4), (1, 4), (1, 5), (3, 5),
		])

	def test_analyzer_recomputes_ratio_cv_and_lag(self):
		result = CAMPAIGN.analyze(synthetic_runs())
		self.assertTrue(result["pass"])
		self.assertEqual(result["three_core_to_one_core_ratio"], 3.0)
		self.assertEqual(result["metrics"]["1"]["cv_percent"], 0.0)
		self.assertEqual(result["metrics"]["3"]["measurement_max_finality_lag_blocks"], 5)
		self.assertEqual(result["metrics"]["3"]["drain_max_finality_lag_blocks"], 25)

	def test_analyzer_fails_thresholds_without_rewriting_metrics(self):
		runs = synthetic_runs(100, 239, 16)
		result = CAMPAIGN.analyze(runs)
		self.assertFalse(result["pass"])
		self.assertIn("three-core/one-core-finalized-throughput-ratio-below-2.4", result["failures"])
		self.assertIn("1-core-finality-lag-above-15-blocks", result["failures"])

	def test_analyzer_rejects_schedule_state_and_metric_drift(self):
		runs = synthetic_runs()
		runs[0], runs[1] = runs[1], runs[0]
		with self.assertRaisesRegex(ValueError, "schedule drift"):
			CAMPAIGN.analyze(runs)
		runs = synthetic_runs()
		runs[-1]["state_sha256"] = "c" * 64
		with self.assertRaisesRegex(ValueError, "state/workload digest drift"):
			CAMPAIGN.analyze(runs)
		runs = synthetic_runs()
		runs[-1]["finalized_throughput"] = 999
		with self.assertRaisesRegex(ValueError, "dishonest finalized-throughput"):
			CAMPAIGN.analyze(runs)
		runs = synthetic_runs()
		runs[-1]["lag_samples_sha256"] = "0" * 64
		with self.assertRaisesRegex(ValueError, "digest-mismatched"):
			CAMPAIGN.analyze(runs)

	def test_generated_configs_change_only_declared_core_count(self):
		with tempfile.TemporaryDirectory() as directory:
			one = CAMPAIGN.generated_config(1, Path(directory)).read_text()
			three = CAMPAIGN.generated_config(3, Path(directory)).read_text()
			self.assertEqual(one.replace("num_cores = 1", "num_cores = CORE"), three.replace("num_cores = 3", "num_cores = CORE"))

	def test_tuning_probe_changes_only_both_collator_instance_limits(self):
		with tempfile.TemporaryDirectory() as directory:
			baseline = CAMPAIGN.generated_config(1, Path(directory)).read_text()
			tuning = CAMPAIGN.generated_config(1, Path(directory), 32).read_text()
			delta = ', "--max-runtime-instances=32"'
			self.assertEqual(tuning.count(delta), 2)
			self.assertEqual(tuning.replace(delta, ""), baseline)

	def test_probe_two_selects_only_the_declared_isolated_origin_binary(self):
		with tempfile.TemporaryDirectory() as directory:
			baseline = CAMPAIGN.generated_config(1, Path(directory), 32).read_text()
			binary = Path("/private/tmp/cord-origin-probe2-target/release/origin")
			probe_two = CAMPAIGN.generated_config(1, Path(directory), 32, binary).read_text()
			self.assertEqual(probe_two.count(str(binary)), 1)
			self.assertEqual(
				probe_two.replace(str(binary), "target/release/origin"),
				baseline,
			)


if __name__ == "__main__":
	unittest.main()
