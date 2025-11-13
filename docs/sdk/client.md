# Client Guide

The `Client` type (re-exported as `oc::Client`) wraps Subxt’s reconnecting RPC client with Origin-specific metadata guards.

## Connecting

```rust
use oc::{Client, ChainFlavor};

// Auto-detects whether you’re talking to Orb, Loom, etc.
let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
```

`Client::connect` performs:
1. JSON-RPC websocket handshake (with automatic retry + backoff).
2. Metadata hash validation against `origin-rs/metadata/cord.scale`.
3. Chain-prefix discovery (`client.chain_prefix().await`).

If the metadata hash differs, the call returns an error containing both hashes. Refresh the local metadata with `cargo run -p origin-rs --example update-metadata` and restart your node/build.

## Namespaced Builders

```rust
let tx = client.tx();      // Transaction helper facade
let query = client.query(); // Runtime view facade
```

Both builders share the same reconnecting RPC state, so they automatically retry when the node restarts.

## Chain Prefix & Runtime Version

```rust
let ss58_prefix = client.chain_prefix().await; // Ss58AddressFormat
let runtime_version = client.runtime_version().await?;
```

Use this to format linked accounts (`utils::format_account`) or when rendering CLI output.

## Metadata Blob

Utilities such as `examples/update-metadata.rs` use `client.fetch_metadata_blob().await` to dump the raw SCALE metadata onto disk. This is the same blob Subxt uses internally.

## Error Handling

Most methods return `Error`, which categorises transport, RPC, codec, signer, parameter, and timeout failures. Pattern-match on the variant or display the error string; it already includes context such as pallet/call names.
