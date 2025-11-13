# Registers

`pallet-register` lets you publish typed registries where each entry (packet) follows a schema. The `RegistryBlueprint` builder encodes every input required by `Register::create_registry`: an info blob, attribute schema, the lookup/template specs, and the token derivation logic. By keeping that logic in one place you can reason about how schema changes cascade into packet creation and lookups.

Inside the walkthrough we submit the registry extrinsic, wait for the `RegistryCreated` event, and immediately log the resulting token. The event confirms that the maintainer (resolved from the signer’s entity token) now controls the registry, and the builder guarantees that all lookup keys obey the "non-optional" constraint enforced by the pallet. The blueprint is reused later when we prepare packet attributes, so the example doubles as a regression harness for validating schema churn.
