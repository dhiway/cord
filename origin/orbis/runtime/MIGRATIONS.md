# Commons runtime clean-genesis contract

Foundation and Commons are new-network runtimes launched from a clean genesis. Commons therefore has no
single-block predecessor migrations: `Migrations = ()`. Score, Honour, and Orbis Storage Transaction
Storage initialize their current storage layouts directly from genesis.

Do not add compatibility migrations for pre-Origin/Orbis state, contract-backed state, or Orbis Storage
V0–V7 layouts. A future migration is valid only for state produced by a released Origin/Orbis
runtime after genesis. The generic `pallet_migrations` facility remains available for such future
network upgrades; Commons has no launch-time multi-block migration configured.
