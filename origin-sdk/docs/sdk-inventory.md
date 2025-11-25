// Auto-generated summary — November 24, 2025
// Current SDK surface mapped from origin-sdk/src (query + tx helpers + extrinsic builders).
// Source of truth for pallets remains docs/pallet-inventory.md; use this file to spot gaps/duplication.

# SDK Inventory (current)

## Query Facade (`src/query`)
- `Query::entity()`, `registry()`, `packet()`, `token()`; `Query::using(signer)` mirrors the same with signer-bound variants.

### Entity (view-only helpers)
- `overview(entity)` → `EntityStateView`
- `overview_nested(entity)` → expands to nested
- `maybe_overview(entity)` → `Option<EntityStateView>`
- `details(entity)` / `details_nested(entity)` / `maybe_details(entity)`
- `nym(entity)`
- `linked_accounts(entity)`
- `controller_account(entity)`
- `account_history(entity)`
- `attribute_version(entity, key)` / `attribute_versions(entity)`
- `attribute_history(entity)` / `attribute_history_for_key(entity, key)` / `attribute_history_entry(entity, key, version)`

### Registry (view-only helpers)
- `details(registry)` / `details_nested(registry)` / `maybe_details(registry)`
- `overview(registry)` / `overview_nested(registry)` / `maybe_overview(registry)`
- `delegate_permissions(registry, delegate)`
- `query_count(registry, account)`
- `lookup_specs(registry)`
- `attribute(registry, key)` / `attributes(registry)` / `maybe_attribute(registry, key)`
- `token_specs(registry)`
- `packet_metadata(registry, packet)` / `maybe_packet_metadata(registry, packet)`
- `packet_snapshot(registry, packet, version)` / `maybe_packet_snapshot(registry, packet, version)`
- `packet_snapshot_by_token(token, version)`
- `lookup_snapshot(registry, digest, version)`
- `list_by_token(prefix, version, cursor, limit)`
- `list_by_digest(prefix, version, cursor, limit)`

### Packet (view-only helpers)
- `state(packet_pointer, version)` / `state_nested(packet_pointer, version)`

### Token (view-only helpers)
- `timeline(token, start, limit)`
- `resolve_identifier(token)` / `maybe_resolve_identifier(token)`
- `pallet_index_of(name)` / `pallet_name(index)` / `next_pallet_index()` / `genesis_network_id()` / `resolve_pallet(index)`
- `state_version(token)`
- `state_event(token, version)` / `maybe_state_event(token, version)`

## Tx Facade (`src/tx`)

### EntityTx / EntityTxWithSigner
- build + submit: `set_info_from_input`, `set_info_from_nested`
- attribute ops: `submit_rotate_attribute_from_view`, `submit_rotate_attributes_from_nested`, `submit_add_attributes_from_nested`, `submit_remove_attribute`
- account/controller ops: `submit_set_linked_account`, `submit_revoke_linked_account(force)`, `submit_rotate_controller(force)`
- maintenance ops: `submit_clear_everything(force)`, `submit_set_entity_nym`, `submit_remove_entity_nym`

### RegistryTx / RegistryTxWithSigner
- registry create: dynamic builder + `submit_create`, `submit_create_from_nested`
- delegate perms: `set_delegate_permissions`, `remove_delegate_permissions`
- lifecycle: `revoke_registry`, `restore_registry`, `delete_registry`
- packet issuance helper: `submit_packet_from_nested` (with schema validation)
- info updates: `submit_update_registry_info_from_view`

### PacketTx / PacketTxWithSigner
- issue: from nested (`submit_issue_from_nested`), raw bytes (`issue/submit_issue`), or JSON (`issue_from_raw`, signer variant)
- update/revoke/restore/delete packet builders + submit helpers
- `set_packet_status` builder + `submit_set_packet_status`
- uses schema validation helper `validate_packet_against_schema`

### TokenTx / TokenTxWithSigner
- `submit_rotate_attribute(token, key, value_bytes)` (builder-backed)

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

## Known Gaps vs Pallet Inventory (docs/pallet-inventory.md)
- Missing tx surfaces/builders for entity: `remove_attribute`, `add_attributes`, `set_linked_account`, `revoke_linked_account{,_for}`, `rotate_controller{,_for}`, `clear_everything{,_for}`, `set_entity_nym`, `remove_entity_nym`.
- Missing tx surfaces/builders for register: `update_registry_info` is builder-only (no typed input); no builders for `set_packet_status`, `delete_packet` (only `remove_packet` alias), `restore_packet`, `revoke_packet` in typed form.
- Missing token tx surface for `rotate_attribute`.
- View ergonomics: only some maybe_* helpers; nested expand/flatten not consistently applied across pallets.

Use this inventory alongside the pallet inventory to drive the refactor plan (module split, type mirrors, schema transforms, and demo refresh).
