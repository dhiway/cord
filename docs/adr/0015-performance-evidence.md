# ADR 0015: Performance evidence contract

- Status: proposed
- Date: 2026-07-13
- Extends: ADRs 0001-0008

## Context

Orbis currently uses a 6000 ms Aura slot, velocity 3, relay-parent offset 1 and unincluded capacity 12. Three effective Orbis blocks per relay slot could imply 500 extrinsics/block at 250 ext/s, including about 125 storage intents/block at the planned mix, near the current limit of 128. Headline best-block throughput cannot prove finality, storage or content quality.

## Decision

Use three independent classes: E finalized extrinsics, Q finalized-hash reads/subscriptions and C content retrieval. `docs/evidence/performance/service-slo-manifest.json` freezes numeric targets, topology, workload, duration, seed and network/provider conditions; it must be cryptographically ratified before P1. Results conform to `benchmark-result.schema.json`, retain raw failures/outliers and report p50/p95/p99 plus bootstrap 95% CI over five interleaved runs.

Require 3-core/1-core finalized throughput >=2.4x, CV <=10%, lag <=15. Require storage headroom `1-p95(storage_count/block)/limit >= .20`, p95 weight/length/proof/DB <=80% and no observation >90%. E/Q/C denominators include every declared valid request; retries deduplicate rather than erase failures.

## Drivers

Finalized truth; reproducibility; independent bottleneck attribution; capacity headroom.

## Alternatives

Single mixed headline number (rejected); means/max-only reports (rejected); silent outlier deletion (rejected).

## Authority and security

The checked-in service manifest is proposed and unsigned; P1 is blocked until named approvers ratify it. Final complete-domain SLO claims wait for P6.

## Consequences

This decision creates a blocking release contract: implementation and evidence must conform, and any exception requires an explicit ADR revision and independent review.

## Data/API compatibility

Origin and Orbis start from a new genesis. No legacy state import, Solidity ABI/API facade, dual write, deployment-address compatibility, or old-client support is permitted. Only forward runtime upgrades within the new network receive migration support.

## Verification

Schema validation; reproducible one-/three-core runs; raw hash and environment capture; machine threshold verdict; independent verifier.

## Reversal

Change targets/topology only through a new signed manifest that invalidates prior dependent results.

## Follow-ups

Name approvers/signing mechanism; implement E/Q/C clients; reproduce P1 authoring baseline and run final P6 campaign.
