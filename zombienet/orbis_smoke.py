#!/usr/bin/env python3
"""Verify a running native Origin/Orbis topology and its core assignments."""

import argparse
import json
import time
import urllib.request


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
    first = data[offset]
    mode = first & 0b11
    if mode == 0:
        return first >> 2, offset + 1
    if mode == 1:
        return int.from_bytes(data[offset : offset + 2], "little") >> 2, offset + 2
    if mode == 2:
        return int.from_bytes(data[offset : offset + 4], "little") >> 2, offset + 4
    length = (first >> 2) + 4
    start = offset + 1
    return int.from_bytes(data[start : start + length], "little"), start + length


def decode_claim_queue(encoded):
    data = bytes.fromhex(encoded.removeprefix("0x"))
    entries, offset = compact(data, 0)
    queue = {}
    for _ in range(entries):
        core = int.from_bytes(data[offset : offset + 4], "little")
        offset += 4
        count, offset = compact(data, offset)
        queue[core] = []
        for _ in range(count):
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--relay", default="http://127.0.0.1:9900")
    parser.add_argument("--orbis", default="http://127.0.0.1:9910")
    parser.add_argument("--para-id", type=int, default=1006)
    parser.add_argument("--expected-cores", type=int, default=3)
    parser.add_argument("--expected-block-rate", type=int, default=3)
    parser.add_argument("--wait", type=int, default=30)
    parser.add_argument("--startup-timeout", type=int, default=180)
    args = parser.parse_args()

    relay_version = rpc(args.relay, "state_getRuntimeVersion")
    orbis_version = rpc(args.orbis, "state_getRuntimeVersion")
    if relay_version["specName"] != "origin" or orbis_version["specName"] != "orbis":
        raise RuntimeError(
            f"unexpected runtimes: {relay_version['specName']}/{orbis_version['specName']}"
        )

    target_block_rate = runtime_api_u32(
        args.orbis, "TargetBlockRate_target_block_rate"
    )
    relay_parent_offset = runtime_api_u32(
        args.orbis, "RelayParentOffsetApi_relay_parent_offset"
    )
    if target_block_rate != args.expected_block_rate:
        raise RuntimeError(
            f"Orbis target block rate is {target_block_rate}, "
            f"expected {args.expected_block_rate}"
        )
    if relay_parent_offset != 1:
        raise RuntimeError(
            f"Orbis relay-parent offset is {relay_parent_offset}, expected 1"
        )

    deadline = time.monotonic() + args.startup_timeout
    before = snapshot(args.relay, args.orbis)
    while before["orbis_best"] == 0 or before["orbis_finalized"] == 0:
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Orbis did not start and finalize before timeout: {before}")
        time.sleep(6)
        before = snapshot(args.relay, args.orbis)
    time.sleep(args.wait)
    after = snapshot(args.relay, args.orbis)
    for chain in ("relay", "orbis"):
        if after[f"{chain}_best"] <= before[f"{chain}_best"]:
            raise RuntimeError(f"{chain} best block did not advance: {before} -> {after}")
        if after[f"{chain}_finalized"] <= before[f"{chain}_finalized"]:
            raise RuntimeError(f"{chain} finality did not advance: {before} -> {after}")

    encoded = rpc(args.relay, "state_call", ["ParachainHost_claim_queue", "0x"])
    queue = decode_claim_queue(encoded)
    assigned = sorted(core for core, paras in queue.items() if args.para_id in paras)
    if len(assigned) < args.expected_cores:
        raise RuntimeError(
            f"para {args.para_id} has {len(assigned)} claim-queue cores, "
            f"expected at least {args.expected_cores}: {queue}"
        )

    print(
        json.dumps(
            {
                "status": "ok",
                "before": before,
                "after": after,
                "para_id": args.para_id,
                "claim_queue_cores": assigned,
                "target_block_rate": target_block_rate,
                "relay_parent_offset": relay_parent_offset,
                "relay_spec_version": relay_version["specVersion"],
                "orbis_spec_version": orbis_version["specVersion"],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
