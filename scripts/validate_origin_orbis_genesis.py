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

"""Validate deterministic clean-break candidates and the fail-closed production launch gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs/genesis/origin-orbis-clean-genesis-manifest.json"
RUNBOOK = ROOT / "docs/runbooks/origin-orbis-clean-genesis-launch.md"
EVIDENCE = ROOT / "docs/evidence/p5"
IDENTITY = ROOT / "docs/genesis/orbis-candidate-genesis-identity.json"
LAUNCH_APPROVAL = ROOT / "docs/genesis/origin-orbis-production-launch-approval.json"
METADATA_HASH = ROOT / "origin/orbis/runtime/vectors/transaction-policy-v8/metadata-hash.json"
COMMAND_LOG: list[dict] = []

MASK64 = (1 << 64) - 1
XX_P1 = 11400714785074694791
XX_P2 = 14029467366897019727
XX_P3 = 1609587929392839161
XX_P4 = 9650029242287828579
XX_P5 = 2870177450012600261


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(data: object) -> bytes:
    return (json.dumps(data, sort_keys=True, separators=(",", ":")) + "\n").encode()


def read_json(path: Path) -> object:
    return json.loads(path.read_text())


def rotate_left(value: int, count: int) -> int:
    return ((value << count) | (value >> (64 - count))) & MASK64


def xx_round(accumulator: int, value: int) -> int:
    accumulator = (accumulator + value * XX_P2) & MASK64
    return (rotate_left(accumulator, 31) * XX_P1) & MASK64


def xxhash64(data: bytes, seed: int) -> int:
    """Dependency-free XXH64 used by FRAME's Twox128 storage prefixes."""
    length, cursor = len(data), 0
    if length >= 32:
        vectors = [
            (seed + XX_P1 + XX_P2) & MASK64,
            (seed + XX_P2) & MASK64,
            seed & MASK64,
            (seed - XX_P1) & MASK64,
        ]
        while cursor <= length - 32:
            for index in range(4):
                value = int.from_bytes(data[cursor + index * 8:cursor + index * 8 + 8], "little")
                vectors[index] = xx_round(vectors[index], value)
            cursor += 32
        result = sum(rotate_left(value, count) for value, count in zip(vectors, (1, 7, 12, 18))) & MASK64
        for value in vectors:
            result ^= xx_round(0, value)
            result = (result * XX_P1 + XX_P4) & MASK64
    else:
        result = (seed + XX_P5) & MASK64
    result = (result + length) & MASK64
    while cursor <= length - 8:
        result ^= xx_round(0, int.from_bytes(data[cursor:cursor + 8], "little"))
        result = (rotate_left(result, 27) * XX_P1 + XX_P4) & MASK64
        cursor += 8
    if cursor <= length - 4:
        result ^= (int.from_bytes(data[cursor:cursor + 4], "little") * XX_P1) & MASK64
        result = (rotate_left(result, 23) * XX_P2 + XX_P3) & MASK64
        cursor += 4
    while cursor < length:
        result ^= (data[cursor] * XX_P5) & MASK64
        result = (rotate_left(result, 11) * XX_P1) & MASK64
        cursor += 1
    result ^= result >> 33
    result = (result * XX_P2) & MASK64
    result ^= result >> 29
    result = (result * XX_P3) & MASK64
    return (result ^ (result >> 32)) & MASK64


def twox128(value: str) -> bytes:
    data = value.encode()
    return xxhash64(data, 0).to_bytes(8, "little") + xxhash64(data, 1).to_bytes(8, "little")


def exact_hex(value: object, length: int, field: str) -> None:
    assert isinstance(value, str) and value.startswith("0x"), f"{field}: missing 0x prefix"
    raw = value[2:]
    assert len(raw) == length * 2 and raw == raw.lower(), f"{field}: wrong hex length/case"
    bytes.fromhex(raw)


