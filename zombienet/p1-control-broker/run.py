#!/usr/bin/env python3
"""Fail-closed, hash-bound Origin/Orbis P1 live campaign orchestrator."""

import argparse, hashlib, json, os, re, shlex, signal, socket, subprocess, sys, tempfile, time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RELAY_PROTOCOL, RELAY_FORK, RELAY_ID = "p1-origin-control-v1", "p1-origin-control-20260713", "origin-p1-control"
ORBIS_PROTOCOL, ORBIS_FORK, ORBIS_ID = "p1-orbis-broker-v1", "p1-orbis-broker-20260713", "orbis-p1-broker"
NODE_NAMES = ["alice", "bob", "charlie", "dave", "eve", "ferdie", "orbis-alice", "orbis-bob"]
# Zombienet v1.3.133 owns --node-key and deterministically derives it as SHA-256(name).
PROVIDER_NODE_KEYS = [hashlib.sha256(name.encode()).hexdigest() for name in NODE_NAMES]
# Zombienet does not own arguments after the collator's ``--`` separator. Pin the two embedded
# relay full-node identities there so the complete isolated peer ledger is deterministic.
INTERNAL_RELAY_NODE_KEYS = ["09" * 32, "0a" * 32]
NODE_KEYS = PROVIDER_NODE_KEYS + INTERNAL_RELAY_NODE_KEYS
INTERNAL_RELAY_ARGS = [
    ["--node-key", INTERNAL_RELAY_NODE_KEYS[0], "--no-mdns"],
    ["--node-key", INTERNAL_RELAY_NODE_KEYS[1], "--no-mdns"],
]
PROVIDER_OWNED_ARGS = {
    "--base-path", "--chain", "--collator", "--insecure-validator-i-know-what-i-do",
    "--listen-addr", "--name", "--no-mdns", "--no-telemetry", "--node-key",
    "--parachain-id", "--port", "--prometheus-external", "--prometheus-port",
    "--rpc-cors", "--rpc-methods", "--rpc-port", "--unsafe-rpc-external", "--validator",
    "--ws-port",
}
RPC_PORTS = [11802, 11812, 11822, 11832, 11842, 11852, 11862, 11872]
RELAY_RPC_PORTS, ORBIS_RPC_PORTS = RPC_PORTS[:6], RPC_PORTS[6:]
ALL_PORTS = [11801,11802,11803,11811,11812,11813,11821,11822,11823,11831,11832,11833,
             11841,11842,11843,11851,11852,11853,11861,11862,11863,11871,11872,11873]
ORIGIN_WORKER_NAMES = ("origin-prepare-worker", "origin-execute-worker")


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def origin_worker_binaries(origin):
    """Return the executable PVF workers colocated with the selected Origin binary.

    The relay resolves these exact basenames relative to its own executable.  Accepting a worker
    from PATH, another target directory, or a symlink outside the sealed binary directory would
    make the live campaign's executable inputs differ from its hash ledger.
    """
    origin = Path(origin).resolve(strict=True)
    workers = {}
    for name in ORIGIN_WORKER_NAMES:
        adjacent = origin.parent / name
        try:
            worker = adjacent.resolve(strict=True)
        except FileNotFoundError as error:
            raise RuntimeError(f"missing adjacent Origin PVF worker: {adjacent}") from error
        if worker.parent != origin.parent or adjacent.is_symlink():
            raise RuntimeError(f"Origin PVF worker must be a regular adjacent file: {adjacent}")
        if not worker.is_file() or not os.access(worker, os.X_OK):
            raise RuntimeError(f"adjacent Origin PVF worker is not executable: {worker}")
        workers[name] = worker
    return workers


