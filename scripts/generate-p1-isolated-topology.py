#!/usr/bin/env python3
"""Generate a disposable P1 topology isolated from every same-host Origin network.

Isolation is deliberate and redundant: the relay genesis differs, Zombienet assigns a random
protocol/fork ID, and wrapper commands replace Zombienet's name-derived libp2p node keys with
campaign-specific key files. The generated manifest hashes every launch input.
"""

import argparse
import hashlib
import json
import pathlib
import secrets

NODES = ("alice", "bob", "charlie", "dave", "eve", "fredie")
COLLATORS = ("orbis-alice", "orbis-bob")
PORTS = {9801: 11801, 9802: 11802, 9803: 11803, 9804: 11804, 9805: 11805, 9806: 11806, 9810: 11810, 9811: 11811}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wrapper(binary, names, key_dir):
    cases = "".join(f"    {name}) key={key_dir / (name + '.key')} ;;\n" for name in names)
    return f'''#!/usr/bin/env bash
set -euo pipefail
name=unknown
pre=()
post=()
after=0
while (($#)); do
  if ((after)); then post+=("$1"); shift; continue; fi
  case "$1" in
    --) after=1; shift ;;
    --name) name="$2"; pre+=("$1" "$2"); shift 2 ;;
    --node-key) shift 2 ;;
    *) pre+=("$1"); shift ;;
  esac
done
if [[ "$name" == unknown ]]; then exec {binary} "${{pre[@]}}"; fi
case "$name" in
{cases}    *) echo "unexpected P1 node name: $name" >&2; exit 64 ;;
esac
if ((after)); then exec {binary} "${{pre[@]}}" --node-key-file="$key" -- "${{post[@]}}"; fi
exec {binary} "${{pre[@]}}" --node-key-file="$key"
'''


def generate(repo, output, seed):
    source = repo / "zombienet" / "testnet-elastic.toml"
    text = source.read_text()
    required = (
        "[settings]\ntimeout = 300",
        'default_command = "target/release/origin"',
        'command = "target/release/origin-omni-node"',
        'name = "alice"\nrpc_port = 9801',
    )
    for marker in required:
        if marker not in text:
            raise RuntimeError(f"topology source no longer contains required marker: {marker!r}")

    output.mkdir(parents=True, exist_ok=False)
    key_dir = output / "keys"
    key_dir.mkdir()
    for name in NODES + COLLATORS:
        material = hashlib.sha256(f"{seed}:{name}".encode()).hexdigest()
        (key_dir / f"{name}.key").write_text(material + "\n")

    origin_wrapper = output / "origin-wrapper.sh"
    orbis_wrapper = output / "orbis-wrapper.sh"
    origin_wrapper.write_text(wrapper(repo / "target/release/origin", NODES, key_dir))
    orbis_wrapper.write_text(wrapper(repo / "target/release/origin-omni-node", COLLATORS, key_dir))
    origin_wrapper.chmod(0o700)
    orbis_wrapper.chmod(0o700)

    text = text.replace("[settings]\ntimeout = 300", "[settings]\ntimeout = 300\nisolate_env = true", 1)
    for old, new in PORTS.items():
        text = text.replace(f" = {old}", f" = {new}").replace(f":{old}", f":{new}")
    text = text.replace(
        'name = "alice"\nrpc_port = 11801',
        'name = "alice"\nrpc_port = 11801\nbalance = 2000000000001',
        1,
    )
    text = text.replace('default_command = "target/release/origin"', f'default_command = "{origin_wrapper}"', 1)
    text = text.replace('command = "target/release/origin-omni-node"', f'command = "{orbis_wrapper}"')
    topology = output / "testnet.toml"
    topology.write_text(text)

    inputs = [topology, origin_wrapper, orbis_wrapper, *sorted(key_dir.glob("*.key"))]
    manifest = {
        "schema_version": 1,
        "source": str(source),
        "source_sha256": sha256(source),
        "isolation": {
            "distinct_relay_genesis_balance": 2000000000001,
            "zombienet_isolate_env": True,
            "campaign_specific_node_keys": True,
        },
        "ports": {str(key): value for key, value in PORTS.items()},
        "artifacts_sha256": {str(path.relative_to(output)): sha256(path) for path in inputs},
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--seed", default=None, help="unique campaign seed; random when omitted")
    args = parser.parse_args()
    manifest = generate(args.repo.resolve(), args.output.resolve(), args.seed or secrets.token_hex(32))
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
