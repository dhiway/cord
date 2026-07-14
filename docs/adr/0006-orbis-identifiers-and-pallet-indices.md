# ADR 0006: Orbis identifiers, locations, and pallet-index bands

- Status: accepted
- Date: 2026-07-11

## Decision

Orbis parachain and token-network ID is `1006`; its canonical relay-relative XCM location is
`Parent/Parachain(1006)`. The chain protocol ID is `orbis`. The EVM chain ID is reserved as
`420001006` to avoid treating the parachain ID as a public Ethereum chain ID.

Runtime pallet indices are append-only within these bands:

| Band | Purpose |
|---:|---|
| `0..=39` | system, consensus, payment, XCM |
| `40..=49` | generic utilities |
| `50` | Coretime Broker (fixed by the relay's callback encoding) |
| `51..=79` | existing CORD entity/token/register functionality |
| `80..=89` | assets (local 80-82, foreign 83/85, pool 84/86, uniques 87, NFTs 88, rates 89) |
| `90` | People/People-Lite compatible identity and aliases |
| `91..=109` | frozen Orbis identity/application map, including Revive at 100 |
| `110` | Orbis Storage transaction storage and proof inherent |
| `111..=124` | frozen Orbis Storage, payment-policy, storage-service and issuance map |
| `125..=129` | reserved application extensions |
| `200..=214` | application asset and payment extensions (conversion 200, asset payment 201) |
| `215..=239` | meta-tx, fee policy, safety controls |
| `249` | migrations |
| `255` | Sudo |

## Frozen completion map

This map is the sole normative assignment source within the broad bands above.

Manifest version 1 freezes the following previously reserved application indices. Adding,
removing, or reassigning an entry requires Architect/Critic replanning, an ADR update, and a
`docs/orbis-completion-manifest.toml` version increment.

| Index | Pallet |
|---:|---|
| `51` | Orbis Token compatibility fork |
| `52` | Orbis Register compatibility fork |
| `53` | Orbis Entity compatibility fork |
| `54` | Orbis Feeless compatibility fork |
| `96` | Resources |
| `97` | Score |
| `98` | Game |
| `99` | Honour |
| `100` | Revive |
| `101` | Proof of Ink |
| `102` | Coinage |
| `103` | Airdrop |
| `104` | Storage Initialization |
| `110` | Orbis Storage Transaction Storage |
| `111` | Orbis Storage HOP Promotion |
| `112` | PGAS |
| `113` | PGAS Allowance |
| `114` | Alias Accounts |
| `115` | Origin Restriction |
| `116` | DOTNS Gateway |
| `120` | Storage Provider |
| `121` | Drive Registry |
| `122` | S3 Registry |
| `123` | Vesting |
| `124` | Claims |

Indices `200+`, `249`, and `255` remain protocol/operations space and are unchanged.

## Ownership and scope lock

Token, Register, Entity, and Feeless are byte-compatible Orbis-owned packages below
`origin/orbis/pallets/`; Origin and CORD retain their existing packages and consumers. Unmodified
SDK pallets remain pinned dependencies rather than local forks. Modified Individuality, Orbis Storage,
or Web3 Storage behavior belongs below `origin/orbis/pallets/` with source revision, license,
adaptation notes, and upstream test provenance.

Orbis retains only Sudo/root administration. Staking, nominations, elections, public governance,
MobRule governance, redundant MembersSubscriber, production bridges/frontends, full Ethereum
parity, tokenomics redesign, live migration, and production launch are excluded. The completion
manifest is the finite pallet/API/node/benchmark/migration/E2E/exclusion ledger; unlisted Web3
Storage code is not implicitly in scope.

Indices must never be reused after a released runtime. Assets, People, Revive, and Orbis Storage receive
explicit indices and metadata snapshot tests before their first release.

Score and Honour became native at indices 97 and 99 in spec 29. Both are Orbis-owned forks of
Individuality Community `28b7d07dab05bbd05f6b664278b5c83841e212d3`, retain Apache-2.0
provenance, and declare storage version 1. Score's enterprise manager is a named genesis-backed
account rotatable only by root; manager calls accept that account or root. Honour binds directly
to Orbis Members and Timestamp. Neither introduces staking, treasury, or governance origins.

Broker index `50` is a protocol constraint, not a local layout preference: the SDK relay Coretime
pallet encodes `notify_core_count`, `notify_revenue`, and reservation callbacks to pallet `50` on
the configured broker chain. Orbis therefore places Entity at `53` before its first release.
