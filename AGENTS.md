# Repository Guidelines

## Project Structure & Module Organization
- `node/`: binaries, CLI, RPC entrypoints, and service wiring.
- `runtimes/`: runtime crates (Braid, Loom), chain specs, and benchmark configs.
- `pallets/`: FRAME pallets unique to CORD; keep pallets cohesive with local `mod tests` plus `tests/` suites.
- `primitives/`, `utilities/`, `test-utils/`: shared types, helpers, and testing harnesses.
- `origin/`: relay/container components; coordinate with node services.
- Supporting directories: `docs/`, `scripts/`, `docker/`, `zombienet/` for documentation, automation, containers, and network tests.

## Build, Test, and Development Commands
- `cargo build --release`: full optimized build, produces `./target/release/cord`.
- `./target/release/cord --dev` or `--chain braid-dev|loom-dev`: run a development node locally.
- `cargo +nightly fmt --all -- --check`: enforce repo rustfmt settings (tabs, width 100).
- `cargo clippy --all --all-targets --features=runtime-benchmarks -- -D warnings`: lint with production flags.
- `cargo test --release --all --all-targets --features=runtime-benchmarks`: run full suite; deterministic tests only.
- Scripts: `scripts/run-local-cluster.sh`, `scripts/setup-dev-chain.sh`, and `scripts/run_benches_for_runtime.sh` for multi-node smoke, chain setup, and benches.

## Coding Style & Naming Conventions
- Rust 2021 toolchain pinned via `rust-toolchain.toml`; respect `.rustfmt.toml` and `.editorconfig` (tabs for `.rs`, spaces elsewhere).
- Naming: `snake_case` modules/files, `PascalCase` types/traits, `SCREAMING_SNAKE_CASE` consts.
- Keep modules focused; prefer crate-local `mod tests` and limit cross-crate coupling to primitives/utilities.

## Testing Guidelines
- Unit tests live beside code; integration tests in `crate/tests/` or `tests/` directories.
- Enable benches or runtime-specific checks with `--features runtime-benchmarks` or helper scripts.
- Avoid nondeterminism (no network/time) so CI stays green.

## Commit & Pull Request Guidelines
- Commit subjects: imperative, optional scope prefix (e.g., `node: improve RPC logging`).
- Before pushing: run fmt, clippy, and tests listed above; document behavior or flag changes under `docs/` when relevant.
- PRs: summarize rationale, link issues, describe risks (runtime changes, new benches), and note CI-impacting toggles; provide screenshots/logs for user-facing changes.

## Security & Configuration Tips
- Never commit secrets; use local env vars or templates like `scripts/config.toml.example`.
- Quickly validate Docker images with `docker run --rm dhiway/cord --version`; expose ports per README when running dev nodes.
