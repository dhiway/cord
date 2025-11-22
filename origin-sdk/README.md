# Origin SDK (dynamic)

Dynamic Subxt SDK focused on view-only reads and async, event-driven extrinsics for Origin runtimes.

## Status
Initial scaffold following the modernization plan (view-only API, dynamic calls, nonce queue, batch + meta-tx hooks). Implementations are stubbed and ready for iterative build-out.

## Quickstart
```rust
use origin_sdk::OriginClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signer = MultiKeySigner::from_seed("//Alice", "")?;
    let client = OriginClient::connect("ws://localhost:9944", signer).await?;
    let _ = client.view().call::<origin_sdk::types::EntityStateView>("Entity", "overview", ()).await;
    Ok(())
}
```

## Layout
- `src/client`: connection, signers, nonce queue, submit/view/event facades.
- `src/extrinsic`: dynamic builder, batch, meta-tx helpers.
- `src/types`: view structs and errors built atop `origin-primitives`.
- `src/util`: retry, codec, ttl, hex helpers.
- `examples/`: tiny end-to-end usage snippets.

## Next Steps
- Fill view engine with pallet view calls (no storage RPC).
- Wire nonce queue + submit-and-watch with event filters.
- Implement meta-tx wrapper for relayer flows.
- Add integration tests against `./target/release/cord --dev`.
