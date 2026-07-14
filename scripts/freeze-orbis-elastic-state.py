#!/usr/bin/env python3
"""Freeze the deterministic clean-genesis input used by every P1 elastic run."""

import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/evidence/performance/p1-elastic-genesis-state.json"


def sha_bytes(value):
	return hashlib.sha256(value).hexdigest()


def sha_file(path):
	return sha_bytes(path.read_bytes())


def output(command):
	return subprocess.check_output(command, cwd=ROOT)


def main():
	origin = ROOT / "target/release/origin"
	orbis = ROOT / "target/release/origin-omni-node"
	if not origin.is_file() or not orbis.is_file():
		raise SystemExit("build current release origin/origin-omni-node binaries first")
	origin_command = [str(origin), "build-spec", "--chain", "origin-local", "--raw"]
	orbis_command = [str(orbis), "build-spec", "--chain", "orbis-local", "--raw"]
	origin_raw = output(origin_command)
	orbis_raw = output(orbis_command)
	manifest = {
		"schema_version": 1,
		"scope": "p1-orbis-elastic-clean-genesis",
		"branch": subprocess.check_output(["git", "branch", "--show-current"], cwd=ROOT, text=True).strip(),
		"commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
		"reset_policy": "fresh-zombienet-network-and-clean-genesis-before-every-run",
		"data_migration": False,
		"backward_compatibility": False,
		"inputs": {
			"origin_binary_sha256": sha_file(origin),
			"orbis_binary_sha256": sha_file(orbis),
			"origin_chain_spec_source_sha256": sha_file(ROOT / "origin/base/cli/src/chain_spec.rs"),
			"orbis_chain_spec_source_sha256": sha_file(ROOT / "origin/orbis/node/src/chain_spec.rs"),
			"elastic_topology_sha256": sha_file(ROOT / "zombienet/testnet-elastic.toml"),
		},
		"raw_genesis": {
			"origin": {"command": "target/release/origin build-spec --chain origin-local --raw", "sha256": sha_bytes(origin_raw)},
			"orbis": {"command": "target/release/origin-omni-node build-spec --chain orbis-local --raw", "sha256": sha_bytes(orbis_raw)},
		},
	}
	OUT.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
	print(OUT.relative_to(ROOT), sha_file(OUT))


if __name__ == "__main__":
	main()
