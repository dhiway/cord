# Origin SDK Inventory

> Live map of the Origin Rust SDK (`origin-rs`). Use this document with
> `origin-rs/docs/pallet-inventory.md` and `origin-rs/docs/elements.md` when
> planning changes so the SDK surface always matches the runtime pallets.

## Purpose and scope
- Enumerate every public SDK module, helper, and runtime interaction point.
- Show how view helpers, extrinsic builders, schema transforms, and client
  plumbing fit together.
- Serve as the seed for comprehensive SDK documentation (architecture +
  reference).

## Module map

| Path | Responsibility | Highlights |
| --- | --- | --- |
| `src/config.rs` | Subxt config wrapper | `OriginConfig`, `OrbisConfig`, compatibility-envelope builders for relay, Orbis Storage, and Revive extensions. |
| `src/client/` | Connections, signers, queues, and facades | `OriginClient`, `ConnectionBuilder`, `ViewClient`, event streaming, meta-tx entrypoints. |
| `src/query/` | View helpers grouped per pallet | `Query` facade plus entity/registry/packet/token clients requiring authorization. |
| `src/tx/` | Tx pipeline + pallet-specific helpers | `TxClient`, `AccountTx`, per-pallet submitters, batch/meta helpers, nonce management. |
| `src/extrinsic/` | Dynamic call builder + typed transformers | `DynamicCallBuilder`, pallet call encoders, `MetaTxClient`. |
| `src/schema/` | Nested ↔ flat transforms mirroring pallet types | Entity, registry, and packet flatten/expand/validation helpers. |
| `src/types/` | SDK-facing type mirrors + re-exports | View structs, extrinsic inputs, identifiers, error enums. |
| `src/util/` | Supporting helpers | JSON↔element codecs, retry/backoff, ttl math, hex helpers. |
| `docs/` | Reference documentation | Pallet inventory, element cheatsheet, this SDK inventory. |
| `examples/` | End-to-end snippets | Demo binaries covering entity flows, registry + packet issuance, and token resolution. |

## Client layer (`src/client`)

### OriginClient and ConnectionBuilder
- `OriginClient::connect(endpoint)` establishes a WebSocket connection with the
  default retry/backoff (`RetryPolicy`) and builds the shared tx pipeline.
- `OriginClient::builder()` exposes `ConnectionBuilder` so callers can override
  `endpoint`, `RetryPolicy`, RPC timeout, auto-reconnect flag, or the
  `TxPipelineConfig`.
- Exposed handles: `online()` (raw `subxt::OnlineClient`), `metadata()`,
  `signer_from_account()`, `tx_config()`, `view()`, `query()`, `tx()`,
  `events()`, `meta_tx()`/`metatx()` plus camelCase aliases, and `call()` for
  ad-hoc `DynamicCall` construction.
- `Connection` caches metadata and the live `OnlineClient`. Failed connects are
  retried according to the configured policy.

### Signers and account helpers
- `Signer` trait provides async `sign_payload` + `account_id`; used by both tx
  pipeline workers and meta-tx flows.
- Built-in implementations: `MultiKeySigner` (sr25519/ed25519/ecdsa),
  `Sr25519Signer`, and `OriginSigner` (preferred wrapper, buildable from an
  `OriginAccount`).
- `SubxtSignerAdapter` bridges async signing into `subxt::tx::Signer`.

### View engine
- All views are executed via `ViewClient::call(pallet, function, args)` which
  hits `RuntimeViewFunction_execute_view_function` and decodes directly into the
  requested type (no storage RPC usage).
- `ViewClient::authorization_for(&signer, pallet, function)` builds the
  `Authorization` envelope (with TTL derived from the latest block) needed by
  pallets that gate their views.

### Event streaming
- `client.events().subscribe(Some("Entity"))` tails finalized blocks and returns
  `EventEnvelope { block, pallet, variant, fields }` via an unbounded channel.
  Subscription restarts honor the same retry policy as the connection.