def validate_inputs(manifest: dict) -> tuple[dict, dict, dict]:
    origin = read_json(ROOT / manifest["inputs"]["origin"])
    orbis = read_json(ROOT / manifest["inputs"]["orbis"])
    assert isinstance(origin, dict) and set(origin) == {"root_key", "validators", "endowed_accounts"}
    assert isinstance(orbis, dict) and set(orbis) == {
        "relay_chain", "token_network_id", "root_key", "collators", "endowed_accounts",
        "feeless_accounts",
    }

    exact_hex(origin["root_key"], 32, "origin.root_key")
    assert len(origin["validators"]) >= manifest["authority_policy"]["origin_minimum_validators"]
    origin_accounts = []
    origin_session = []
    for index, validator in enumerate(origin["validators"]):
        assert set(validator) == {
            "account_id", "babe", "grandpa", "para_validator", "para_assignment",
            "authority_discovery", "beefy",
        }
        exact_hex(validator["account_id"], 32, f"origin.validators[{index}].account_id")
        origin_accounts.append(validator["account_id"])
        for field in ("babe", "grandpa", "para_validator", "para_assignment", "authority_discovery"):
            exact_hex(validator[field], 32, f"origin.validators[{index}].{field}")
            origin_session.append(validator[field])
        exact_hex(validator["beefy"], 33, f"origin.validators[{index}].beefy")
        assert validator["beefy"][2:4] in ("02", "03")
    assert len(set(origin_accounts)) == len(origin_accounts)
    assert len(set(origin_session)) == len(origin_session)
    assert origin["root_key"] not in origin_accounts
    assert len(set(origin["endowed_accounts"])) == len(origin["endowed_accounts"])
    assert {origin["root_key"], *origin_accounts} <= set(origin["endowed_accounts"])

    assert orbis["relay_chain"] == "origin"
    assert orbis["token_network_id"] == manifest["authority_policy"]["orbis_fixed_token_network_id"]
    exact_hex(orbis["root_key"], 32, "orbis.root_key")
    assert len(orbis["collators"]) >= manifest["authority_policy"]["orbis_minimum_collators"]
    collator_accounts, aura_keys = [], []
    for index, collator in enumerate(orbis["collators"]):
        assert set(collator) == {"account_id", "aura_id"}
        exact_hex(collator["account_id"], 32, f"orbis.collators[{index}].account_id")
        exact_hex(collator["aura_id"], 32, f"orbis.collators[{index}].aura_id")
        collator_accounts.append(collator["account_id"])
        aura_keys.append(collator["aura_id"])
    assert len(set(collator_accounts)) == len(collator_accounts)
    assert len(set(aura_keys)) == len(aura_keys)
    assert orbis["root_key"] not in collator_accounts
    assert len(set(orbis["endowed_accounts"])) == len(orbis["endowed_accounts"])
    assert {orbis["root_key"], *collator_accounts, *orbis["feeless_accounts"]} <= set(orbis["endowed_accounts"])
    assert orbis["feeless_accounts"] == manifest["bootstrap_state"]["orbis_feeless_accounts"]

    origin_authority_plane = {
        origin["root_key"], *origin_accounts, *origin_session,
        *(validator["beefy"] for validator in origin["validators"]),
    }
    orbis_authority_plane = {orbis["root_key"], *collator_accounts, *aura_keys}
    assert origin_authority_plane.isdisjoint(orbis_authority_plane), (
        "ADR 0009 requires distinct Origin and Orbis governance/block-authority keys"
    )

    identities = {
        "origin_input_sha256": sha256(canonical(origin)),
        "orbis_input_sha256": sha256(canonical(orbis)),
        "authority_counts": {"origin_validators": len(origin_accounts), "orbis_collators": len(collator_accounts)},
    }
    return origin, orbis, identities


def run(command: list[str]) -> bytes:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env={**os.environ, "NO_COLOR": "true"},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    COMMAND_LOG.append({
        "command": command,
        "exit_code": completed.returncode,
        "stdout_sha256": sha256(completed.stdout),
        "stderr": completed.stderr.decode(errors="replace"),
    })
    if completed.returncode:
        raise RuntimeError(f"command failed ({completed.returncode}): {' '.join(command)}\n{completed.stderr.decode(errors='replace')}")
    return completed.stdout


def run_rejected(command: list[str], marker: str | tuple[str, ...]) -> None:
    markers = (marker,) if isinstance(marker, str) else marker
    completed = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    COMMAND_LOG.append({
        "command": command,
        "exit_code": completed.returncode,
        "stdout_sha256": sha256(completed.stdout),
        "stderr": completed.stderr.decode(errors="replace"),
        "expected_rejection": markers,
    })
    output = (completed.stdout + completed.stderr).decode(errors="replace")
    assert completed.returncode != 0 and any(value in output for value in markers), (
        f"command did not fail closed with one of {markers!r}: {' '.join(command)}\n{output}"
    )


