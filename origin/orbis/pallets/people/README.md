# Orbis People pallet

This Commons-owned pallet provides the chain's native identity, registrar judgement, sub-account,
and username state. It is derived from the CORD/SDK identity implementation, which in turn tracks
upstream FRAME Identity, but is physically owned here so Orbis changes do not mutate CORD pallets.
The imported source snapshot is CORD commit `0af25278` (`pallets/identity`).

Compatibility rules:

- keep the `People` runtime pallet name and index `90` stable;
- preserve storage prefixes, call indices, events, and SCALE types unless a runtime migration is
  included in the same release;
- port applicable fixes from upstream FRAME Identity before adding Orbis-specific behavior;
- use Root/Sudo for registrar and username-authority administration; do not add public governance.
