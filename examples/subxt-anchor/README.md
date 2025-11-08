# CORD Subxt Anchor

This crate hosts a Subxt-based walkthrough that exercises the entity, register, and packet pallets end-to-end. It connects to a local `cord` node (usually launched with `cargo run -p node/cli -- --dev --tmp`) and shows, step by step, how identifiers, tokens, registries, and packets are created.

## Running the walkthrough

```bash
# Terminal 1 – start a local node
cargo run -p node/cli -- --dev --tmp

# Terminal 2 – run the Subxt example using Alice as the signer
# (each run auto-generates a unique label like `anchor-demo-ab12cd34`)
cargo run -p cord-subxt-anchor -- walkthrough

> Metadata guard: on startup the CLI compares the node's metadata hash with
> `examples/subxt-anchor/metadata/cord.scale`. If they differ you'll see a descriptive
> error; run `cargo run -p cord-subxt-anchor --bin dump_metadata -- --output \
> examples/subxt-anchor/metadata/cord.scale`, rebuild/restart your node, and retry.
```

Sample output:

```
┌───────────────┬────────────────────────────────────┬──────────────────────────────────────────────────────────────┐
│ Stage         │ Token                              │ Outcome                                                      │
├───────────────┼────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ Context       │ anchor-demo-ab12cd34               │ Identifiers → registry → packet (run ab12cd34)              │
│ Identifiers   │ 4wRytEZPrwsrN4...                  │ Entity profile set via pallet-entity::set_info               │
│ Registers     │ 5HUAow9wN5Avvd...                  │ Registry minted with pallet-register::create_registry        │
│ Packets       │ 6DYfsQYqV7Q1Ww...                  │ Packet anchored under registry 5HUAow9wN5Avvd...             │
│ Packet        │ 6DYfsQYqV7Q1Ww...                  │ Packet anchored and ready for pallet testing                 │
└───────────────┴────────────────────────────────────┴──────────────────────────────────────────────────────────────┘
```

Use the `docs` subcommand to read the concept explainers:

```bash
cargo run -p cord-subxt-anchor -- docs identifiers
```

### Release binary / reproducible builds

If you prefer to run the prebuilt binary under `examples/subxt-anchor/target/subxt-example`,
rebuild it after every change to the example or the runtime so that the metadata hash and
signed extension tuple stay in sync:

```bash
examples/subxt-anchor/scripts/build-release.sh
```

The explainers live in `docs/examples/subxt-anchor/*.md` so they can also be rendered inside other documentation toolchains.

## Quick remark probe

If you just want to verify RPC connectivity and signing without running the full walkthrough, use the lightweight remark probe:

```bash
cargo run -p cord-subxt-anchor --bin remark_probe -- --url ws://127.0.0.1:9944 --signer //Alice --message "ping from probe"
```

It sends a `system.remark` extrinsic with the same signed-extension tuple as the CORD runtime and prints the in-block extrinsic index when the transaction succeeds.

## Refreshing metadata

The Subxt macro consumes `examples/subxt-anchor/metadata/cord.scale`. Regenerate it whenever the
runtime changes so the bindings stay in sync:

```bash
cargo run -p cord-subxt-anchor --bin dump_metadata -- \
  --output examples/subxt-anchor/metadata/cord.scale
```

This dumps the metadata straight from the checked-in runtime (Wasm build not required). If you want
to audit against a running node instead, you can still use `subxt metadata --url …` as before. Commit
the refreshed file together with any runtime updates.

## Troubleshooting

- **`Invalid Transaction (1010)` immediately after submission** – the node is serving an older runtime
  than the metadata baked into `cord.scale`. Regenerate the metadata via
  `cargo run -p cord-subxt-anchor --bin dump_metadata -- --output examples/subxt-anchor/metadata/cord.scale`,
  rebuild/restart your node (or connect to one built from the same revision), and retry.
- **Metadata mismatch error at startup** – the walkthrough detected the issue above before submitting
  anything. Follow the same regeneration/restart steps and rerun the command.
