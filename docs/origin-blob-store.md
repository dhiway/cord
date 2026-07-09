# Origin Blob Store (initial spec)

This document defines the initial on-chain API and state machine to host Walrus-like blob
coordination inside the Origin Hub system runtime (`cord/origin/hub/system`).

## Goals (initial)

- Track blob metadata and retention state on-chain.
- Use Substrate-native signatures (`MultiSignature`) for user authorizations.
- Support registration of Publisher and Aggregator entities (signing keys).
- Implement auto-renew retention with a grace window:
  - **Period**: 30 days (epoch-like, block-derived)
  - **Grace**: 60 days (2 periods)
  - **Reads**: allowed during grace
  - **Writes**: blocked for owners in grace
- Start with an offchain **local filesystem** blob backend; the chain does not store blob bytes.

## Defaults (6s block time)

- `PeriodLengthBlocks = 432_000` blocks (30 days)
- `GracePeriods = 2` periods (60 days)

## Roles

Two operator roles exist:

- **Publisher**: accepts user-signed authorizations and registers blobs on behalf of users, then
  confirms successful storage.
- **Aggregator**: performs archival actions after governance authorizes archival of expired blobs.

Roles are registered on-chain by Root for the initial phase.

## Authorization (user-signed)

Publishers may call `register_blob_by_publisher` only with an authorization signed by the blob
owner. The authorization payload is SCALE-encoded and must include a trailing `valid_until` block
number (little-endian `u32`) to enable TTL checking.

The payload must bind at least:

- publisher account
- blob id
- root hash
- size
- encoding
- nonce (replay protection)
- valid_until (TTL reference block, as the final field)

The pallet stores a short hash of `(account, payload, signature)` to prevent replay.

## Blob lifecycle (high level)

`Registered -> Active -> Grace -> Expired -> ArchiveAuthorized -> Archived`

- `Registered`: metadata exists; publisher has not confirmed storage yet.
- `Active`: storage confirmed; renewals are charged at each period boundary.
- `Grace`: renewal failed; reads remain allowed but **writes by this owner are blocked**.
- `Expired`: grace elapsed without successful renewal; eligible for archival.
- `ArchiveAuthorized`: Root has authorized archival.
- `Archived`: Aggregator confirms bytes were archived/removed in the offchain backend.

## Governance

Root is the governance origin for:

- archival authorization of expired blobs
- pricing updates (when implemented)

