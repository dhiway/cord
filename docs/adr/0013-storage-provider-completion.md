# ADR 0013: Storage and provider completion

- Status: accepted
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Orbis composes TransactionStorage 110, HopPromotion 111 and Resources. Soft temporary authorization and renewed hard capacity are distinct. The new network also needs one native provider reference on an isolated reservation plus bounded Provider, Drive and S3 registries.

## Decision

Keep TransactionStorage as the sole on-chain content-commitment/provenance ledger. Its first-network schema is storage version 8 and includes a separate optional reservation-to-provider-agreement map. A reservation owner may attach one active, sufficiently sized native agreement before storing content; the reference survives closure for tombstone audit and is removed with the tombstone. There is no V7 state upgrade, backfill or migration hook. Add bounded zero-stake, Sudo-authorized Provider, Drive and S3 registries.

The chain stores commitments, authorization, capacity, renewal/deletion state and provider references. CID/DAG-PB/UnixFS reconstruction, Bitswap/gateway transport and private content remain off-chain. Provider accountability includes health/capacity, failure, re-replication and audit events. Every cleanup/expiry path is bounded and cursor-driven.

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

Complete provider services, the P4 SDK slice and feature-complete reference journeys before the consolidated production-readiness campaign.
