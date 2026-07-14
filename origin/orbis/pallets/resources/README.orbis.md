# Orbis Resources provenance

Ported from `paritytech/individuality` commit
`28b7d07dab05bbd05f6b664278b5c83841e212d3` under Apache-2.0.

The Commons native adaptation retains calls 0–13, modifies call 12 to use atomic
two-phase Orbis Storage reservations, adds calls 15 and 17, and intentionally leaves call indices 14
and 16 unused. Root/Sudo remains the sole manager. Native Orbis Members, Personhood and People Lite
provide membership and consumer integration.

The copied `sp_core::twox_64` calls are mapped to `sp_io::hashing::twox_64` for the pinned CORD SDK
without changing transaction tags. Storage V1 and the isolated reservation protocol are frozen by
manifest version 2.