def runtime_pallet_names() -> list[str]:
    return [
        "System", "ParachainSystem", "Timestamp", "ParachainInfo", "WeightReclaim",
        "Balances", "TransactionPayment", "Indices", "SkipFeelessPayment", "Authorship",
        "CollatorSelection", "Session", "Aura", "AuraExt", "Scheduler", "XcmpQueue",
        "PolkadotXcm", "CumulusXcm", "MessageQueue", "Utility", "Multisig", "Proxy",
        "Broker", "Token", "Register", "Entity", "Feeless", "Assets", "AssetsFreezer",
        "AssetsHolder", "ForeignAssets", "PoolAssets", "ForeignAssetsFreezer",
        "PoolAssetsFreezer", "Uniques", "Nfts", "AssetRate", "People", "ChunksManager",
        "Members", "MembersNotifier", "PeopleLite", "Personhood", "Resources", "Score",
        "Honour", "Attestation", "Revive", "Names",
        "StorageProvider", "Drive", "S3", "AssetConversion", "AssetTxPayment", "MetaTx",
        "TxPause", "SafeMode", "VerifySignature", "CoretimeControl", "MultiBlockMigrations",
        "Sudo",
    ]


def storage_key(pallet: str, item: str) -> str:
    return "0x" + (twox128(pallet) + twox128(item)).hex()


def decode_compact_vec_32(encoded: bytes, field: str) -> list[str]:
    assert encoded and encoded[0] & 0b11 == 0, f"{field} must use one-byte compact length"
    count = encoded[0] >> 2
    assert len(encoded) == 1 + count * 32, f"{field} has an unexpected SCALE length"
    return ["0x" + encoded[1 + index * 32:1 + (index + 1) * 32].hex() for index in range(count)]


def inspect_bootstrap_state(top: dict, inventory: dict[str, list[str]], manifest: dict) -> dict:
    orbis_input = read_json(ROOT / manifest["inputs"]["orbis"])

    def value(pallet: str, item: str) -> bytes:
        return bytes.fromhex(top[storage_key(pallet, item)].removeprefix("0x"))

    collators = [item["account_id"] for item in orbis_input["collators"]]
    decoded = {
        "parachain_id": int.from_bytes(value("ParachainInfo", "ParachainId"), "little"),
        "token_network_id": int.from_bytes(value("Token", "GenesisNetworkId"), "little"),
        "sudo_root": "0x" + value("Sudo", "Key").hex(),
        "collator_invulnerables": decode_compact_vec_32(
            value("CollatorSelection", "Invulnerables"), "CollatorSelection.Invulnerables"
        ),
        "session_validators": decode_compact_vec_32(
            value("Session", "Validators"), "Session.Validators"
        ),
        "names_registrars": decode_compact_vec_32(value("Names", "Registrars"), "Names.Registrars"),
        "feeless_non_default_keys": [
            key for key in inventory.get("Feeless", [])
            if bytes.fromhex(key.removeprefix("0x"))[16:32] != twox128(":__STORAGE_VERSION__:")
        ],
    }
    reservation_prefix = twox128("Names") + twox128("BootstrapReservations")
    reservations = []
    for encoded_key in inventory["Names"]:
        key = bytes.fromhex(encoded_key.removeprefix("0x"))
        if not key.startswith(reservation_prefix):
            continue
        encoded_label = key[48:]
        assert encoded_label and encoded_label[0] & 0b11 == 0
        label_length = encoded_label[0] >> 2
        assert len(encoded_label) == label_length + 1
        reservations.append(encoded_label[1:].decode("ascii"))
    decoded["names_root_reservations"] = sorted(reservations)

    assert decoded["parachain_id"] == manifest["authority_policy"]["orbis_fixed_para_id"]
    assert decoded["token_network_id"] == manifest["authority_policy"]["orbis_fixed_token_network_id"]
    assert decoded["sudo_root"] == orbis_input["root_key"]
    assert decoded["collator_invulnerables"] == collators
    assert decoded["session_validators"] == collators
    assert decoded["names_registrars"] == [orbis_input["root_key"]]
    assert decoded["names_root_reservations"] == sorted(
        manifest["bootstrap_state"]["orbis_names_root_reservations"]
    )
    assert not decoded["feeless_non_default_keys"]
    return decoded


