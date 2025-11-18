# SDK Operations (origin-rs)

This page gives you the practical surface of `origin-rs`: how to connect, build authorizations, compose extrinsics for entity/register/packet flows, and read runtime views. Pair it with [`docs/origin-overview.md`](../origin-overview.md) when you need pallet-level background.

## Connect once, reuse everywhere

```rust
use oc::{Client, ChainFlavor};

// Auto-detects hub/relay flavor and validates runtime metadata hash.
let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;

// Namespaced builders share the same reconnecting RPC layer.
let tx = client.tx();      // extrinsic composer
let view = client.query(); // runtime views
```

- `Client::connect` rejects mismatched metadata and exposes `runtime_version()` + `chain_prefix()` helpers for diagnostics/UI.
- Use `origin-rs/examples/quickstart.rs` if you just want a connectivity probe.

## View authorizations

All runtime views require a signed authorization payload. Use the SDK helper to mint one with an explicit context (pallet + view name) and a fresh reference block:

```rust
use oc::query::auth::AuthorizationBuilder;
use sp_core::Pair; // any Subxt signer works

let auth = AuthorizationBuilder::generate_view_authorization(
    &signer,
    &AuthorizationBuilder::view_context("Entity", "details"),
    client.view_auth_reference_block().await?, // helper that returns a fresh block height
    None, // auto nonce
)?;
```

`AuthorizationRequest` values plug directly into the query builders below. The demos expose a convenience helper: `demo::util::fresh_authorization_with_client(&client, &signer)` (good for quick scripts).

## Extrinsics (tx) by domain

**Entity (identity & accounts)**

- `entity_set_info_json(json)` – mint entity + profile packet in one call (expects JSON with `display`, `web`, `email`, optional `attributes` map).
- `entity_add_attributes(entries)` / `entity_rotate_attributes(entries)` / `entity_rotate_attribute(entry)` / `entity_remove_attribute_utf8|hex(key)` – CRUD per-attribute values.
- `entity_set_linked_account(account_ss58)` / `entity_revoke_linked_account(account_ss58)` – manage linked accounts.
- `entity_rotate_controller(token, new_controller)` – hand off control to another account.
- `entity_clear_everything(token)` – purge profile + bindings (controller only).
- `entity_set_entity_nym(prefix)` / `entity_remove_entity_nym(token)` – manage `.nym.org.in` names.

**Register (registries & delegates)**

- `register_create_registry_json(spec)` – create a registry from a JSON blueprint (info blob, attribute schema, token spec, lookup specs).
- `register_set_delegate(registry, delegate_account, roles)` / `register_remove_delegate(registry, delegate_token)` – maintain delegate roles.
- `register_update_info_json|text(registry, info)` – rotate the registry info element.
- Lifecycle: `register_revoke`, `register_restore`, `register_delete`.

**Packet (registry-scoped records)**

- `packet_create_json(registry, attributes, schema_view)` – compose + create a packet that satisfies registry schema (returns a `DynamicPayload` for submission).
- `packet_update_json(registry, packet, attributes, schema_view)` – patch attributes while preserving schema constraints.
- Lifecycle: `packet_revoke`, `packet_restore`, `packet_remove`.

**Submission helpers**

- `TxSubmitter` manages nonce, retries, and progress hooks.
- Use `TxExecutor` (from `src/demo/util.rs`) when you want built-in meta-tx vs. direct flows without re-writing glue.
- Batch calls with `tx.utility_batch`/`tx.utility_batch_all`, or wrap any call in `tx.meta_dispatch` for relayed execution.

## View operations (query) by domain

**Entity queries** (`client.query().entity()`)
- `details(auth, token)` – latest profile packet.
- `linked_accounts`, `controller_account`, `account_token(account)`, `account_history`.
- Attribute history helpers: `attribute_history`, `attribute_history_for_key`, `attribute_history_entry`, `attribute_version`, `attribute_versions`.
- Nym helpers: `entity_nym(token)`, `entity_nym_lookup(auth, raw_nym_bytes)`.

**Register queries** (`client.query().register()`)
- Registry surface: `details`, `schema` (helper), `lookup_specs`, `overview` (info + lookup specs), `delegate_permissions`, `query_count`, `token_fingerprint`.
- Schema introspection: `attribute` (single key) and `attributes` (all keys with type + optional flag).
- Packet views: `packet_snapshot`, `packet_snapshot_by_token`, `packet_metadata`.
- Lookup anchors: `lookup_snapshot` / `list_by_token` / `list_by_digest` (see type definitions under `origin_primitives::registry`).

**Token queries** (`client.query().token()`)
- `resolve_identifier` / `resolve_pallet` – decode what a token points to.
- Timeline + state: `state_version`, `timeline` (with cursor + limit), `history` helper.
- Network config: `genesis_network_id`, `pallet_index_of`, `pallet_name`, `next_pallet_index`.

All query builders return typed records; failures map to `Error::View` variants that preserve the runtime’s `AuthorizationError` code for easy branching.

## Putting it together (minimal flow)

```rust
use oc::{Client, ChainFlavor};
use oc::tx::signer::{Keypair, DevAccount};
use oc::tx::TxSubmitter;
use origin_primitives::view_api::{AuthorizationRequest, EntityAccountTokenRequest, TokenTimelineRequest};

let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
let signer = Keypair::dev(DevAccount::Alice);

// 1) Bootstrap an entity if needed.
let set_info = client.tx().entity_set_info_json(serde_json::json!({
    "display": "Demo Org",
    "email": "ops@example.com",
    "attributes": { "country": "US" }
})).await?;
TxSubmitter::new(&client, &signer)
    .submit_with_progress(set_info, "set entity info", |_| {})
    .await?;

// 2) Fetch profile + timeline.
let auth = oc::query::auth::AuthorizationBuilder::generate_view_authorization(
    &signer,
    &oc::query::auth::AuthorizationBuilder::view_context("Entity", "details"),
    client.view_auth_reference_block().await?,
    None,
)?;
let entity_token = client.query().entity().account_token(&EntityAccountTokenRequest {
    auth: auth.clone(),
    account: signer.account_id(),
}).await?
    .expect("entity token exists");
let profile = client.query().entity().details(&auth, &entity_token).await?;
let (events, _cursor) = client.query().token().timeline(&TokenTimelineRequest {
    auth: auth.clone(),
    token: entity_token.clone(),
    start: None,
    limit: Some(10),
}).await?;
println!("Profile: {profile:#?}\nRecent events: {events:#?}");
```

For richer CLI examples, run the bundled demos in `origin-rs/examples`: `entity-demo`, `register-demo`, `packet-demo`, and `state` (view-only resolver).
