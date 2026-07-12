# Orbis entity pallet

Orbis-owned compatibility fork of `origin/pallets/entity` at CORD revision
`8d6110d3a279d4206e451fabf550e4919d61e965`. Package:
`pallet-orbis-entity`. License: `GPL-3.0-or-later`.

Slice 0 changes ownership and Rust package identity only. Call indices, storage prefixes, storage
versions, SCALE types and behavior remain byte-compatible with the Origin pallet. Origin and CORD
consumers continue to use the original package; only the Orbis runtime uses this fork.

The complete Origin unit/mock/benchmark source suite was copied with the fork and is the
upstream-compatibility test provenance; Orbis additionally snapshots runtime pallet indices, call
variant metadata, and storage names.

Future changes must be reconciled with the pinned Origin source and recorded in
`docs/orbis-completion-manifest.toml`.