def inspect_raw_storage(raw: dict, manifest: dict) -> dict:
    top = raw["genesis"]["raw"]["top"]
    prefix_names = {twox128(name): name for name in runtime_pallet_names()}
    inventory: dict[str, list[str]] = {}
    unknown = []
    unapproved_items = []
    decoded_items: dict[str, dict[str, int]] = {}
    storage_version_suffix = twox128(":__STORAGE_VERSION__:")
    approved_items = manifest["bootstrap_state"]["approved_non_default_storage_items"]
    approved_item_hashes = {
        pallet: {twox128(item): item for item in items}
        for pallet, items in approved_items.items()
    }
    approved_well_known = {
        value.encode() for value in manifest["bootstrap_state"]["approved_well_known_storage_keys"]
    }
    for encoded_key in top:
        key = bytes.fromhex(encoded_key.removeprefix("0x"))
        if len(key) >= 16 and key[:16] in prefix_names:
            name = prefix_names[key[:16]]
            item_prefix = key[16:32]
            if item_prefix == storage_version_suffix:
                item = ":__STORAGE_VERSION__:"
            else:
                item = approved_item_hashes.get(name, {}).get(item_prefix)
                if item is None:
                    unapproved_items.append({
                        "pallet": name,
                        "storage_key": encoded_key,
                        "item_prefix": item_prefix.hex(),
                    })
                    item = "unapproved"
        elif key.startswith(b":"):
            name = "well-known"
            item = key.decode(errors="replace")
            if key not in approved_well_known:
                unapproved_items.append({"pallet": name, "storage_key": encoded_key, "decoded": item})
        else:
            unknown.append(encoded_key)
            name = "unknown"
            item = "unknown"
        inventory.setdefault(name, []).append(encoded_key)
        decoded_items.setdefault(name, {})[item] = decoded_items.setdefault(name, {}).get(item, 0) + 1
    assert not unknown, f"unallowlisted raw storage prefixes: {unknown}"
    assert not unapproved_items, f"unapproved raw storage items: {unapproved_items}"
    assert "0x3a636f6465" in top, "raw storage omits :code"

    domain_checks = {}
    for pallet in manifest["bootstrap_state"]["default_empty_pallet_prefixes"]:
        allowed_keys = {"0x" + (twox128(pallet) + storage_version_suffix).hex()}
        decoded_configuration = {}
        for item, policy in manifest["bootstrap_state"]["default_configuration_storage"].get(pallet, {}).items():
            key = storage_key(pallet, item)
            allowed_keys.add(key)
            assert key in top, f"{pallet}.{item} default configuration is absent"
            encoded = bytes.fromhex(top[key].removeprefix("0x"))
            widths = {"u32": 4, "u64": 8, "u128": 16}
            width = widths[policy["scale_type"]]
            assert len(encoded) == width, f"{pallet}.{item} has unexpected SCALE width"
            decoded = int.from_bytes(encoded, "little")
            assert decoded == policy["value"], f"{pallet}.{item}={decoded}, expected {policy['value']}"
            decoded_configuration[item] = decoded
        absent_items = {}
        for item in manifest["bootstrap_state"]["must_remain_absent_storage"].get(pallet, []):
            key = storage_key(pallet, item)
            absent_items[item] = key not in top
            assert key not in top, f"{pallet}.{item} must be absent at clean genesis"
        keys = inventory.get(pallet, [])
        unexpected = [key for key in keys if key not in allowed_keys]
        domain_checks[pallet] = {
            "keys": keys,
            "decoded_default_configuration": decoded_configuration,
            "declared_absent_storage": absent_items,
            "unexpected_domain_records": unexpected,
        }
        assert not unexpected, f"{pallet} contains non-default genesis records: {unexpected}"
    bootstrap_state = inspect_bootstrap_state(top, inventory, manifest)
    return {
        "top_key_count": len(top),
        "decoded_prefix_counts": {name: len(keys) for name, keys in sorted(inventory.items())},
        "decoded_storage_item_counts": decoded_items,
        "unknown_prefixes": unknown,
        "unapproved_storage_items": unapproved_items,
        "empty_domain_prefix_checks": domain_checks,
        "decoded_bootstrap_state": bootstrap_state,
    }


