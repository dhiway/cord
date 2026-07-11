# Orbis-owned pallets

This directory contains pallets and runtime APIs owned by the Orbis system chain.

- Keep CORD-generic pallets in the repository-level `pallets/` directory.
- Keep Origin relay-only pallets in `origin/pallets/`.
- Prefer the pinned SDK pallet directly when Orbis requires no source change.
- Vendor or adapt an upstream Asset Hub, People, Bulletin, Coretime, or Storage pallet here only
  when Orbis-specific behavior is required.
- Record the upstream repository and commit in the pallet README and in
  `docs/orbis-native-capability-matrix.md`.
- Preserve upstream license headers and keep the crate on the workspace SDK dependency graph.
- Do not introduce staking or governance origins. Required administration must be explicitly
  mapped to Sudo/root and covered by origin tests.

`transaction-storage/` is the first moved Orbis-owned upstream pallet. Its Cargo package names are
unchanged so the relocation does not alter runtime metadata or downstream APIs.
