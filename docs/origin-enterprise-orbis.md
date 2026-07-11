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

The `origin-orbis` system-chain binary resolves only the Orbis runtime. Development and local
Orbis specs are selected with `--chain orbis-dev` and `--chain orbis-local`, respectively.
For example, a local authority collator can be started with:

```text
origin-orbis --chain orbis-local --collator --alice -- --chain origin-local
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

## Core allocation

Orbis replaces Origin Hub as Origin's system-chain Coretime Broker. The relay runtime authorizes
parachain `1006` as its sole `BrokerId`; Orbis contains `pallet_broker` at SDK-required index `50`
with Sudo/root administration.

Origin Sudo must directly assign Orbis one permanent bootstrap core before starting Orbis. Once
Orbis is authoring, its Broker requests the required relay core count and sends assignments over
privileged XCM. A full-core workload is `Task(para_id)` with all `57,600` parts. Reserving that
workload multiple times assigns multiple cores to the same parachain; three reservations for task
`1006` are the Orbis three-core elastic-scaling configuration.

Other enterprise parachains are registered by Origin Sudo and receive cores from Orbis Sudo using
the same full-core reservations. Direct relay
`Coretime.assign_core` remains only the bootstrap, test, and emergency override path. Acceptance
must test bootstrap plus Orbis-driven request, assignment, renewal, and release.

### Enterprise allocation sequence

1. Origin Sudo registers Orbis and calls relay `Coretime.assign_core` with one complete
   `Task(1006)` assignment so Orbis can author its first blocks.
2. Orbis Sudo configures Broker with `limit_cores_offered = Some(0)`. Broker rotations are active,
   but no cores can be bought in a public sale.
3. Orbis Sudo submits three `Broker.reserve` calls, each containing one `ScheduleItem` with
   `mask = CoreMask::complete()` and `assignment = Task(1006)`.
4. Orbis Sudo calls `Broker.start_sales(end_price, 0)`. The price is an inert development value:
   with the offer limit at zero, the call starts lifecycle rotation and requests exactly the three
   reserved cores rather than opening a market.
5. After the Broker's two documented sale-period boundaries, it sends full-core assignments for
   cores `0..2` to Origin. Origin accepts them only because the XCM parachain origin is `1006`.
6. To allocate another registered parachain, Orbis Sudo reserves `Task(para_id)` and calls
   `Broker.request_core_count` with the new total reservation/lease count. To release it, Sudo
   calls `Broker.unreserve` with the reservation's current index and requests the reduced count.

Reservation indices are positional and can change after an earlier entry is removed; operators
must read `Broker.Reservations` immediately before an `unreserve`. Core-count changes are staged by
Origin and become active across the relay session boundary; they do not require validator restart
or staking operations.

## Sponsored and fee-free transactions

`MetaTx.dispatch` is a sponsored transaction: the inner signer authorizes the call and owns the
inner nonce, while the outer signed relayer submits the extrinsic and pays ordinary transaction
fees. The relayer does not inherit the inner signer's authority, and a stale nonce or invalid
signature rejects the inner transaction.

The separate zero-fee model is restricted to dispatchables explicitly annotated by Entity and
Register. Root must first add the account to `Feeless`; each account may consume at most 16 such
transactions per block. Orbis uses `ChargeOrSkipFeeless`, which consumes a quota unit during
transaction preparation before skipping payment. Exhausted accounts return to normal fee handling
during validation and cannot consume another skip during preparation.

Utility batches, proxies, multisig calls, Revive calls, asset calls, and other unannotated outer
calls are not fee-free even when they contain or dispatch an annotated call. This deny-by-default
outer-call behavior prevents a batch from multiplying one quota unit into several operations.