def source_and_dependency_scan() -> dict:
    sources = [
        ROOT / "origin/base/cli/src/chain_spec.rs",
        ROOT / "origin/base/runtime/src/genesis_config_presets.rs",
        ROOT / "origin/orbis/node/src/chain_spec.rs",
        ROOT / "origin/orbis/runtime/src/genesis_config_presets.rs",
    ]
    forbidden_identifiers = (
        "legacy_checkpoint", "legacy_import", "legacy_export", "old_network_dependency",
        "legacy_client", "state_transform_input", "predecessor_chain",
    )
    source_findings = []
    for path in sources:
        content = path.read_text().lower()
        for token in forbidden_identifiers:
            if token in content:
                source_findings.append({"path": str(path.relative_to(ROOT)), "token": token})
    metadata = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version", "1"]))
    scoped = {"origin", "origin-node-cli", "origin-foundation-runtime", "origin-omni-node", "origin-commons-runtime"}
    dependency_findings = []
    for package in metadata["packages"]:
        if package["name"] not in scoped:
            continue
        for dependency in package["dependencies"]:
            normalized = dependency["name"].lower().replace("_", "-")
            if any(token in normalized for token in ("legacy-client", "checkpoint-import", "old-network")):
                dependency_findings.append({"package": package["name"], "dependency": normalized})
    assert not source_findings and not dependency_findings
    return {
        "source_files": {str(path.relative_to(ROOT)): sha256(path.read_bytes()) for path in sources},
        "forbidden_identifier_findings": source_findings,
        "old_network_dependency_findings": dependency_findings,
    }


def command_version(command: list[str]) -> str:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        env={**os.environ, "NO_COLOR": "true"},
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def repository_path(path: Path) -> str:
    return str(path.resolve().relative_to(ROOT))


