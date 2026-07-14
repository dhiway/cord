# ADR 0019: Application publication and discovery

## Status

Accepted for the first Origin SDK application platform release.

## Context

Commons Names can own a human-readable name and bind validated content commitments. Static
application manifests and bundles can be stored through native storage. Search, ranking and opt-in
catalog presentation do not need to become ownership state, and no current journey proves that a
second on-chain publication registry is a security invariant.

## Decision

- `OriginAppManifestV1` is content-addressed; Names stores only its canonical commitment.
- Name ownership, transfer, controller authority, content updates and revocation remain native Names
  operations.
- The first discovery surface is a reconstructible off-chain index of finalized Names/application
  events. It may store publication visibility, categories and ranking inputs, but it is never an
  ownership or content authority.
- Publication and retraction are SDK/index operations tied to the current finalized name authority.
  A registry contract and deployment contract are forbidden.
- A bounded Names role extension may be proposed only if the delegated-publisher journey proves that
  existing controllers are too broad. An app-registry pallet requires a new ADR with two journeys or
  a consensus security invariant.

## Consequences

Applications can publish, resolve and launch without a contract or new pallet. The index can be
rebuilt, replaced or operated privately. Search failure cannot change the canonical name or content
commitment. Runtime growth remains evidence-driven rather than copied from another network.
