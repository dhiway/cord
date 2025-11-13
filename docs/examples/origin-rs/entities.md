# Entities

Entities hold the descriptive metadata (display name, legal text, URLs, etc.) that gives human context to a signer account. In `builders::build_entity_info` we construct a full `EntityInfo` struct using the same bounded types the runtime expects (`Elum` for values, `Attributes` for dynamic keys). That struct is passed to `Entity::set_info`, meaning you can tweak the helper or feed it with data from fixtures to simulate real organizations.

The example also showcases a resilient approach to entity onboarding: check `Ss58OfActiveAccounts` first, reuse the token if it exists, and only call `set_info` when the storage entry is empty. This keeps developer nodes tidy while still exercising the pallet logic whenever a fresh account is used.