def build_evidence(
    origin_node: Path,
    orbis_node: Path,
    compact_wasm: Path,
    metadata_hash_path: Path,
    subwasm: str,
    manifest: dict,
) -> tuple[dict, dict]:
    origin_chain = f"origin-candidate:{manifest['inputs']['origin']}"
    orbis_chain = f"orbis-candidate:{manifest['inputs']['orbis']}"
    base = ["build-spec", "--raw", "--disable-default-bootnode", "--chain"]
    origin_a = run([str(origin_node), *base, origin_chain])
    origin_b = run([str(origin_node), *base, origin_chain])
    orbis_a = run([str(orbis_node), *base, orbis_chain])
    orbis_b = run([str(orbis_node), *base, orbis_chain])
    assert origin_a == origin_b, "Origin raw chain spec is non-deterministic"
    assert orbis_a == orbis_b, "Orbis raw chain spec is non-deterministic"
    origin_raw, orbis_raw = json.loads(origin_a), json.loads(orbis_a)
    assert origin_raw["id"] == "origin-candidate" and origin_raw["chainType"] == "Local"
    assert orbis_raw["id"] == "orbis-candidate" and orbis_raw["chainType"] == "Local"

    # Candidate inputs and raw JSON can never bypass the production approval ceremony.
    run_rejected(
        [str(origin_node), *base, f"origin-production:{manifest['inputs']['origin']}"],
        (
            "activation_state is not production-approved",
            "production launch payload SHA-256 does not match the embedded release artifact",
        ),
    )
    run_rejected(
        [str(orbis_node), *base, f"orbis-production:{manifest['inputs']['orbis']}"],
        (
            "activation_state is not production-approved",
            "production launch payload SHA-256 does not match the embedded release artifact",
        ),
    )
    with tempfile.TemporaryDirectory() as rejection_dir:
        origin_live = Path(rejection_dir) / "origin-live.json"
        orbis_live = Path(rejection_dir) / "orbis-live.json"
        origin_live.write_text(json.dumps({**origin_raw, "chainType": "Live"}))
        orbis_live.write_text(json.dumps({**orbis_raw, "chainType": "Live"}))
        run_rejected([str(origin_node), *base, str(origin_live)], "refusing Live Origin chain spec")
        run_rejected([str(orbis_node), *base, str(orbis_live)], "refusing Live Orbis chain spec")

    with tempfile.TemporaryDirectory() as temporary:
        head_path = Path(temporary) / "orbis-head"
        run([str(orbis_node), "export-genesis-head", "--chain", orbis_chain, str(head_path)])
        encoded = bytes.fromhex(head_path.read_text().strip().removeprefix("0x"))
    # Header<BlockNumber=u32>: parent hash (32), compact zero (1), state root (32).
    assert len(encoded) >= 65 and encoded[32] == 0, "unexpected Orbis genesis header encoding"
    orbis_state_root = "0x" + encoded[33:65].hex()

    orbis_header_hash = "0x" + hashlib.blake2b(encoded, digest_size=32).hexdigest()
    orbis_top = orbis_raw["genesis"]["raw"]["top"]
    embedded_code_blob = bytes.fromhex(orbis_top["0x3a636f6465"].removeprefix("0x"))
    metadata_hash = read_json(metadata_hash_path)
    compact_wasm_bytes = compact_wasm.read_bytes()
    compact_wasm_sha256 = sha256(compact_wasm_bytes)
    assert metadata_hash["compact_wasm_sha256"] == compact_wasm_sha256, (
        "metadata hash fixture and compact Wasm do not describe the same runtime"
    )
    runtime_info = json.loads(run([subwasm, "info", "--json", str(compact_wasm)]))
    assert runtime_info["compression"]["compressed"] is False, (
        "the supplied on-chain-release compact Wasm must be uncompressed"
    )
    assert runtime_info["metadata_version"] == 14, "Commons runtime metadata must be V14"
    core_version = runtime_info["core_version"]
    assert {
        "specName": core_version["specName"],
        "specVersion": core_version["specVersion"],
        "transactionVersion": core_version["transactionVersion"],
    } == {
        "specName": metadata_hash["runtime"],
        "specVersion": metadata_hash["spec_version"],
        "transactionVersion": metadata_hash["transaction_version"],
    }, "embedded Commons Core_version and RFC-78 manifest identity differ"
    exact_hex(metadata_hash["metadata_hash"], 32, "metadata_hash.metadata_hash")
    with tempfile.TemporaryDirectory() as runtime_dir:
        embedded_path = Path(runtime_dir) / "embedded-code.blob"
        decompressed_path = Path(runtime_dir) / "embedded-code.wasm"
        embedded_path.write_bytes(embedded_code_blob)
        run([subwasm, "decompress", str(embedded_path), str(decompressed_path)])
        embedded_code = decompressed_path.read_bytes()
    assert embedded_code == compact_wasm_bytes, (
        "Orbis chain-spec :code does not decompress to the supplied on-chain-release compact Wasm; "
        "build origin-omni-node with --features on-chain-release-build"
    )
    raw_storage = inspect_raw_storage(orbis_raw, manifest)
    scan = source_and_dependency_scan()
    revision = command_version(["git", "rev-parse", "HEAD"])
    branch = command_version(["git", "branch", "--show-current"])

    subwasm_name = Path(subwasm).name
    identity = {
        "schema_version": 1,
        "scope": "orbis-deterministic-candidate-genesis",
        "candidate_only": True,
        "production_activation": False,
        "approval_status": manifest["approval"]["status"],
        "activation_state": "candidate-pending",
        "production_activation_ready": False,
        "chain": {"id": "orbis-candidate", "relay_chain": "origin-candidate", "para_id": 1006},
        "genesis": {
            "header_hash": orbis_header_hash,
            "state_root": orbis_state_root,
            "raw_chain_spec_sha256": sha256(orbis_a),
            "raw_storage_sha256": sha256(canonical(orbis_raw["genesis"]["raw"])),
        },
        "derivation": {
            "command": (
                "python3 scripts/validate_origin_orbis_genesis.py "
                f"--origin-node {repository_path(origin_node)} "
                f"--orbis-node {repository_path(orbis_node)} "
                f"--compact-wasm {repository_path(compact_wasm)} "
                f"--metadata-hash-manifest {repository_path(metadata_hash_path)} "
                f"--subwasm {subwasm_name} "
                "--write-evidence"
            ),
            "header_encoding": "SCALE sp_runtime::generic::Header<BlockNumber=u32, BlakeTwo256>",
            "header_hash_algorithm": "Blake2b-256 over exact SCALE genesis header bytes",
            "state_root_offset": "bytes[33:65] after 32-byte parent hash and compact-encoded block zero",
            "deterministic_build_count": 2,
        },
        "runtime": {
            "node_binary_path": str(orbis_node.relative_to(ROOT)),
            "node_binary_sha256": sha256(orbis_node.read_bytes()),
            "embedded_code_blob_sha256": sha256(embedded_code_blob),
            "embedded_code_blob_bytes": len(embedded_code_blob),
            "embedded_code_sha256": sha256(embedded_code_blob),
            "embedded_code_decompressed_sha256": sha256(embedded_code),
            "embedded_code_decompressed_bytes": len(embedded_code),
            "core_version": {
                "spec_name": core_version["specName"],
                "spec_version": core_version["specVersion"],
                "transaction_version": core_version["transactionVersion"],
            },
            "compact_wasm_path": str(compact_wasm.relative_to(ROOT)),
            "compact_wasm_sha256": compact_wasm_sha256,
            "metadata_hash_manifest": str(metadata_hash_path.relative_to(ROOT)),
            "metadata_hash_manifest_sha256": sha256(metadata_hash_path.read_bytes()),
            "metadata_hash_manifest_canonical_sha256": sha256(canonical(metadata_hash)),
            "metadata_hash": metadata_hash["metadata_hash"],
            "spec_version": metadata_hash["spec_version"],
            "transaction_version": metadata_hash["transaction_version"],
        },
        "source": {
            "git_revision": revision,
            "git_branch": branch,
            "worktree_clean": not bool(command_version(["git", "status", "--short"])),
            "chain_spec_path": "origin/orbis/node/src/chain_spec.rs",
            "chain_spec_sha256": scan["source_files"]["origin/orbis/node/src/chain_spec.rs"],
            "source_file_sha256": scan["source_files"],
        },
        "environment": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "rustc": command_version(["rustc", "--version"]),
            "cargo": command_version(["cargo", "--version"]),
            "subwasm": command_version([subwasm, "--version"]),
            "subwasm_executable": subwasm_name,
            "subwasm_executable_sha256": sha256(Path(subwasm).read_bytes()),
            "build_profile": "release; metadata compact Wasm uses on-chain-release-build",
        },
        "raw_storage_verification": raw_storage,
        "clean_break_scan": scan,
    }

    generated = {
        "origin_raw_chain_spec_sha256": sha256(origin_a),
        "origin_raw_storage_sha256": sha256(canonical(origin_raw["genesis"]["raw"])),
        "orbis_raw_chain_spec_sha256": sha256(orbis_a),
        "orbis_raw_storage_sha256": sha256(canonical(orbis_raw["genesis"]["raw"])),
        "orbis_genesis_state_root": orbis_state_root,
        "orbis_genesis_header_hash": orbis_header_hash,
        "orbis_candidate_identity_path": str(IDENTITY.relative_to(ROOT)),
        "orbis_candidate_identity_sha256": sha256(canonical(identity)),
        "deterministic_rebuilds": 2,
        "raw_storage_checks_pass": not raw_storage["unknown_prefixes"] and all(
            not check["unexpected_domain_records"]
            for check in raw_storage["empty_domain_prefix_checks"].values()
        ) and not raw_storage["unapproved_storage_items"],
        "clean_break_scan_pass": not scan["forbidden_identifier_findings"]
        and not scan["old_network_dependency_findings"],
    }
    return generated, identity


