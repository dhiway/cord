# Orbis provenance

Vendored from `paritytech/individuality` commit `28b7d07dab05bbd05f6b664278b5c83841e212d3`.
Orbis binds People Lite directly to native `Members`; verifier attestation allowances are managed by
Root/Sudo. Consumer registration is initially a no-op until the native Resources pallet is wired.

The `sp_core` hashing imports are mapped to `sp_io::hashing` for CORD SDK `release-v1.24.0`;
encoded payloads and hashing algorithms are unchanged.
