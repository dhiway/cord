# ADR 0002: Orbis runtime and node ownership

- Status: accepted
- Date: 2026-07-11

## Decision

Orbis is a dedicated parachain runtime at `origin/orbis/runtime`. It does not add application load
to the Origin Hub/Coretime Broker runtime. The `origin-hub` omni-node binary owns chain-spec
selection for both Hub and Orbis while the stock runtime resolver remains sufficient.

A dedicated Orbis node wrapper becomes mandatory if the network gate cannot prove all of:
slot-based multi-candidate authoring, V3 descriptors, Bulletin proof-inherent construction, two
collators, restart/sync, and retention. Custom Bulletin runtime APIs and RPCs remain Orbis-owned;
they must not be added to the relay node.
