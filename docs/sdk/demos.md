# Demo Architecture

The executable demos live under `origin-rs/examples/` and share common utilities so they can stay focused on business logic. Use them as blueprints for your own CLIs.

## Shared Utilities

- `demo::util::TxExecutor` – abstracts direct vs. relayed submissions.
- `demo::util::LogSink` – consistent log formatting (`Validated`, `Broadcasted`, etc.).
- `demo::util::fresh_authorization_with_client` – fetches the latest reference block and wraps `AuthorizationBuilder` for runtime views.
- `demo::entity` – snapshot structs + renderers used by both `entity-demo` and the global `state` viewer.
- `demo::register` – formatting helpers for registry snapshots (kind/status, schema, lookup specs, token timelines).

## Example Breakdown

| Example | Key Concepts |
|---------|--------------|
| `entity-demo.rs` | Entity bootstrap, attribute planning (`AttributePlan`), nym management, view-only mode, JSON output. |
| `register-demo.rs` | Maintainer check, registry blueprint JSON, schema + lookup inspection. |
| `packet-demo.rs` | Maintainer + registry + delegate chain, packet creation via delegate signer, packet snapshot rendering, token timeline fetch. |
| `state.rs` | Unified view-only CLI. Accepts a token, resolves its type, and dispatches to the entity/registry/packet snapshot renderers. |
| `quickstart.rs` | RPC sanity check (identity, runtime version, metadata hash, chain prefix). |
| `inspect-view.rs` | Run arbitrary view calls from the CLI (handy for new pallets). |
| `update-metadata.rs` | Simple helper that writes the latest metadata blob to `metadata/cord.scale`. |

## Customising Flows

- To test ed25519 end-to-end, change `let signer = tx::signer::dev(…)` to `dev_with(…, KeyAlgorithm::Ed25519)` in the demo source.
- Adjust logging or progress output by editing `LogSink` once—the changes propagate to every demo.
- Demos accept `--json` so you can script them in CI and diff the results.

For CLI flag descriptions and walkthroughs, see [`origin-rs/examples/README.md`](../../origin-rs/examples/README.md).
