# ADR 0014: New-genesis clean break and native cutover

- Status: accepted
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Origin and Orbis are a new network. There are no deployed users, legacy state, stable Solidity APIs or old clients to migrate. Retaining compatibility code would create duplicate authorities, attack surface and maintenance cost.

## Decision

Launch from deterministic new genesis containing only approved native bootstrap state. Do not build legacy exports/imports, checkpoints, dual writes, old-network cutover, ABI/read facades, deployment-address preservation or backward-compatible client shims.

For each migrated domain, delete superseded contracts, interfaces, ABI/type bindings, deployment/proxy/address tooling, contract-only RPC/indexers/SDK wrappers, feature flags, dependencies, fixtures/mocks, CI/package/config/docs and unreachable duplicate state machines in the same delivery phase. Semantic evidence may remain immutable and non-buildable.

Generic Revive survives only for a named unrelated application with an owner, dependency path and test. Speculative reuse and future migration are not valid allowlist reasons. M9 requires zero unowned survivors, callable migrated-domain contracts, deprecated facades and dead product paths.

## Drivers

Clean authority; smaller attack surface; lower maintenance; explicit ownership.

## Alternatives

Compatibility grace period, data migration, dormant legacy modules or deprecation (all rejected); remove Revive globally (separate live-use decision).

## Authority and security

Cutover is intentionally irreversible at product/API level. Future native runtime schema changes still require normal storage-version/try-runtime/forward-fix discipline.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Clean-genesis and native-cutover scanners; metadata/descriptor/package/container diffs; repository symbol/address/selector scans; live-use Revive allowlist audit.

## Reversal

Reversal before genesis means rebuild the plan. After genesis, only a new explicit network/ADR may change the no-legacy premise.

## Follow-ups

Define cleanup validator commands per language/package and complete M6/M7/M9 at P5/P7.
