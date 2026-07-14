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

"""Fail-closed live evidence probe for Origin/Orbis P1 control and Broker gates.

This probe intentionally does not submit privileged extrinsics.  It verifies the
running, hashable network facts that can be observed without custody of operator
keys and can inject reversible process-loss faults.  The resulting JSON always
lists the privileged scenarios that still need a signed, finalized E2E campaign;
an observation or fault run must not be represented as the complete P1 gate.
"""

import argparse
import json
import os
import pathlib
import shlex
import signal
import subprocess
import time
import urllib.request


REQUIRED_PRIVILEGED_SCENARIOS = (
    "origin-validator-admission-removal-session-rotation",
    "orbis-collator-admission-removal-session-rotation",
    "origin-and-orbis-runtime-upgrade-forward-fix",
    "transaction-pause-resume",
    "operator-key-compromise-and-recovery",
    "broker-request-reserve-assign-renew-resize-release",
    "broker-delayed-and-duplicate-xcm",
    "broker-session-boundary-and-full-node-restart",
)


def rpc(url, method, params=None):
    request = urllib.request.Request(
        url,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}
        ).encode(),
        headers={"content-type": "application/json"},
    )
    response = json.load(urllib.request.urlopen(request, timeout=10))
    if "error" in response:
        raise RuntimeError(f"{method}: {response['error']}")
    return response["result"]


def compact(data, offset):
    if offset >= len(data):
        raise ValueError("truncated compact integer")
    first = data[offset]
    mode = first & 0b11
    if mode == 0:
        return first >> 2, offset + 1
    if mode == 1:
        end = offset + 2
        if end > len(data):
            raise ValueError("truncated two-byte compact integer")
        return int.from_bytes(data[offset:end], "little") >> 2, end
    if mode == 2:
        end = offset + 4
        if end > len(data):
            raise ValueError("truncated four-byte compact integer")
        return int.from_bytes(data[offset:end], "little") >> 2, end
    length = (first >> 2) + 4
    start, end = offset + 1, offset + 1 + length
    if end > len(data):
        raise ValueError("truncated big compact integer")
    return int.from_bytes(data[start:end], "little"), end


def decode_claim_queue(encoded):
    data = bytes.fromhex(encoded.removeprefix("0x"))
    entries, offset = compact(data, 0)
    queue = {}
    for _ in range(entries):
        if offset + 4 > len(data):
            raise ValueError("truncated claim-queue core index")
        core = int.from_bytes(data[offset : offset + 4], "little")
        offset += 4
        count, offset = compact(data, offset)
        queue[core] = []
        for _ in range(count):
            if offset + 4 > len(data):
                raise ValueError("truncated claim-queue para id")
            queue[core].append(int.from_bytes(data[offset : offset + 4], "little"))
            offset += 4
    if offset != len(data):
        raise ValueError(f"claim queue has {len(data) - offset} trailing bytes")
    return queue


def height(url, finalized=False):
    block_hash = rpc(url, "chain_getFinalizedHead") if finalized else None
    header = rpc(url, "chain_getHeader", [block_hash] if block_hash else [])
    return int(header["number"], 16)


def snapshot(relay, orbis):
    return {
        "relay_best": height(relay),
        "relay_finalized": height(relay, True),
        "orbis_best": height(orbis),
        "orbis_finalized": height(orbis, True),
    }


def runtime_api_u32(url, method):
    encoded = rpc(url, "state_call", [method, "0x"])
    data = bytes.fromhex(encoded.removeprefix("0x"))
    if len(data) != 4:
        raise RuntimeError(f"{method} returned {len(data)} bytes, expected SCALE u32")
    return int.from_bytes(data, "little")


def claim_queue_cores(relay, para_id):
    queue = decode_claim_queue(rpc(relay, "state_call", ["ParachainHost_claim_queue", "0x"]))
    return sorted(core for core, paras in queue.items() if para_id in paras)


def wait_for_progress(relay, orbis, before, timeout, poll=3):
    deadline = time.monotonic() + timeout
    while True:
        after = snapshot(relay, orbis)
        if all(after[key] > before[key] for key in before):
            return after
        if time.monotonic() >= deadline:
            raise RuntimeError(
                f"best/finalized progress did not recover within {timeout}s: "
                f"{before} -> {after}"
            )
        time.sleep(poll)


def parse_fault(value):
    try:
        role, rpc_port, pid = value.split(":", 2)
        return role, int(rpc_port), int(pid)
    except ValueError as error:
        raise argparse.ArgumentTypeError("fault must be ROLE:RPC_PORT:PID") from error


