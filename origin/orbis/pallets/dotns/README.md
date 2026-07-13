# Native Orbis DotNS

This crate is the CORD-owned, native DotNS state machine intended for Orbis pallet index `116`.
It deliberately has no Solidity ABI, Revive contract caller, dispatcher address, ERC-721 facade,
pricing, escrow, refunds, legacy state import, or compatibility API.

The first storage version starts empty. Names are derived from the Orbis genesis hash, their
parent identifier, and an ASCII-only lowercase label. Registration is commit/reveal; ownership,
controllers, resolver records, reservations, expiry, renewal, transfer, reverse names and
emergency controls are bounded FRAME state.

Resolver values remain references rather than duplicated domain state. Runtime-configured O(1)
validators reject unknown subject commitments, non-live attestations, and unknown content
commitments before a name record is changed. Orbis wires the subject and attestation validators to
the native attestation pallet and the content validator to the Bulletin transaction ledger.
