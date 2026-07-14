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
receipt V2/V3. Scheduler lookahead is three; async-backing candidate depth and ancestry length are
six so the three-core authoring pipeline has enough relay history during handover and fork races.

Origin keeps its six-second relay slot. Orbis uses a relay-parent offset of one, a target block rate
of three, and unincluded-segment capacity twelve. With three assigned cores the slot-based node
targets three Orbis blocks per Origin slot (an effective two-second block interval). The pinned SDK
already carries versioned multi-block `ParachainBlockData` and node-side block-bundle validation, so
this is enabled through runtime APIs rather than by copying an out-of-envelope SDK patch.

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

The `origin-omni-node` system-chain binary resolves only the Orbis runtime. Development and local
Orbis specs are selected with `--chain orbis-dev` and `--chain orbis-local`, respectively.
Build the production-shaped binary and materialize a raw development spec with:

```text
cargo build --release -p origin-omni-node
target/release/origin-omni-node build-spec --chain orbis-dev --raw \
  --disable-default-bootnode > orbis-dev-raw.json
```

The generated spec identifies relay chain `origin-dev`, parachain `1006`, and embeds the Orbis
WASM runtime. The preset pre-funds Revive's code-deposit account with one existential deposit so
the first Solidity code-upload hold has a valid destination.

### Clean production genesis

The live aliases fail closed: `--chain origin`, `--chain origin-relay`, and `--chain orbis` never
fall back to Alice/Bob or a local chain. Deterministic evidence uses the explicit candidate schemes,
which always emit non-live `Local` specs:

```text
target/release/origin build-spec \
  --chain origin-candidate:docs/genesis/origin-launch-input.candidate.json --raw \
  --disable-default-bootnode > origin-candidate-raw.json

target/release/origin-omni-node build-spec \
  --chain orbis-candidate:docs/genesis/orbis-launch-input.candidate.json --raw \
  --disable-default-bootnode > orbis-candidate-raw.json
```

Production schemes accept reviewed input only when the embedded five-owner launch envelope verifies:

```text
target/release/origin build-spec \
  --chain origin-production:/secure/reviewed-origin-genesis.json --raw \
  --disable-default-bootnode > origin-raw.json

target/release/origin-omni-node build-spec \
  --chain orbis-production:/secure/reviewed-orbis-genesis.json --raw \
  --disable-default-bootnode > orbis-raw.json
```

Origin input contains `root_key`, at least four `validators`, and `endowed_accounts`. Each
validator supplies exact lowercase `0x` public values for `account_id`, BABE, GRANDPA, parachain
validator, parachain assignment, authority-discovery and compressed BEEFY keys. Accounts and
session keys must be unique, and root/validator accounts must be explicitly endowed.

Orbis input contains the exact live Origin `relay_chain` id, `token_network_id`, `root_key`, at
least two fixed collator account/Aura pairs, explicit endowments, and a separate explicit feeless
list. Well-known development identities are rejected by both builders. Neither input accepts a
checkpoint, contract deployment, legacy state, client-compatibility flag or migration source.
Bare JSON with `chainType: Live` is rejected, so a path cannot bypass the production scheme. Final
production files and their genesis hashes remain operator approval artifacts and are not invented
or promoted from the checked-in placeholder authority keys.

For example, a local authority collator can be started with:

```text
origin-omni-node --chain orbis-local --collator --alice -- --chain origin-local
```

Orbis uses Aura with multiple blocks per slot and async backing. Collator membership is an
enterprise operation: `CollatorSelection` accepts updates only from root, permissionless
candidacy is disabled (`MaxCandidates = 0`), and the candidacy bond is zero. Sudo-managed
invulnerables and their session keys therefore determine the active collator set without staking,
elections, referenda, or councils.

## Assets and Revive

Orbis local assets use compact `u32` identifiers and pallet index `80`. Signed accounts may create
assets by reserving the configured deposit; Sudo/root retains force-management authority. Revive
is at pallet index `100`, accepts Solidity/EVM bytecode, maps AccountId32 accounts automatically,
and uses enterprise EVM chain ID `420001006`. The ERC-20 precompile prefix for Orbis assets is
`0x0120`.

Revive's separate PolkaVM benchmark fixtures require the `resolc` compiler. Compile-only benchmark
validation may use the upstream-supported escape hatch:

```text
SKIP_WASM_BUILD=1 SKIP_PALLET_REVIVE_FIXTURES=1 \
  cargo check -p origin-commons-runtime --features runtime-benchmarks
```

The CORD tree does not ship contract bytecode fixtures. Revive remains generic infrastructure for
explicit future applications; all Origin/Orbis platform capabilities use pallets and runtime APIs.

