# ADR 0004: Origin and Orbis core-allocation control planes

- Status: accepted
- Date: 2026-07-11

## Decision

Origin keeps two distinct Sudo-administered control planes:

1. Relay `Coretime.assign_core` is the bootstrap, protocol-validation, and emergency override path.
2. Orbis `Broker` is the sole normal lifecycle path for requesting cores and allocating,
   renewing, changing, and releasing workloads for Orbis and other parachains.

The relay runtime sets `BrokerId = 1006`. Relay Coretime accepts its management calls only from
Origin root or the Orbis parachain origin. Orbis Broker administration uses `EnsureRoot`; no
staking, bonding, elections, referenda, or councils participate.

Origin Sudo directly assigns Orbis one permanent bootstrap core before Orbis begins authoring.
After startup, Orbis Sudo reserves full-core schedules. Repeating a full-core `Task(para_id)`
reservation assigns multiple distinct cores to the same parachain and enables elastic scaling.
Broker sale rotation is configured with `limit_cores_offered = Some(0)`: lifecycle processing is
active, but public purchases have no offered cores. Enabling a non-zero public offer limit is a
future runtime-policy decision.

A direct relay assignment is not evidence that Broker lifecycle allocation works. Acceptance must
exercise both the bootstrap override and Orbis-driven request, reservation, assignment, renewal,
and release paths.
