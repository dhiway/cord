# @cord-network/origin-sdk-resources

Native Commons resource registration, statement allowance, and long-term storage claim helpers.
`profile` combines personhood, allowance, consumer, and statement state at exactly one finalized hash.
Proof-bearing transactions use typed `AsPerson`, `PeopleLiteAuth`, and `AsResources` authorization
objects through the generated Commons transaction adapter.

Permissionless cleanup, off-chain-worker maintenance, forced demotion, and reservation expiry are
explicitly excluded from the application package.
