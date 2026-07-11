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
| `50..=79` | existing CORD entity/token/register functionality |
| `80..=89` | assets |
| `90..=99` | People identity adapters and pallets |
| `100..=109` | Revive and Solidity support |
| `110..=129` | Bulletin durable storage and proof support |
| `200..=239` | meta-tx, fee policy, safety controls |
| `249` | migrations |
| `255` | Sudo |

Indices must never be reused after a released runtime. Assets, People, Revive, and Bulletin receive
explicit indices and metadata snapshot tests before their first release.
