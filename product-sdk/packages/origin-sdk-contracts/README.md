# `@cord-network/origin-sdk-contracts`

Explicit optional access to Commons `pallet_revive` for application-owned business logic. The
package is not an umbrella dependency and rejects contracts claiming to implement canonical name,
identity, personhood, attestation, storage, provider, asset, payment, or sponsorship authority.

Applications supply a reviewed ABI definition and generated codecs. Runtime adapters own Revive
dry-run, call, and instantiate encoding from generated Commons metadata; applications never supply
pallet indices or RPC endpoints.
