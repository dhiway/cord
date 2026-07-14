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

"""Fail-closed and tamper tests for the P1 canonical materializer."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("materialize-p1-verification.py")
spec = importlib.util.spec_from_file_location("p1_materializer", SCRIPT)
module = importlib.util.module_from_spec(spec)
assert spec.loader
spec.loader.exec_module(module)
HEAD = "a" * 40


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def binding(path: Path, role=None):
    value = {"path": str(path), "sha256": module.sha256(path)}
    if role is not None:
        value["role"] = role
    return value


def binary(root: Path, name: str):
    path = root / name
    path.write_bytes((name + "\n").encode())
    return path


def case_record():
    return {
        "status": "pass",
        "input_hashes": {"input": "1" * 64},
        "output_hashes": {"output": "2" * 64},
        "finalized_blocks": [1],
        "events": [{"event": "ok"}],
        "assertions": {"required": True},
    }


def fixture(root: Path) -> Path:
    source_files = {}
    for role in sorted(module.REQUIRED_SOURCE_ROLES):
        source_file = root / "source" / role
        source_file.parent.mkdir(parents=True, exist_ok=True)
        source_file.write_text(role + "\n")
        source_files[role] = source_file
    scenario = root / "scenarios.json"
    phases = {
        "control": [{"id": "AC7-ONE"}],
        "broker-pre-restart": [{"id": "AC8-PRE"}],
        "broker-post-restart": [{"id": "AC8-POST"}],
    }
    write_json(
        scenario,
        {
            "schema": "cord.p1-control-broker-scenarios.v1",
            "campaign_id": "origin-orbis-p1-control-broker-v1",
            "phases": phases,
        },
    )
    control_topology = root / "control.toml"
    control_topology.write_text("control topology\n")
    control_bins = {
        name: binary(root, name)
        for name in ("origin", "orbis", "driver", "origin_upgrade_wasm", "orbis_upgrade_wasm")
    }
    candidate_manifest = root / "candidate-manifest.json"
    write_json(
        candidate_manifest,
        {
            "schema": "cord.p1-upgrade-candidates.v1",
            "status": "pass",
            "authoritative": True,
            "production_versions_unchanged": True,
            "source": {"commit": HEAD},
            "candidates": {
                "origin": {
                    "sha256": module.sha256(control_bins["origin_upgrade_wasm"]),
                    "spec_name": "origin",
                    "current_spec_version": 9901,
                    "candidate_spec_version": 9902,
                    "higher_spec": True,
                    "features": ["p1-upgrade-candidate"],
                },
                "orbis-fast": {
                    "sha256": module.sha256(control_bins["orbis_upgrade_wasm"]),
                    "spec_name": "orbis",
                    "current_spec_version": 29,
                    "candidate_spec_version": 30,
                    "higher_spec": True,
                    "features": ["fast-runtime", "p1-upgrade-candidate"],
                },
            },
        },
    )
    control_root = root / "control-raw"
    raw_log = control_root / "raw/driver.log"
    raw_log.parent.mkdir(parents=True)
    raw_log.write_text("finalized raw receipt\n")
    raw_hashes = [{"path": "raw/driver.log", "sha256": module.sha256(raw_log), "bytes": raw_log.stat().st_size}]
    write_json(control_root / "raw-hashes.json", raw_hashes)
    versions = {
        str(port): {
            "specName": "origin" if port < 6 else "orbis",
            "specVersion": 9901 if port < 6 else 29,
        }
        for port in range(8)
    }
    control_raw = {
        "schema": "cord.p1-control-broker-evidence.v1",
        "status": "pass",
        "prepared_only": False,
        "campaign_id": "origin-orbis-p1-control-broker-v1",
        "inputs": {
            "origin_binary_sha256": module.sha256(control_bins["origin"]),
            "orbis_binary_sha256": module.sha256(control_bins["orbis"]),
            "driver_sha256": module.sha256(control_bins["driver"]),
            "origin_upgrade_wasm_sha256": module.sha256(control_bins["origin_upgrade_wasm"]),
            "orbis_upgrade_wasm_sha256": module.sha256(control_bins["orbis_upgrade_wasm"]),
            "topology_sha256": module.sha256(control_topology),
            "scenario_manifest_sha256": module.sha256(scenario),
        },
        "preflight": {"status": "pass", "topology_sha256": module.sha256(control_topology), "versions": versions},
        "cases": {
            "control": {"cases": {"AC7-ONE": case_record()}},
            "broker-pre-restart": {"cases": {"AC8-PRE": case_record()}},
            "full-restart-runner": {"status": "pass"},
            "broker-post-restart": {"cases": {"AC8-POST": case_record()}},
        },
        "raw_hashes": raw_hashes,
    }
    control_path = control_root / "control-broker-evidence.json"
    write_json(control_path, control_raw)

    smoke_topology = root / "elastic.toml"
    smoke_topology.write_text("elastic topology\n")
    smoke_origin, smoke_orbis = binary(root, "smoke-origin"), binary(root, "smoke-orbis")
    smoke = root / "smoke.json"
    write_json(
        smoke,
        {
            "status": "ok",
            "before": {"relay_best": 1, "relay_finalized": 1, "orbis_best": 1, "orbis_finalized": 1},
            "after": {"relay_best": 3, "relay_finalized": 2, "orbis_best": 7, "orbis_finalized": 4},
            "para_id": 1006,
            "claim_queue_cores": [0, 1, 2],
            "target_block_rate": 3,
            "relay_parent_offset": 1,
            "observed_finalized_block_ratio": 3.0,
            "measurement_seconds": 60,
            "relay_spec_version": 9901,
            "orbis_spec_version": 29,
        },
    )

    proof_topology = root / "proof.toml"
    proof_topology.write_text("proof topology\n")
    proof_driver = binary(root, "proof-driver")
    proof_bins = {name: binary(root, "proof-" + name) for name in ("origin", "origin_orbis", "proof_fault_origin_orbis")}
    proof_root = root / "proof-evidence"
    metadata_hash = "0x" + "11" * 32
    samples = []
    cases = []
    manifest_hash = "3" * 64
    for index in range(10):
        sample = {
            "storage_tx_count": 50,
            "block_weight_ref_time": 50,
            "block_weight_proof_size": 50,
            "block_length_bytes": 50,
            "block_weight_ref_time_limit": 100,
            "block_weight_proof_size_limit": 100,
            "block_length_limit_bytes": 100,
            "database_bytes": 1,
            "finality_lag_blocks": 1,
            "metadata_hash": metadata_hash,
            "block_weights_constant_scale": "0x01",
            "block_length_constant_scale": "0x02",
        }
        samples.append(sample)
        capacity = proof_root / f"case-{index}/capacity.json"
        write_json(capacity, {"runtime_constants": {"max_block_transactions": 100}, "samples": [sample]})
        cases.append(
            {
                "case": f"case-{index}",
                "status": "pass",
                "manifest_sha256": manifest_hash,
                "topology_sha256": module.sha256(proof_topology),
                "driver_sha256": module.sha256(proof_driver),
                "binaries_sha256": {name: module.sha256(path) for name, path in proof_bins.items()},
                "artifacts": [
                    {
                        "kind": "capacity-samples",
                        "path": str(capacity.relative_to(proof_root)),
                        "sha256": module.sha256(capacity),
                    }
                ],
            }
        )
    aggregate = module.recompute_capacity(samples, 100)
    aggregate["status"] = "pass"
    campaign = root / "campaign-verdict.json"
    write_json(
        campaign,
        {
            "schema_version": 1,
            "status": "pass",
            "errors": [],
            "manifest_sha256": manifest_hash,
            "topology_sha256": module.sha256(proof_topology),
            "driver_sha256": module.sha256(proof_driver),
            "binaries_sha256": {name: module.sha256(path) for name, path in proof_bins.items()},
            "global_cleanup": {"status": "pass"},
            "cases": cases,
            "capacity_ac13_p1": aggregate,
        },
    )
    proof_verdict = root / "proof-verdict.json"
    write_json(
        proof_verdict,
        {"overall_status": "pass", "production_e2e": {"status": "pass", "verdict_sha256": module.sha256(campaign)}},
    )

    inputs = root / "inputs.json"
    write_json(
        inputs,
        {
            "schema": module.INPUT_SCHEMA,
            "source": {
                "commit": HEAD,
                "files": [binding(path, role) for role, path in sorted(source_files.items())],
            },
            "ac7_ac8": {
                "raw": binding(control_path),
                "scenario_manifest": binding(scenario),
                "candidate_manifest": binding(candidate_manifest),
                "topology": binding(control_topology),
                "binaries": {name: binding(path) for name, path in control_bins.items()},
                "runtime": {
                    "origin": {"spec_name": "origin", "current_spec_version": 9901, "candidate_spec_version": 9902},
                    "orbis": {"spec_name": "orbis", "current_spec_version": 29, "candidate_spec_version": 30, "transaction_version": 8},
                },
            },
            "ac10": {
                "raw": binding(smoke),
                "topology": binding(smoke_topology),
                "binaries": {"origin": binding(smoke_origin), "orbis": binding(smoke_orbis)},
                "runtime": {
                    "origin": {"spec_name": "origin", "spec_version": 9901},
                    "orbis": {"spec_name": "orbis", "spec_version": 29, "transaction_version": 8},
                },
                "unincluded_segment_capacity": 12,
            },
            "ac13": {
                "raw": binding(campaign),
                "proof_verdict": binding(proof_verdict),
                "evidence_root": str(proof_root),
                "topology": binding(proof_topology),
                "driver": binding(proof_driver),
                "binaries": {name: binding(path) for name, path in proof_bins.items()},
                "runtime": {"orbis": {"spec_name": "orbis", "spec_version": 29, "transaction_version": 8, "metadata_hash": metadata_hash}},
            },
        },
    )
    return inputs


class MaterializerTests(unittest.TestCase):
    def run_main(self, argv):
        with mock.patch.object(module, "current_head", return_value=HEAD), mock.patch.object(sys, "argv", [str(SCRIPT), *argv]):
            return module.main()

    def test_materializes_then_requires_independent_review_for_index(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture(root)
            output = root / "out"
            self.assertEqual(self.run_main(["--inputs", str(inputs), "--output-dir", str(output)]), 2)
            self.assertFalse((output / "index.json").exists())
            artifacts = {name: module.sha256(output / name) for name in module.OUTPUTS.values()}
            review = root / "review.json"
            write_json(
                review,
                {
                    "schema": module.REVIEW_SCHEMA,
                    "status": "pass",
                    "independent": True,
                    "reviewer": "independent-test-reviewer",
                    "source_commit": HEAD,
                    "artifacts": artifacts,
                },
            )
            self.assertEqual(
                self.run_main(["--inputs", str(inputs), "--output-dir", str(output), "--review", str(review)]), 0
            )
            index = json.loads((output / "index.json").read_text())
            self.assertTrue(index["pass"])
            self.assertEqual(set(index["criteria"]), set(module.OUTPUTS))

    def test_tampered_control_raw_artifact_produces_no_outputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture(root)
            (root / "control-raw/raw/driver.log").write_text("tampered\n")
            output = root / "out"
            self.assertEqual(self.run_main(["--inputs", str(inputs), "--output-dir", str(output)]), 1)
            self.assertFalse(output.exists())

    def test_prepared_control_result_cannot_be_promoted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture(root)
            ledger = json.loads(inputs.read_text())
            raw_path = Path(ledger["ac7_ac8"]["raw"]["path"])
            raw = json.loads(raw_path.read_text())
            raw["status"] = "prepared"
            write_json(raw_path, raw)
            ledger["ac7_ac8"]["raw"] = binding(raw_path)
            write_json(inputs, ledger)
            output = root / "out"
            self.assertEqual(self.run_main(["--inputs", str(inputs), "--output-dir", str(output)]), 1)
            self.assertFalse(output.exists())

    def test_tampered_capacity_sample_fails_hash_binding(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture(root)
            capacity = root / "proof-evidence/case-0/capacity.json"
            payload = json.loads(capacity.read_text())
            payload["samples"][0]["storage_tx_count"] = 99
            write_json(capacity, payload)
            output = root / "out"
            self.assertEqual(self.run_main(["--inputs", str(inputs), "--output-dir", str(output)]), 1)
            self.assertFalse(output.exists())

    def test_bad_review_never_creates_index(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = fixture(root)
            output = root / "out"
            review = root / "review.json"
            write_json(
                review,
                {
                    "schema": module.REVIEW_SCHEMA,
                    "status": "pass",
                    "independent": True,
                    "reviewer": "reviewer",
                    "source_commit": HEAD,
                    "artifacts": {name: "0" * 64 for name in module.OUTPUTS.values()},
                },
            )
            self.assertEqual(
                self.run_main(["--inputs", str(inputs), "--output-dir", str(output), "--review", str(review)]), 1
            )
            self.assertFalse((output / "index.json").exists())


if __name__ == "__main__":
    unittest.main()
