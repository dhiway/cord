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

"""Deterministic unit checks for the P1 live evidence decoder and guardrails."""

import importlib.util
import pathlib
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).parents[1] / "zombienet" / "p1_control_broker.py"
SPEC = importlib.util.spec_from_file_location("p1_control_broker", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def load_script(name):
    path = pathlib.Path(__file__).parent / name
    spec = importlib.util.spec_from_file_location(name.removesuffix(".py"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


TOPOLOGY = load_script("generate-p1-isolated-topology.py")
RECEIPTS = load_script("verify-p1-control-receipts.py")


class P1ControlBrokerTests(unittest.TestCase):
    def test_claim_queue_decodes_multiple_cores_and_paras(self):
        # Vec<(u32, Vec<u32>)>: [(0, [1006]), (2, [1006, 2000])].
        encoded = "0x08" + "00000000" + "04" + "ee030000" + "02000000" + "08" + "ee030000d0070000"
        self.assertEqual(MODULE.decode_claim_queue(encoded), {0: [1006], 2: [1006, 2000]})

    def test_claim_queue_rejects_trailing_or_truncated_data(self):
        with self.assertRaisesRegex(ValueError, "trailing"):
            MODULE.decode_claim_queue("0x00ff")
        with self.assertRaisesRegex(ValueError, "truncated"):
            MODULE.decode_claim_queue("0x0400")

    def test_fault_argument_is_strict(self):
        self.assertEqual(MODULE.parse_fault("collator:9811:123"), ("collator", 9811, 123))
        with self.assertRaises(Exception):
            MODULE.parse_fault("collator:9811")

    def test_unsafe_fault_pid_is_rejected_before_process_lookup(self):
        with self.assertRaisesRegex(RuntimeError, "unsafe PID"):
            MODULE.validate_fault_target("orbis-collator", 9811, 1)

    def test_complete_gate_is_explicitly_out_of_scope(self):
        self.assertEqual(len(MODULE.REQUIRED_PRIVILEGED_SCENARIOS), 8)
        self.assertIn(
            "broker-request-reserve-assign-renew-resize-release",
            MODULE.REQUIRED_PRIVILEGED_SCENARIOS,
        )

    def test_ratification_signatures_and_payload_are_current(self):
        verdict = MODULE.verify_p0_ratification(pathlib.Path(__file__).parents[1])
        self.assertEqual(verdict["status"], "PASS")
        self.assertEqual(len(verdict["payload_sha256"]), 64)

    def test_isolated_topology_has_distinct_genesis_protocol_and_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "campaign"
            manifest = TOPOLOGY.generate(pathlib.Path(__file__).parents[1], output, "unit-test")
            config = (output / "testnet.toml").read_text()
            self.assertTrue(manifest["isolation"]["zombienet_isolate_env"])
            self.assertIn("isolate_env = true", config)
            self.assertIn("balance = 2000000000001", config)
            self.assertIn("rpc_port = 11801", config)
            self.assertEqual(len(list((output / "keys").glob("*.key"))), 8)

    def test_receipt_verifier_passes_only_complete_finalized_campaign(self):
        operations = []
        for scenario, events in RECEIPTS.REQUIRED.items():
            operation = {
                "scenario": scenario,
                "status": "PASS",
                "finalized": True,
                "events": list(events),
            }
            if scenario in (
                "origin-validator-lifecycle",
                "orbis-collator-lifecycle",
                "operator-key-recovery",
            ):
                operation.update(session_before=1, session_after=2)
            if scenario.endswith("upgrade-rejection"):
                operation.update(
                    spec_version_before=29,
                    spec_version_after=29,
                    unauthorized_rejected=True,
                )
            if scenario.endswith("pause-resume"):
                operation.update(paused_call_rejected=True, unpaused_call_succeeded=True)
            if scenario == "broker-lifecycle":
                operation.update(
                    requested=True,
                    reserved=True,
                    assigned=True,
                    renewed=True,
                    resized=True,
                    released=True,
                    para_id=1006,
                    claim_queue_cores_after_assign=3,
                )
            if scenario == "broker-xcm-faults":
                operation.update(duplicate_rejected=True, delayed_delivered_once=True)
            if scenario == "persistent-restart":
                operation.update(
                    genesis_unchanged=True,
                    claim_queue_unchanged=True,
                    best_progress=True,
                    finalized_progress=True,
                    old_pid=100,
                    new_pid=101,
                )
            operations.append(operation)
        report = {
            "schema_version": 1,
            "p0_ratification": {"status": "PASS", "payload_sha256": "a" * 64},
            "topology": {
                "distinct_genesis": True,
                "isolate_env": True,
                "campaign_specific_node_keys": True,
                "unexpected_peer_count": 0,
                "equivocation_log_count": 0,
                "relay_genesis_hash": "isolated",
                "canonical_relay_genesis_hash": "canonical",
            },
            "operations": operations,
        }
        self.assertEqual(RECEIPTS.validate(report, pathlib.Path("."))["status"], "PASS")
        operations[0]["events"] = []
        with self.assertRaisesRegex(ValueError, "missing events"):
            RECEIPTS.validate(report, pathlib.Path("."))


if __name__ == "__main__":
    unittest.main()
