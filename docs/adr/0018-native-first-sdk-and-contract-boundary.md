# ADR 0018: Native-first Commons SDK and contract boundary

## Status

Accepted for the new Foundation/Commons network.

## Context

Commons already composes the shared identity, personhood, attestation, Names, Resources, durable
storage/provider, Drive/S3, asset, sponsorship and Revive capabilities required by emerging
applications. The new network has no predecessor data or ABI compatibility obligation. Recreating
those authorities in application contracts would produce duplicate state, ambiguous upgrades and a
worse developer surface.

## Decision

1. Commons pallets and runtime APIs are the only canonical authorities for shared identity,
   personhood, attestation, names, resources, storage/provider, assets and sponsorship.
2. Origin SDK domain packages bind metadata-generated calls and finalized reads directly to those
   native capabilities. They must not expose contract addresses, ABIs or native-versus-contract
   selection.
3. Hosts own product identity, endpoint policy, device permissions, accounts, active signing
   consent, local storage, preimage transport and statement transport.
4. Off-chain services own content delivery, indexing, search, ranking and notifications. On-chain
   state contains bounded commitments and authorization only.
5. `pallet_revive` remains available through the optional `origin-sdk-contracts` leaf for
   application-owned business logic. The umbrella and native domains cannot depend on it.
6. A new pallet is admitted only when at least two concrete app journeys require the same canonical
   transition or a security invariant cannot be enforced outside consensus. SDK convenience is not
   sufficient justification.

## Consequences

The stack has one authority per shared domain and a smaller default bundle. Contract deployment is
not required to use Commons. App teams retain an explicit programmable extension path, but they own
its lifecycle. Because this is a clean-break network, replaced contract facades and duplicate
fixtures are deleted rather than deprecated or migrated.