def validate_policy(manifest: dict) -> None:
    clean_break = manifest["clean_break"]
    assert all(value is False for key, value in clean_break.items() if key.startswith("legacy_"))
    assert clean_break["old_network_dependency"] is False
    assert clean_break["pre_launch_recovery"] == "regenerate-chain-spec"
    assert clean_break["post_launch_recovery"] == "storage-versioned-forward-runtime-upgrade"
    assert manifest["bootstrap_state"]["orbis_names_root_reservations"] == ["origin", "orbis", "system"]
    assert manifest["bootstrap_state"]["orbis_permissionless_collator_candidates"] == 0
    assert manifest["authority_policy"]["origin_and_orbis_authority_planes_separated"] is True
    assert manifest["candidate_status"] == "deterministic-fixture-only-not-production-approved"
    assert manifest["activation_state"] == "candidate-pending"
    assert manifest["production_activation"] is False
    assert manifest["production_activation_ready"] is False
    assert manifest["approval"]["status"] == "PENDING"
    launch = read_json(LAUNCH_APPROVAL)
    assert launch["payload"]["activation_state"] == "candidate-pending"
    assert launch["payload"]["production_activation"] is False
    assert launch["payload"]["network_launch_approval"] is False
    assert launch["payload"]["campaign_authorized"] is False
    assert launch["payload"]["final_genesis_status"] == "PENDING"
    assert launch["payload"]["approval_timing"] == {
        "envelope_finalized_at": None,
        "launch_epoch": None,
        "launch_not_after": None,
        "max_signature_age_seconds": 604800,
        "max_launch_window_seconds": 86400,
    }
    assert launch["signatures"] == []
    assert launch["derived_status"]["production_activation_ready"] is False
    runbook = RUNBOOK.read_text()
    for marker in (
        "regenerate\nthe complete chain spec", "storage layout change must increment",
        "There is no old-network rollback", "not production authorities",
    ):
        assert marker in runbook, f"runbook missing required control: {marker}"


