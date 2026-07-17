# ADR 0013: Storage and provider completion

- Status: accepted
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Commons composes Resources with bounded Provider, Drive and S3 registries. Resource claims remain application entitlements; provider agreements, manifests, checkpoints, object metadata and content commitments belong to the native storage-provider plane. The removed transaction-retention and hop-promotion implementation is not part of the new network.

## Decision

Provider, Drive and S3 are the only on-chain storage authorities. Provider owns governed zero-stake providers, agreements, manifests, checkpoints, challenges and deletion acknowledgement. Drive and S3 own bounded application metadata. Resources may issue storage claims, but its long-term storage adapter fails closed until it is explicitly bound to the canonical Provider authority; it must never fabricate a second reservation ledger.

The chain stores commitments, authorization, capacity, renewal/deletion state and provider references. CID/DAG-PB/UnixFS reconstruction, Bitswap/gateway transport and private content remain off-chain. Provider accountability includes health/capacity, failure, re-replication and audit events. Fixed-finalized checkpoint duties use the dedicated quorum/confirmation/publication lanes, while finalized manifest-deletion duties produce idempotent `StorageProvider::acknowledge_manifest_deletion` requests through the metadata-derived signer/nonce/finality pipeline. Generic provider-root and agreement-deletion calls are not part of the Commons runtime or developer contract.

Until the atomic object API cutover completes, authenticated `POST /commit` and `POST /delete` route identities remain reserved and return unavailable before body parsing or byte-store mutation. Pending root/deletion journal structures remain private, dormant implementation bearers only; no production route or worker drains them. The cutover must either replace the mutation and completion unit atomically or delete these bearers. It must not publish a duplicate generic checkpoint, root, retention, or deletion protocol.

## Drivers

One ledger; bounded proof/state; provider accountability; transport separation.

## Alternatives

Second storage authority (rejected); contract registry (rejected); on-chain content bytes (rejected); permissionless staking/tokenomics (out of R1).

## Authority and security

Node/provider services become release-critical. Commons starts directly with the Provider, Drive and S3 schemas; no removed retention-plane state or compatibility route exists.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Checkpoint and challenge missing/late/invalid/duplicate/fork/restart/disk scenarios; provider authorization/provenance/renew/delete APIs; CID integrity/failover; E/Q/C resource/headroom gates.

## Reversal

Before activation remove planned additions. After activation forward-fix the native schema; do not re-enable storage contracts or duplicate ledgers.

## Follow-ups

Complete the P4 atomic object-completion cutover, provider services, SDK slice and feature-complete reference journeys before the consolidated production-readiness campaign.
