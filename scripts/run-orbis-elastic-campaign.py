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

"""Run or analyze the fail-closed Orbis one-core/three-core P1 campaign.

The runner never fabricates performance numbers. Execution requires binaries built from the
checked-out revision, a frozen baseline-state artifact and an external finalized-successful-call
workload driver. A preflight failure writes a blocker report with all metrics set to null.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "zombienet/testnet-elastic.toml"
SLO = ROOT / "docs/evidence/performance/service-slo-manifest.json"
RUNTIME = ROOT / "origin/orbis/runtime/src/lib.rs"
RELAY = "http://127.0.0.1:9801"
ORBIS = "http://127.0.0.1:9810"
SCHEDULE = [(1, 1), (3, 1), (3, 2), (1, 2), (1, 3), (3, 3), (3, 4), (1, 4), (1, 5), (3, 5)]
DEFAULT_STATE = ROOT / "docs/evidence/performance/p1-elastic-genesis-state.json"
DEFAULT_WORKLOAD = (
	'target/release/examples/orbis_elastic_workload '
	'--endpoint ws://127.0.0.1:9810 '
	'--duration-seconds "$CAMPAIGN_MEASUREMENT_SECONDS" '
	'--repetition "$CAMPAIGN_REPETITION" --cores "$ORBIS_CORES" '
	'--campaign-seed "$CAMPAIGN_SEED"'
)


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def command_output(command: list[str]) -> str:
	return subprocess.check_output(command, cwd=ROOT, text=True, stderr=subprocess.STDOUT).strip()


def chain_spec_identity(binary: Path, chain: str) -> dict:
	encoded = subprocess.check_output(
		[str(binary), "build-spec", "--chain", chain, "--disable-default-bootnode"],
		cwd=ROOT,
		text=True,
		stderr=subprocess.DEVNULL,
	)
	spec = json.loads(encoded)
	return {"id": spec.get("id"), "protocol_id": spec.get("protocolId")}


def rpc(url: str, method: str, params: list | None = None):
	request = urllib.request.Request(
		url,
		data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}).encode(),
		headers={"content-type": "application/json"},
	)
	response = json.load(urllib.request.urlopen(request, timeout=10))
	if "error" in response:
		raise RuntimeError(f"{method}: {response['error']}")
	return response["result"]


def height(url: str, finalized: bool = False) -> int:
	block_hash = rpc(url, "chain_getFinalizedHead") if finalized else None
	header = rpc(url, "chain_getHeader", [block_hash] if block_hash else [])
	return int(header["number"], 16)


def snapshot() -> dict:
	return {
		"relay_best": height(RELAY),
		"relay_finalized": height(RELAY, True),
		"orbis_best": height(ORBIS),
		"orbis_finalized": height(ORBIS, True),
	}


def preflight(args) -> tuple[list[str], dict]:
	blockers: list[str] = []
	inputs: dict = {
		"branch": command_output(["git", "branch", "--show-current"]),
		"commit": command_output(["git", "rev-parse", "HEAD"]),
		"topology_sha256": sha256(CONFIG),
		"slo_sha256": sha256(SLO),
		"runtime_source_sha256": sha256(RUNTIME),
		"runner_sha256": sha256(Path(__file__).resolve()),
		"smoke_sha256": sha256(ROOT / "zombienet/orbis_smoke.py"),
		"bootstrap_sha256": sha256(ROOT / "origin-rs/examples/bootstrap_orbis_core.rs"),
		"bootstrap_binary_sha256": sha256(ROOT / "target/release/examples/bootstrap_orbis_core"),
		"ports": {"relay": 9801, "orbis": 9810},
		"schedule": [{"cores": cores, "repetition": repetition} for cores, repetition in SCHEDULE],
	}
	if inputs["branch"] != "sm-update-sub-0x63":
		blockers.append(f"wrong branch: {inputs['branch']!r}; expected sm-update-sub-0x63")
	binaries = {
		"origin": Path(args.origin_binary).resolve() if args.origin_binary else ROOT / "target/release/origin",
		"origin-orbis": ROOT / "target/release/origin-omni-node",
	}
	for name, binary in binaries.items():
		if not binary.is_file():
			blockers.append(f"missing release binary: {binary.relative_to(ROOT)}")
			continue
		version = command_output([str(binary), "--version"])
		inputs[f"{name}_binary_sha256"] = sha256(binary)
		inputs[f"{name}_version"] = version
		reported_revision = version.rsplit("-", 1)[-1]
		if len(reported_revision) < 7 or not inputs["commit"].startswith(reported_revision):
			blockers.append(
				f"{name} binary is not source-bound to HEAD {inputs['commit']}: version reports {version!r}; "
				"rebuild both release binaries from the checked-out commit"
			)
	if not blockers:
		inputs["chain_spec_identity"] = {
			"relay": chain_spec_identity(binaries["origin"], "origin-local"),
			"orbis": chain_spec_identity(binaries["origin-orbis"], "orbis-local"),
		}
		if inputs["chain_spec_identity"] != {
			"relay": {"id": "origin_local", "protocol_id": "0rigin"},
			"orbis": {"id": "orbis-local", "protocol_id": "orbis"},
		}:
			blockers.append("chain-spec id/protocol isolation contract drift")
	if shutil.which("zombienet") is None:
		blockers.append("zombienet is not installed or not on PATH")
	else:
		inputs["zombienet_version"] = command_output(["zombienet", "version"])

	config = CONFIG.read_text()
	for required in (
		'rpc_port = 9801',
		'rpc_port = 9810',
		'id = 1006',
		'num_cores = 3',
		'name = "orbis-alice"',
		'name = "orbis-bob"',
	):
		if required not in config:
			blockers.append(f"elastic topology is missing {required!r}")
	runtime = RUNTIME.read_text()
	for pattern, label in (
		(r"const RELAY_PARENT_OFFSET: u32 = 1;", "relay-parent offset 1"),
		(r"const BLOCK_PROCESSING_VELOCITY: u32 = 3;", "block-processing velocity 3"),
		(r"const UNINCLUDED_SEGMENT_CAPACITY: u32 = \(3 \+ RELAY_PARENT_OFFSET\) \* BLOCK_PROCESSING_VELOCITY;", "unincluded-segment capacity 12 formula"),
	):
		if not re.search(pattern, runtime):
			blockers.append(f"runtime source does not prove {label}")

	slo = json.loads(SLO.read_text())
	if slo["network"] != {
		"aura_slot_ms": 6000,
		"orbis_collators": 2,
		"orbis_cores": [1, 3],
		"origin_validators": 6,
		"para_id": 1006,
		"relay_parent_offset": 1,
		"target_block_rate": 3,
		"unincluded_segment_capacity": 12,
	}:
		blockers.append("service SLO topology drifted from the frozen campaign topology")
	if slo["campaign"]["interleaved_runs"] < 5:
		blockers.append("campaign requires at least five repetitions per core count")
	if args.tuning_probe:
		if (args.warmup_seconds, args.measurement_seconds, args.cooldown_seconds) != (600, 300, 0):
			blockers.append("tuning-probe windows must remain frozen at warmup=600s, measurement=300s, cooldown=0s")
		if args.diagnostic_core != 1 or args.max_runtime_instances != 32 or args.stop_lag_above != 15:
			blockers.append("tuning probe 1 requires one core, both collators at max-runtime-instances=32, and stop lag 15")
	else:
		if (args.warmup_seconds, args.measurement_seconds, args.cooldown_seconds) != (600, 1800, 300):
			blockers.append("execution windows must remain frozen at warmup=600s, measurement=1800s, cooldown=300s")
		if args.max_runtime_instances is not None:
			blockers.append("acceptance campaign topology may not set the tuning-only max-runtime-instances delta")
	if args.origin_binary and not args.tuning_probe:
		blockers.append("an Origin binary override is permitted only for a tuning probe")
	inputs["origin_binary_path"] = str(binaries["origin"])
	if args.driver_grace_seconds != 150:
		blockers.append("runner process grace must remain frozen at 150s for the 120s finality drain")
	if args.sample_seconds != 1:
		blockers.append("finality lag sampling must remain frozen at one-second intervals")
	inputs["campaign_parameters"] = {
		"warmup_seconds": args.warmup_seconds,
		"measurement_seconds": args.measurement_seconds,
		"cooldown_seconds": args.cooldown_seconds,
		"driver_finality_drain_seconds": 120,
		"runner_process_grace_seconds": args.driver_grace_seconds,
		"sample_seconds": args.sample_seconds,
		"tuning_probe": args.tuning_probe,
		"max_runtime_instances": args.max_runtime_instances,
		"stop_lag_above": args.stop_lag_above,
	}
	if args.host_manifest:
		host_manifest = Path(args.host_manifest)
		if not host_manifest.is_file():
			blockers.append(f"host manifest does not exist: {host_manifest}")
		else:
			inputs["host_manifest_path"] = str(host_manifest.resolve())
			inputs["host_manifest_sha256"] = sha256(host_manifest)
	if not args.state_file:
		blockers.append("no frozen baseline state supplied; pass --state-file")
	elif not Path(args.state_file).is_file():
		blockers.append(f"baseline state file does not exist: {args.state_file}")
	else:
		state_path = Path(args.state_file)
		inputs["state_path"] = str(state_path.resolve())
		inputs["state_sha256"] = sha256(state_path)
		try:
			state = json.loads(state_path.read_text())
		except Exception as error:
			blockers.append(f"frozen baseline state is invalid JSON: {error}")
		else:
			if state.get("schema_version") != 1 or state.get("branch") != inputs["branch"] or state.get("commit") != inputs["commit"] or state.get("reset_policy") != "fresh-zombienet-network-and-clean-genesis-before-every-run" or state.get("data_migration") is not False or state.get("backward_compatibility") is not False:
				blockers.append("frozen baseline state is not bound to clean genesis at the current revision")
			state_inputs = state.get("inputs", {})
			for key in ("origin_binary_sha256", "orbis_binary_sha256"):
				actual_key = key.replace("orbis_binary", "origin-orbis_binary")
				if key == "origin_binary_sha256" and args.tuning_probe and args.origin_binary:
					continue
				if state_inputs.get(key) != inputs.get(actual_key):
					blockers.append(f"frozen baseline state {key} drift")
	if not args.workload_command:
		blockers.append("no finalized-successful-call workload driver supplied; pass --workload-command")
	else:
		inputs["workload_command_sha256"] = hashlib.sha256(args.workload_command.encode()).hexdigest()
		driver = ROOT / "target/release/examples/orbis_elastic_workload"
		if not driver.is_file():
			blockers.append("missing frozen workload driver binary: target/release/examples/orbis_elastic_workload")
		else:
			inputs["workload_driver_binary_sha256"] = sha256(driver)
		inputs["workload_driver_source_sha256"] = sha256(ROOT / "origin-rs/examples/orbis_elastic_workload.rs")
	return blockers, inputs


def generated_config(
	cores: int,
	directory: Path,
	max_runtime_instances: int | None = None,
	origin_binary: Path | None = None,
) -> Path:
	source = CONFIG.read_text()
	updated, replacements = re.subn(r"num_cores = 3", f"num_cores = {cores}", source)
	if replacements != 2:
		raise RuntimeError(f"expected two num_cores replacements, observed {replacements}")
	if max_runtime_instances is not None:
		needle = '"--authoring=slot-based"]'
		replacement = f'"--authoring=slot-based", "--max-runtime-instances={max_runtime_instances}"]'
		updated, replacements = updated.replace(needle, replacement), updated.count(needle)
		if replacements != 2:
			raise RuntimeError(f"expected two Orbis collator runtime-instance insertions, observed {replacements}")
	if origin_binary is not None:
		needle = 'default_command = "target/release/origin"'
		replacement = f'default_command = "{origin_binary.resolve()}"'
		updated, replacements = updated.replace(needle, replacement), updated.count(needle)
		if replacements != 1:
			raise RuntimeError(f"expected one isolated Origin binary insertion, observed {replacements}")
	path = directory / f"testnet-elastic-{cores}-core.toml"
	path.write_text(updated)
	return path


def copy_node_logs(zombienet_log: Path, run_dir: Path) -> None:
	"""Retain the eight native-node logs advertised by Zombienet."""
	log_text = zombienet_log.read_text(errors="replace")
	paths = set(re.findall(r"tail -f\s+(\S+/(?:alice|bob|charlie|dave|eve|fredie|orbis-alice|orbis-bob)\.log)", log_text))
	target = run_dir / "node-logs"
	target.mkdir(exist_ok=True)
	for raw in sorted(paths):
		source = Path(raw)
		if source.is_file():
			shutil.copy2(source, target / source.name)


def wait_rpc(timeout: int = 300):
	deadline = time.monotonic() + timeout
	last = None
	while time.monotonic() < deadline:
		try:
			versions = (rpc(RELAY, "state_getRuntimeVersion"), rpc(ORBIS, "state_getRuntimeVersion"))
			if versions[0]["specName"] == "origin" and versions[1]["specName"] == "orbis":
				return
		except Exception as error:  # network is expected to refuse while starting
			last = error
		time.sleep(2)
	raise RuntimeError(f"Origin/Orbis RPC did not become ready: {last}")


def isolated_network_identity(timeout: int = 90) -> dict:
	expected_peers = {"relay": 7, "orbis": 1}
	deadline = time.monotonic() + timeout
	while True:
		relay_health = rpc(RELAY, "system_health")
		orbis_health = rpc(ORBIS, "system_health")
		observed_peers = {"relay": relay_health.get("peers"), "orbis": orbis_health.get("peers")}
		if any(observed_peers[name] > expected_peers[name] for name in expected_peers):
			raise RuntimeError(
				f"network isolation failure: expected peer counts {expected_peers}, observed {observed_peers}; "
				"another topology may share genesis/protocol"
			)
		if observed_peers == expected_peers and not relay_health.get("isSyncing") and not orbis_health.get("isSyncing"):
			break
		if time.monotonic() >= deadline:
			raise RuntimeError(
				f"network readiness failure: expected peer counts {expected_peers}, observed {observed_peers}"
			)
		time.sleep(2)
	return {
		"expected_peer_count": expected_peers,
		"observed_peer_count": observed_peers,
		"relay_genesis_hash": rpc(RELAY, "chain_getBlockHash", [0]),
		"orbis_genesis_hash": rpc(ORBIS, "chain_getBlockHash", [0]),
	}


def assert_frozen_inputs(inputs: dict) -> None:
	paths = {
		"runner_sha256": Path(__file__).resolve(),
		"smoke_sha256": ROOT / "zombienet/orbis_smoke.py",
		"bootstrap_sha256": ROOT / "origin-rs/examples/bootstrap_orbis_core.rs",
		"bootstrap_binary_sha256": ROOT / "target/release/examples/bootstrap_orbis_core",
		"slo_sha256": SLO,
		"origin_binary_sha256": Path(inputs["origin_binary_path"]),
		"origin-orbis_binary_sha256": ROOT / "target/release/origin-omni-node",
		"workload_driver_binary_sha256": ROOT / "target/release/examples/orbis_elastic_workload",
		"workload_driver_source_sha256": ROOT / "origin-rs/examples/orbis_elastic_workload.rs",
		"topology_sha256": CONFIG,
		"runtime_source_sha256": RUNTIME,
		"state_sha256": Path(inputs["state_path"]),
	}
	for key, path in paths.items():
		if inputs.get(key) != sha256(path):
			raise RuntimeError(f"frozen campaign input drift during execution: {key}")
	if inputs.get("host_manifest_path") and inputs.get("host_manifest_sha256") != sha256(Path(inputs["host_manifest_path"])):
		raise RuntimeError("frozen campaign input drift during execution: host_manifest_sha256")


def finality_sample(elapsed: float, phase: str, current: dict, cores: int, repetition: int, inputs: dict, topology_sha256: str) -> dict:
	return {
		"elapsed_seconds": round(elapsed, 6),
		"phase": phase,
		"cores": cores,
		"repetition": repetition,
		"relay_best": current["relay_best"],
		"relay_finalized": current["relay_finalized"],
		"orbis_best": current["orbis_best"],
		"orbis_finalized": current["orbis_finalized"],
		"orbis_finality_lag_blocks": current["orbis_best"] - current["orbis_finalized"],
		"state_sha256": inputs["state_sha256"],
		"workload_command_sha256": inputs["workload_command_sha256"],
		"generated_topology_sha256": topology_sha256,
	}


def validate_lag_samples(run: dict) -> tuple[int, int, int]:
	path = Path(run.get("lag_samples_file", ""))
	if not path.is_absolute():
		path = ROOT / path
	if not path.is_file() or sha256(path) != run.get("lag_samples_sha256"):
		raise ValueError("finality-lag sample file is missing or digest-mismatched")
	try:
		samples = [json.loads(line) for line in path.read_text().splitlines() if line]
	except Exception as error:
		raise ValueError(f"invalid finality-lag JSONL: {error}") from error
	if not samples:
		raise ValueError("finality-lag sample file is empty")
	measurement, drain = [], []
	measurement_elapsed = []
	previous = -1.0
	for sample in samples:
		elapsed = sample.get("elapsed_seconds")
		phase = sample.get("phase")
		if not isinstance(elapsed, (int, float)) or isinstance(elapsed, bool) or elapsed < previous:
			raise ValueError("finality-lag samples are not monotonic")
		if previous >= 0 and elapsed - previous > 2.5:
			raise ValueError("finality-lag sampling gap exceeds the per-second evidence tolerance")
		previous = elapsed
		for key in ("relay_best", "relay_finalized", "orbis_best", "orbis_finalized", "orbis_finality_lag_blocks"):
			if not isinstance(sample.get(key), int) or isinstance(sample.get(key), bool) or sample[key] < 0:
				raise ValueError(f"invalid finality-lag sample field: {key}")
		if sample["orbis_finality_lag_blocks"] != sample["orbis_best"] - sample["orbis_finalized"]:
			raise ValueError("dishonest per-second finality-lag sample")
		for key in ("cores", "repetition", "state_sha256", "workload_command_sha256", "generated_topology_sha256"):
			if sample.get(key) != run.get(key):
				raise ValueError(f"finality-lag sample identity drift: {key}")
		if phase == "measurement" and elapsed <= run["measurement_seconds"]:
			measurement.append(sample["orbis_finality_lag_blocks"])
			measurement_elapsed.append(elapsed)
		elif phase == "drain" and elapsed >= run["measurement_seconds"]:
			drain.append(sample["orbis_finality_lag_blocks"])
		else:
			raise ValueError("finality-lag sample phase/elapsed mismatch")
	if not measurement or measurement_elapsed[-1] < run["measurement_seconds"] - 2.5 or previous < run["measurement_seconds"]:
		raise ValueError("finality-lag samples do not cover the full frozen measurement window")
	measurement_max = max(measurement)
	drain_max = max(drain) if drain else 0
	if measurement_max != run.get("measurement_max_finality_lag_blocks") or drain_max != run.get("drain_max_finality_lag_blocks"):
		raise ValueError("finality-lag summary does not match raw phase samples")
	return measurement_max, drain_max, len(samples)


def run_one(args, cores: int, repetition: int, run_dir: Path, inputs: dict) -> dict:
	assert_frozen_inputs(inputs)
	config = generated_config(cores, run_dir, args.max_runtime_instances, args.origin_binary)
	zombienet_log = run_dir / "zombienet.log"
	log = zombienet_log.open("w")
	process = subprocess.Popen(
		["zombienet", "-p", "native", "spawn", str(config)],
		cwd=ROOT,
		stdout=log,
		stderr=subprocess.STDOUT,
		start_new_session=True,
	)
	try:
		wait_rpc(args.startup_timeout)
		network_identity = isolated_network_identity()
		subprocess.run(
			[
				str(ROOT / "target/release/examples/bootstrap_orbis_core"),
				"--endpoint", "ws://127.0.0.1:9801", "--first-core", "0", "--cores", str(cores),
		],
		cwd=ROOT,
		check=True,
		stdout=(run_dir / "bootstrap.out").open("w"),
		stderr=subprocess.STDOUT,
	)
		smoke = subprocess.run(
		[
			sys.executable, "zombienet/orbis_smoke.py", "--relay", RELAY, "--orbis", ORBIS,
			"--expected-cores", str(cores), "--expected-block-rate", "3",
			"--minimum-finalized-block-ratio", "0.8", "--wait", str(args.smoke_seconds),
		],
		cwd=ROOT,
			capture_output=True,
			text=True,
		)
		(run_dir / "smoke.out").write_text(smoke.stdout)
		(run_dir / "smoke.err").write_text(smoke.stderr)
		if smoke.returncode:
			raise RuntimeError(f"Orbis smoke failed ({smoke.returncode}): {smoke.stderr.strip()}")
		smoke_result = json.loads(smoke.stdout)
		time.sleep(args.warmup_seconds)
		before = snapshot()
		topology_sha256 = sha256(config)
		environment = os.environ.copy()
		environment.update({
			"ORIGIN_RPC": RELAY,
			"ORBIS_RPC": ORBIS,
			"ORBIS_WS": "ws://127.0.0.1:9810",
			"ORBIS_CORES": str(cores),
			"CAMPAIGN_REPETITION": str(repetition),
			"CAMPAIGN_SEED": "20260713",
			"CAMPAIGN_MEASUREMENT_SECONDS": str(args.measurement_seconds),
			"CAMPAIGN_STATE_SHA256": inputs["state_sha256"],
		})
		started = time.monotonic()
		workload = subprocess.Popen(
			args.workload_command,
			cwd=ROOT,
			env=environment,
			shell=True,
			stdout=subprocess.PIPE,
			stderr=subprocess.PIPE,
			text=True,
			start_new_session=args.tuning_probe,
		)
		samples_path = run_dir / "finality-lag-samples.jsonl"
		measurement_max_lag = before["orbis_best"] - before["orbis_finalized"]
		drain_max_lag = 0
		next_sample = started
		breach_sample = None
		with samples_path.open("w") as samples:
			initial = finality_sample(0.0, "measurement", before, cores, repetition, inputs, topology_sha256)
			samples.write(json.dumps(initial, sort_keys=True) + "\n")
			samples.flush()
			if args.tuning_probe and initial["orbis_finality_lag_blocks"] > args.stop_lag_above:
				breach_sample = initial
			while workload.poll() is None and breach_sample is None:
				next_sample += args.sample_seconds
				delay = next_sample - time.monotonic()
				if delay > 0:
					time.sleep(delay)
				current = snapshot()
				elapsed_now = time.monotonic() - started
				phase = "measurement" if elapsed_now <= args.measurement_seconds else "drain"
				sample = finality_sample(elapsed_now, phase, current, cores, repetition, inputs, topology_sha256)
				samples.write(json.dumps(sample, sort_keys=True) + "\n")
				samples.flush()
				if phase == "measurement":
					measurement_max_lag = max(measurement_max_lag, sample["orbis_finality_lag_blocks"])
					if args.tuning_probe and sample["orbis_finality_lag_blocks"] > args.stop_lag_above:
						breach_sample = sample
						break
				else:
					drain_max_lag = max(drain_max_lag, sample["orbis_finality_lag_blocks"])
				if elapsed_now > args.measurement_seconds + args.driver_grace_seconds:
					workload.kill()
					stdout, stderr = workload.communicate()
					(run_dir / "workload.out").write_text(stdout)
					(run_dir / "workload.err").write_text(stderr)
					raise RuntimeError("workload driver exceeded measurement duration plus grace")
		if breach_sample is not None:
			os.killpg(workload.pid, signal.SIGTERM)
			try:
				stdout, stderr = workload.communicate(timeout=10)
			except subprocess.TimeoutExpired:
				os.killpg(workload.pid, signal.SIGKILL)
				stdout, stderr = workload.communicate()
			(run_dir / "workload.out").write_text(stdout)
			(run_dir / "workload.err").write_text(stderr)
			(run_dir / "first-lag-breach.json").write_text(json.dumps(breach_sample, indent=2, sort_keys=True) + "\n")
			after = snapshot()
			assert_frozen_inputs(inputs)
			return {
				"cores": cores,
				"repetition": repetition,
				"measurement_seconds": args.measurement_seconds,
				"observed_measurement_seconds": breach_sample["elapsed_seconds"],
				"completed_measurement_window": False,
				"first_lag_breach": breach_sample,
				"measurement_max_finality_lag_blocks": measurement_max_lag,
				"drain_max_finality_lag_blocks": 0,
				"lag_samples_file": samples_path.relative_to(ROOT).as_posix(),
				"lag_samples_sha256": sha256(samples_path),
				"claim_queue_cores": smoke_result["claim_queue_cores"],
				"runtime_contract": {"velocity": 3, "relay_parent_offset": 1, "unincluded_segment_capacity": 12},
				"before": before,
				"after": after,
				"state_sha256": inputs["state_sha256"],
				"workload_command_sha256": inputs["workload_command_sha256"],
				"generated_topology_sha256": topology_sha256,
				"network_identity": network_identity,
			}
		stdout, stderr = workload.communicate()
		(run_dir / "workload.out").write_text(stdout)
		(run_dir / "workload.err").write_text(stderr)
		elapsed = time.monotonic() - started
		if workload.returncode:
			raise RuntimeError(f"workload driver failed ({workload.returncode}): {stderr.strip()}")
		if elapsed < args.measurement_seconds:
			raise RuntimeError(f"workload driver stopped early after {elapsed:.2f}s")
		try:
			workload_result = json.loads(stdout)
		except Exception as error:
			raise RuntimeError(f"workload driver did not emit one JSON result: {error}") from error
		if set(workload_result) != {"attempted_calls", "finalized_successful_calls", "failed_calls", "state_sha256"}:
			raise RuntimeError("workload result fields do not match the frozen driver protocol")
		if workload_result["state_sha256"] != inputs["state_sha256"]:
			raise RuntimeError("workload driver state digest drift")
		attempted, calls, failed = (
			workload_result["attempted_calls"],
			workload_result["finalized_successful_calls"],
			workload_result["failed_calls"],
		)
		if any(not isinstance(value, int) or isinstance(value, bool) or value < 0 for value in (attempted, calls, failed)) or attempted != calls + failed:
			raise RuntimeError("workload call accounting is invalid")
		after = snapshot()
		elapsed = time.monotonic() - started
		final_phase = "measurement" if elapsed <= args.measurement_seconds else "drain"
		final_sample = finality_sample(elapsed, final_phase, after, cores, repetition, inputs, topology_sha256)
		with samples_path.open("a") as samples:
			samples.write(json.dumps(final_sample, sort_keys=True) + "\n")
		if final_phase == "measurement":
			measurement_max_lag = max(measurement_max_lag, final_sample["orbis_finality_lag_blocks"])
		else:
			drain_max_lag = max(drain_max_lag, final_sample["orbis_finality_lag_blocks"])
		assert_frozen_inputs(inputs)
		lag_samples_file = samples_path.relative_to(ROOT).as_posix()
		return {
			"cores": cores,
			"repetition": repetition,
			"finalized_successful_calls": calls,
			"attempted_calls": attempted,
			"failed_calls": failed,
			"measurement_seconds": args.measurement_seconds,
			"observed_measurement_seconds": args.measurement_seconds,
			"completed_measurement_window": True,
			"finalized_throughput": calls / args.measurement_seconds,
			"measurement_max_finality_lag_blocks": measurement_max_lag,
			"drain_max_finality_lag_blocks": drain_max_lag,
			"lag_samples_file": lag_samples_file,
			"lag_samples_sha256": sha256(samples_path),
			"claim_queue_cores": smoke_result["claim_queue_cores"],
			"runtime_contract": {"velocity": 3, "relay_parent_offset": 1, "unincluded_segment_capacity": 12},
			"before": before,
			"after": after,
			"state_sha256": inputs["state_sha256"],
			"workload_command_sha256": inputs["workload_command_sha256"],
			"generated_topology_sha256": topology_sha256,
			"network_identity": network_identity,
		}
	finally:
		if process.poll() is None:
			os.killpg(process.pid, signal.SIGTERM)
			try:
				process.wait(timeout=20)
			except subprocess.TimeoutExpired:
				os.killpg(process.pid, signal.SIGKILL)
		log.flush()
		copy_node_logs(zombienet_log, run_dir)
		log.close()


def coefficient_of_variation(values: list[float]) -> float | None:
	mean = statistics.fmean(values)
	return None if mean == 0 else statistics.pstdev(values) / mean * 100


def analyze(runs: list[dict]) -> dict:
	observed = [(run.get("cores"), run.get("repetition")) for run in runs]
	if observed != SCHEDULE:
		raise ValueError(f"run schedule drift: {observed!r}")
	state = {run.get("state_sha256") for run in runs}
	workload = {run.get("workload_command_sha256") for run in runs}
	if len(state) != 1 or len(workload) != 1:
		raise ValueError("state/workload digest drift across interleaved runs")
	by_core = {1: [], 3: []}
	genesis_by_core = {1: set(), 3: set()}
	for run in runs:
		if run.get("runtime_contract") != {"velocity": 3, "relay_parent_offset": 1, "unincluded_segment_capacity": 12}:
			raise ValueError("runtime authoring contract drift")
		cores = run["cores"]
		network = run.get("network_identity", {})
		if network.get("expected_peer_count") != {"relay": 7, "orbis": 1} or network.get("observed_peer_count") != {"relay": 7, "orbis": 1}:
			raise ValueError("run lacks exact peer-count isolation evidence")
		if not isinstance(network.get("relay_genesis_hash"), str) or not isinstance(network.get("orbis_genesis_hash"), str):
			raise ValueError("run lacks genesis isolation evidence")
		genesis_by_core[cores].add((network["relay_genesis_hash"], network["orbis_genesis_hash"]))
		assigned = run.get("claim_queue_cores", [])
		if not isinstance(assigned, list) or len(set(assigned)) != len(assigned) or len(assigned) < cores:
			raise ValueError(f"run {cores}/{run['repetition']} lacks claim-queue assignments")
		calls, duration = run.get("finalized_successful_calls"), run.get("measurement_seconds")
		if not isinstance(calls, int) or isinstance(calls, bool) or calls < 0 or not isinstance(duration, int) or duration <= 0:
			raise ValueError("invalid finalized-call denominator")
		measurement_lag, drain_lag, sample_count = validate_lag_samples(run)
		run["validated_lag_sample_count"] = sample_count
		if drain_lag < 0 or measurement_lag < 0:
			raise ValueError("invalid phase-specific finality-lag observation")
		computed = calls / duration
		if not math.isclose(computed, run.get("finalized_throughput", -1), rel_tol=1e-12):
			raise ValueError("dishonest finalized-throughput value")
		by_core[cores].append(computed)
	if any(len(values) != 1 for values in genesis_by_core.values()):
		raise ValueError("clean-genesis identity drift across repetitions")
	if next(iter(genesis_by_core[1]))[0] == next(iter(genesis_by_core[3]))[0]:
		raise ValueError("one-core and three-core relay genesis identities unexpectedly match")
	metrics = {}
	for cores in (1, 3):
		metrics[str(cores)] = {
			"throughput_samples": by_core[cores],
			"median_finalized_successful_calls_per_second": statistics.median(by_core[cores]),
			"cv_percent": coefficient_of_variation(by_core[cores]),
			"measurement_max_finality_lag_blocks": max(run["measurement_max_finality_lag_blocks"] for run in runs if run["cores"] == cores),
			"drain_max_finality_lag_blocks": max(run["drain_max_finality_lag_blocks"] for run in runs if run["cores"] == cores),
		}
	ratio = metrics["3"]["median_finalized_successful_calls_per_second"] / metrics["1"]["median_finalized_successful_calls_per_second"] if metrics["1"]["median_finalized_successful_calls_per_second"] else 0
	failures = []
	if ratio < 2.4:
		failures.append("three-core/one-core-finalized-throughput-ratio-below-2.4")
	for cores in ("1", "3"):
		if metrics[cores]["median_finalized_successful_calls_per_second"] == 0:
			failures.append(f"{cores}-core-zero-finalized-throughput")
		if metrics[cores]["cv_percent"] is None or metrics[cores]["cv_percent"] > 10:
			failures.append(f"{cores}-core-cv-above-10-percent")
		if metrics[cores]["measurement_max_finality_lag_blocks"] > 15:
			failures.append(f"{cores}-core-finality-lag-above-15-blocks")
	return {"metrics": metrics, "three_core_to_one_core_ratio": ratio, "failures": failures, "pass": not failures}


def write(path: Path | None, value: dict):
	encoded = json.dumps(value, indent=2, sort_keys=True) + "\n"
	if path:
		path.parent.mkdir(parents=True, exist_ok=True)
		path.write_text(encoded)
	else:
		print(encoded, end="")


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument("--execute", action="store_true")
	parser.add_argument("--analyze", type=Path)
	parser.add_argument("--output", type=Path)
	parser.add_argument("--artifacts-dir", type=Path)
	parser.add_argument("--state-file", default=str(DEFAULT_STATE))
	parser.add_argument("--workload-command", default=DEFAULT_WORKLOAD)
	parser.add_argument("--startup-timeout", type=int, default=300)
	parser.add_argument("--smoke-seconds", type=int, default=30)
	parser.add_argument("--warmup-seconds", type=int, default=600)
	parser.add_argument("--measurement-seconds", type=int, default=1800)
	parser.add_argument("--cooldown-seconds", type=int, default=300)
	parser.add_argument("--sample-seconds", type=int, default=1)
	parser.add_argument("--driver-grace-seconds", type=int, default=150)
	parser.add_argument("--diagnostic-core", type=int, choices=(1, 3))
	parser.add_argument("--diagnostic-repetition", type=int, default=0)
	parser.add_argument("--tuning-probe", action="store_true")
	parser.add_argument("--max-runtime-instances", type=int)
	parser.add_argument("--stop-lag-above", type=int, default=15)
	parser.add_argument("--host-manifest", type=Path)
	parser.add_argument("--origin-binary", type=Path)
	args = parser.parse_args()
	if args.analyze:
		raw = json.loads(args.analyze.read_text())
		if not isinstance(raw, dict) or raw.get("schema_version") != 1 or raw.get("campaign_executed") is not True or not isinstance(raw.get("runs"), list):
			raise SystemExit("analysis input must be a schema-v1 executed campaign envelope")
		identity = raw.get("inputs", {})
		if identity.get("commit") != command_output(["git", "rev-parse", "HEAD"]) or identity.get("branch") != "sm-update-sub-0x63":
			raise SystemExit("analysis input is not bound to the current P1 branch revision")
		runs = raw["runs"]
		result = analyze(runs)
		write(args.output, {"schema_version": 1, "campaign_executed": True, "inputs": identity, "runs": runs, **result})
		return 0 if result["pass"] else 1
	blockers, inputs = preflight(args)
	if blockers:
		write(args.output, {
			"schema_version": 1,
			"scope": "p1-orbis-elastic-campaign",
			"status": "blocked",
			"campaign_executed": False,
			"blockers": blockers,
			"inputs": inputs,
			"metrics": None,
			"performance_claim": False,
			"acceptance": {
				"minimum_three_core_to_one_core_ratio": 2.4,
				"maximum_cv_percent": 10,
				"maximum_finality_lag_blocks": 15,
				"block_processing_velocity": 3,
				"relay_parent_offset": 1,
				"unincluded_segment_capacity": 12,
			},
			"validation_commands": [
				"python3 scripts/test-orbis-elastic-campaign.py",
				"python3 scripts/run-orbis-elastic-campaign.py --output docs/evidence/performance/p1-elastic-campaign-status.json",
			],
		})
		return 2
	if not args.execute:
		write(args.output, {"schema_version": 1, "status": "ready", "campaign_executed": False, "inputs": inputs, "metrics": None, "performance_claim": False})
		return 0
	if args.artifacts_dir is None:
		raise SystemExit("--execute requires --artifacts-dir so raw evidence is retained")
	runs = []
	base = args.artifacts_dir.resolve()
	base.mkdir(parents=True, exist_ok=True)
	if any(base.iterdir()):
		raise SystemExit(f"artifacts directory must be empty: {base}")
	if args.host_manifest:
		shutil.copy2(args.host_manifest, base / "host-environment.json")
	if args.diagnostic_core is not None:
		run_dir = base / f"diagnostic-{args.diagnostic_core}-core-{args.diagnostic_repetition}"
		run_dir.mkdir()
		run = run_one(args, args.diagnostic_core, args.diagnostic_repetition, run_dir, inputs)
		run["evidence_files"] = {
			path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path): sha256(path)
			for path in sorted(run_dir.rglob("*")) if path.is_file()
		}
		if args.tuning_probe and not run["completed_measurement_window"]:
			samples_path = Path(run["lag_samples_file"])
			if not samples_path.is_absolute():
				samples_path = ROOT / samples_path
			samples = [json.loads(line) for line in samples_path.read_text().splitlines() if line]
			measurement_lag = max(sample["orbis_finality_lag_blocks"] for sample in samples)
			drain_lag, sample_count = 0, len(samples)
			if samples[-1] != run["first_lag_breach"] or measurement_lag <= args.stop_lag_above:
				raise RuntimeError("tuning probe breach evidence is inconsistent")
		else:
			measurement_lag, drain_lag, sample_count = validate_lag_samples(run)
		time.sleep(args.cooldown_seconds)
		diagnostic_pass = run["completed_measurement_window"] and measurement_lag <= 15
		write(args.output, {
			"schema_version": 1,
			"scope": "p1-orbis-elastic-tuning-probe" if args.tuning_probe else "p1-orbis-elastic-bounded-diagnostic",
			"status": "diagnostic-pass" if diagnostic_pass else "diagnostic-fail",
			"campaign_executed": False,
			"diagnostic_executed": True,
			"performance_claim": False,
			"inputs": inputs,
			"run": run,
			"attribution": {
				"measurement_max_finality_lag_blocks": measurement_lag,
				"drain_max_finality_lag_blocks": drain_lag,
				"raw_sample_count": sample_count,
				"steady_state_gate_maximum_blocks": 15,
			},
			"metrics": None,
		})
		return 0 if diagnostic_pass else 1
	for index, (cores, repetition) in enumerate(SCHEDULE, 1):
		run_dir = base / f"{index:02d}-{cores}-core-{repetition}"
		run_dir.mkdir()
		run = run_one(args, cores, repetition, run_dir, inputs)
		run["evidence_files"] = {
			path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path): sha256(path)
			for path in sorted(run_dir.iterdir()) if path.is_file()
		}
		runs.append(run)
		write(base / "campaign-progress.json", {
			"schema_version": 1,
			"campaign_executed": False,
			"completed_runs": len(runs),
			"required_runs": len(SCHEDULE),
			"inputs": inputs,
			"runs": runs,
			"metrics": None,
			"performance_claim": False,
		})
		if index != len(SCHEDULE):
			time.sleep(args.cooldown_seconds)
	result = analyze(runs)
	write(args.output, {"schema_version": 1, "scope": "p1-orbis-elastic-campaign", "status": "pass" if result["pass"] else "fail", "campaign_executed": True, "inputs": inputs, "runs": runs, **result})
	return 0 if result["pass"] else 1


if __name__ == "__main__":
	raise SystemExit(main())
