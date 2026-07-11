# ADR 0004: Origin core assignment and subscription control planes

- Status: accepted
- Date: 2026-07-11

## Decision

Origin keeps two distinct Sudo-administered control planes:

1. Relay `Coretime.assign_core` is the direct protocol-validation and emergency assignment path.
2. Origin Hub `Broker` is the subscription lifecycle path for allocation, renewal, change, and
   release.

A direct relay assignment is not evidence that Broker subscription works. Orbis acceptance tests
must exercise both paths and prove simultaneous assignment to three cores. Neither path requires
staking, bonding, elections, referenda, or councils; enterprise Sudo/root is the authority.
