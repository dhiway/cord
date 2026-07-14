# P5 native SDK freeze

The first supported clean-break Rust and TypeScript SDK contract is frozen by:

- `docs/sdk/native-version-matrix.json` — Origin/Orbis runtime, transaction, metadata, API,
  storage-schema, and service-protocol versions;
- `docs/sdk/vectors/native-sdk-v1.json` — shared attestation, DotNS, and storage-provider
  semantic vectors (not raw extrinsic bytes or contract ABI fixtures);
- `product-sdk/packages/descriptors/generated/orbis-descriptor.json` — 132 exact typed native
  host methods; and
- `sdk-native-coverage.report.json` — 356 strict semantic/design mappings, reported separately
  from the 132 distinct routes actually executed by both Rust and TypeScript harnesses.

`sdk-freeze-ratification-envelope.json` binds those contracts with canonical payload SHA-256
`27c6effe52c60fade8e9fe341ffe874cb6e286e97944c51c7f0532052361e9fe`.
It is deliberately unsigned: all five approval slots remain pending and production activation is
blocked. The contract is bound to deterministic candidate header
`0x066f97db…c6e3` and identity artifact SHA-256 `824b76b9…6b2b`; this candidate is not
production-approved genesis. SDK network contracts freeze it as `candidate-pending` with
`production_activation_ready=false`; development requires explicit candidate opt-in and production
mode rejects it.
Fresh runtime, SDK, security, performance, and architecture owner signatures are required.

Focused reproduction:

```sh
npm --prefix product-sdk run check:descriptors
npm --prefix product-sdk run validate:ratification
npm --prefix product-sdk run validate:sdk-freeze
cargo test -p origin-rs product_sdk::version --lib
python3 scripts/validate-p5-native-launch.py
```

The native-launch validator builds and checks both provider binaries, verifies their generated CLI
contracts, starts a temporary provider with candidate-safe parameters, exercises local health,
identity, capacity and replica-readiness endpoints, and proves clean shutdown. It also executes the
retained generic Revive fixture test rather than trusting its allowlist label. Raw command output is
stored under `raw/` and hash-bound by `index.json`.

This local executable evidence does **not** claim provider registration, heartbeat or outbox receipt
finality. Those require a live candidate network and remain explicitly deferred to P6; the operator
report is `local-executable-pass-live-chain-deferred`, not a live-chain bootstrap pass.
