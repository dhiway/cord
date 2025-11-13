# Identifiers

CORD derives every on-chain identifier from pallet-specific material and the global SS58 checksum rules. Each identifier is encoded as a Base58 string so that humans can visually compare and copy them with minimal friction. In the origin-rs demos (`origin-rs/examples/entity-demo.rs`) we call `Entity::set_info` whenever the signer does not already own an entity token. The emitted `EntityInfoSet` event includes the freshly minted `Ss58Identifier`, which we render inside the CLI output so developers can immediately see which pallet produced the identifier.

Because SS58 identifiers are stable, the walkthrough also reuses the token if it already exists. The storage fetch issued before `set_info` demonstrates how to look up the active token for any account via `Entity::Ss58OfActiveAccounts`. This pattern is the fastest way to assert whether a dev account is ready for downstream flows that expect SS58 identifiers.