def validate_fault_target(role, rpc_port, pid):
    if pid <= 1:
        raise RuntimeError(f"refusing unsafe PID {pid}")
    try:
        listeners = subprocess.check_output(
            [
                "lsof",
                "-nP",
                "-a",
                "-p",
                str(pid),
                f"-iTCP:{rpc_port}",
                "-sTCP:LISTEN",
            ],
            text=True,
            stderr=subprocess.STDOUT,
        )
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise RuntimeError(
            f"PID {pid} is not the listener on RPC port {rpc_port}; refusing fault"
        ) from error
    if str(pid) not in listeners:
        raise RuntimeError(
            f"PID {pid} is not the listener on RPC port {rpc_port}; refusing fault"
        )

    version = rpc(f"http://127.0.0.1:{rpc_port}", "state_getRuntimeVersion")
    expected = "orbis" if role.startswith("orbis-") else "origin"
    if version["specName"] != expected:
        raise RuntimeError(
            f"fault role {role} expected {expected}, RPC {rpc_port} is "
            f"{version['specName']}"
        )


def verify_p0_ratification(repo):
    command = ["npm", "--prefix", "product-sdk", "run", "validate:ratification"]
    completed = subprocess.run(
        command,
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    output = completed.stdout.strip()
    if completed.returncode or "p0_targets_ratified=true" not in output:
        raise RuntimeError(f"P0 ratification validation failed: {output}")
    payload_hash = None
    for item in output.split():
        if item.startswith("payload_sha256="):
            payload_hash = item.split("=", 1)[1]
    if not payload_hash or len(payload_hash) != 64:
        raise RuntimeError(f"P0 validator omitted payload SHA-256: {output}")
    return {
        "command": " ".join(command),
        "status": "PASS",
        "scope": "p0-targets-and-client-contracts-only",
        "payload_sha256": payload_hash,
        "production_activation_ready": False,
        "raw_verdict": output.splitlines()[-1],
    }


def wait_rpc(url, expected_spec, timeout):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            version = rpc(url, "state_getRuntimeVersion")
            if version["specName"] != expected_spec:
                raise RuntimeError(
                    f"restart endpoint is {version['specName']}, expected {expected_spec}"
                )
            return version
        except Exception as error:  # endpoint is expected to be unavailable during restart
            last_error = error
            time.sleep(1)
    raise RuntimeError(f"RPC {url} did not recover before {timeout}s: {last_error}")


def restart_persistent_node(role, rpc_port, pid, relay, orbis, para_id, timeout):
    validate_fault_target(role, rpc_port, pid)
    expected_spec = "orbis" if role.startswith("orbis-") else "origin"
    url = f"http://127.0.0.1:{rpc_port}"
    genesis_before = rpc(url, "chain_getBlockHash", [0])
    claim_queue_before = claim_queue_cores(relay, para_id)
    progress_before = snapshot(relay, orbis)
    command = subprocess.check_output(
        ["ps", "-ww", "-p", str(pid), "-o", "command="], text=True
    ).strip()
    if not command:
        raise RuntimeError(f"cannot recover command for PID {pid}")

    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        listener = subprocess.run(
            ["lsof", "-nP", f"-iTCP:{rpc_port}", "-sTCP:LISTEN"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if listener.returncode != 0:
            break
        time.sleep(0.5)
    else:
        raise RuntimeError(f"PID {pid} did not terminate cleanly")

    restarted = subprocess.Popen(
        shlex.split(command),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    try:
        version = wait_rpc(url, expected_spec, timeout)
        genesis_after = rpc(url, "chain_getBlockHash", [0])
        if genesis_after != genesis_before:
            raise RuntimeError(
                f"persistent restart changed genesis: {genesis_before} -> {genesis_after}"
            )
        progress_after = wait_for_progress(relay, orbis, progress_before, timeout)
        claim_queue_after = claim_queue_cores(relay, para_id)
        if claim_queue_after != claim_queue_before:
            raise RuntimeError(
                "persistent restart changed para claim queue: "
                f"{claim_queue_before} -> {claim_queue_after}"
            )
    except Exception:
        restarted.terminate()
        raise
    return {
        "role": role,
        "rpc_port": rpc_port,
        "old_pid": pid,
        "new_pid": restarted.pid,
        "command": command,
        "genesis_hash": genesis_after,
        "spec_name": version["specName"],
        "spec_version": version["specVersion"],
        "before": progress_before,
        "after_reconnect": progress_after,
        "claim_queue_before": claim_queue_before,
        "claim_queue_after": claim_queue_after,
        "claim_queue_unchanged": True,
        "best_and_finalized_progress_after_reconnect": True,
    }


def observe(args):
    ratification = verify_p0_ratification(args.repo)
    relay_version = rpc(args.relay, "state_getRuntimeVersion")
    orbis_version = rpc(args.orbis, "state_getRuntimeVersion")
    if (relay_version["specName"], relay_version["specVersion"]) != ("origin", 9901):
        raise RuntimeError(f"unexpected Origin runtime: {relay_version}")
    if (
        orbis_version["specName"],
        orbis_version["specVersion"],
        orbis_version["transactionVersion"],
    ) != ("orbis", 29, 8):
        raise RuntimeError(f"unexpected Orbis runtime: {orbis_version}")

    target_rate = runtime_api_u32(args.orbis, "TargetBlockRate_target_block_rate")
    parent_offset = runtime_api_u32(args.orbis, "RelayParentOffsetApi_relay_parent_offset")
    if target_rate != 3 or parent_offset != 1:
        raise RuntimeError(
            f"unexpected authoring contract: rate={target_rate}, offset={parent_offset}"
        )

    assigned = claim_queue_cores(args.relay, args.para_id)
    if len(assigned) < args.expected_cores:
        raise RuntimeError(
            f"para {args.para_id} has {len(assigned)} cores, expected "
            f"{args.expected_cores}: {assigned}"
        )

    before = snapshot(args.relay, args.orbis)
    after = wait_for_progress(args.relay, args.orbis, before, args.progress_timeout)
    report = {
        "schema_version": 1,
        "scope": "p1-control-broker-observation-and-reversible-process-faults",
        "scope_verdict": "PASS",
        "p1_gate_verdict": "BLOCKED",
        "reason": (
            "Live progress, runtime identity, Broker-origin claim queue and reversible "
            "process-loss checks do not prove privileged lifecycle mutations."
        ),
        "relay": {
            "endpoint": args.relay,
            "spec_name": relay_version["specName"],
            "spec_version": relay_version["specVersion"],
            "genesis_hash": rpc(args.relay, "chain_getBlockHash", [0]),
            "health": rpc(args.relay, "system_health"),
        },
        "orbis": {
            "endpoint": args.orbis,
            "spec_name": orbis_version["specName"],
            "spec_version": orbis_version["specVersion"],
            "transaction_version": orbis_version["transactionVersion"],
            "genesis_hash": rpc(args.orbis, "chain_getBlockHash", [0]),
            "health": rpc(args.orbis, "system_health"),
            "target_block_rate": target_rate,
            "relay_parent_offset": parent_offset,
        },
        "para_id": args.para_id,
        "claim_queue_cores": assigned,
        "progress": {"before": before, "after": after},
        "faults": [],
        "persistent_restarts": [],
        "p0_ratification": ratification,
        "unexecuted_privileged_scenarios": list(REQUIRED_PRIVILEGED_SCENARIOS),
    }

    for role, rpc_port, pid in args.fault:
        validate_fault_target(role, rpc_port, pid)
        fault_before = snapshot(args.relay, args.orbis)
        started = time.monotonic()
        os.kill(pid, signal.SIGSTOP)
        try:
            time.sleep(args.fault_hold)
            during = wait_for_progress(
                args.relay, args.orbis, fault_before, args.progress_timeout
            )
        finally:
            os.kill(pid, signal.SIGCONT)
        recovered = wait_for_progress(
            args.relay, args.orbis, during, args.progress_timeout
        )
        report["faults"].append(
            {
                "role": role,
                "rpc_port": rpc_port,
                "pid": pid,
                "signal": "SIGSTOP/SIGCONT",
                "hold_seconds": args.fault_hold,
                "elapsed_seconds": round(time.monotonic() - started, 3),
                "before": fault_before,
                "during_fault": during,
                "after_recovery": recovered,
                "progress_during_fault": True,
                "progress_after_recovery": True,
            }
        )
    for role, rpc_port, pid in args.restart:
        report["persistent_restarts"].append(
            restart_persistent_node(
                role,
                rpc_port,
                pid,
                args.relay,
                args.orbis,
                args.para_id,
                args.progress_timeout,
            )
        )
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--relay", default="http://127.0.0.1:9801")
    parser.add_argument("--orbis", default="http://127.0.0.1:9810")
    parser.add_argument(
        "--repo",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parents[1],
    )
    parser.add_argument("--para-id", type=int, default=1006)
    parser.add_argument("--expected-cores", type=int, default=3)
    parser.add_argument("--progress-timeout", type=int, default=90)
    parser.add_argument("--fault-hold", type=int, default=24)
    parser.add_argument(
        "--fault",
        action="append",
        type=parse_fault,
        default=[],
        metavar="ROLE:RPC_PORT:PID",
        help="reversibly SIGSTOP/SIGCONT a local node while proving both chains progress",
    )
    parser.add_argument(
        "--restart",
        action="append",
        type=parse_fault,
        default=[],
        metavar="ROLE:RPC_PORT:PID",
        help="SIGTERM and restart a local node with the same command and persistent base path",
    )
    args = parser.parse_args()
    print(json.dumps(observe(args), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
