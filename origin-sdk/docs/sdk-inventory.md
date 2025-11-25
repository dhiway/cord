// Auto-generated summary — November 25, 2025
// Current SDK surface mapped from origin-sdk/src (query + tx helpers + extrinsic builders).
// Source of truth for pallets remains docs/pallet-inventory.md; use this file to spot gaps/duplication.

# SDK Inventory (current)

## Query Facade (`src/query`)
- Entry: `client.query().using(&signer)` (signer required for all view calls).
- Pallet helpers: `.entity()`, `.registry()`, `.packet()`, `.token()` each return signer-bound clients.

### Entity (view-only helpers)
- `overview(entity)` → `Option<EntityStateView>`
- `overview_nested(entity)` → expands to nested `Option<EntityNestedValue>`
- `details(entity)` / `details_nested(entity)` → `Option<EntityInfoView>`
- `account_token(account)` → `Option<Ss58Identifier>`
- `linked_accounts(entity)` → `Option<Vec<AccountId32>>`
- `linked_account_count(entity)` → `u32`
- `controller_account(entity)` → `Option<AccountId32>`
- `is_controller(entity, account)` / `is_linked_account(entity, account)` → `bool`
- `account_history(entity)` → `Option<Vec<AccountUnbindEntryView>>`
- `has_nym(entity)` / `token_of_nym(nym)` / `nym(entity)`
- `entity_attribute_keys(entity)` → `Option<Vec<Vec<u8>>>`
- `has_attribute(entity, key)` → `bool`
- `attribute_version(entity, key)` → `Option<u64>`
- `attribute_versions(entity)` → `Option<Vec<(Vec<u8>, u64)>>`
- `attribute_history(entity)` / `attribute_history_for_key(entity, key)` → `Option<Vec<AttributeHistoryEntryView>>`
- `attribute_history_entry(entity, key, version)` → `Option<AttributeHistoryEntryView>`

### Registry (view-only helpers)
- `details(registry)` / `details_nested(registry)` → `Option<RegistryStateView>`
- `delegate_permissions(registry, delegate)` → `Option<RegistryPermissions>`
- `query_count(registry, account)` → `Option<u64>`
- `lookup_specs(registry)` → `Option<Vec<LookupSpecView>>`
- `attribute(registry, key)` / `attributes(registry)` → `Option<(ElementType,bool)>` / `Option<Vec<RegistryAttributeView>>`
- `token_specs(registry)` → `Option<Vec<Vec<u8>>>`
- `packet_metadata(registry, packet)` → `Option<PacketMetadataView>`
- `packet_state(registry, packet, version)` / `packet_for_token(token, version)` → `Option<PacketStateView>`
- `packet_lookup_snapshot(registry, digest, version)` → `Option<PacketStateView>`
- `packets_by_digest(digest, offset, limit)` → `Option<Vec<PacketPointer>>`
- `list_by_token(prefix, version, cursor, limit)` → `Option<(Vec<PacketStateView>, Option<Ss58Identifier>)>`
- `list_by_digest(prefix, version, cursor, limit)` → `Option<(Vec<PacketStateView>, Option<Vec<u8>>)>`
- `registry_exists(registry)` → `bool`
- `registry_status(registry)` / `registry_attribute_keys(registry)` → `Option<RegistryStatus>` / `Option<Vec<Vec<u8>>>`
- `registry_is_active` / `registry_is_revoked` / `registry_is_deleted` → `bool`
- `registry_delegates(registry)` → `Option<Vec<(Ss58Identifier, RegistryPermissions)>>`
- `has_registry_permissions(registry, delegate, required)` / `is_delegate(registry, delegate)` → `bool`
- `registry_maintainer(registry)` → `Option<Ss58Identifier>`
- `packet_exists(registry, packet)` / `packet_status(packet)` / `packet_controller(packet)` → `bool` / `Option<PacketStatus>` / `Option<Ss58Identifier>`

### Packet (view-only helpers)
- `state(packet_pointer, version)` → `Option<PacketStateView>`
- `state_nested(packet_pointer, version)` → `Option<PacketNestedValue>`

