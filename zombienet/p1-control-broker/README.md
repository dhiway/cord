# P1 live control/Broker campaign

Build the repository-owned driver separately from the live run (the runner itself never builds or
downloads anything):

```sh
cargo build --release -p origin-rs --example p1_live_driver
python3 zombienet/p1-control-broker/validate_static.py
```

Then run from the repository root with hash-pinned, separately built fast-profile binaries and the
driver above:

```sh
python3 zombienet/p1-control-broker/run.py \
  --origin-binary /absolute/path/to/origin \
  --orbis-binary /absolute/path/to/fast-profile/origin-omni-node \
  --driver "$PWD/target/release/examples/p1_live_driver" \
  --origin-upgrade-wasm /absolute/path/to/distinct/origin_runtime.compact.compressed.wasm \
  --orbis-upgrade-wasm /absolute/path/to/distinct/fast/orbis_runtime.compact.compressed.wasm
```

This command never builds or downloads anything. `--prepare-only` produces a `prepared` verdict,
never `pass`. A live pass requires every scenario in `scenarios.json`, raw finalized evidence from
the driver, the runner-owned full-process restart, post-restart recovery, and a clean contamination
preflight.

The driver is invoked once for each manifest phase. It must accept the arguments supplied by
`run.py`, write the requested JSON file, and return zero only after each phase case has finalized.
Each case record must contain `status: "pass"`, non-empty `input_hashes`, `output_hashes`,
`finalized_blocks`, `events`, and `assertions`. The driver must use the two supplied candidate Wasm
files and reject a candidate whose on-chain `:code` or spec version does not change.

Production session timing is not modified by this harness. The Orbis binary must be a separately
hash-recorded `fast-runtime` artifact; the driver must confirm the fast session period on-chain.

The driver fails closed and writes typed `failed` or `capability-gap` records rather than converting
missing evidence into a pass. Rotated-key authorship is bound to finalized Aura PreRuntime digest
slots and the active authority vector. Deterministic delay uses the Orbis fast-runtime-only
`CoretimeControl` transport hold/release calls; normal production builds expose the calls but reject
them. No mock, missing runtime surface, or prepared-only result is accepted as AC7/AC8 evidence.
