# Orbis provenance

Vendored from `paritytech/individuality` commit `28b7d07dab05bbd05f6b664278b5c83841e212d3`.
This is the authoritative full-personhood application pallet; `pallet-orbis-people` remains the
public identity/registrar/username pallet. Personhood management is Root/Sudo-only and membership
roots are stored by native Orbis `Members`.

The `sp_core::twox_64` import is mapped to `sp_io::hashing::twox_64` for CORD SDK
`release-v1.24.0`; transaction tags are byte-for-byte unchanged.
