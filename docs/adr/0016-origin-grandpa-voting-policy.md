# ADR 0016: Origin GRANDPA vote-target tuning boundary

- Status: rejected for release; isolated implementation patches retained as evidence, not applied
- Date: 2026-07-13
- Extends: ADR 0015

## Context

The stopped one-core P1 diagnostic is not an acceptance result. After the complete 600-second warm-up it retained 158 one-second measurement samples over 157 seconds. Fifty-eight samples exceeded the 15-Orbis-block ceiling; every breach coincided with relay best-minus-finalized lag 3. The maximum was 18. Orbis Alice also logged exactly 85 `Ran out of free WASM instances` messages, and every one of the six co-located relay authorities failed the SDK reference-hardware check. These are separate confounders and must remain visible.

Origin links `polkadot-service` from the pinned `dhiway/sdk` graph at `cc190ea83c590b6a14a6b9771ab02c81618dc118`. The local `node/cli/src/service.rs` is not the service used by the Origin binary. The pinned SDK composes GRANDPA's default as `BeforeBestBlockBy(2)` followed by `ThreeQuartersOfTheUnfinalizedChain`. Consequently, changing the local service would be ineffective, and changing a consensus primitive or runtime would violate the no-upstream-primitive-forks boundary.

## Decision

Use two sequential, attributable, non-acceptance probes:

1. **Topology hygiene only.** Add `--max-runtime-instances=32` to both Orbis collators. Freeze all other binaries, sources, topology, genesis state, workload, ports and sampling. Stop at the first measurement sample above 15 blocks.
2. **Node-policy probe only if probe 1 breaches.** Retain the hygiene setting and use a single immutable descendant of the pinned SDK that exposes the existing vote-target distance. The SDK default remains exactly 2 and still composes `BeforeBestBlockBy(n)` followed by `ThreeQuartersOfTheUnfinalizedChain`; values are validated in `1..=32`. Origin explicitly selects 1. Stop at the first measurement breach.

Each probe retains its full warm-up and raw one-second measurement samples but makes no throughput, SLO or production-hardware claim. A complete declared probe measurement window with no lag above 15 is only an unlock condition for the ten-run campaign; it is not campaign evidence. A breach immediately rejects that tuning and cannot be averaged away.

The SDK change is coordinated with the default-off proof-fault adapter on one immutable SDK descendant and one dependency graph. All SDK git declarations and lock entries must move atomically to that descendant without version changes. No dependency pin is changed until that reviewed descendant exists.

## Security and architecture constraints

- The SDK default is behavior-preserving at distance 2; non-Origin callers use `Default::default()`.
- The three-quarters safety restriction remains mandatory and is composed explicitly after the distance rule.
- Existing malus behavior can only add a further restrictive rule; it is not repurposed to reduce distance.
- Origin's distance 1 is node-service configuration, not a runtime, chain-spec, storage or consensus-primitive fork.
- The patch changes no pallet, runtime, primitive, transaction format, state or genesis data.
- This new network needs no legacy migration or backward-compatibility path.
- Rollback is a one-line return to the SDK default; unsuccessful probes do not authorize release changes.

## Evidence boundary

Canonical details and patch hashes are in `docs/evidence/performance/p1-grandpa-tuning-decision.json`. The original diagnostic remains invalidated and `performance_claim=false`. Local host/reference-hardware failures prevent production-hardware conclusions even if a local probe stays within the lag ceiling.

## Probe outcome

Both probes breached the absolute 15-block ceiling after a complete 600-second warm-up. The runtime-instance hygiene probe stopped at lag 17 after 4.008009 seconds. The isolated Origin `n=1` probe, retaining that hygiene, stopped at lag 17 after 82.007314 seconds. The latter did not complete its 300-second window, so the campaign remains locked and the isolated patches are not release candidates.

The cross-probe timing difference is not causal evidence: the rebuilt Origin network produced a different relay genesis hash from probe 1. Probe 2 still fails independently against its own absolute ceiling. Its host clock offset was +52.288 ms against the 50 ms gate, and all six relay authorities failed the SDK reference-hardware check. No performance or production-hardware claim is permitted.

## Verification

Run `python3 scripts/test-grandpa-voting-policy-patches.py`. SDK compilation and tests must be rerun on the combined immutable descendant before applying the CORD patch. After build provenance is frozen, execute probe 1 and, only when required, probe 2. A full campaign remains blocked until an independent review accepts the sustained probe and the environment/evidence manifest is complete.