def write_orbis_command_wrapper(path, orbis):
    """Write a hashable compatibility wrapper for the embedded relay full-node identities.

    Zombienet v1.3.133 accepts ``relay_chain_args`` in the topology but does not forward that
    field to the native collator command.  Its generated command does retain an argument separator,
    so this wrapper appends the two internal-only identity arguments after that separator.  Spec
    generation and CLI capability probes pass through unchanged.
    """
    orbis = Path(orbis).resolve(strict=True)
    key_cases = "\n".join(
        f'  {name}) key="{key}" ;;'
        for name, key in zip(NODE_NAMES[-2:], INTERNAL_RELAY_NODE_KEYS)
    )
    script = f"""#!/bin/sh
set -eu
real={shlex.quote(str(orbis))}
name=
take_name=0
separators=0
after_separator=0
for argument in "$@"; do
  if [ "$take_name" -eq 1 ]; then
    name=$argument
    take_name=0
  elif [ "$argument" = "--name" ]; then
    take_name=1
  else
    case "$argument" in --name=*) name=${{argument#--name=}} ;; esac
  fi
  if [ "$argument" = "--" ]; then
    separators=$((separators + 1))
    after_separator=1
    continue
  fi
  if [ "$after_separator" -eq 1 ]; then
    case "$argument" in
      --node-key|--node-key=*|--no-mdns)
        echo "refusing pre-existing embedded relay identity argument" >&2
        exit 64
        ;;
    esac
  fi
done
case "$name" in
{key_cases}
  "") exec "$real" "$@" ;;
  *)
    if [ "$separators" -eq 0 ]; then exec "$real" "$@"; fi
    echo "refusing unknown collator name with embedded relay command: $name" >&2
    exit 64
    ;;
esac
if [ "$separators" -ne 1 ]; then
  echo "expected one collator/relay argument separator, found $separators" >&2
  exit 64
fi
exec "$real" "$@" --node-key "$key" --no-mdns
"""
    path.write_text(script)
    path.chmod(0o555)
    return path


def run_logged(command, log, **kwargs):
    with open(log, "ab") as output:
        output.write(("$ " + shlex.join([str(x) for x in command]) + "\n").encode())
        result = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT, **kwargs)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}); see {log}: {command}")


def capture(command, log, **kwargs):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, **kwargs)
    Path(log).write_bytes(result.stdout)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}); see {log}: {command}")
    return result.stdout.decode(errors="replace")


def capture_json(command, destination, log, **kwargs):
    with open(destination, "wb") as output, open(log, "wb") as errors:
        result = subprocess.run(command, stdout=output, stderr=errors, **kwargs)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}); see {log}: {command}")
    json.loads(destination.read_text())


def rpc(port, method, params=None, timeout=10):
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}",
        data=json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":params or []}).encode(),
        headers={"content-type":"application/json"},
    )
    response = json.load(urllib.request.urlopen(request, timeout=timeout))
    if "error" in response:
        raise RuntimeError(f"RPC {port} {method}: {response['error']}")
    return response["result"]


def wait_rpc(port, deadline):
    last = None
    while time.monotonic() < deadline:
        try:
            return rpc(port, "system_health", timeout=2)
        except Exception as error:
            last = error
            time.sleep(2)
    raise RuntimeError(f"RPC {port} did not become ready: {last}")


def block_snapshot(port):
    finalized = rpc(port, "chain_getFinalizedHead")
    best = rpc(port, "chain_getHeader")
    final = rpc(port, "chain_getHeader", [finalized])
    return {"best":int(best["number"],16), "finalized":int(final["number"],16), "finalized_hash":finalized}


def patch_spec(source, destination, chain_id, protocol, fork, relay_chain=None):
    spec = json.loads(source.read_text())
    spec.update({"id":chain_id, "protocolId":protocol, "forkId":fork, "telemetryEndpoints":None})
    if relay_chain is not None:
        extensions = spec.setdefault("extensions", {})
        extensions.update({"relay_chain":relay_chain, "para_id":1006})
        extensions.pop("relayChain", None); extensions.pop("paraId", None)
    write_json(destination, spec)


def topology_spec_locator(spec, specs_dir):
    """Return a basename that native Zombienet can safely stage below ``network/cfg``.

    Native Zombienet uses the chain value in a generated filename before launching the binary.
    Passing an absolute path therefore creates an invalid nested path below its cfg directory. The
    Zombienet process runs from ``specs_dir``, so a validated basename still resolves to the exact
    hash-bound spec without allowing traversal or ambiguous lookup.
    """
    spec = Path(spec).resolve(strict=True)
    specs_dir = Path(specs_dir).resolve(strict=True)
    if spec.parent != specs_dir or Path(spec.name).name != spec.name:
        raise RuntimeError(f"chain spec must be a direct child of the staged spec directory: {spec}")
    return spec.name


