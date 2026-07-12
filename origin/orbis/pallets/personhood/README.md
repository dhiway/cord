# People Pallet

A pallet managing the registry of proven individuals.

## Overview

The People pallet stores and manages identifiers of individuals who have proven their personhood. It
tracks their personal IDs, organizes their cryptographic keys into rings, and allows them to use
contextual aliases through authentication in extensions. When transactions include cryptographic
proofs of belonging to the people set, the pallet's transaction extension verifies these proofs
before allowing the transaction to proceed. This enables other pallets to check if actions come from
unique persons while preserving privacy through the ring-based structure.

The pallet accepts new persons after they prove their uniqueness elsewhere, stores their
information, and supports removing persons via suspensions. While other systems (e.g., wallets)
generate the proofs, this pallet handles the storage of all necessary data and verifies the proofs
when used.

## Key Features

- **Stores Identity Data**: Tracks personal IDs and cryptographic keys of proven persons
- **Organizes Keys**: Groups keys into rings to enable privacy-preserving proofs
- **Verifies Proofs**: Checks personhood proofs attached to transactions
- **Links Accounts**: Allows connecting blockchain accounts to contextual aliases
- **Manages Registry**: Adds proven persons and supports removing them

## Interface

### Dispatchable Functions

- `set_alias_account(origin, account)`: Link an account to a contextual alias. Once linked, this
  allows the account to dispatch transactions as a person with the alias origin using a regular
  signed transaction with a nonce, providing a simpler alternative to attaching full proofs.
- `unset_alias_account(origin)`: Remove an account-alias link.
- `force_recognize_personhood`: Recognize a set of people without any additional checks.
- `set_personal_id_account`: Set a personal id account.
- `unset_personal_id_account`: Unset the personal id account.
- `migrate_included_key`: Migrate the key for a person who was onboarded and is currently included
  in a ring.
- `migrate_onboarding_key`: Migrate the key for a person who is currently onboarding. The operation
  is instant, replacing the old key in the onboarding queue.
- `clean_up_stale_aliases`: Remove stale alias-to-account mappings in bulk. Submitted by the
  offchain worker as an authorized transaction when aliases become stale (context removed or ring
  revision changed).
- `create_people_collection`: Create the people collection. Valid only if it doesn't exist yet.
- `under_alias`: Dispatch a call under an alias origin, using a stored alias-to-account mapping.

Note: Ring management (onboarding, building, merging, key migration) is handled by the
`pallet-members` crate which provides member set management as a service.

### Offchain worker

- Stale alias cleanup: Periodically scans `AccountToAlias` for aliases whose context is no longer
  in `AccountContexts` and submits an authorized `clean_up_stale_aliases` transaction to remove
  them. Runs at intervals configured by `OffchainWorkInterval`.

### Transaction Extension

The pallet provides the `AsPerson` transaction extension that allows transactions to be dispatched
with special origins: `PersonalIdentity` and `PersonalAlias`. These origins prove the transaction
comes from a unique person, either through their identity or through a contextual alias. To make use
of the personhood system, other pallets should check for these origins.

The extension verifies the proof of personhood during transaction validation and, if valid,
transforms the transaction's origin into one of these special origins.

Warning: The transaction extension `AsPersonInfo` does not provide spam protection for the origins
`PersonalIdentity` and `PersonalAlias` in contexts within `AccountContexts`. This means a user
can spam valid but potentially failing calls without restriction. Another transaction extension
must handle spam protection. Using `pallet-origin-restriction` is advised.

## Usage

Other pallets can verify personhood through origin checks:

- `EnsurePersonalIdentity`: Verifies the origin represents a specific person using their PersonalId
- `EnsurePersonalAlias`: Verifies the origin has a valid alias for any context
- `EnsurePersonalAliasInContext`: Verifies the origin has a valid alias for a specific context
- `EnsureRevisedPersonalAlias`: Verifies the origin has a valid alias for any context and includes
  the revision of the member's ring
- `EnsureRevisedPersonalAliasInContext`: Verifies the origin has a valid alias for a specific
  context and includes the revision of the member's ring

Licence: Apache-2.0
