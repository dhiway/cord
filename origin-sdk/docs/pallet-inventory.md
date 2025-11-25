// Auto-updated for typed views (November 25, 2025)
// Source of truth: origin/pallets/{entity,register,token} + origin/primitives

# Pallet Inventory (entity, register, token)

The table below lists the calls and view functions the SDK must mirror 1:1. Field order matches pallet definitions; optionality and wrappers are noted. All types are SCALE encoded.

## pallet-entity

- Extrinsics
  - `set_info(info: Box<EntityInfoPacket>) -> DispatchResult`
  - `rotate_attributes(ops: Vec<(Vec<u8>, Data)>)`
  - `add_attributes(ops: Vec<(Vec<u8>, Data)>)`
  - `remove_attribute(key: Vec<u8>)`
  - `rotate_attribute(key: Vec<u8>, val: Data)`
  - `set_linked_account(account: AccountId)`
  - `revoke_linked_account(account: AccountId)`
  - `revoke_linked_account_for(token: Ss58Identifier, account: AccountId)` (Force origin)
  - `rotate_controller(token: Ss58Identifier, new_controller: AccountId)`
  - `rotate_controller_for(token: Ss58Identifier, new_controller: AccountId)` (Force origin)
  - `clear_everything(token: Ss58Identifier)`
  - `clear_everything_for(token: Ss58Identifier)` (Force origin)
  - `set_entity_nym(prefix: Vec<u8>)`
  - `remove_entity_nym(token: Ss58Identifier)`

- Views (typed, auth-maps to Option/bool/0)
  - `details(token) -> Option<EntityInfoView>`
  - `account_token(account) -> Option<Ss58Identifier>`
  - `linked_accounts(token) -> Option<Vec<AccountId>>`
  - `linked_account_count(token) -> u32` (0 on auth failure)
  - `is_linked_account(token, account) -> bool`
  - `controller_account(token) -> Option<AccountId>`
  - `is_controller(token, account) -> bool`
  - `account_history(token) -> Option<Vec<AccountUnbindEntryView<AccountId>>>`
  - `has_nym(token) -> bool`
  - `token_of_nym(nym: Vec<u8>) -> Option<Ss58Identifier>`
  - `entity_nym(token) -> Option<Vec<u8>>`
  - `overview(token, history_limit: Option<u32>) -> Option<EntityStateView<AccountId>>`
  - `entity_attribute_keys(token) -> Option<Vec<Vec<u8>>>`
  - `has_attribute(token, key) -> bool`
  - `attribute_version(token, key: Attribute) -> Option<u64>`
  - `attribute_versions(token) -> Option<Vec<(Vec<u8>, u64)>>`
  - `attribute_history(token) -> Option<Vec<AttributeHistoryEntryView>>`
  - `attribute_history_for_key(token, key: Attribute) -> Option<Vec<AttributeHistoryEntryView>>`
  - `attribute_version_history(token, key: Attribute, version: u64) -> Option<AttributeHistoryEntryView>`

- Core types
  - `EntityInfoView { display: ElementView, web: ElementView, email: ElementView, attributes: Option<Vec<AttributeValueView>> }`
  - `EntityStateView { info: EntityInfoView, nym: Option<Vec<u8>>, linked_accounts: Vec<AccountId>, history: Vec<AttributeHistoryEntryView> }`
  - `Data = Element<MaxRawDataLength>` (runtime bound)
  - `AttributeUpdate = (Vec<u8>, Data)`

## pallet-register

- Extrinsics
  - `create_registry(info: Data, kind: RegistryKind, attributes: Vec<RegistryAttributeSpec>, token_spec: LookupSpec, lookup_specs: Vec<LookupSpec>)`
  - `update_registry_info(registry: Ss58Identifier, info: Data)`
  - `set_delegate_permissions(registry: Ss58Identifier, delegate: AccountId, roles: Vec<RegistryPermissions>)`
  - `remove_delegate_permissions(registry: Ss58Identifier, delegate: Ss58Identifier)`
  - `revoke_registry(registry: Ss58Identifier)`
  - `restore_registry(registry: Ss58Identifier)`
  - `delete_registry(registry: Ss58Identifier)`
  - `create_packet(registry: Ss58Identifier, attributes: Vec<(Vec<u8>, Element)>)`
  - `update_packet(registry: Ss58Identifier, packet: Ss58Identifier, attributes: Vec<(Vec<u8>, Element)>)`
  - `revoke_packet(registry: Ss58Identifier, packet: Ss58Identifier)`
  - `restore_packet(registry: Ss58Identifier, packet: Ss58Identifier)`
  - `delete_packet(registry: Ss58Identifier, packet: Ss58Identifier)`
  - `set_packet_status(registry: Ss58Identifier, packet: Ss58Identifier, status: PacketStatus)`

