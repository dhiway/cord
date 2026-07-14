# Orbis Score Wasm benchmark evidence — 2026-07-13

This evidence was produced with the registered `origin-omni-node` node/runtime benchmark CLI. It is
not a completion-manifest transition.

- Node: `origin-omni-node 0.9.9-ee159750e87`
- Toolchain: `rustc 1.93.0 (254b59607 2026-01-19)`, `cargo 1.93.0`
- Chain: `orbis-dev`
- Execution: compiled Wasm
- Steps/repeat/min-duration: `20 / 10 / 0`
- Runtime compact Wasm SHA-256: `7bb2b93783280dc512373b124ed1c9ed66eb3dbf434a1be9b4e9b2234df333ac`
- Full raw results: `orbis-score-2026-07-13.json`
- Full CLI output: `orbis-score-2026-07-13.log`
- Redeem/payout rerun after fixing destination existential funding: `orbis-score-watch-2026-07-13.json` and `.log`

Reproduction:

```sh
SKIP_PALLET_REVIVE_FIXTURES=1 cargo build -p origin-omni-node --release --features runtime-benchmarks

target/release/origin-omni-node benchmark pallet \
  --chain orbis-dev --pallet pallet_orbis_score --extrinsic '*' \
  --exclude-extrinsics pallet_orbis_score::redeem_credit \
  --steps 20 --repeat 10 --min-duration 0 \
  --execution wasm --wasm-execution compiled \
  --output /tmp/orbis-score-measured.rs \
  --json-file docs/benchmarks/orbis-score-2026-07-13.json

target/release/origin-omni-node benchmark pallet \
  --chain orbis-dev --pallet pallet_orbis_score \
  --extrinsic 'redeem_credit,set_payout_account' \
  --steps 20 --repeat 10 --min-duration 0 \
  --execution wasm --wasm-execution compiled \
  --output /tmp/orbis-score-watch-measured.rs \
  --json-file docs/benchmarks/orbis-score-watch-2026-07-13.json
```

`set_payout_account` recorded a 25,000,000 ps minimum, 379 measured proof bytes and a 3,676-byte
estimated proof with seven reads. The checked-in weight mechanically applies a 2× ref-time margin
and charges three writes for the non-zero balance-transfer branch; a unit test freezes the
`configured >= measured` relation. `redeem_credit` recorded 91,000,000 ps, 632 measured/6,196
estimated proof bytes, five reads and four writes; the existing upstream 109,934,000 ps / 7,404
proof / seven-read / seven-write weight remains conservatively above the Orbis measurement.
