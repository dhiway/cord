# Honour pallet

## Orbis provenance and adaptation

This Apache-2.0 Orbis-owned fork was imported from Individuality Community commit
`28b7d07dab05bbd05f6b664278b5c83841e212d3`. It retains the upstream vote contexts,
ring-proof verification, anti-replay/nullifier handling, point freeze, call mortality, benchmarks,
tests, and conservative weights. SDK hashing calls were aligned to the pinned Orbis SDK through
`sp_io::hashing`; there is no behavioral hashing change.

Orbis binds Honour directly to its native `Members` ring and `Timestamp` clock. The
`VoterAuth` transaction extension is part of every applicable Origin-policy surface and defaults
to `None` where no proof is supplied. A non-member or suspended member cannot construct a valid
proof against the active People ring. Honour has no staking, treasury, elections, or public
governance dependency. Runtime index 99 and storage version 1 are protocol surfaces.