def render_topology(template, relay_spec, orbis_spec, specs_dir, origin, orbis):
    relay_locator = topology_spec_locator(relay_spec, specs_dir)
    orbis_locator = topology_spec_locator(orbis_spec, specs_dir)
    if relay_locator == orbis_locator:
        raise RuntimeError("relay and Orbis staged spec basenames collide")
    return (template.read_text()
            .replace("__RELAY_SPEC__", relay_locator)
            .replace("__ORBIS_SPEC__", orbis_locator)
            .replace("__ORIGIN_BINARY__", str(origin))
            .replace("__ORBIS_BINARY__", str(orbis)))


def relay_genesis_hash(binary, spec, log):
    """Initialize an ephemeral relay DB from ``spec`` and read its genesis hash.

    The Origin relay CLI does not expose a parachain-style genesis-head command. ``export-state``
    is the supported command that initializes block zero from a chain spec; ``chain-info`` then
    reports the full hash from that exact ephemeral database. The exported multi-megabyte state is
    discarded because the hash-bearing chain-info record and both commands are retained in ``log``.
    """
    log = Path(log)
    log.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="relay-genesis-db-", dir=log.parent) as base:
        export = [str(binary), "export-state", "--chain", str(spec), "--base-path", base, "0"]
        info = [str(binary), "chain-info", "--chain", str(spec), "--base-path", base]
        with log.open("wb") as evidence:
            evidence.write(("$ " + shlex.join(export) + "\n").encode())
            result = subprocess.run(
                export, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=evidence, check=False
            )
            if result.returncode:
                raise RuntimeError(f"command failed ({result.returncode}); see {log}: {export}")
            evidence.write(("$ " + shlex.join(info) + "\n").encode())
            result = subprocess.run(
                info, cwd=ROOT, stdout=subprocess.PIPE, stderr=evidence, check=False
            )
            evidence.write(result.stdout)
            if result.returncode:
                raise RuntimeError(f"command failed ({result.returncode}); see {log}: {info}")
        try:
            record = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise RuntimeError(f"chain-info did not return JSON; see {log}: {error}") from error
    genesis = record.get("genesis_hash")
    if not isinstance(genesis, str) or not re.fullmatch(r"0x[0-9a-fA-F]{64}", genesis):
        raise RuntimeError(f"chain-info returned an invalid genesis hash; see {log}: {genesis!r}")
    if genesis == "0x" + "00" * 32:
        raise RuntimeError(f"chain-info returned an uninitialized genesis hash; see {log}")
    if (
        record.get("best_number") != 0
        or record.get("finalized_number") != 0
        or record.get("best_hash") != genesis
        or record.get("finalized_hash") != genesis
    ):
        raise RuntimeError(f"ephemeral relay database is not genesis-only; see {log}: {record}")
    return genesis.lower()


def parachain_genesis_hash(binary, spec, log):
    """Hash the SCALE genesis header emitted by the supported Orbis CLI command."""
    command = [str(binary), "export-genesis-head", "--chain", str(spec)]
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    Path(log).write_bytes(
        ("$ " + shlex.join(command) + "\n").encode() + result.stdout + result.stderr
    )
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}); see {log}: {command}")
    text = result.stdout.decode(errors="replace").strip()
    if not re.fullmatch(r"0x[0-9a-fA-F]+", text) or len(text) <= 66 or len(text) % 2:
        raise RuntimeError(f"invalid Orbis genesis header hex in {log}")
    return "0x" + hashlib.blake2b(bytes.fromhex(text[2:]), digest_size=32).hexdigest()


