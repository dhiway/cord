# ADR 0011: Identity, personhood and attestation completion

- Status: accepted for implementation
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

ADR 0007 makes Members the sole local personhood-root authority and rejects a local MembersSubscriber copy. Orbis already composes Entity/Register/Token, People, Members, People-Lite, Personhood, Resources, Score and Honour. Native schema/attestation remains the primary verified gap.

## Decision

Preserve the existing topology and I1-I10 in `docs/architecture/identity-topology-contract.md`.
Add one Commons-owned pallet, `pallet-orbis-attestation`, at append-only runtime index **105**. The
pallet owns both the immutable bounded schema registry and attestation lifecycle so that Orbis does
not acquire competing schema or credential authorities. It provides issuer/subject/schema/status
commitments, direct/delegated/batched issue, expiry and revocation, uniqueness/parent links,
nonce/deadline/replay checks and bounded indexing. Private claims and presentations remain off-chain.

The CORD-local `pallets/schema` and `pallets/statement` implementations remain available to the
existing CORD runtimes but are not composed into Orbis: their ChainSpace/Identifier authority model
is not the clean-genesis Orbis identity topology and adapting them in-place would couple unrelated
runtimes. Shared concepts and tests may be reused, but Orbis owns its implementation below
`origin/orbis/pallets/attestation`.

Cross-pallet links are stable IDs and traits/runtime APIs, never mutable record copies. Composite SDK reads pin one finalized hash or use an explicitly versioned atomic snapshot. Context includes genesis, spec/application, collection, root/context version, payload hash, nonce and expiry. Root rotation has a declared finalized activation/overlap/revocation policy.

## Drivers

Single identity truth; privacy; replay safety; bounded state; atomic reads.

## Alternatives

Replace all existing identity pallets (rejected); retain contract attestations (rejected); store private claims on-chain (rejected).

## Authority and security

Existing pallets require authority and invariant audit rather than replacement. Only admitted Coinage/Game/Proof-of-Ink behavior may enter later.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Domain contract, runtime/API bounds, I1-I10 tests, proof/issuer/delegation/replay negatives, weights and Rust/TS/host conformance. Zero authoritative Revive or ABI surface.

## Reversal

Before activation remove the new pallets. After activation use a forward runtime upgrade; do not restore contract authority.

## Follow-ups

Implement the index-105 pallet, versioned runtime API, Rust/TypeScript SDK slice and cleanup gate.
