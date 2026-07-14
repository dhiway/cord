# P1 control and Broker evidence

This directory contains **preparatory partial evidence**, not a P1 gate pass. P0 targets and
client contracts are ratified by the canonical five-role Ed25519 envelope
(`payload_sha256=18a7fe95a300632121d0f448cb7bf3e1bc0e6776e254bc26bc1437a5f19c790a`).
That ratification does not claim production activation or substitute for the privileged,
finalized P1 lifecycle receipts required below.

## Reproduce the verified subset

```sh
cargo test -p pallet-authorities --lib -- --nocapture
cargo test -p origin-runtime 'genesis_config_presets::tests::' --lib -- --nocapture
cargo test -p origin-orbis-runtime 'broker_' --lib -- --nocapture
cargo test -p origin-orbis-runtime \
  orbis_sudo_can_reserve_multiple_full_cores_for_one_parachain --lib -- --nocapture
python3 scripts/test-p1-control-broker.py

# Generate a same-host-safe disposable topology. This changes genesis, protocol/fork IDs,
# primary libp2p node keys and RPC ports while retaining the six-validator/two-collator shape.
rm -rf /tmp/origin-p1-control
python3 scripts/generate-p1-isolated-topology.py \
  --output /tmp/origin-p1-control
zombienet -p native spawn /tmp/origin-p1-control/testnet.toml

zombienet -p native spawn zombienet/testnet-elastic.toml
cargo run -p origin-rs --example bootstrap_orbis_core -- \
  --endpoint ws://127.0.0.1:9801 --cores 3
python3 zombienet/orbis_smoke.py \
  --relay http://127.0.0.1:9801 --orbis http://127.0.0.1:9810 \
  --expected-cores 3 --expected-block-rate 3
python3 zombienet/p1_control_broker.py \
  --relay http://127.0.0.1:11801 --orbis http://127.0.0.1:11810
```

For a reversible local process-loss exercise, resolve the disposable node PID from its RPC
listener and pass it explicitly:

```sh
pid=$(lsof -nP -iTCP:11811 -sTCP:LISTEN | awk 'NR == 2 { print $2 }')
python3 zombienet/p1_control_broker.py \
  --relay http://127.0.0.1:11801 --orbis http://127.0.0.1:11810 \
  --fault orbis-collator-bob:11811:"$pid"
```

The probe rejects unexpected Origin/Orbis versions, fewer than three claim-queue cores, wrong
Orbis authoring APIs, loss of best/finalized progress, unsafe PIDs and failed recovery. Its JSON
always says `p1_gate_verdict: BLOCKED` because observation and process signals cannot substitute
for signed, finalized lifecycle operations.

The live mutation driver is `origin-rs/examples/p1_admin.rs`. It provides finalized Sudo
authority/collator mutations, session key staging, pause/unpause plus rejection probes, safe
invalid authorized-upgrade rejection, and Broker reserve/request/sale/purchase/renew/assign calls.
`scripts/verify-p1-control-receipts.py` remains fail-closed until every required event,
session boundary, XCM fault invariant and persistent restart receipt is present and hash-bound.

## Remaining blocking work

`control-scenarios.json` and `broker-e2e.json` enumerate the missing signed authority, upgrade,
pause and Broker-XCM receipts. Direct relay bootstrap assignment is not proof of Broker
request/reserve/assign/renew/resize/release. Delayed/duplicate XCM, enacted Orbis session
rotation and a full persistent node restart still require an exclusive-host campaign.
`broker-control-api-gaps.json` records two fail-closed gaps found from the current branch: bare
Broker/Coretime XCM has no duplicate-rejection/request-receipt envelope, and enacted Orbis
collator rotation uses a six-hour constant session. Neither is represented as a pass.

The 2026-07-13 11:52--11:53 Asia/Kolkata port-only isolation attempt cross-peered with a
same-genesis topology and produced equivocation logs. `control-contaminated-window.json` rejects
that entire window; no threshold or gate result includes it. All further campaigns must use the
generated distinct-genesis + `isolate_env` + campaign-node-key topology and assert zero unexpected
peers/equivocations before collecting receipts.
