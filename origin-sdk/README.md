# Origin SDK (dynamic)

Dynamic Subxt SDK focused on view-only reads and async, event-driven extrinsics for Origin runtimes.

## Status
Async, view-first Subxt SDK with dynamic calls, signer-agnostic connection (signers are passed per-call), nonce-less submit queue, batch + meta-tx helpers, and event-driven tx resolution.

## Quickstart
```rust
use origin_sdk::OriginClient;
use origin_sdk::client::signer::MultiKeySigner;
use origin_primitives::Ss58Identifier;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer = MultiKeySigner::from_seed("//Alice")?;
    let client = OriginClient::connect("wss://origin.rpc").await?;

    let entity_id = Ss58Identifier::try_from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW")?;

    // Pure view call, no storage RPC
    let entity = client.query().using(&signer).entity().overview(entity_id).await?;

    // Non-blocking extrinsic helper: returns TxHandle, caller decides when to await.
    let handle = client
        .tx()
        .using(&signer)
        .entity()
        .submit_rotate_attribute_view(
            entity_id,
            b"email",
            origin_primitives::element::ElementView::Raw(b"a@b.c".to_vec()),
        )
        .await?;
    let _inclusion = handle.wait_in_block().await?;

    Ok(())
}
```

## Layout
- `src/client`: connection, signers, nonce-less submit queue, submit/view/event facades.
- `src/extrinsic`: dynamic builder, batch, meta-tx helpers.
- `src/types`: view structs and errors built atop `origin-primitives`.
- `src/util`: retry, codec, ttl, hex helpers.
- `examples/`: tiny end-to-end usage snippets.
- `docs/`: element mapping cheatsheet.

## Demos (run with cargo)
- Entity create/rotate (signer or meta-tx):
  - `cargo run -p origin-sdk --example demo_entity_simple -- --endpoint ws://localhost:9944 --seed //Alice [--meta]`
  - Data: `examples/data_entity.json`
- Registry + Packet end-to-end:
  - `cargo run -p origin-sdk --example demo_registry_packet -- --endpoint ws://localhost:9944 --seed //Alice [--meta]`
  - Data: `examples/data_registry_packet.json`
- Token resolver:
  - `cargo run -p origin-sdk --example demo_token -- --endpoint ws://localhost:9944 --token <ss58>`

## Next Steps
- Fill view engine with pallet view calls (no storage RPC). **Done**
- Wire nonce-less submit-and-watch with event-driven progress handles. **Done**
- Implement meta-tx wrapper for relayer flows. **Available in `extrinsic::metatx`**
- Add integration tests against `./target/release/cord --dev`.

## New typed surface (entity/register/token)
- Nested ↔ flat helpers live in `schema::*`.
- Typed extrinsic inputs live in `types::*_input`; call `tx().using(signer).entity().submit_set_info_from_nested`, `tx().using(signer).registry().submit_create_from_nested`, `tx().using(signer).registry().submit_packet_from_nested`. All return `TxHandle` for caller-managed awaiting.
- Views decode to pallet-aligned structs and can be expanded with `overview_nested` / `details_nested`.