### Meta-transaction entrypoint
- `OriginClient::meta_tx()` / `metatx()` produce a `MetaTxClient` that can
  attach a signer later. `meta_tx_with(signer)` attaches a meta key immediately.
- `MetaTxClient` exposes `prepare_and_sign`, `prepare_and_sign_with_metadata`,
  `prepare_and_sign_with_metadata_hash`, `submit_signed`, `submit_signed_with`,
  `sign_and_submit`, and `sign_submit_and_wait_checked` which ensures the
  pallet `MetaTx::Dispatched` event reported success.

## Query (view) API (`src/query`)

- Entrypoint: `client.query().using(&signer)` returns a signer-bound
  `QueryWithSigner`. Every helper below returns `Result<_, OriginSdkError>` and
  calls pallet `#[view_functions]` only.
- Facets: `.entity()`, `.registry()`, `.packet()`, `.token()`.

#### Entity views (`EntityClientWithSigner`)

| Helper | Return | Notes |
| --- | --- | --- |
| `overview(entity)` | `Option<EntityStateViewSdk>` | High-level state (info + controller + metadata). |
| `overview_nested(entity)` | `Option<schema::entity::EntityNestedValue>` | Expands `overview` via schema helpers. |
| `details(entity)` | `Option<EntityInfoViewSdk>` | Raw info struct from pallet. |
| `details_nested(entity)` | `Option<EntityNestedValue>` | Expanded info form. |
| `account_token(account)` | `Option<EntityToken>` | Reverse lookup entity token bound to an account. |
| `linked_accounts(entity)` | `Option<Vec<OriginAccountId>>` | List linked controller accounts. |
| `linked_account_count(entity)` | `u32` | Lightweight counter for pagination. |
| `controller_account(entity)` | `Option<OriginAccountId>` | Current controller. |
| `is_controller(entity, account)` | `bool` | Checks controller equality. |
| `is_linked_account(entity, account)` | `bool` | Checks if an account is linked. |
| `account_history(entity)` | `Option<Vec<AccountUnbindEntryViewSdk>>` | Unbind log for auditing. |
| `has_nym(entity)` | `bool` | Whether an entity has a pallet-assigned nym. |
| `token_of_nym(nym)` | `Option<EntityToken>` | Resolve entity token via nym prefix. |
| `nym(entity)` | `Option<EntityNym>` | Fetch the current nym bytes. |
| `entity_attribute_keys(entity)` | `Option<Vec<Vec<u8>>>` | Known attribute keys. |
| `has_attribute(entity, key)` | `bool` | Boolean existence check. |
| `attribute_version(entity, key)` | `Option<u64>` | Latest version for a key. |
| `attribute_versions(entity)` | `Option<Vec<(Vec<u8>, u64)>>` | All keys with their latest version. |
| `attribute_history(entity)` | `Option<Vec<AttributeHistoryEntryViewSdk>>` | Full attribute history (all keys). |
| `attribute_history_for_key(entity, key)` | `Option<Vec<AttributeHistoryEntryViewSdk>>` | History filtered by key. |
| `attribute_history_entry(entity, key, version)` | `Option<AttributeHistoryEntryViewSdk>` | Single version snapshot. |

#### Registry views (`RegistryClientWithSigner`)

