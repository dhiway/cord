#!/usr/bin/env python3
"""Fail closed unless the proof-retention topology is isolated from 98xx."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import urllib.request
import urllib.error


def rpc(port: int, method: str, params: list[object] | None = None) -> object:
	payload = json.dumps(
		{"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}
	).encode()
	request = urllib.request.Request(
		f"http://127.0.0.1:{port}", payload, {"content-type": "application/json"}
	)
	response = json.load(urllib.request.urlopen(request, timeout=5))
	if "error" in response:
		raise RuntimeError(f"{method} on {port}: {response['error']}")
	return response["result"]


def network(ports: list[int]) -> dict[str, object]:
	nodes = []
	for port in ports:
		nodes.append(
			{
				"port": port,
				"chain": rpc(port, "system_chain"),
				"genesis_hash": rpc(port, "chain_getBlockHash", [0]),
				"peer_id": rpc(port, "system_localPeerId"),
				"peers": rpc(port, "system_peers"),
			}
		)
	return {"nodes": nodes}


def baseline_network(path: Path, name: str) -> dict[str, object]:
	baseline = json.loads(path.read_text())[name]
	return {
		"nodes": [
			{
				"port": None,
				"chain": baseline["chain"],
				"genesis_hash": baseline["genesis_hash"],
				"peer_id": peer_id,
				"peers": [],
			}
			for peer_id in baseline["peer_ids"]
		]
	}


def peer_ids(net: dict[str, object]) -> set[str]:
	return {str(node["peer_id"]) for node in net["nodes"]}  # type: ignore[index]


def visible_peer_ids(net: dict[str, object]) -> set[str]:
	return {
		str(peer["peerId"])
		for node in net["nodes"]  # type: ignore[index]
		for peer in node["peers"]
	}


def one_value(net: dict[str, object], key: str) -> str:
	values = {str(node[key]) for node in net["nodes"]}  # type: ignore[index]
	if len(values) != 1:
		raise RuntimeError(f"isolated nodes disagree on {key}: {sorted(values)}")
	return values.pop()


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument(
		"--output",
		type=Path,
		default=Path("docs/evidence/p1/storage-proof/campaign/isolation-preflight.json"),
	)
	parser.add_argument(
		"--performance-baseline",
		type=Path,
		default=Path("docs/evidence/p1/storage-proof/performance-topology-baseline.json"),
		help="fallback comparison used when the exclusive 98xx topology is offline",
	)
	args = parser.parse_args()

	isolated_relay = network(list(range(10801, 10807)))
	isolated_orbis = network([10810, 10811])
	try:
		performance_relay = network([9801])
		performance_orbis = network([9810, 9811])
		performance_source = "live-rpc"
	except (OSError, urllib.error.URLError):
		performance_relay = baseline_network(args.performance_baseline, "relay")
		performance_orbis = baseline_network(args.performance_baseline, "orbis")
		performance_source = args.performance_baseline.as_posix()

	relay_spec = json.loads(
		Path("docs/evidence/p1/storage-proof/campaign/generated/origin-proof-isolated.json")
		.read_text()
	)
	orbis_spec = json.loads(
		Path("docs/evidence/p1/storage-proof/campaign/generated/orbis-proof-isolated.json")
		.read_text()
	)

	checks = {
		"relay_genesis_distinct": one_value(isolated_relay, "genesis_hash")
		!= one_value(performance_relay, "genesis_hash"),
		"orbis_genesis_distinct": one_value(isolated_orbis, "genesis_hash")
		!= one_value(performance_orbis, "genesis_hash"),
		"relay_local_peer_ids_distinct": peer_ids(isolated_relay).isdisjoint(
			peer_ids(performance_relay) | visible_peer_ids(performance_relay)
		),
		"orbis_local_peer_ids_distinct": peer_ids(isolated_orbis).isdisjoint(
			peer_ids(performance_orbis) | visible_peer_ids(performance_orbis)
		),
		"relay_peer_sets_do_not_overlap": visible_peer_ids(isolated_relay).isdisjoint(
			peer_ids(performance_relay) | visible_peer_ids(performance_relay)
		),
		"orbis_peer_sets_do_not_overlap": visible_peer_ids(isolated_orbis).isdisjoint(
			peer_ids(performance_orbis) | visible_peer_ids(performance_orbis)
		),
		"relay_protocol_namespace": relay_spec.get("protocolId")
		== "origin-proof-retention-v1"
		and relay_spec.get("forkId") == "origin-proof-retention-v1",
		"orbis_protocol_namespace": orbis_spec.get("protocolId")
		== "orbis-proof-retention-v1"
		and orbis_spec.get("forkId") == "orbis-proof-retention-v1",
		"isolated_relay_expected_peer_count": all(
			len(node["peers"]) == 7 for node in isolated_relay["nodes"]  # type: ignore[index]
		),
		"isolated_orbis_expected_peer_count": all(
			len(node["peers"]) == 1 for node in isolated_orbis["nodes"]  # type: ignore[index]
		),
	}
	result = {
		"schema_version": 1,
		"status": "pass" if all(checks.values()) else "fail",
		"checks": checks,
		"isolated": {"relay": isolated_relay, "orbis": isolated_orbis},
		"performance_comparison": {
			"source": performance_source,
			"relay": performance_relay,
			"orbis": performance_orbis,
		},
		"spec_protocols": {
			"relay": {"id": relay_spec.get("id"), "protocolId": relay_spec.get("protocolId"), "forkId": relay_spec.get("forkId")},
			"orbis": {"id": orbis_spec.get("id"), "protocolId": orbis_spec.get("protocolId"), "forkId": orbis_spec.get("forkId"), "relay_chain": orbis_spec.get("relay_chain")},
		},
	}
	args.output.parent.mkdir(parents=True, exist_ok=True)
	args.output.write_text(json.dumps(result, indent=2) + "\n")
	print(json.dumps(result, indent=2))
	return 0 if result["status"] == "pass" else 2


if __name__ == "__main__":
	raise SystemExit(main())
