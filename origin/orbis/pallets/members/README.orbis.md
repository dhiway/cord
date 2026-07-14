# Orbis provenance

Vendored from `paritytech/individuality` commit `28b7d07dab05bbd05f6b664278b5c83841e212d3`.
Orbis maps privileged membership maintenance to Root/Sudo and uses its native Chunks Manager.
The upstream `README.md` describes the ring-membership protocol and remains authoritative.

SDK compatibility adaptations replace newer `frame_support::error::BadOrigin` and
`sp_core::twox_64` paths with their `release-v1.24.0` equivalents; protocol behavior is unchanged.
