# P5 native SDK freeze

The first supported clean-break Rust and TypeScript SDK contract is frozen by:

- `docs/sdk/native-version-matrix.json` — Origin/Orbis runtime, transaction, metadata, API,
  storage-schema, and service-protocol versions;
- `docs/sdk/vectors/native-sdk-v1.json` — shared attestation, DotNS, and storage-provider
  semantic vectors (not raw extrinsic bytes or contract ABI fixtures);
- `product-sdk/packages/descriptors/generated/orbis-descriptor.json` — 143 exact typed native
  host methods, including identity/personhood and sponsored transaction operations; and
- `sdk-native-coverage.report.json` — 2,780 classified design entries and 14 exact adopted
  semantic bindings, reported separately from the 143 distinct routes actually executed by both
  Rust and TypeScript harnesses.

`sdk-freeze-ratification-envelope.json` binds those contracts with canonical payload SHA-256
`7616683dc759fd687d562ccb475d0ea329735d4a231abfb4880ef0df5ebdbb2e`.
It is deliberately unsigned: all five approval slots remain pending and production activation is
blocked. The contract is bound to deterministic candidate header
`0x98cd5590…ce03` and identity artifact SHA-256 `d1c9f8ba…c4ec`; this candidate is not
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
