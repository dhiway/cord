# Orbis runtime migration contract

Origin and Orbis are new networks launched from a clean genesis. The runtime therefore has no
single-block predecessor migrations: `Migrations = ()`. Score, Honour, and Orbis Storage Transaction
Storage initialize their current storage layouts directly from genesis.

Do not add compatibility migrations for pre-Origin/Orbis state, contract-backed state, or Orbis Storage
V0–V7 layouts. A future migration is valid only for state produced by a released Origin/Orbis
runtime after genesis. The generic `pallet_migrations` facility remains available for such future
network upgrades; it has no launch-time multi-block migration configured.
