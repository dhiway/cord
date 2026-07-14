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

"""Fail-closed verifier for a signed/finalized P1 control and Broker campaign ledger."""

import argparse
import hashlib
import json
import pathlib

REQUIRED = {
    "origin-validator-lifecycle": {
        "AuthorityManager.QueuedRemoval", "AuthorityManager.Planned",
        "AuthorityManager.Enacted", "AuthorityManager.QueuedAdd", "Session.NewSession",
    },
    "orbis-collator-lifecycle": {"CollatorSelection.NewInvulnerables", "Session.NewSession"},
    "origin-upgrade-rejection": {
        "System.UpgradeAuthorized", "System.RejectedInvalidAuthorizedUpgrade",
    },
    "orbis-upgrade-rejection": {
        "System.UpgradeAuthorized", "System.RejectedInvalidAuthorizedUpgrade",
    },
    "origin-pause-resume": {"TxPause.CallPaused", "TxPause.CallUnpaused"},
    "orbis-pause-resume": {"TxPause.CallPaused", "TxPause.CallUnpaused"},
    "operator-key-recovery": {
        "AuthorityManager.QueuedRemoval", "AuthorityManager.QueuedAdd", "Session.NewSession",
    },
    "broker-lifecycle": {
        "Broker.ReservationMade", "Broker.CoreCountRequested", "Broker.CoreCountChanged",
        "Broker.CoreAssigned", "Broker.Renewable", "Broker.Renewed",
        "Broker.ReservationCancelled",
    },
    "broker-xcm-faults": set(),
    "persistent-restart": set(),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(report, report_dir):
    require(report.get("schema_version") == 1, "unsupported campaign schema")
    ratification = report.get("p0_ratification", {})
    require(ratification.get("status") == "PASS", "P0 signature/payload validation is absent")
    require(len(ratification.get("payload_sha256", "")) == 64, "P0 payload SHA-256 is absent")

    topology = report.get("topology", {})
    require(topology.get("distinct_genesis") is True, "campaign genesis is not distinct")
    require(topology.get("isolate_env") is True, "protocol/fork isolation is not proven")
    require(topology.get("campaign_specific_node_keys") is True, "node-key isolation is not proven")
    require(topology.get("unexpected_peer_count") == 0, "unexpected/cross-topology peers were observed")
    require(topology.get("equivocation_log_count") == 0, "equivocation logs invalidate campaign")
    require(
        topology.get("relay_genesis_hash") != topology.get("canonical_relay_genesis_hash"),
        "relay genesis matches canonical topology",
    )

    for relative, expected in report.get("artifacts_sha256", {}).items():
        path = report_dir / relative
        require(path.is_file(), f"missing raw receipt artifact {relative}")
        require(sha256(path) == expected, f"receipt artifact hash mismatch: {relative}")

    operations = report.get("operations", [])
    indexed = {operation.get("scenario"): operation for operation in operations}
    require(len(indexed) == len(operations), "duplicate or unnamed campaign scenario")
    for scenario, expected_events in REQUIRED.items():
        require(scenario in indexed, f"missing required scenario {scenario}")
        operation = indexed[scenario]
        require(operation.get("status") == "PASS", f"scenario {scenario} did not pass")
        require(operation.get("finalized") is True, f"scenario {scenario} lacks finality")
        missing = expected_events - set(operation.get("events", []))
        require(not missing, f"scenario {scenario} missing events: {sorted(missing)}")

    rotations = ("origin-validator-lifecycle", "orbis-collator-lifecycle", "operator-key-recovery")
    for scenario in rotations:
        operation = indexed[scenario]
        require(
            operation.get("session_after", 0) > operation.get("session_before", 0),
            f"{scenario} lacks enacted session rotation",
        )
    for scenario in ("origin-upgrade-rejection", "orbis-upgrade-rejection"):
        operation = indexed[scenario]
        require(
            operation.get("spec_version_after") == operation.get("spec_version_before"),
            f"{scenario} changed runtime version",
        )
        require(operation.get("unauthorized_rejected") is True, f"{scenario} lacks rejection")
    for scenario in ("origin-pause-resume", "orbis-pause-resume"):
        operation = indexed[scenario]
        require(operation.get("paused_call_rejected") is True, f"{scenario} lacks rejection")
        require(operation.get("unpaused_call_succeeded") is True, f"{scenario} lacks recovery")

    broker = indexed["broker-lifecycle"]
    for key in ("requested", "reserved", "assigned", "renewed", "resized", "released"):
        require(broker.get(key) is True, f"Broker lifecycle did not prove {key}")
    require(broker.get("para_id") == 1006, "Broker lifecycle targeted wrong parachain")
    require(broker.get("claim_queue_cores_after_assign", 0) >= 3, "fewer than three cores")

    xcm = indexed["broker-xcm-faults"]
    require(xcm.get("duplicate_rejected") is True, "duplicate Broker/Coretime message not rejected")
    require(xcm.get("delayed_delivered_once") is True, "delayed message not delivered exactly once")

    restart = indexed["persistent-restart"]
    require(restart.get("genesis_unchanged") is True, "persistent restart changed genesis")
    require(restart.get("claim_queue_unchanged") is True, "persistent restart changed claim queue")
    require(
        restart.get("best_progress") is True and restart.get("finalized_progress") is True,
        "finality did not recover after restart",
    )
    require(restart.get("new_pid") != restart.get("old_pid"), "restart reused old process")
    return {"status": "PASS", "p1_gate_verdict": "PASS", "verified_scenarios": sorted(REQUIRED)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=pathlib.Path)
    args = parser.parse_args()
    report = json.loads(args.report.read_text())
    try:
        verdict = validate(report, args.report.resolve().parent)
    except (ValueError, KeyError, TypeError) as error:
        print(json.dumps({"status": "FAIL", "p1_gate_verdict": "BLOCKED", "reason": str(error)}, indent=2))
        raise SystemExit(1)
    print(json.dumps(verdict, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
