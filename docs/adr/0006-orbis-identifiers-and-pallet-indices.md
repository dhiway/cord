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
| `91..=99` | reserved People adapters |
| `100..=109` | Revive and Solidity support |
| `110` | Bulletin transaction storage and proof inherent |
| `111..=129` | reserved Bulletin extensions |
| `200..=239` | meta-tx, fee policy, safety controls |
| `249` | migrations |
| `255` | Sudo |

Indices must never be reused after a released runtime. Assets, People, Revive, and Bulletin receive
explicit indices and metadata snapshot tests before their first release.

Broker index `50` is a protocol constraint, not a local layout preference: the SDK relay Coretime
pallet encodes `notify_core_count`, `notify_revenue`, and reservation callbacks to pallet `50` on
the configured broker chain. Orbis therefore places Entity at `53` before its first release.
