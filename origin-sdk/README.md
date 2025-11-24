# Origin SDK (dynamic)

Dynamic Subxt SDK focused on view-only reads and async, event-driven extrinsics for Origin runtimes.

## Status
Async, view-first Subxt SDK with dynamic calls, signer-agnostic connection, nonce queue, batch + meta-tx helpers.

## Quickstart
```rust
use origin_sdk::OriginClient;
use origin_sdk::client::signer::MultiKeySigner;
use origin_primitives::Ss58Identifier;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer = MultiKeySigner::from_seed("//Alice")?;
    let client = OriginClient::connect("ws://localhost:9944").await?;

    let entity_id = Ss58Identifier::try_from("5FLSigC9H8J9tDFkhiBSGAL7iFusJqSQuJtVUXwwc7G7R6nW")?;

    // Pure view call, no storage RPC
    let entity = client.query().using(&signer).entity().overview(entity_id).await?;

    // One-liner extrinsic
    client
        .query()
        .using(&signer)
        .entity()
        .tx()
        .submit_rotate_attribute(entity_id, b"email", scale_value::Value::from_bytes(b"a@b.c"))
        .await?;

    Ok(())
}
```

## Layout
- `src/client`: connection, signers, nonce queue, submit/view/event facades.
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
- Fill view engine with pallet view calls (no storage RPC).
- Wire nonce queue + submit-and-watch with event filters.
- Implement meta-tx wrapper for relayer flows.
- Add integration tests against `./target/release/cord --dev`.