def reports(manifest: dict, identities: dict, generated: dict | None) -> dict[str, dict]:
    candidate_verified = bool(
        generated
        and generated["deterministic_rebuilds"] == 2
        and generated["raw_storage_checks_pass"]
        and generated["clean_break_scan_pass"]
    )
    common = {
        "schema_version": 1,
        "verdict": "pass" if candidate_verified else "not-run",
        "pass": candidate_verified,
        "scope": manifest["scope"],
        "candidate_only": True,
        "activation_state": "candidate-pending",
        "production_activation_ready": False,
        "production_activation": False,
        "approval_status": manifest["approval"]["status"],
        "inputs": identities,
        "generated": generated,
    }
    return {
        "genesis-verdict.json": {**common, "acceptance_criteria": "AC21", "native_from_block_zero": candidate_verified, "default_domain_state_verified_from_raw_storage": candidate_verified, "approved_production_state_root": None, "production_approval_claim": False},
        "launch-upgrade.json": {**common, "acceptance_criteria": "AC22", "pre_launch_failure_action": "regenerate-chain-spec", "post_launch_failure_action": "storage-versioned-forward-runtime-upgrade", "old_network_rollback": False},
        "clean-genesis.report.json": {**common, "acceptance_criteria": "M6", "legacy_records_verified_from_domain_prefixes": 0 if candidate_verified else None, "legacy_checkpoint_export_import_scan_findings": 0 if candidate_verified else None, "old_network_dependency_scan_findings": 0 if candidate_verified else None},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--origin-node", type=Path)
    parser.add_argument("--orbis-node", type=Path)
    parser.add_argument("--compact-wasm", type=Path)
    parser.add_argument("--metadata-hash-manifest", type=Path, default=METADATA_HASH)
    parser.add_argument("--subwasm", default="subwasm")
    parser.add_argument("--write-evidence", action="store_true")
    args = parser.parse_args()
    if bool(args.origin_node) != bool(args.orbis_node):
        parser.error("--origin-node and --orbis-node must be supplied together")
    if args.write_evidence and not args.origin_node:
        parser.error("--write-evidence requires both freshly built node binaries")
    if args.origin_node and not args.compact_wasm:
        parser.error("binary validation requires --compact-wasm from the metadata-hash build")
    subwasm = None
    if args.origin_node:
        subwasm = shutil.which(args.subwasm)
        if subwasm is None:
            parser.error(f"--subwasm executable not found: {args.subwasm}")
        subwasm = str(Path(subwasm).resolve())
    manifest = read_json(MANIFEST)
    assert isinstance(manifest, dict)
    validate_policy(manifest)
    _, _, identities = validate_inputs(manifest)
    identity = None
    if args.origin_node:
        generated, identity = build_evidence(
            args.origin_node.resolve(),
            args.orbis_node.resolve(),
            args.compact_wasm.resolve(),
            args.metadata_hash_manifest.resolve(),
            subwasm,
            manifest,
        )
    else:
        generated = None
    artifacts = reports(manifest, identities, generated)
    if args.write_evidence:
        EVIDENCE.mkdir(parents=True, exist_ok=True)
        assert identity is not None
        IDENTITY.write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n")
        identity_sha256 = sha256(IDENTITY.read_bytes())
        for payload in artifacts.values():
            payload["generated"]["orbis_candidate_identity_sha256"] = identity_sha256
        for name, payload in artifacts.items():
            (EVIDENCE / name).write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        raw_log = EVIDENCE / "genesis-validator.raw.log"
        raw_log.write_text("\n".join(json.dumps(entry, sort_keys=True) for entry in COMMAND_LOG) + "\n")
        index_entries = [
            {"acceptance_criteria": name, "path": f"docs/evidence/p5/{path}", "sha256": sha256((EVIDENCE / path).read_bytes())}
            for name, path in (("AC21", "genesis-verdict.json"), ("AC22", "launch-upgrade.json"), ("M6", "clean-genesis.report.json"))
        ]
        index_entries.extend([
            {"acceptance_criteria": "AC21/M6", "path": str(IDENTITY.relative_to(ROOT)), "sha256": identity_sha256},
            {"acceptance_criteria": "AC21/M6", "path": str(raw_log.relative_to(ROOT)), "sha256": sha256(raw_log.read_bytes())},
        ])
        (EVIDENCE / "verification-index.json").write_text(json.dumps({
            "schema_version": 1,
            "scope": manifest["scope"],
            "candidate_only": True,
            "production_activation": False,
            "entries": index_entries,
        }, indent=2, sort_keys=True) + "\n")
    print(json.dumps({
        "verdict": "pass" if generated is not None else "configuration-pass-chain-spec-not-run",
        "generated_chain_specs": generated is not None,
        "artifacts": list(artifacts),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, KeyError, ValueError, RuntimeError) as error:
        print(json.dumps({"verdict": "fail", "error": str(error)}), file=sys.stderr)
        raise SystemExit(1)