### Unified native identity-bound audit flow

`native_identity_attestation_name_asset_and_storage_journey` proves the clean-break application
path without a migrated-domain contract or ABI. It composes People identity, native Attestation,
an Assets transfer, native storage commitment, Orbis Names resolution, Drive root reference and S3
object reference against the same audit digest. No migrated-domain contract or duplicate authority
is retained.

## People identity

Orbis exposes its SDK-compatible People/People-Lite slice as `People` at pallet index `90`. It is
adapted onto CORD's maintained identity pallet rather than linking Individuality Community's older
FRAME graph. Accounts may self-publish bounded identity information and manage lightweight aliases
and subaccounts. Only Sudo/root can add registrars or username authorities and forcibly remove an
identity; no council, referendum, deposit, or stake is required for administration.

Application authorization should require the configured registrar judgement when a verified
person is needed; merely publishing self-claimed display data is not equivalent to an attestation.

## Durable Bulletin storage

`TransactionStorage` is at pallet index `110`. It is adapted from Polkadot Bulletin Chain revision
`b6c2827d2326` onto CORD's SDK graph and provides bounded authorized storage, BLAKE2/CID content
lookup, retention and renewal accounting, a permanent-storage cap, transaction indexing, proof
inherents, and both the SDK transaction-storage API and Bulletin authorization query API.

Sudo/root manages authorizers. An authorizer grants an account explicit transaction and byte
allowances before signed `store` or `renew` calls are accepted. The transaction extension consumes
the authorization before dispatch, so a failed call cannot reuse its allowance. Storage mutations
must be direct extrinsics: Utility-wrapped mutations and XCM `Transact` storage mutations are
rejected recursively. Root remains an emergency direct authorizer and storage origin.

The initial development limits are 128 indexed transactions per block, 256 KiB per transaction,
16 GiB total permanent storage, and 14 days for an authorization. These are safety limits, not
production storage economics. The Orbis node must keep the transaction-storage inherent provider
enabled; once retained data reaches its proof window, a block missing the expected proof is invalid.

## Multi-core test topology

`zombienet/testnet.toml` and `zombienet/testnet-elastic.toml` run two Sudo-selected Orbis
invulnerables (`Alice` and `Bob`) with slot-based authoring. The relay test configuration uses six
validators and exposes three schedulable cores with one validator per core; all three are assigned
to task `1006` through the bootstrap/Broker sequence below. A single-collator topology is not valid
elastic-scaling evidence.

The 2.4-times acceptance campaign must compare one and three reservations with the same two
collators, validator set, workload, state, warm-up, and duration. Run five interleaved repetitions
per core count, report median finalized successful calls per second and coefficient of variation,
and reject a run set above 10% variation or 15 parachain blocks of steady-state finality lag.

After launching the standard topology and submitting its bootstrap assignment, verify relay and
Orbis best-block/finality progress plus the relay claim queue without external Python packages:

```text
python3 zombienet/orbis_smoke.py --expected-cores 3 --expected-block-rate 3
```

The verifier waits through initial session activation, requires both chains' best and finalized
heights to advance, checks Orbis's target-block-rate and relay-parent-offset runtime APIs, and
requires task `1006` to appear on at least three distinct claim-queue cores. Its default short
smoke gate also requires at least 2.4 Orbis best blocks per Origin best block during the measurement
window; this is a protocol sanity check, not a replacement for the five-run workload benchmark.

## Core allocation

Orbis replaces Origin Hub as Origin's system-chain Coretime Broker. The relay runtime authorizes
parachain `1006` as its sole `BrokerId`; Orbis contains `pallet_broker` at SDK-required index `50`
with Sudo/root administration.

Origin Sudo may directly assign Orbis its three bootstrap/test cores before starting Orbis. Once
Orbis is authoring, its Broker requests the required relay core count and sends assignments over
privileged XCM. A full-core workload is `Task(para_id)` with all `57,600` parts. Reserving that
workload multiple times assigns multiple cores to the same parachain; three reservations for task
`1006` are the Orbis three-core elastic-scaling configuration.

For a development network, submit the Sudo bootstrap assignment from the repository root:

```text
cargo run -p origin-rs --example bootstrap_orbis_core -- \
  --endpoint ws://127.0.0.1:9900 --first-core 0 --cores 3
```

The example defaults the assignment start to the current best relay block plus two and submits all
three `Coretime.assign_core` calls as one Sudo-wrapped `Utility.batch_all`, so a failure cannot leave
a partial core set. It waits for finalization. Assigning multiple cores with this relay-admin tool
is an emergency/test override; it is not an Orbis Broker subscription.

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