def port_preflight():
    if len(ALL_PORTS) != len(set(ALL_PORTS)) or not all(11800 <= p <= 11899 for p in ALL_PORTS):
        raise RuntimeError("topology ports are not unique within 118xx")
    occupied = []
    for port in ALL_PORTS:
        sock = socket.socket(); sock.settimeout(.1)
        if sock.connect_ex(("127.0.0.1", port)) == 0: occupied.append(port)
        sock.close()
    if occupied: raise RuntimeError(f"refusing contaminated/occupied 118xx ports: {occupied}")


def validate_template(template):
    text = template.read_text()
    if "isolate_env = true" not in text: raise RuntimeError("settings.isolate_env=true is mandatory")
    names = re.findall(r'^name = "([^"]+)"$', text, re.M)
    if names != NODE_NAMES:
        raise RuntimeError(f"provider-owned node identity ledger mismatch: {names}")
    explicit = []
    for match in re.finditer(r"^args = (\[.*\])$", text, re.M):
        values = json.loads(match.group(1))
        if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
            raise RuntimeError("topology args must be arrays of strings")
        explicit.extend(values)
    duplicates = sorted({
        token.split("=", 1)[0]
        for token in explicit
        if token.split("=", 1)[0] in PROVIDER_OWNED_ARGS
    })
    if duplicates:
        raise RuntimeError(f"topology duplicates Zombienet-owned arguments: {duplicates}")
    internal = []
    for match in re.finditer(r"^relay_chain_args = (\[.*\])$", text, re.M):
        values = json.loads(match.group(1))
        if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
            raise RuntimeError("topology relay_chain_args must be arrays of strings")
        internal.append(values)
    if internal != INTERNAL_RELAY_ARGS:
        raise RuntimeError(f"internal relay identity ledger mismatch: {internal}")
    found = sorted({int(x) for x in re.findall(r"(?:p2p|rpc|prometheus)_port = (\d+)", text)})
    if found != sorted(ALL_PORTS): raise RuntimeError(f"topology port ledger mismatch: {found}")


def generated_genesis(network_dir, origin, orbis, raw_dir):
    matches = {"relay":[], "orbis":[]}
    for path in network_dir.rglob("*.json"):
        try: spec = json.loads(path.read_text())
        except Exception: continue
        if spec.get("protocolId") == RELAY_PROTOCOL and spec.get("forkId") == RELAY_FORK: matches["relay"].append(path)
        if spec.get("protocolId") == ORBIS_PROTOCOL and spec.get("forkId") == ORBIS_FORK: matches["orbis"].append(path)
    if not matches["relay"] or not matches["orbis"]:
        raise RuntimeError(f"cannot locate final specs with exact protocol/fork IDs: {matches}")
    relay_hashes = {relay_genesis_hash(origin,p,raw_dir/f"generated-relay-{i}.log") for i,p in enumerate(matches["relay"])}
    orbis_hashes = {parachain_genesis_hash(orbis,p,raw_dir/f"generated-orbis-{i}.log") for i,p in enumerate(matches["orbis"])}
    if len(relay_hashes) != 1 or len(orbis_hashes) != 1:
        raise RuntimeError(f"generated spec genesis ambiguity: {relay_hashes}/{orbis_hashes}")
    expected = {"relay":relay_hashes.pop(), "orbis":orbis_hashes.pop()}
    if expected["relay"] == expected["orbis"]: raise RuntimeError("relay/Orbis genesis collision")
    ledger = {chain:[{"path":str(p),"sha256":sha256(p)} for p in paths] for chain,paths in matches.items()}
    return expected, ledger


def derive_peer_ids(binary, output):
    key_dir=output/"node-keys"; key_dir.mkdir()
    peers=[]
    for index,key in enumerate(NODE_KEYS):
        path=key_dir/f"node-{index+1:02d}.key"; path.write_text(key+"\n")
        text=capture([str(binary),"key","inspect-node-key","--file",str(path)],output/"raw"/f"peer-id-{index+1:02d}.log",cwd=ROOT)
        matches=re.findall(r"12D3Koo[1-9A-HJ-NP-Za-km-z]+",text)
        if len(matches) != 1: raise RuntimeError(f"cannot derive unique peer ID for fixed key {index+1}")
        peers.append(matches[0])
    if len(set(peers)) != len(NODE_KEYS): raise RuntimeError("fixed node keys yielded duplicate peer IDs")
    return peers


