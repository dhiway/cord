# ADR 0013: Storage and provider completion

- Status: accepted
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Orbis composes TransactionStorage 110, HopPromotion 111 and Resources. Soft temporary authorization and renewed hard capacity are distinct. The new network also needs one native provider reference on an isolated reservation plus bounded Provider, Drive and S3 registries.

## Decision

Keep TransactionStorage as the sole on-chain content-commitment/provenance ledger. Its first-network schema is storage version 8 and includes a separate optional reservation-to-provider-agreement map. A reservation owner may attach one active, sufficiently sized native agreement before storing content; the reference survives closure for tombstone audit and is removed with the tombstone. There is no V7 state upgrade, backfill or migration hook. Add bounded zero-stake, Sudo-authorized Provider, Drive and S3 registries.

The chain stores commitments, authorization, capacity, renewal/deletion state and provider references. CID/DAG-PB/UnixFS reconstruction, Bitswap/gateway transport and private content remain off-chain. Provider accountability includes health/capacity, failure, re-replication and audit events. The active provider completion contract is checkpoint-v2 plus canonical manifest deletion: fixed-finalized checkpoint duties enter the dedicated quorum/confirmation/publication lanes, while finalized manifest-deletion duties produce idempotent `StorageProvider::acknowledge_manifest_deletion` requests through the metadata-derived signer/nonce/finality pipeline. Generic provider-root and agreement-deletion calls are not part of the Commons runtime or developer contract.

Until P4 completes the atomic object API cutover, authenticated `POST /commit` and `POST /delete` route identities remain reserved and return unavailable before body parsing or DiskStore mutation. Pending root/deletion journal structures remain private, dormant implementation bearers only; no production route or worker drains them. P4 must either replace the mutation and completion unit atomically or delete these bearers. It must not publish a duplicate generic checkpoint, root, or deletion protocol.

## Drivers

One ledger; bounded proof/state; provider accountability; transport separation.

## Alternatives

Second storage authority (rejected); contract registry (rejected); on-chain content bytes (rejected); permissionless staking/tokenomics (out of R1).

## Authority and security

Node/provider services become release-critical. V8 is the clean-genesis TransactionStorage schema, not an upgrade from an existing Orbis network.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Proof missing/late/invalid/duplicate/fork/prune/restart/disk scenarios; authorization/provenance/renew/delete APIs; CID integrity/failover; E/Q/C resource/headroom gates.

## Reversal

Before activation remove planned additions. After activation forward-fix the native schema; do not re-enable storage contracts or duplicate ledgers.

## Follow-ups

Complete the P4 atomic object-completion cutover, provider services, SDK slice and feature-complete reference journeys before the consolidated production-readiness campaign.