| Helper | Return | Notes |
| --- | --- | --- |
| `details(registry)` | `Option<RegistryStateViewSdk>` | Flat registry state. |
| `details_nested(registry)` | `Option<schema::registry::RegistryNestedSchema>` | Expanded registry view. |
| `delegate_permissions(registry, delegate)` | `Option<RegistryPermissions>` | Current delegate roles bitset. |
| `query_count(registry, account)` | `Option<u64>` | Usage accounting per querying account. |
| `lookup_specs(registry)` | `Option<Vec<LookupSpecViewSdk>>` | Configured lookup combiners. |
| `attribute(registry, key)` | `Option<(ElementType, bool)>` | Schema entry for a single key. |
| `attributes(registry)` | `Option<Vec<RegistryAttributeViewSdk>>` | Full schema view. |
| `token_specs(registry)` | `Option<Vec<Vec<u8>>>` | Keys used when deriving token identifiers. |
| `packet_metadata(registry, packet)` | `Option<PacketMetadataViewSdk>` | Off-chain metadata envelope. |
| `packet_state(registry, packet, version)` | `Option<PacketStateViewSdk>` | Packet snapshot by ID/version. |
| `packet_for_token(token, version)` | `Option<PacketStateViewSdk>` | Reverse lookup via issued token. |
| `lookup_snapshot(registry, digest, version)` | `Option<PacketStateViewSdk>` | Resolve via lookup digest. |
| `packets_by_digest(digest, offset, limit)` | `Option<Vec<PacketPointer>>` | Digest index pagination. |
| `list_by_token(prefix, version, cursor, limit)` | `Option<(Vec<PacketStateViewSdk>, Option<Ss58Identifier>)>` | Token-index pagination helper. |
| `list_by_digest(prefix, version, cursor, limit)` | `Option<(Vec<PacketStateViewSdk>, Option<Vec<u8>>) >` | Digest-index pagination helper. |
| `registry_exists(registry)` | `bool` | Existence guard before expensive reads. |
| `registry_status(registry)` | `Option<RegistryStatus>` | Active/Revoked/Deleted. |
| `registry_attribute_keys(registry)` | `Option<Vec<Vec<u8>>>` | Schema key list only. |
| `registry_is_active(registry)` | `bool` | Convenience status checks. |
| `registry_is_revoked(registry)` | `bool` | — |
| `registry_is_deleted(registry)` | `bool` | — |
| `registry_delegates(registry)` | `Option<Vec<(Ss58Identifier, RegistryPermissions)>>` | All delegates + permissions. |
| `has_registry_permissions(registry, delegate, required)` | `bool` | Bitwise permission validation. |
| `is_delegate(registry, delegate)` | `bool` | Quick membership check. |
| `registry_maintainer(registry)` | `Option<Ss58Identifier>` | Maintainer account ID. |
| `packet_exists(registry, packet)` | `bool` | Existence guard. |
| `packet_status(packet)` | `Option<PacketStatus>` | Packet lifecycle status. |
| `packet_controller(packet)` | `Option<Ss58Identifier>` | Who currently controls the packet. |

#### Packet views (`PacketClientWithSigner`)

| Helper | Return | Notes |
| --- | --- | --- |
| `state(packet_pointer, version)` | `Option<PacketStateViewSdk>` | Convenience wrapper for Register::packet_state. |
| `lookup(registry, digest, version)` | `Option<PacketStateViewSdk>` | Direct digest lookup (same as registry helper). |
| `state_nested(packet_pointer, version)` | `Option<schema::packet::PacketNestedValue>` | Expanded attribute view. |

#### Token views (`TokenClientWithSigner`)

| Helper | Return | Notes |
| --- | --- | --- |
| `timeline(token, start, limit)` | `Option<TokenTimelineViewSdk>` | Event window for a token. |
| `resolve_identifier(token)` | `Option<TokenLookupView>` | Decodes identifiers into (network, pallet, local id). |
| `pallet_index_of(name)` | `Option<u16>` | Reverse-lookup pallet index via name bytes. |
| `pallet_name(index)` | `Option<String>` | Friendly pallet name for an index. |
| `next_pallet_index()` | `Option<u16>` | Next assignable pallet index in runtime metadata. |
| `genesis_network_id()` | `Option<u16>` | Network id shipped in genesis for tokens. |
| `state_version(token)` | `Option<u32>` | Latest token state version. |
| `state_event(token, version)` | `Option<TokenStateEventViewSdk>` | Snapshot event for a specific version. |
| `has_history(token)` | `bool` | Whether timeline/history is maintained. |
| `latest_state_event(token)` | `Option<TokenStateEventViewSdk>` | Most recent state change. |
| `recent_timeline(token, limit)` | `Option<Vec<TokenStateEventViewSdk>>` | Bounded timeline helper. |
| `resolve_pallet(index)` | `Option<String>` | Human-readable name via runtime metadata. |