def network_preflight(expected_genesis, topology_hash, expected_peers):
    for port in RPC_PORTS: wait_rpc(port, time.monotonic()+300)
    relay_genesis = {rpc(p,"chain_getBlockHash",[0]) for p in RELAY_RPC_PORTS}
    orbis_genesis = {rpc(p,"chain_getBlockHash",[0]) for p in ORBIS_RPC_PORTS}
    if relay_genesis != {expected_genesis["relay"]}: raise RuntimeError(f"relay genesis contamination: {relay_genesis}")
    if orbis_genesis != {expected_genesis["orbis"]}: raise RuntimeError(f"Orbis genesis contamination: {orbis_genesis}")
    if relay_genesis == orbis_genesis: raise RuntimeError("relay and Orbis genesis are not distinct")
    versions = {str(p):rpc(p,"state_getRuntimeVersion") for p in RPC_PORTS}
    if any(versions[str(p)]["specName"] != "origin" for p in RELAY_RPC_PORTS): raise RuntimeError("unexpected relay runtime")
    if any(versions[str(p)]["specName"] != "orbis" for p in ORBIS_RPC_PORTS): raise RuntimeError("unexpected Orbis runtime")
    local = {str(p):rpc(p,"system_localPeerId") for p in RPC_PORTS}
    local_set=set(local.values()); allowed=set(expected_peers)
    if len(local_set) != 8 or not local_set.issubset(allowed):
        raise RuntimeError(f"local peer IDs do not match fixed keys: {local}")
    deadline=time.monotonic()+90; peer_sets={}
    while True:
        union=set(); peer_sets={}
        for port in RPC_PORTS:
            observed={x["peerId"] for x in rpc(port,"system_peers")}; union.update(observed)
            unexpected=observed-allowed
            if unexpected: raise RuntimeError(f"peer contamination on {port}: {sorted(unexpected)}")
            peer_sets[str(port)]=sorted(observed)
        union.update(local_set)
        if union == allowed: break
        if time.monotonic() >= deadline: raise RuntimeError(f"fixed peer set incomplete: missing {sorted(allowed-union)}")
        time.sleep(3)
    return {"status":"pass","topology_sha256":topology_hash,
            "relay_genesis":next(iter(relay_genesis)),"orbis_genesis":next(iter(orbis_genesis)),
            "relay_protocol_id":RELAY_PROTOCOL,"relay_fork_id":RELAY_FORK,
            "orbis_protocol_id":ORBIS_PROTOCOL,"orbis_fork_id":ORBIS_FORK,
            "expected_peers":sorted(allowed),"local_peers":local,"peer_sets":peer_sets,"versions":versions}


def invoke_driver(driver, phase, manifest, evidence_dir, args):
    output = evidence_dir/f"driver-{phase}.json"; log = evidence_dir/"raw"/f"driver-{phase}.log"
    command = [str(driver),"--phase",phase,"--manifest",str(manifest),
               "--relay","ws://127.0.0.1:11802","--orbis","ws://127.0.0.1:11862",
               "--relay-upgrade-wasm",str(args.origin_upgrade_wasm.resolve()),
               "--orbis-upgrade-wasm",str(args.orbis_upgrade_wasm.resolve()),
               "--output",str(output),"--evidence-dir",str(evidence_dir)]
    run_logged(command, log, cwd=ROOT, timeout=args.phase_timeout)
    if not output.is_file(): raise RuntimeError(f"driver did not write {output}")
    result = json.loads(output.read_text())
    expected = {case["id"] for case in json.loads(manifest.read_text())["phases"][phase]}
    records = result.get("cases", {})
    if set(records) != expected: raise RuntimeError(f"driver {phase} case mismatch: {set(records)} != {expected}")
    required = ("input_hashes","output_hashes","finalized_blocks","events","assertions")
    for case_id, record in records.items():
        if record.get("status") != "pass": raise RuntimeError(f"case did not pass: {case_id}")
        if any(not record.get(field) for field in required):
            raise RuntimeError(f"case lacks raw/hash/finality evidence: {case_id}")
    return result