### Token (view-only helpers)
- `timeline(token, start, limit)` → `Option<TokenTimelineView>`
- `resolve_identifier(token)` → `Option<DecodedIdentifier>`
- `pallet_index_of(name)` / `pallet_name_view(index)` / `next_pallet_index()` / `genesis_network_id()` / `resolve_pallet(index)` → `Option<_>`
- `state_version(token)` → `Option<u32>`
- `state_event(token, version)` / `latest_state_event(token)` / `recent_timeline(token, limit)` → `Option<TokenStateEventView>` / `Option<Vec<TokenStateEventView>>`
- `has_history(token)` → `bool`

## Tx Facade (`src/tx`)
- Entry: `client.tx().using(&signer)` (signer required). All helpers return `TxHandle`; caller chooses `wait_in_block()` / `wait_finalized()`.

### EntityTx
- build + submit: `set_info_from_input`, `set_info_from_nested`
- attribute ops: `submit_rotate_attribute_from_view`, `submit_rotate_attributes_from_nested`, `submit_add_attributes_from_nested`, `submit_remove_attribute`
- account/controller ops: `submit_set_linked_account`, `submit_revoke_linked_account(force)`, `submit_rotate_controller(force)`
- maintenance ops: `submit_clear_everything(force)`, `submit_set_entity_nym`, `submit_remove_entity_nym`

### RegistryTx
- registry create: dynamic builder + `submit_create`, `submit_create_from_nested`
- delegate perms: `set_delegate_permissions` / `submit_set_delegate_permissions` (typed `DelegatePermissionsInput`), `remove_delegate_permissions` / `submit_remove_delegate_permissions` (typed `RemoveDelegatePermissionsInput`)
- lifecycle: `revoke_registry`, `restore_registry`, `delete_registry`
- packet issuance helper: `submit_packet_from_nested` (with schema validation)
- info updates: `submit_update_registry_info_from_view`

### PacketTx
- issue: from nested (`submit_issue_from_nested`), raw bytes (`issue/submit_issue`), or JSON (`issue_from_raw`, signer variant)
- update/revoke/restore/delete packet builders + submit helpers
- `set_packet_status` builder + `submit_set_packet_status`
- uses schema validation helper `validate_packet_against_schema`

### TokenTx
- `submit_rotate_attribute(token, key, value_bytes)` / `submit_rotate_attribute_view`

## Extrinsic Builders (`src/extrinsic/calls.rs`)

### entity
- `rotate_attribute_call(token, key, raw_value)`
- `rotate_attribute_from_element(token, key, ElementInput)`
- `rotate_attributes_from_input(ops)`
- `add_attributes_from_input(ops)`
- `rotate_attribute_from_json(token, key, expected_kind, json_value)`
- `rotate_attributes_from_json(token, schema, json_obj)`
- `set_info_from_struct(info)`
- `remove_attribute_call`, `set_linked_account_call`, `revoke_linked_account_call`, `revoke_linked_account_for_call`,
  `rotate_controller_call`, `rotate_controller_for_call`, `clear_everything_call`, `clear_everything_for_call`,
  `set_entity_nym_call`, `remove_entity_nym_call`

### registry
- `create_call(registry_id, schema_raw, config_raw)`
- `create_from_structs(registry_id, schema_struct, config_struct)`
- `create_from_input(registry_id, RegistryCreateInput)`
- `update_info_from_input(registry, info_element)`

### packet
- `issue_call(registry_id, registry_schema_view, json_body)`
- `issue_from_flat(registry_id, [(key, encoded_element_bytes)])`
- `issue_from_input(registry_id, PacketAttributesInput)`
- `update_from_input`, `revoke_call`, `restore_call`, `delete_call`, `set_status_call`

### token
- `rotate_attribute_call(token, key, value_bytes)`
- `rotate_attribute_from_json(token, key, expected_kind, json_value)`

## Alignment Notes (entity / register / token)
- Views: all helpers call pallet `#[pallet::view_functions]` only; no storage RPC usage for these pallets.
- Types: IDs are `Ss58Identifier`; accounts are `AccountId32`; view outputs use `origin_primitives` view structs.
- Extrinsics: inputs constructed from explicit SDK structs and schema transforms (no ad-hoc SCALE building).
- Tx flow: helpers return `TxHandle`; caller controls awaiting and concurrency; nonce handled by SDK queue.

Use this inventory alongside the pallet inventory to drive the refactor plan (module split, type mirrors, schema transforms, and demo refresh).
