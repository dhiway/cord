# origin-rs (Origin Rust Client)

`origin-rs`—also referred to as the **Origin Rust Client** or simply **oc**—is a lightweight SDK for composing Origin/CORD transactions and runtime view calls with [Subxt]. It ships with:

- A reconnecting `Client` that guards against metadata drift.
- Multi-algorithm signing helpers (`sr25519` by default, `ed25519` optional).
- Transaction utilities (`TxSubmitter`, `TxExecutor`, meta-transaction helpers).
- Query facades for the entity, register, and token pallets.
- Runnable demos that double as living documentation.

[Subxt]: https://github.com/paritytech/subxt

---

## Quick Start

```bash
# 1. Start a local dev node (new terminal)
cargo run -p node/cli -- --dev --tmp

# 2. Probe RPC connectivity
cargo run -p origin-rs --example quickstart

# 3. Run the entity demo in transaction mode
cargo run -p origin-rs --example entity-demo

# 4. View an existing entity snapshot (requires --token)
cargo run -p origin-rs --example entity-demo -- \
  --mode view --token <identifier>

# 5. Use the unified state viewer (auto-detects entity/registry/packet)
cargo run -p origin-rs --example state -- --token <identifier>
```

All demos accept `--node ws://…` and share the same `--mode` / `--flow` / `--display` / `--json` switches. See the [Demo Playbook](examples/README.md) for details.

---

## Architecture Snapshot

| Layer | Description |
|-------|-------------|
| `Client` | Reconnecting RPC client. Exposes `tx()`/`query()` builders and fetches the chain prefix + metadata hash. |
| `tx::signer` | `Keypair` + `KeyAlgorithm` abstractions for dev, mnemonic, or secret-URI keys. Automatically emits the right `MultiSignature`. |
| `tx::submitter` | `TxSubmitter` + `TxOptions` manage nonce reservation, retries, and structured progress logs. |
| `tx::meta` | `MetaSigner`, `MetaTxOptions`, and `dispatch_call_with_meta` for relayed flows. |
| `query::{entity,register,token}` | Strongly typed view helpers plus `AuthorizationBuilder` to sign view payloads. |
| `demo::{util,entity}` | CLI/logging utilities shared by all examples. |

Complete API notes and usage snippets live under [`docs/sdk/`](../docs/sdk/README.md).

---

## Multi-Algorithm Signers

```rust
use oc::tx::signer::{DevAccount, KeyAlgorithm, Keypair};

// Default: sr25519 dev Alice
let signer = Keypair::dev(DevAccount::Alice);

// Explicit ed25519
let ed = Keypair::dev_with(DevAccount::Alice, KeyAlgorithm::Ed25519);

// Secret URI or mnemonic-style inputs
let custom = Keypair::from_secret_uri(KeyAlgorithm::Sr25519, "//Charlie//demo", None)?;
```

The SDK never asks you to pick a signature scheme per call. When you pass a `Keypair` into `TxSubmitter`, `TxExecutor`, or `AuthorizationBuilder`, the helper inspects the emitted `MultiSignature` bytes and infers the scheme automatically.

---

## Transaction Helpers

```rust
use oc::{Client, ChainFlavor};
use oc::tx::{TxSubmitter, TxOptions};
use oc::tx::signer::{DevAccount, Keypair};

let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
let signer = Keypair::dev(DevAccount::Alice);
let mut submitter = TxSubmitter::new(&client, &signer);
let call = client.tx().entity_set_entity_nym("demo-nym").await?;

submitter
    .submit_with_progress(call, "Set entity nym", |stage| println!("{stage:?}"))
    .await?;
```

For relayed flows, wrap the call via `client.tx().meta_dispatch(call, &meta_signer, MetaTxOptions::default())` and submit the returned payload with your relayer’s `TxSubmitter`.

---

## Runtime Views

1. Fetch the latest block height (e.g., `let reference_block = client.view_auth_reference_block().await?;`) and create an authorization with `demo::util::fresh_authorization(reference_block, &signer)` or `AuthorizationBuilder::generate_view_authorization(&keypair, &AuthorizationBuilder::default_context(), reference_block, None)`.
2. Call a view facade:

```rust
let reference_block = client.view_auth_reference_block().await?;
let auth = fresh_authorization(reference_block, &signer)?;
let details = client
    .query()
    .entity()
    .details(&auth, &token_identifier)
    .await?;
```

Token timelines, register packet snapshots, and lookup specs follow the same pattern.

---

## Demo Playbook

- `entity-demo` – attribute rotation, timeline/history rendering, view mode.
- `register-demo` – maintainer setup, registry mint, schema/lookup inspection.
- `packet-demo` – entity + registry + delegate + packet snapshot with token timeline.
- `state` – single-entry CLI that accepts any token, resolves what it represents, and invokes the right snapshot renderer (entity / registry / packet) with JSON or table output.
- `quickstart` – RPC probe.
- `inspect-view` – ad-hoc runtime view inspector.
- `update-metadata` – writes the latest runtime metadata to `origin-rs/metadata/origin(.hub).scale` based on the connected chain flavor.
- `xcm-token-transfer` – submits an XCM v5 `reserve_transfer_assets` from a hub parachain to a sibling (defaults to Alice on the source hub). Handy for smoke-testing multi-hub setups started via the updated zombienet config.

See [examples/README.md](examples/README.md) for CLI usage and flow diagrams.

### Refreshing Metadata

```bash
cargo run -p origin-rs --example update-metadata -- --node ws://127.0.0.1:9944
```

Commit the refreshed `origin-rs/metadata/origin.scale` (relay) or `origin-rs/metadata/origin-hub.scale` (hub) whenever the runtime changes so the dynamic client can reuse cached metadata offline.

---

## Further Reading

| Doc | Description |
|-----|-------------|
| [`docs/sdk/README.md`](../docs/sdk/README.md) | SDK docs index with per-module guides. |
| [`examples/README.md`](examples/README.md) | Demo/reference CLI guide. |
| `origin-rs/src/` | Source of truth for helpers and module implementations. |

Happy building! Let us know if you need additional modules documented.