## Transaction layer (`src/tx`)

### TxClient and AccountTx
- `client.tx().using(signer)` returns `AccountTx`, which owns a per-account queue
  and nonce allocator (`AccountTxQueue` inside `TxPipeline`).
- Submission helpers: `submit`, `submit_with_nonce`, `submit_immediate`,
  `submit_immediate_with_nonce`. Manual nonce management is supported when
  `TxSubmitMode::ManagedQueueWithOverride` or `Manual` is configured.
- Nonce utilities: `warm_nonce`, `allocate_nonce`, `refresh_nonce`.
- Scopes: `.entity()`, `.registry()`, `.packet()`, `.token()`, `.batch()`.
- Accessors: `client()`, `origin_client()`, `signer()`, `config()`.

### Tx pipeline config
- `TxPipelineConfig` governs nonce strategy (`NonceMode::RpcPerTx` or
  `NonceMode::LocalCache`), queue capacity, retry policy for stale nonces,
  max retry attempts, and whether to prewarm nonces on first use.
- `TxSubmitMode` selects between managed queues (default), managed with explicit
  nonce overrides, or fully manual submission.

### TxHandle, TxOutcome, and BatchBuilder
- `TxHandle { hash, wait_in_block, wait_finalized }` wraps a Subxt progress
  stream. Outcomes include `block` (if known) and any decoded events.
- `BatchBuilder` collects `DynamicCall`s and submits them via `Utility::batch`
  or `Utility::batch_all`, exposing `call`, `call_many`, `mode_batch`,
  `mode_batch_all`, `submit`, and `submit_and_wait_finalized`.

### EntityTx (`src/tx/entity.rs`)
- Builders: `set_info_from_input`, `set_info_from_nested`.
- Submitters: `submit_set_info_from_input`, `submit_set_info_from_nested`.
- Attribute ops: `submit_rotate_attribute_from_view`,
  `submit_rotate_attributes_from_nested`, `submit_add_attributes_from_nested`,
  `submit_remove_attribute`.
- Account/controller ops: `submit_set_linked_account`,
  `submit_revoke_linked_account(token_for_force, account, force)`,
  `submit_rotate_controller(token_for_force, new_controller, force)`.
- Maintenance ops: `submit_clear_everything(token_for_force, force)`,
  `submit_set_entity_nym`, `submit_remove_entity_nym`.
- Dynamic escape hatch: `set_info(Value)` and `rotate_attribute(key, Value)` emit
  raw `DynamicCall`s for batching or meta-tx flows.

### RegistryTx (`src/tx/registry.rs`)
- Builders: `create(registry_id, info_bytes)`, `revoke_registry`, `restore_registry`,
  `delete_registry`.
- Submitters: `submit_create`, `submit_create_from_nested`,
  `submit_revoke_registry`, `submit_restore_registry`, `submit_delete_registry`.
- Delegate permissions: `set_delegate_permissions`, `remove_delegate_permissions`
  (builders) plus `submit_set_delegate_permissions` and
  `submit_remove_delegate_permissions`.
- Info updates: `submit_update_registry_info_from_view`.
- Packet issuance shortcut: `submit_packet_from_nested` fetches the registry
  schema via views, validates nested packet data, and dispatches the
  `Register::create_packet` extrinsic.

### PacketTx (`src/tx/packet.rs`)
- Issuance: `submit_issue_from_nested` (schema-aware), `issue` (raw call builder),
  `submit_issue` (raw bytes), and `issue_from_raw` (JSON body validated against
  live schema view).
- Lifecycle: `update_packet` (dynamic builder), `revoke_packet`,
  `restore_packet`, `delete_packet`, `set_packet_status` plus submitters
  (`submit_revoke_packet`, `submit_restore_packet`, `submit_delete_packet`,
  `submit_set_packet_status`).
