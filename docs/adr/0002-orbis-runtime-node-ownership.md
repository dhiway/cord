# ADR 0002: Orbis system-chain and node ownership

- Status: accepted; supersedes the earlier sibling-chain decision
- Date: 2026-07-11

## Decision

Orbis is Origin's unified system chain and replaces the Origin Hub runtime. It owns application
capabilities and the sole Coretime Broker. The Origin relay recognizes Orbis parachain `1006` as
`Coretime::BrokerId`; no other parachain may issue Broker-origin core-count or assignment calls.

The existing `origin-hub` omni-node binary may remain as a transitional executable name, but its
canonical built-in specs and runtime are Orbis. The legacy `origin/hub/system` runtime is retired
after Orbis Broker bootstrapping and allocation tests pass.

Orbis must always retain a bootstrap core. If stock omni-node cannot prove slot-based
multi-candidate authoring, V3 descriptors, Bulletin proof-inherent construction, two collators,
restart/sync, and retention, a dedicated Orbis node wrapper becomes mandatory.
