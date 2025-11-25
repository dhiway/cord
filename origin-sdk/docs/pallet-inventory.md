// Auto-generated November 24, 2025 (SDK sync point)
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

- Views (all `Result<..., AuthorizationError>`)
  - `details(token) -> Vec<u8>`
  - `account_token(account) -> Vec<u8>`
  - `linked_accounts(token) -> Vec<u8>`
  - `controller_account(token) -> Vec<u8>`
  - `account_history(token) -> Vec<AccountUnbindEntryView<AccountId>>`
  - `entity_nym(token) -> Vec<u8>`
  - `overview(token, history_limit: Option<u32>) -> EntityStateView<AccountId>`
  - `attribute_version(token, key: Attribute) -> u64`
  - `attribute_versions(token) -> Vec<(Vec<u8>, u64)>`
  - `attribute_history(token) -> Vec<AttributeHistoryEntryView>`
  - `attribute_history_for_key(token, key: Attribute) -> Vec<AttributeHistoryEntryView>`
  - `attribute_history_entry(token, key: Attribute, version: u64) -> AttributeHistoryEntryView`

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

- Views (all `Result<..., AuthorizationError>`)
  - `details(registry) -> RegistryStateView`
  - `overview(registry) -> RegistryStateView`
  - `delegate_permissions(registry, delegate) -> RegistryPermissions`
  - `query_count(registry, account) -> u32`
  - `lookup_specs(registry) -> Vec<LookupSpec>`
  - `attribute(registry, key) -> (ElementType, bool)`
  - `attributes(registry) -> Vec<(Vec<u8>, ElementType, bool)>`
  - `token_specs(registry) -> Vec<Vec<u8>>`
  - `packet_metadata(registry, packet) -> PacketMetadataView`
  - `packet_snapshot(registry, packet, version: Option<u32>) -> PacketStateView`
  - `packet_snapshot_by_token(token, version: Option<u32>) -> Option<PacketStateView>`
  - `lookup_snapshot(registry, digest, version: Option<u32>) -> PacketStateView`
  - `list_by_token(prefix: Vec<u8>, version: Option<u32>, cursor: Option<Ss58Identifier>, limit: Option<u32>) -> (Vec<PacketSnapshot>, Option<Ss58Identifier>)`
  - `list_by_digest(prefix: Vec<u8>, version: Option<u32>, cursor: Option<Vec<u8>>, limit: Option<u32>) -> (Vec<PacketSnapshot>, Option<Vec<u8>>)`

- Core types
  - `RegistryStateView { registry, maintainer, info: ElementView, kind: RegistryKind, status: RegistryStatus, attributes: Vec<RegistryAttributeView>, token_spec: Vec<Vec<u8>>, lookup_specs: Vec<Vec<Vec<u8>>> }`
  - `RegistryAttributeSpec { key: Vec<u8>, kind: ElementType, optional: bool }`
  - `LookupSpec = Single(Vec<u8>) | Combo(Vec<Vec<u8>>)`
  - `PacketStateView` / `PacketMetadataView` (use MaxRawDataLength/MaxAdditionalAttributes bounds from runtime)

## pallet-token

- Extrinsics (subset relevant to SDK surface)
  - `rotate_attribute(token: Ss58Identifier, key: Vec<u8>, value: Vec<u8>)` (attribute updates)
  - pallet indices/resolution handled in runtime for identifier mapping

- Views (all `Result<..., AuthorizationError>`)
  - `timeline(token, start: Option<u32>, limit: Option<u32>) -> TokenTimelineView`
  - `resolve_identifier(token) -> DecodedIdentifier`
  - `state_event(token, version: u32) -> TokenStateEventView`
  - `state_version(token) -> u32`
  - `maybe_*` variants via `Result<Option<...>, AuthorizationError>` for state_event/resolution
  - pallet index helpers: `pallet_index_of(name) -> u16`, `pallet_name(index) -> Vec<u8>`, `next_pallet_index() -> u16`, `genesis_network_id() -> u32`, `resolve_pallet(index) -> Vec<u8>`

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
