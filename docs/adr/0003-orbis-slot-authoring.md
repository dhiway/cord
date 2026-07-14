# ADR 0003: Slot-based elastic authoring

- Status: accepted
- Date: 2026-07-11

## Decision

Orbis uses Aura slot-based authoring with `AllowMultipleBlocksPerSlot = true` and the Cumulus
fixed-velocity consensus hook. It keeps six-second Aura/Origin relay slots, advertises a target
block rate of three, builds one relay parent behind the relay tip, and accepts an unincluded segment
of twelve blocks. This enables the pinned SDK's multi-block collation bundles and targets an
effective two-second Orbis block interval when all three subscribed cores are scheduled. The
runtime exposes scheduling V3, relay-parent-offset, target-block-rate, and unincluded-segment APIs.
Relay configuration enables Assignments V2, ElasticScalingMVP, CandidateReceiptV2/V3, scheduler
lookahead three, and candidate depth/ancestry six.

A compile pass is not the acceptance gate. Before application-pallet completion, a local network
must demonstrate more than one Orbis candidate backed per relay block with two collators. The final
performance gate is five controlled one-core and three-core runs, with at least 2.4x finalized
sustained throughput, coefficient of variation at most 10%, and steady-state parachain finality lag
at most 15 parachain blocks.