- Views (typed, auth-maps to Option/bool/0)
  - `delegate_permissions(registry, delegate) -> Option<RegistryPermissions>`
  - `lookup_specs(registry) -> Option<Vec<LookupSpecView>>`
  - `registry_attributes(registry) -> Option<Vec<RegistryAttributeView>>`
  - `registry_attribute(registry, key) -> Option<(ElementType, bool)>`
  - `registry_details(registry) -> Option<RegistryStateView>`
  - `registry_token_specs(registry) -> Option<Vec<Vec<u8>>>`
  - `registry_exists(registry) -> bool`
  - `registry_status(registry) -> Option<RegistryStatus>`
  - `registry_attribute_keys(registry) -> Option<Vec<Vec<u8>>>`
  - `registry_is_active / registry_is_revoked / registry_is_deleted -> bool`
  - `registry_delegates(registry) -> Option<Vec<(Ss58Identifier, RegistryPermissions)>>`
  - `has_registry_permissions(registry, delegate, required) -> bool`
  - `is_delegate(registry, delegate) -> bool`
  - `registry_maintainer(registry) -> Option<Ss58Identifier>`
  - `query_count(registry, account) -> Option<u64>`
  - `packet_state(registry, packet, version: Option<u32>) -> Option<PacketStateView>`
  - `packet_metadata(registry, packet) -> Option<PacketMetadataView>`
  - `packet_for_token(token, version: Option<u32>) -> Option<PacketStateView>`
  - `packet_lookup_snapshot(registry, digest, version: Option<u32>) -> Option<PacketStateView>`
  - `packets_by_digest(digest, offset: Option<u32>, limit: Option<u32>) -> Option<Vec<PacketPointer>>`
  - `list_by_token(prefix: Vec<u8>, version: Option<u32>, cursor: Option<Ss58Identifier>, limit: Option<u32>) -> Option<(Vec<PacketStateView>, Option<Ss58Identifier>)>`
  - `list_by_digest(prefix: Vec<u8>, version: Option<u32>, cursor: Option<Vec<u8>>, limit: Option<u32>) -> Option<(Vec<PacketStateView>, Option<Vec<u8>>)>`
  - `packet_exists(registry, packet) -> bool`
  - `packet_status(packet) -> Option<PacketStatus>`
  - `packet_controller(packet) -> Option<Ss58Identifier>`

- Core types
  - `RegistryStateView { registry, maintainer, info: ElementView, kind: RegistryKind, status: RegistryStatus, attributes: Vec<RegistryAttributeView>, token_spec: Vec<Vec<u8>>, lookup_specs: Vec<Vec<Vec<u8>>> }`
  - `RegistryAttributeSpec { key: Vec<u8>, kind: ElementType, optional: bool }`
  - `LookupSpec = Single(Vec<u8>) | Combo(Vec<Vec<u8>>)`
  - `PacketStateView` / `PacketMetadataView` (use MaxRawDataLength/MaxAdditionalAttributes bounds from runtime)

## pallet-token

- Extrinsics (subset relevant to SDK surface)
  - `rotate_attribute(token: Ss58Identifier, key: Vec<u8>, value: Vec<u8>)` (attribute updates)
  - pallet indices/resolution handled in runtime for identifier mapping

- Views (typed, auth-maps to Option/bool)
  - `timeline(token, start: Option<u32>, limit: Option<u32>) -> Option<TokenTimelineView>`
  - `resolve_identifier(token) -> Option<DecodedIdentifier>`
  - `state_event(token, version: u32) -> Option<TokenStateEventView>`
  - `state_version(token) -> Option<u32>`
  - `latest_state_event(token) -> Option<TokenStateEventView>`
  - `recent_timeline(token, limit: Option<u32>) -> Option<Vec<TokenStateEventView>>`
  - `has_history(token) -> bool`
  - pallet index helpers: `pallet_index_of(name) -> Option<u16>`, `pallet_name_view(index) -> Option<String>`, `next_pallet_index() -> Option<u16>`, `genesis_network_id() -> Option<u16>`, `resolve_pallet(index) -> Option<String>`

- Core types
  - `TokenStateEventView<H256>`, `TokenTimelineView<H256>`, `DecodedIdentifier`

# Alignment Rules for SDK
- All extrinsic inputs must be mirrored as Rust structs/enums in `origin-sdk/src/types/*` with identical field order and bounds.
- All views must decode into these mirrors; `None` ↔ `AuthorizationError::NotFound`.
- Nested ↔ flat transforms:
  - Entity: map developer-friendly nested attributes to `(Attribute, Element)` lists used in pallets.
  - Registry: map nested schema (attributes + lookup specs) to bounded lists.
  - Packet: flatten packet bodies against registry schema before submission; expand views back to nested.

# Runtime Bounds (current)
- `MaxRawDataLength = 4096` bytes (elements)
- `MaxAdditionalAttributes = 32` (attributes per entity/registry/packet)
- History limits: `DefaultEntityOverviewHistory`, `MaxEntityOverviewHistory` (see runtime constants).
