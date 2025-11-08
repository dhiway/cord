# CORD Subxt Anchor

This crate hosts a Subxt-based walkthrough that exercises the entity, register, and packet pallets end-to-end. It connects to a local `cord` node (usually launched with `cargo run -p node/cli -- --dev --tmp`) and shows, step by step, how identifiers, tokens, registries, and packets are created.

## Running the walkthrough

```bash
# Terminal 1 – start a local node
cargo run -p node/cli -- --dev --tmp

# Terminal 2 – run the Subxt example using Alice as the signer
cargo run -p cord-subxt-anchor -- walkthrough --label demo-001
```

Sample output:

```
┌───────────────┬────────────────────────────────────┬──────────────────────────────────────────────────────────────┐
│ Stage         │ Token                              │ Outcome                                                      │
├───────────────┼────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ Context       │ demo-001                           │ Demonstrating identifiers → registry → packet               │
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

The explainers live in `docs/examples/subxt-anchor/*.md` so they can also be rendered inside other documentation toolchains.

## Quick remark probe

If you just want to verify RPC connectivity and signing without running the full walkthrough, use the lightweight remark probe:

```bash
cargo run -p cord-subxt-anchor --bin remark_probe -- --url ws://127.0.0.1:9944 --signer //Alice --message "ping from probe"
```

It sends a `system.remark` extrinsic with the same signed-extension tuple as the CORD runtime and prints the finalized extrinsic index when the transaction succeeds.

## Refreshing metadata

The Subxt macro consumes `examples/subxt-anchor/metadata/cord.scale`. Regenerate it whenever the runtime changes by pointing `subxt` at a local node:

```bash
subxt metadata --url ws://127.0.0.1:9944 --output examples/subxt-anchor/metadata/cord.scale
```

Commit the refreshed file together with any runtime updates so the example stays in sync.