def process_exists(pid):
    try: os.kill(pid,0); return True
    except ProcessLookupError: return False


def can_connect(port):
    sock=socket.socket(); sock.settimeout(.2); result=sock.connect_ex(("127.0.0.1",port))==0; sock.close(); return result


def descendant_node_commands(network_dir, origin, orbis):
    text = subprocess.check_output(["ps","-ax","-ww","-o","pid=,command="], text=True)
    commands=[]; names={Path(origin).name,Path(orbis).name}
    for line in text.splitlines():
        match=re.match(r"\s*(\d+)\s+(.*)",line)
        if not match or str(network_dir) not in match.group(2) or "--base-path" not in match.group(2): continue
        try: argv=shlex.split(match.group(2))
        except ValueError: continue
        if argv and Path(argv[0]).name in names: commands.append((int(match.group(1)),argv))
    if len(commands) != 8: raise RuntimeError(f"full restart requires eight node processes, found {len(commands)}")
    return commands


def validate_launch_commands(network_dir, origin, orbis):
    """Prove the actual native process argv contains the complete internal identity ledger."""
    commands = descendant_node_commands(network_dir, origin, orbis)
    orbis_name = Path(orbis).name
    expected = dict(zip(NODE_NAMES[-2:], INTERNAL_RELAY_NODE_KEYS))
    observed = {}
    ledger = []
    for pid, argv in commands:
        ledger.append({"pid":pid, "argv":argv})
        if Path(argv[0]).name != orbis_name:
            continue
        if argv.count("--name") != 1 or argv.count("--") != 1:
            raise RuntimeError(f"ambiguous Orbis launch command: {argv}")
        name = argv[argv.index("--name") + 1]
        if name not in expected or name in observed:
            raise RuntimeError(f"Orbis launch identity ledger mismatch: {name}")
        suffix = argv[argv.index("--") + 1:]
        pinned = ["--node-key", expected[name], "--no-mdns"]
        if suffix[-len(pinned):] != pinned or suffix.count("--node-key") != 1 or suffix.count("--no-mdns") != 1:
            raise RuntimeError(f"embedded relay launch identity mismatch for {name}: {suffix}")
        observed[name] = expected[name]
    if observed != expected:
        raise RuntimeError(f"embedded relay launch ledger incomplete: {observed}")
    return {"schema":"cord.p1-launch-command-ledger.v1", "internal_relay_keys":observed,
            "commands":ledger}


def full_restart(network_dir, origin, orbis, evidence_dir, expected_genesis, topology_hash, expected_peers):
    before={str(p):block_snapshot(p) for p in RPC_PORTS}; commands=descendant_node_commands(network_dir,origin,orbis)
    old=[pid for pid,_ in commands]
    for pid in old: os.kill(pid,signal.SIGTERM)
    deadline=time.monotonic()+30
    while time.monotonic()<deadline and any(process_exists(pid) for pid in old): time.sleep(.5)
    for pid in old:
        if process_exists(pid): os.kill(pid,signal.SIGKILL)
    time.sleep(2)
    if any(can_connect(p) for p in RPC_PORTS):
        raise RuntimeError("RPC reopened before runner restart; refusing ambiguous evidence")
    replacements=[]
    for index,(_,argv) in enumerate(commands):
        log=open(evidence_dir/"raw"/f"restart-node-{index}.log","ab")
        replacements.append((subprocess.Popen(argv,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT),log))
    for port in RPC_PORTS: wait_rpc(port,time.monotonic()+300)
    preflight=network_preflight(expected_genesis,topology_hash,expected_peers)
    deadline=time.monotonic()+300; after=None
    while time.monotonic()<deadline:
        after={str(p):block_snapshot(p) for p in RPC_PORTS}
        if all(after[str(p)]["best"]>before[str(p)]["best"] and after[str(p)]["finalized"]>before[str(p)]["finalized"] for p in RPC_PORTS): break
        time.sleep(5)
    else: raise RuntimeError(f"post-restart progress/finality failed: {before} -> {after}")
    record={"status":"pass","old_pids":old,"new_pids":[p.pid for p,_ in replacements],
            "before":before,"after":after,"contamination_preflight":preflight}
    write_json(evidence_dir/"full-restart.json",record)
    return record,replacements