- Validator: `validate_packet_against_schema` ensures packet attributes match a
  `(key, ElementType, optional)` schema and aborts on extra/missing items.

### TokenTx (`src/tx/token.rs`)
- Attribute rotation: `submit_rotate_attribute(token, key, raw_bytes)` and
  `submit_rotate_attribute_view(token, key, ElementView)` which SCALE-encodes via
  `TokenAttributeInput`.

### Pipeline internals (`src/tx/mod.rs`, `client/pipeline.rs`, `client/nonce.rs`)
- `TxPipeline` holds `AccountTxQueue`s keyed per account, spawns workers, and
  wires the shared `NonceManager`.
- `NonceManager` supports RPC-per-tx fetching or cached increments with periodic
  refresh. Methods: `allocate`, `refresh`, `account_entry`.
- `build_params_with_nonce` and `submit_with_params` wrap Subxt extrinsic
  builders with custom `OriginExtrinsicParams`.

## Extrinsic builder reference (`src/extrinsic`)

### Dynamic call builder
- `DynamicCallBuilder::call(pallet, function, args)` returns a reusable
  `DynamicCall` that can become a `DynamicPayload` or be encoded via
  `DynamicCall::encode_call_data(metadata)`.

### Entity call helpers (`extrinsic::calls::entity`)
- Attribute + info calls: `rotate_attribute_call`, `rotate_attribute_from_element`,
  `rotate_attributes_from_input`, `add_attributes_from_input`,
  `rotate_attribute_from_json`, `rotate_attributes_from_json`,
  `set_info_from_struct`.
- Account/controller maintenance: `remove_attribute_call`,
  `set_linked_account_call`, `revoke_linked_account_call`,
  `revoke_linked_account_for_call`, `rotate_controller_call`,
  `rotate_controller_for_call`, `clear_everything_call`,
  `clear_everything_for_call`, `set_entity_nym_call`, `remove_entity_nym_call`.
- Helpers: `element_to_value`, `entity_info_value`, `attributes_to_value` keep
  SCALE layout aligned with pallet expectations.

### Registry call helpers (`extrinsic::calls::registry`)
- Creation: `create_call` (raw schema/config bytes), `create_from_structs`
  (serde-serialize), `create_from_input` (typed builder).
- Updates and permissions: `update_info_from_input`,
  `set_delegate_permissions_from_input`,
  `remove_delegate_permissions_from_input`.

### Packet call helpers (`extrinsic::calls::packet`)
- Issuance: `issue_call` (JSON validated against schema view),
  `issue_from_flat`, `issue_from_input`.
- Maintenance: `update_from_input`, `revoke_call`, `restore_call`, `delete_call`,
  `set_status_call`.
- JSON validators: `validate_element_type`, `build_packet_attributes`,
  `element_from_json` enforce ElementType compatibility before submission.

### Token call helpers (`extrinsic::calls::token`)
- Attribute rotation: `rotate_attribute_call`, `rotate_attribute_from_json`,
  `rotate_attribute_from_element`.

### Meta-tx helper (`extrinsic::metatx`)
- `MetaTxClient` (documented earlier) lives here and uses `DynamicCall`s to
  build calls that can be signed/relayed out-of-band.

## Meta-transaction internals (`src/tx/meta.rs` and `src/extrinsic/metatx.rs`)

- Constants and enums: `META_TX_VERSION`, `MetadataHashMode` (Disabled/Enabled).
- Mortality + extension layout: `Mortality { era, hash }`,
  `MetaTxBareExt { spec_version, tx_version, genesis_hash, mortality, nonce, metadata, metadata_hash }`
  implements `Encode/Decode` plus `implicit_bytes()` for the additional signed
  payload.
- Wire format: `SignedMetaTxWire { call, extension_version, verify, bare }`.
- Runtime-friendly bundle: `SignedMetaTx { wire, call_value }` with `encode`,
  `decode_with_metadata`, and cloning logic that preserves `Value`.
