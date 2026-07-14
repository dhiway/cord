# Origin token pallet

`pallet-origin-token` is the single CORD-owned token implementation shared by the Foundation and
Commons runtimes. There is no Orbis fork, compatibility facade, or duplicate state machine.

Call indices, storage prefixes, storage versions, SCALE types, tests, benchmarks, and weights are
owned here. Runtime-specific policy belongs in Foundation or Commons composition, not in a copied
pallet. Changes to the shared surface must be recorded in `docs/orbis-completion-manifest.toml`.
