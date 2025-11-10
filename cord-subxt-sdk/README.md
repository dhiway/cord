# CORD Subxt SDK

This crate hosts a Subxt-based SDK plus walkthrough that exercises the entity, register, and packet pallets end-to-end. It connects to a local `cord` node (usually launched with `cargo run -p node/cli -- --dev --tmp`) and shows, step by step, how identifiers, tokens, registries, and packets are created. The library portion (`cord_subxt_sdk::CordSdk`) is designed to be embedded in other tooling, while the default binary demonstrates a polished developer experience.

## Running the walkthrough

```bash
# Terminal 1 – start a local node
cargo run -p node/cli -- --dev --tmp

# Terminal 2 – run the SDK walkthrough using Alice as the signer
# (each run auto-generates a unique label like `anchor-demo-ab12cd34`)
cargo run -p cord-subxt-sdk -- walkthrough

> Metadata guard: on startup the CLI compares the node's metadata hash with
> `cord-subxt-sdk/metadata/cord.scale`. If they differ you'll see a descriptive
> error; run `cargo run -p cord-subxt-sdk --bin dump_metadata -- --output \
> cord-subxt-sdk/metadata/cord.scale`, rebuild/restart your node, and retry.
```

Sample output:

```
Context: anchor-demo-521e5906 identifiers → registry → packet (run 521e5906)

Entity
  Token    : 4wRytEZPrwsrN4...
  Outcome  : Entity profile set via pallet-entity::set_info
  Profile  : display=CORD showcase for anchor-demo-521e5906; attributes=2

Registry (viewed via RuntimeViewFunction)
  Token    : 5HUAow9wN5Avvd...
  Outcome  : Registry minted with pallet-register::create_registry
  Kind     : Raw
  Status   : Active
  Maintainer: 4wRytEZPrwsrN4...
  Token Spec: [record_id + controller]
  Lookups  : [record_id + controller], [payload_hash], [expires_at]
  Schema   :
    - record_id (Raw)
    - controller (Token)
    - payload_hash (Hash)
    - payload_salt (Raw)
    - expires_at (U64) [optional]
    - notes (Raw) [optional]

Packet (viewed via RuntimeViewFunction)
  Token    : 6DYfsQYqV7Q1Ww...
  Outcome  : Packet anchored under registry 5HUAow9wN5Avvd...
  Controller: 4wRytEZPrwsrN4...
  Status   : Active
  Attributes:
    - record_id = Raw(anchor-demo-521e5906-packet)
    - payload_hash = Hash(0xf0cb...)
    - controller = Token(4wRytEZPrwsrN4...)
    - payload_salt = Raw(salt-anchor-demo-521e5906-...)
    - expires_at = U64(1893456000)
    - notes = None
```

Use the `docs` subcommand to read the concept explainers:

```bash
cargo run -p cord-subxt-sdk -- docs identifiers
```

## Running the standalone examples

Each example expects a local dev node listening on `ws://127.0.0.1:9944`. Launch it once with
`cargo run -p node/cli -- --dev --tmp`, then run any of the samples in a separate terminal:

```bash
# Connectivity probe that prints health/metadata information
cargo run -p cord-subxt-sdk --example quickstart

# Rotate an entity attribute using Alice's dev key
cargo run -p cord-subxt-sdk --example entity-demo

# Create and inspect a fresh registry end-to-end
cargo run -p cord-subxt-sdk --example register-demo

# Mint a registry and submit a packet, then view the packet JSON snapshot
cargo run -p cord-subxt-sdk --example packet-demo
```

Each example hard-codes `ws://127.0.0.1:9944`; edit the file or wrap the `origin::Client::connect`
call if you need to point at a remote endpoint.

### Release binary / reproducible builds

If you prefer to run the prebuilt binary under `cord-subxt-sdk/target/subxt-example`,
rebuild it after every change to the example or the runtime so that the metadata hash and
signed extension tuple stay in sync:

```bash
cord-subxt-sdk/scripts/build-release.sh
```

The explainers live in `docs/examples/subxt-sdk/*.md` so they can also be rendered inside other documentation toolchains.

## Customising walkthrough data

Payloads for every pallet extrinsic now come from `sample_data/demo.json`. The CLI transforms the JSON into the strongly typed structures that the runtime expects, so you can tweak fields without recompiling the binary. To point the walkthrough at a different file, pass `--sample-data /path/to/your.json`. Each JSON template can reference `{label}`, `{base_label}`, and `{run_id}` placeholders, which are resolved per run, and tokens such as the entity or registry identifier can be injected via `"type": "token"` entries. The bundled template defines optional attributes (`expires_at`, `notes`) and multiple lookup specs so you can demonstrate flexible schemas without editing Rust code.

## Quick remark probe

If you just want to verify RPC connectivity and signing without running the full walkthrough, use the lightweight remark probe:

```bash
cargo run -p cord-subxt-sdk --bin remark_probe -- --url ws://127.0.0.1:9944 --signer //Alice --message "ping from probe"
```

It sends a `system.remark` extrinsic with the same signed-extension tuple as the CORD runtime and prints the in-block extrinsic index when the transaction succeeds.

## Refreshing metadata

The Subxt macro consumes `cord-subxt-sdk/metadata/cord.scale`. Regenerate it whenever the
runtime changes so the bindings stay in sync:

```bash
cargo run -p cord-subxt-sdk --bin dump_metadata -- \
  --output cord-subxt-sdk/metadata/cord.scale
```

This dumps the metadata straight from the node you point it at, so there is no runtime build dependency. Commit
the refreshed file together with any runtime updates.

## Troubleshooting

- **`Invalid Transaction (1010)` immediately after submission** – the node is serving an older runtime
  than the metadata baked into `cord.scale`. Regenerate the metadata via
  `cargo run -p cord-subxt-sdk --bin dump_metadata -- --output cord-subxt-sdk/metadata/cord.scale`,
  rebuild/restart your node (or connect to one built from the same revision), and retry.
- **Metadata mismatch error at startup** – the walkthrough detected the issue above before submitting
  anything. Follow the same regeneration/restart steps and rerun the command.
