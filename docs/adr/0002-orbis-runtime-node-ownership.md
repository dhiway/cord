# ADR 0002: Orbis system-chain and node ownership

- Status: accepted; supersedes the earlier sibling-chain decision
- Date: 2026-07-11

## Decision

Orbis is Origin's unified system chain and replaces the Origin Hub runtime. It owns application
capabilities and the sole Coretime Broker. The Origin relay recognizes Orbis parachain `1006` as
`Coretime::BrokerId`; no other parachain may issue Broker-origin core-count or assignment calls.

The canonical `origin-omni-node` exposes only Orbis built-in specs. The superseded Origin Hub
runtime and its duplicate pallets are deleted: Foundation and Commons have one runtime path and one
Origin-stack implementation per shared capability.

Orbis must always retain a bootstrap core. If stock omni-node cannot prove slot-based
multi-candidate authoring, V3 descriptors, Orbis Storage proof-inherent construction, two collators,
restart/sync, and retention, a dedicated Orbis node wrapper becomes mandatory.
