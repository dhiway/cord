# Transactions & Meta-Tx

This guide covers the helpers under `oc::tx` that make composing extrinsics ergonomic.

## TxSubmitter

Handles nonce reservations, retries, and structured logging.

```rust
use oc::tx::{TxSubmitter, TxOptions};

let mut submitter = TxSubmitter::new(&client, &signer);
let call = client.tx().entity_set_entity_nym("demo-nym").await?;

submitter
    .submit_with_progress(call, "Set entity nym", |stage| println!("{stage:?}"))
    .await?;
```

`SubmitStage` events include `Validated`, `Broadcasted`, `InBlock`, `Finalized`, etc. Use them to update CLI output or telemetry.

## TxExecutor (demo utility)

`demo::util::TxExecutor` wraps `TxSubmitter` to support two flows:

- **Direct** – sign & submit locally (default).
- **Relayed** – wrap the call via `MetaTx::dispatch` and submit it using a relayer signer.

If you need the same abstraction in your own project, copy the struct from `src/demo/util.rs`.

## Utility Batch Helpers

```rust
let call1 = client.tx().entity_add_attributes(entries.clone()).await?;
let call2 = client.tx().entity_rotate_attribute(entry).await?;

let batch = client.tx().utility_batch(vec![call1, call2]).await?; // best-effort
let batch_all = client.tx().utility_batch_all(vec![call1, call2]).await?; // fail-fast
```

Submit the returned payload with `TxSubmitter` like any other call.

## Meta Transactions

When relaying on behalf of another signer, use the meta-tx helpers:

```rust
use oc::tx::{MetaTxOptions, MetaSigner};

let call = client.tx().entity_add_attributes(entries).await?;
let meta_payload = client.tx().meta_dispatch(call, &signer, MetaTxOptions::default()).await?;
relayer_submitter.submit_with_progress(meta_payload, "Meta add attributes", handler).await?;
```

Key points:
- `MetaSigner` is implemented for `tx::signer::Keypair`, so any sr25519/ed25519 key can authorise a meta dispatch.
- `MetaTxOptions` lets you override nonce, era, metadata hash, and extension version. Leaving them at `Default::default()` picks sensible values.

## TxOptions

Use `TxOptions` when you need manual control over a single call (without `TxSubmitter`):

```rust
let opts = TxOptions { nonce: Some(NonceMode::Manual(42)), tip: Some(1_000_000_000_000), era: None };
client.tx().sign_and_submit(call, &signer, opts).await?;
```

This is rarely needed, but it’s available for low-level integrations.
