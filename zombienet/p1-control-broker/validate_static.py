#!/usr/bin/env python3
"""Offline structural gate for the P1 manifest and live-driver evidence contract."""

import json
import re
import struct
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CAMPAIGN = ROOT / "origin-rs" / "src" / "p1_campaign.rs"
DRIVER = ROOT / "origin-rs" / "examples" / "p1_live_driver.rs"

PHASES = {
    "control": {
        "AC7-VALIDATOR-REMOVE", "AC7-VALIDATOR-ADMIT", "AC7-COLLATOR-REMOVE",
        "AC7-COLLATOR-ADMIT", "AC7-KEY-ROTATION", "AC7-ORIGIN-UPGRADE",
        "AC7-ORBIS-UPGRADE", "AC7-TX-PAUSE-RECOVERY", "AC7-SAFE-MODE-RECOVERY",
        "AC7-COMPROMISE-RECOVERY",
    },
    "broker-pre-restart": {
        "AC8-BOOTSTRAP", "AC8-REQUEST", "AC8-RESERVE", "AC8-ASSIGN", "AC8-RENEW",
        "AC8-RESIZE-DOWN", "AC8-RESIZE-UP", "AC8-DELAYED", "AC8-DUPLICATE",
        "AC8-OUT-OF-ORDER", "AC8-RECEIPT-REORDER", "AC8-SESSION", "AC8-RELEASE",
    },
    "broker-post-restart": {"AC8-FULL-RESTART", "AC8-RESTART-RECOVERY"},
}

EXTENSIONS = [
    "AuthorizeCall", "CheckNonZeroSender", "CheckSpecVersion", "CheckTxVersion",
    "CheckGenesis", "CheckMortality", "CheckNonce", "CheckWeight",
    "ChargeTransactionPayment", "ValidateStorageCalls", "CheckMetadataHash",
    "ReviveSetOrigin", "WeightReclaim",
]


def quoted(section: str) -> list[str]:
    return re.findall(r'"([A-Za-z0-9_-]+)"', section)


def rust_array(source: str, name: str) -> list[str]:
    match = re.search(rf"const {name}:.*?= &\[(.*?)\];", source, re.S)
    if not match:
        raise AssertionError(f"missing Rust array {name}")
    return quoted(match.group(1))


def main() -> None:
    manifest = json.loads((HERE / "scenarios.json").read_text())
    observed = {
        phase: {case["id"] for case in cases}
        for phase, cases in manifest["phases"].items()
    }
    assert manifest["schema"] == "cord.p1-control-broker-scenarios.v1"
    assert manifest["para_id"] == 1006
    assert observed == PHASES
    all_cases = set().union(*observed.values())
    assert len(all_cases) == 25
    assert sum(map(len, observed.values())) == 25

    campaign = CAMPAIGN.read_text()
    rust_cases = {
        "control": set(rust_array(campaign, "CONTROL_CASES")),
        "broker-pre-restart": set(rust_array(campaign, "BROKER_PRE_RESTART_CASES")),
        "broker-post-restart": set(rust_array(campaign, "BROKER_POST_RESTART_CASES")),
    }
    assert rust_cases == PHASES
    assert "vec![221, 1]" in campaign and "vec![221, 2]" in campaign
    assert "8, 7, 6, 5, 4, 3, 2, 1, 10, 9" in campaign
    request_vector = bytes([221, 1]) + struct.pack("<QH", 0x0102030405060708, 0x090A)
    receipt_vector = bytes([221, 2]) + struct.pack("<QHB", 0x0102030405060708, 0x090A, 3)
    assert request_vector == bytes([221, 1, 8, 7, 6, 5, 4, 3, 2, 1, 10, 9])
    assert receipt_vector == bytes([221, 2, 8, 7, 6, 5, 4, 3, 2, 1, 10, 9, 3])

    driver = DRIVER.read_text()
    assert set(re.findall(r'"(AC[78]-[A-Z-]+)"', driver)) == all_cases
    assert rust_array(driver, "EXTENSIONS") == EXTENSIONS
    assert "control.index() != 221" in driver
    assert "transaction_extensions_by_version" in driver
    assert "candidate Wasm equals live :code" in driver
    assert "fast-runtime session finalized within five minutes" in driver
    assert "full-restart.json" in driver
    assert ".unwrap_or_default();" not in driver
    assert driver.count("CaseRecord::capability_gap(") == 1  # fail-safe missing-record fallback
    assert "aura_slot_from_header" in driver and 'header["parentHash"]' in driver
    assert '"set_transport_hold"' in driver and '"release_held"' in driver

    runner = (HERE / "run.py").read_text()
    assert "export-genesis-" + "state" not in runner
    assert '"export-state"' in runner and '"chain-info"' in runner
    assert '"export-genesis-head"' in runner
    assert "render_topology(" in runner and "cwd=specs" in runner
    assert "origin_worker_binaries(origin)" in runner
    assert '"origin_prepare_worker_sha256"' in runner
    assert '"origin_execute_worker_sha256"' in runner
    assert "write_orbis_command_wrapper" in runner
    assert '"orbis_command_wrapper_sha256"' in runner
    assert "validate_launch_commands" in runner
    assert '"launch_command_ledger_sha256"' in runner
    # Native Zombienet owns identity, paths, ports, RPC exposure, telemetry and
    # validator/collator role flags. Repeating any of them can either make clap
    # reject the process or silently defeat the hash-bound topology ledger.
    topology = (HERE / "topology.toml.in").read_text()
    outer_args = "\n".join(
        line for line in topology.splitlines() if line.startswith("args = ")
    )
    for forbidden in (
        "--base-path", "--chain", "--collator", "--insecure-validator-i-know-what-i-do",
        "--listen-addr", "--name", "--no-mdns", "--no-telemetry", "--node-key",
        "--parachain-id", "--port", "--prometheus-external", "--prometheus-port",
        "--rpc-cors", "--rpc-methods", "--rpc-port", "--unsafe-rpc-external", "--validator",
        "--ws-port",
    ):
        assert forbidden not in outer_args
    internal_args = [
        json.loads(line.removeprefix("relay_chain_args = "))
        for line in topology.splitlines()
        if line.startswith("relay_chain_args = ")
    ]
    assert internal_args == [
        ["--node-key", "09" * 32, "--no-mdns"],
        ["--node-key", "0a" * 32, "--no-mdns"],
    ]
    assert "PROVIDER_NODE_KEYS + INTERNAL_RELAY_NODE_KEYS" in runner

    pallet = (ROOT / "origin" / "pallets" / "coretime-control" / "src" / "lib.rs").read_text()
    origin_runtime = (ROOT / "origin" / "base" / "runtime" / "src" / "lib.rs").read_text()
    orbis_runtime = (ROOT / "origin" / "orbis" / "runtime" / "src" / "coretime.rs").read_text()
    assert "RequestHeld" in pallet and "HeldRequestReleased" in pallet
    assert "type TransportControlEnabled = frame_support::traits::ConstBool<false>;" in origin_runtime
    assert 'cfg!(feature = "fast-runtime")' in orbis_runtime
    print(
        "P1 static gate: pass (25 cases, extension/SCALE pins, Aura binding, fast-only hold gate)"
    )


if __name__ == "__main__":
    main()