def hash_tree(root, excluded):
    return [{"path":str(p.relative_to(root)),"sha256":sha256(p),"bytes":p.stat().st_size}
            for p in sorted(root.rglob("*")) if p.is_file() and p not in excluded]


def parse_args():
    parser=argparse.ArgumentParser()
    parser.add_argument("--origin-binary",type=Path,required=True); parser.add_argument("--orbis-binary",type=Path,required=True)
    parser.add_argument("--driver",type=Path); parser.add_argument("--origin-upgrade-wasm",type=Path); parser.add_argument("--orbis-upgrade-wasm",type=Path)
    parser.add_argument("--zombienet",default="zombienet"); parser.add_argument("--output",type=Path,default=Path("/tmp/cord-p1-live-evidence"))
    parser.add_argument("--phase-timeout",type=int,default=5400); parser.add_argument("--prepare-only",action="store_true")
    return parser.parse_args()


def main():
    args=parse_args(); output=args.output.resolve()
    if output.exists() and any(output.iterdir()): raise RuntimeError(f"output must be absent or empty: {output}")
    (output/"raw").mkdir(parents=True,exist_ok=True)
    evidence_path=output/"control-broker-evidence.json"
    evidence={"schema":"cord.p1-control-broker-evidence.v1","status":"failed",
              "campaign_id":"origin-orbis-p1-control-broker-v1","prepared_only":args.prepare_only,
              "started_unix":int(time.time()),"cases":{},"raw_hashes":[]}
    write_json(evidence_path,evidence); zombie=None; replacements=[]; zombie_log=None
    try:
        origin=args.origin_binary.resolve(strict=True); orbis=args.orbis_binary.resolve(strict=True)
        if not os.access(origin,os.X_OK) or not os.access(orbis,os.X_OK): raise RuntimeError("node binaries must be executable")
        origin_workers=origin_worker_binaries(origin)
        orbis_wrapper=write_orbis_command_wrapper(output/"orbis-command-wrapper.sh",orbis)
        validate_template(HERE/"topology.toml.in"); port_preflight()
        inputs={"origin_binary_sha256":sha256(origin),"orbis_binary_sha256":sha256(orbis),
                "origin_prepare_worker_sha256":sha256(origin_workers["origin-prepare-worker"]),
                "origin_execute_worker_sha256":sha256(origin_workers["origin-execute-worker"]),
                "orbis_command_wrapper_sha256":sha256(orbis_wrapper),
                "topology_template_sha256":sha256(HERE/"topology.toml.in"),
                "scenario_manifest_sha256":sha256(HERE/"scenarios.json")}
        expected_peers=derive_peer_ids(origin,output); inputs["expected_peer_ids"]=expected_peers
        specs=output/"specs"; specs.mkdir(); relay_initial=specs/"relay.initial.json"; orbis_initial=specs/"orbis.initial.json"
        capture_json([str(origin),"build-spec","--chain","origin-local","--raw"],relay_initial,output/"raw"/"relay-build-spec.log",cwd=ROOT)
        capture_json([str(orbis),"build-spec","--chain","orbis-local","--raw"],orbis_initial,output/"raw"/"orbis-build-spec.log",cwd=ROOT)
        relay_spec=specs/"relay.raw.json"; orbis_spec=specs/"orbis.raw.json"
        patch_spec(relay_initial,relay_spec,RELAY_ID,RELAY_PROTOCOL,RELAY_FORK)
        patch_spec(orbis_initial,orbis_spec,ORBIS_ID,ORBIS_PROTOCOL,ORBIS_FORK,RELAY_ID)
        base={"relay":relay_genesis_hash(origin,relay_spec,output/"raw"/"relay-genesis-state.log"),
              "orbis":parachain_genesis_hash(orbis,orbis_spec,output/"raw"/"orbis-genesis-state.log")}
        if base["relay"]==base["orbis"]: raise RuntimeError("base genesis collision")
        inputs.update({"relay_spec_sha256":sha256(relay_spec),"orbis_spec_sha256":sha256(orbis_spec),
                       "base_relay_genesis":base["relay"],"base_orbis_genesis":base["orbis"]})
        topology=output/"topology.toml"
        topology.write_text(render_topology(
            HERE/"topology.toml.in", relay_spec, orbis_spec, specs, origin, orbis_wrapper
        ))
        topology_hash=sha256(topology); inputs["topology_sha256"]=topology_hash; write_json(output/"inputs.json",inputs)
        if args.prepare_only:
            evidence.update({"status":"prepared","inputs":inputs,"reason":"prepare-only never constitutes AC7/AC8 evidence"}); return 0
        if not args.driver or not args.origin_upgrade_wasm or not args.orbis_upgrade_wasm:
            raise RuntimeError("live mode requires --driver and both candidate Wasm paths")
        driver=args.driver.resolve(strict=True)
        for wasm in (args.origin_upgrade_wasm,args.orbis_upgrade_wasm): wasm.resolve(strict=True)
        inputs.update({"driver_sha256":sha256(driver),"origin_upgrade_wasm_sha256":sha256(args.origin_upgrade_wasm),
                       "orbis_upgrade_wasm_sha256":sha256(args.orbis_upgrade_wasm)}); write_json(output/"inputs.json",inputs)
        network_dir=output/"network"; command=[args.zombienet,"-p","native","-l","text","-d",str(network_dir),"-f","spawn",str(topology)]
        zombie_log=open(output/"raw"/"zombienet.log","ab"); zombie=subprocess.Popen(command,cwd=specs,stdout=zombie_log,stderr=subprocess.STDOUT)
        for port in RPC_PORTS: wait_rpc(port,time.monotonic()+300)
        launch_ledger=output/"launch-command-ledger.json"
        write_json(launch_ledger,validate_launch_commands(network_dir,origin,orbis))
        inputs["launch_command_ledger_sha256"]=sha256(launch_ledger); write_json(output/"inputs.json",inputs)
        expected,generated=generated_genesis(network_dir,origin,orbis,output/"raw")
        inputs.update({"relay_genesis":expected["relay"],"orbis_genesis":expected["orbis"],"zombienet_generated_specs":generated}); write_json(output/"inputs.json",inputs)
        preflight=network_preflight(expected,topology_hash,expected_peers); write_json(output/"contamination-preflight.json",preflight)
        evidence["cases"]["control"]=invoke_driver(driver,"control",HERE/"scenarios.json",output,args)
        evidence["cases"]["broker-pre-restart"]=invoke_driver(driver,"broker-pre-restart",HERE/"scenarios.json",output,args)
        restart,replacements=full_restart(network_dir,origin,orbis,output,expected,topology_hash,expected_peers); evidence["cases"]["full-restart-runner"]=restart
        evidence["cases"]["broker-post-restart"]=invoke_driver(driver,"broker-post-restart",HERE/"scenarios.json",output,args)
        evidence.update({"status":"pass","inputs":inputs,"preflight":preflight,"finished_unix":int(time.time())}); return 0
    except Exception as error:
        evidence.update({"status":"failed","error":str(error),"finished_unix":int(time.time())})
        print(f"P1 campaign failed closed: {error}",file=sys.stderr); return 1
    finally:
        for process,log in replacements:
            if process.poll() is None: process.terminate()
            log.close()
        if zombie is not None and zombie.poll() is None:
            zombie.terminate()
            try: zombie.wait(timeout=20)
            except subprocess.TimeoutExpired: zombie.kill()
        if zombie_log is not None: zombie_log.close()
        excluded={evidence_path,output/"raw-hashes.json",output/"control-broker-evidence.sha256"}
        evidence["raw_hashes"]=hash_tree(output,excluded); write_json(evidence_path,evidence)
        write_json(output/"raw-hashes.json",evidence["raw_hashes"])
        (output/"control-broker-evidence.sha256").write_text(f"{sha256(evidence_path)}  {evidence_path.name}\n")


if __name__ == "__main__": sys.exit(main())
