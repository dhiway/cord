# Demo Playbook

Executable guides that showcase how to use **origin-rs** (aka the Origin Rust Client). Launch a dev node first:

```bash
cargo run -p node/cli -- --dev --tmp
```

Every demo accepts `--node ws://…` plus shared UX flags:

| Flag | Description |
|------|-------------|
| `-m, --mode <tx|view>` | Transaction mode (default) vs. read-only mode (requires identifiers). |
| `-f, --flow <direct|relay>` | Direct signing vs. relayed meta-transaction flow (entity/register demos support both; packet-demo relays maintainer calls and submits the packet directly via the delegate). |
| `-d, --display <less|more>` | Compact vs. full CLI tables. `--json` implies full. |
| `-j, --json` | Emit JSON snapshots instead of tables. |
| `-t, --token` | Entity identifier for `entity-demo` view mode. |
| `-r, --registry` | Registry identifier for view mode (register-demo / packet-demo). |
| `-p, --packet` | Packet identifier for `packet-demo` view mode. |

Behind the scenes, each demo uses the shared helpers from `src/demo/util.rs` (logging, `TxExecutor`, authorizations) and `src/demo/entity.rs` (snapshot rendering).

---

## entity-demo

**Purpose:** bootstrap an entity profile (if needed), rotate attributes, set a nym, and render timelines/history.

**Transaction mode:**
1. Generate a run label (`entity-demo-xxxx`).
2. Build a profile JSON and ensure the entity token exists (`entity_set_info_json`).
3. Plan attribute updates (`AttributePlan`) and submit add/rotate extrinsics.
4. Set the entity nym if missing.
5. Fetch attribute history, token timeline, and linked accounts; render tables or JSON.

**View mode:** requires `--token`. Skips writes and only renders the latest snapshot/history.

```bash
cargo run -p origin-rs --example entity-demo -- --flow relay
cargo run -p origin-rs --example entity-demo -- --mode view --token 5F...
```

---

## register-demo

**Purpose:** mint a registry (with maintainer setup) and inspect info / schema / lookup specs.

**Transaction mode:**
1. Ensure the maintainer entity exists.
2. Build the default registry blueprint.
3. Submit `register_create_registry_json` (direct or relayed) and print structured logs.
4. Query `register.details` + `register.lookup_specs` and display them.

**View mode:** requires `--registry`. Only fetches runtime views.

```bash
cargo run -p origin-rs --example register-demo
cargo run -p origin-rs --example register-demo -- --mode view --registry 5G...
```

---

## packet-demo

**Purpose:** demonstrate the full dependency chain (entity → registry → delegate → packet) and render packet snapshots + token timelines.

**Transaction mode:**
1. Ensure the maintainer entity exists.
2. Mint a registry.
3. Assign the delegate via `register_set_delegate`.
4. Compose packet attributes and submit `register::create_packet` with the delegate signer.
5. Fetch `register.packet_snapshot` + `token.timeline` and render output.

**View mode:** requires both `--registry` and `--packet`.

```bash
cargo run -p origin-rs --example packet-demo
cargo run -p origin-rs --example packet-demo -- \
  --mode view --registry 5G... --packet 5E...
```

---

## Utility Examples

| Example | Description |
|---------|-------------|
| `quickstart.rs` | Prints node identity, runtime version, chain prefix, and metadata hash (connectivity probe). |
| `inspect-view.rs` | Calls arbitrary runtime view functions from the CLI. |
| `update-metadata.rs` | Writes the latest runtime metadata to `origin-rs/metadata/cord.scale`. |

Use these helpers when you want lightweight diagnostics without running the full demos.

---

## Tips

- Switch signing algorithms via `tx::signer::Keypair::dev_with(DevAccount::Alice, KeyAlgorithm::Ed25519)` in the demo source to test ed25519 flows.
- `TxExecutor` handles direct vs. relayed submissions uniformly; explore `src/demo/util.rs` if you plan to build your own CLI.
- Every demo emits machine-readable JSON with `--json`, making it easy to plug into automated smoke tests.

For module-level API docs, see [`docs/sdk/`](../docs/sdk/README.md).
