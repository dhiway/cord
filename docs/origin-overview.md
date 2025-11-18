# Origin Functional Overview

Origin assembles four tightly-coupled pallets—`entity`, `register` (registry + packet logic), and `token`—to model people/organizations, governed data registries, packet records, and the shared token/timeline infrastructure that binds them. This guide explains what each pallet does, how they depend on one another, which operations they expose, and how developers typically stitch them together.

## How the pieces fit together
- **Entity** mints a self-sovereign entity token, stores profile attributes, tracks controller/linked accounts, and issues nyms. Every other pallet resolves permissions through these entity tokens.
- **Register** lets an entity act as a maintainer to create registries (schemas + lookup specs), delegate entry rights, and manage packet records that conform to a registry’s schema.
- **Packet** records live inside the register pallet: every packet has its own token, immutable key material derived from registry attributes, lifecycle status (active/revoked/removed), and history.
- **Token (state/timeline)** is the common event ledger for all Origin tokens. Entity, registry, and packet extrinsics write state events; `pallet-token` views expose timelines and identifier resolution.

## Entity pallet (identity + accounts)
**Purpose:** bootstrap an entity token, manage profile data, and control which accounts can act for that entity.

- **Key extrinsics**
  - `set_info` – mint an entity token and set its profile packet.
  - `add_attributes` / `rotate_attributes` / `rotate_attribute` / `remove_attribute` – CRUD per‐attribute values with history.
  - `set_linked_account`, `revoke_linked_account`, `revoke_linked_account_for` – bind/unbind additional accounts.
  - `rotate_controller`, `rotate_controller_for` – change the controller account that owns the token.
  - `set_entity_nym`, `remove_entity_nym` – manage the human-friendly nym suffix (`.nym.org.in`).
  - `clear`, `clear_everything_for` – wipe an entity token (controller only).
- **Read-only views** (authorization-gated)
  - `details` (entity info packet), `account_token`, `controller_account`, `linked_accounts`, `account_history`.
  - Nym lookups: `entity_nym`, `entity_nym_lookup`.
  - Attribute history/version helpers: `attribute_version`, `attribute_versions`, `attribute_history`, `attribute_history_for_key`, `attribute_history_entry`.

## Register pallet (registries + delegates + packets)
**Purpose:** define registry schemas, govern who may write into them, and host packet lifecycles that obey those schemas.

- **Registry extrinsics**
  - `create_registry` – create a registry with maintainer entity, attribute schema, token spec, and lookup specs.
  - `set_delegate_permissions` / `remove_delegate_permissions` – grant/revoke delegate roles; maintainer always retains admin rights.
  - `update_registry_info` – rotate the registry info blob.
  - Lifecycle controls: `revoke_registry`, `restore_registry`, `delete_registry`.
- **Packet extrinsics (scoped to a registry)**
  - `create_packet` – mint a packet that satisfies the registry schema; derives a packet token + lookup anchors.
  - `update_packet` – merge attribute updates and bump version.
  - Lifecycle controls: `revoke_packet`, `restore_packet`, `remove_packet`.
- **Read-only views** (authorization-gated)
  - Registry: `details`, `overview` (summary of info/schema/status), `lookup_specs`, `query_count`, `delegate_permissions`.
  - Packets: `packet_snapshot`, `packet_snapshot_by_token`, `packet_metadata`, `lookup_snapshot` (by digest anchor), `list_by_token`, `list_by_digest`, `token_fingerprint`.

## Packet lifecycle (inside Register)
- **Create**: maintainer/delegate submits `create_packet` with attributes that satisfy registry schema; token + lookup digests emitted.
- **Update**: `update_packet` merges changes while preserving schema constraints.
- **Revoke/Restore**: toggle packet availability without deleting history.
- **Remove**: permanent removal from registry surface (still retains token history in `pallet-token`).
- **Inspect**: use register views to fetch packet snapshots and the token pallet to read timelines.

## Token pallet (shared timeline & identifiers)
**Purpose:** decode tokens, map pallet <-> index, and expose history/timeline APIs for any Origin token.

- **Read-only views** (authorization-gated)
  - Identifier helpers: `resolve_identifier`, `resolve_pallet`, `pallet_index_of`, `pallet_name`, `next_pallet_index`, `genesis_network_id`.
  - State history: `state_version`, `state_event`, `timeline` (cursor + limit), `history` (bounded slice).
- **Written by** entity/register extrinsics via `state_event` to keep append-only timelines for every token.

## Typical developer flow
1) Call `Entity::set_info` to mint an entity token and bind your controller account.  
2) Create a registry with `Register::create_registry` (maintainer = your entity).  
3) Delegate entry rights via `Register::set_delegate_permissions` (optional for multi-party writes).  
4) Issue packets with `Register::create_packet` or `update_packet` as data changes.  
5) Render timelines via `Token::timeline` and fetch snapshots via register/entity views.  
6) Off-chain, use the `origin-rs` SDK (see `docs/sdk/`) to compose these extrinsics, manage authorizations, and ship production-ready apps or services.
