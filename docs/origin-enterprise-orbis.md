# Origin enterprise network and Orbis

Origin is configured as a permissioned enterprise relay network. It does not use staking or
referenda to manage validators:

- `Sudo` is the administrative root.
- `AuthorityManager` is the session manager and accepts only root calls.
- A validator stages session keys with `session.set_keys` and is added with the Sudo-wrapped
  `authority_manager.nominate` call. Removal uses the Sudo-wrapped `authority_manager.remove`
  call and takes effect at session rotation.
- No bond, nomination, election, or staking transaction is required.

## Elastic scaling

The Origin host configuration enables Assignments V2, the elastic-scaling MVP, and candidate
receipt V2/V3. Scheduler lookahead and async-backing depth are both set to three candidates.

Origin validators enable the experimental collator protocol by default. For an explicit mixed
rollout it can be disabled with:

```text
origin --validator --experimental-collator-protocol=false
```

An optional validator-side reputation persistence interval is available:

```text
origin --validator --collator-reputation-persist-interval 600
```

## Orbis identity

Orbis uses parachain ID `1006`. The Origin runtime exposes its XCM location and trusts ORGN
teleports between Origin and Orbis. Registration remains an explicit Sudo operation so the Orbis
genesis head and validation code are supplied from the exact Orbis build being deployed.

The `origin-hub` container binary resolves both Origin Hub and Orbis runtimes. Development and
local Orbis specs are selected with `--chain orbis-dev` and `--chain orbis-local`, respectively.
For example, a local authority collator can be started with:

```text
origin-hub --chain orbis-local --collator --alice -- --chain origin-local
```

Orbis uses Aura with multiple blocks per slot and async backing. Collator membership is an
enterprise operation: `CollatorSelection` accepts updates only from root, permissionless
candidacy is disabled (`MaxCandidates = 0`), and the candidacy bond is zero. Sudo-managed
invulnerables and their session keys therefore determine the active collator set without staking,
elections, referenda, or councils.

## Assets and Solidity

Orbis local assets use compact `u32` identifiers and pallet index `80`. Signed accounts may create
assets by reserving the configured deposit; Sudo/root retains force-management authority. Revive
is at pallet index `100`, accepts Solidity/EVM bytecode, maps AccountId32 accounts automatically,
and uses enterprise EVM chain ID `420001006`. The ERC-20 precompile prefix for Orbis assets is
`0x0120`.

Revive's benchmark fixtures require the `resolc` compiler. Compile-only benchmark validation may
use the upstream-supported escape hatch:

```text
SKIP_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 \
  cargo check -p origin-orbis-runtime --features runtime-benchmarks
```

This escape hatch is not valid for the Solidity fixture acceptance suite; that suite must install
`resolc`, compile the fixture, deploy it, and prove that a contract changes Orbis asset state.

Direct relay `Coretime.assign_core` calls are useful for protocol validation. Enterprise core
subscription lifecycle tests must separately exercise the Origin Hub Broker path for allocation,
renewal, change, and release.
