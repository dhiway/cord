# Commons-owned pallets

This directory contains pallets and runtime APIs owned by the Commons runtime.

- Keep CORD-generic pallets in the repository-level `pallets/` directory.
- Keep Origin relay-only pallets in `origin/pallets/`.
- Prefer the pinned SDK pallet directly when Orbis requires no source change.
- Vendor or adapt an upstream Asset Hub, People, Orbis Storage, Coretime, or Storage pallet here only
  when Orbis-specific behavior is required.
- Record the upstream repository and commit in the pallet README and in
  `docs/orbis-native-capability-matrix.md`.
- Preserve upstream license headers and keep the crate on the workspace SDK dependency graph.
- Do not introduce staking or governance origins. Required administration must be explicitly
  mapped to Sudo/root and covered by origin tests.

`people/`, `people-lite/`, `personhood/`, `chunks-manager/`, `members/`, `members-notifier/`,
`individuality-support/`, `score/`, `honour/`, `hop-promotion/`, and `transaction-storage/` are
Commons-owned native pallets or explicitly recorded adaptations. Their runtime pallet names and
indices are frozen for the new-network launch contract; they do not imply predecessor-state or
contract compatibility.