- Assembly helpers: `build_meta_tx_bare_ext`, `meta_tx_sign_payload`,
  `assemble_meta_tx`, `meta_tx_value_from_signed`,
  `meta_tx_dispatch_arg_type`, `find_meta_tx_type`, and the low-level
  `encode_raw_meta_tx` that mirrors the runtime layout used by
  `MetaTx::dispatch`.
- Verification helpers: `decode_call_value` (uses metadata call type),
  `decode_dispatch_result` (parses MetaTx::Dispatched event payloads).

## Schema transform layer (`src/schema`)

### Entity schema
- `EntityNestedValue` mirrors `EntityInfoView` but keeps nested attributes for
  DX.
- `expand_entity` / `flatten_entity` convert between flat pallet views and
  nested form. `expand_entity_state` returns `(nested_info, attributes, Option<Ss58Identifier>)`.
- `to_entity_input` validates nested data, bounds attribute keys/values, and
  returns `EntityInfoInput` for extrinsics.
- `element_from_view` maps `ElementView` to bounded pallet `Element`.

### Registry schema
- `RegistryNestedSchema` exposes registry id, info, kind, status, attributes,
  token specs, lookup specs, and maintainer in a developer-friendly form.
- `expand_registry` / `flatten_registry` preserve determinism (sorted keys).
- `to_create_input` validates attribute keys, token specs, and lookup specs
  before emitting `RegistryCreateInput`.
- `element_from_view` and `lookup_from_vecs` bound raw bytes and enforce
  pallet-defined limits.

### Packet schema
- `PacketNestedValue { attributes }` parallels the pallet view
  (`Vec<PacketAttributeView>`).
- `expand_packet` / `expand_packet_view` convenience helpers for nested data.
- `flatten_packet` sorts attributes for deterministic extrinsics.
- `validate_and_flatten(nested, schema_view)` checks required/optional
  attributes, rejects unknown keys, and returns bounded
  `PacketAttributesInput`.
- `element_from_view` converts `ElementView` to bounded `PacketElementInput`.

## Shared types and errors (`src/types`)

- Re-export hubs: IDs (`EntityToken`, `RegistryId`, `PacketId`, `TokenId`,
  `OriginAccountId`), identifiers (`Ss58Identifier`, `DecodedIdentifier`),
  view structs (`EntityStateViewSdk`, `RegistryStateViewSdk`, `PacketStateView`,
  `TokenTimelineViewSdk`), and extrinsic input structs (`EntityInfoInput`,
  `RegistryCreateInput`, `PacketAttributesInput`, `DelegatePermissionsInput`,
  `TokenAttributeInput`, etc.).
- `OriginSdkError` centralizes error reporting with variants for connection,
  view, tx, nonce, metadata, schema/validation, encoding/decoding, and config
  issues.
- Supporting modules: `account` (SS58 helpers), `auth` (Authorization builder),
  `packet_input`, `registry`, `token`.

## Utilities (`src/util`)

- `codec`: `element_value_from_json`, `element_view_to_json`, `decode_view` keep
  JSON/documentation-friendly encodings aligned with pallet enums.
- `retry::RetryPolicy` drives connection/event subscription retry loops.
- `ttl::expires_at(current_block, ttl_blocks)` computes block-based expirations
  for Authorization payloads.
- `hex::{encode_hex, decode_hex}` small wrappers used across schema helpers.

## Supporting docs and examples

- `docs/pallet-inventory.md`: pallet-level source of truth (extrinsics,
  storage, view functions). Keep this and the SDK inventory in sync.
- `docs/elements.md`: Element type mapping for schema helpers.
- Example binaries (run with `cargo run -p origin-sdk --example <name> -- --endpoint ws://...`):
  - `demo_entity_simple` with optional `--meta` flag.
  - `demo_registry_packet` end-to-end issuance, uses `examples/data_registry_packet.json`.
  - `demo_token` resolver showcasing the token query module.
Use this document when expanding the SDK surface so that every new module,
helper, extrinsic builder, or schema transform is discoverable and described in
the canonical reference.
