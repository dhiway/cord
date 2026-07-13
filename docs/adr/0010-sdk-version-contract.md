# ADR 0010: SDK and runtime-version contract

- Status: proposed
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

The source graph is pinned to Dhiway SDK `release-v1.24.0@cc190ea83c590b6a14a6b9771ab02c81618dc118`. Orbis is spec 29 / transaction version 8. Rust uses Subxt dynamically; the planned TypeScript Product SDK uses generated PAPI descriptors. Independent clients need one semantic contract and fail-closed drift behavior.

## Decision

Runtime metadata and versioned runtime APIs are exact truth. `docs/sdk/compatibility-manifest.json` binds supported genesis/spec/transaction versions and client failure policy. `docs/sdk/signed-extension-manifest.json` freezes every reachable transaction surface, actor, payer, nonce owner and ordered metadata identifier.

A behavior-only runtime change increments spec version. A SCALE call, signed-extension, or transaction encoding change increments transaction version. Both clients regenerate from the same metadata, execute the same JSON/SCALE vectors, and reject an unknown version, metadata/descriptor hash, missing/reordered extension, or cross-block composite read. Product APIs expose typed native operations, never raw SCALE or migrated-domain ABIs.

## Drivers

One dependency graph; deterministic signing; independent client parity; explicit drift.

## Alternatives

Pin generated clients forever (unsafe drift); TS-through-Rust/WASM (deferred unless measured); best-effort unknown-version decoding (rejected).

## Authority and security

Descriptor regeneration is release work. Dynamic Rust compatibility does not permit semantic guessing. The current JSON manifests remain proposed until named owners sign them.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Runtime extension metadata-order test, Rust/TS vector suites and an induced-drift CI fixture must pass. Current semantic vectors are indexed rather than copied.

## Reversal

Rollback to a prior supported runtime/SDK bundle only before activation; after activation use a forward fix under the same fail-closed rules.

## Follow-ups

Ratify manifests; build Product SDK generators; generate current spec-29/tx-8 SCALE vectors and descriptor hashes.
