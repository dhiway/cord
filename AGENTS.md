# Repository Guidelines

This repository contains the Rust implementation of the CORD node and related components. Use these guidelines to contribute effectively and keep CI green.

## Project Structure & Module Organization
- `node/`: Node binaries, CLI, RPC, service wiring.
- `runtimes/`: Runtime crates (e.g., Braid, Loom) and chain specs.
- `pallets/`: FRAME pallets specific to CORD.
- `primitives/`, `utilities/`, `test-utils/`: Shared types, helpers, and testing utilities.
- `origin/`: Origin relay/container components.
- `docs/`, `scripts/`, `docker/`, `zombienet/`: Documentation, helper scripts, Docker, and network tests.

## Build, Test, and Development Commands
- Build: `cargo build --release` (produces `./target/release/cord`).
- Run (dev): `./target/release/cord --dev` or `--chain braid-dev|loom-dev`.
- Format check: `cargo +nightly fmt --all -- --check`.
- Lint: `cargo clippy --all --all-targets --features=runtime-benchmarks -- -D warnings`.
- Tests: `cargo test --release --all --all-targets --features=runtime-benchmarks`.
- Useful scripts: `scripts/run-local-cluster.sh`, `scripts/setup-dev-chain.sh`, `scripts/run_benches_for_runtime.sh`.

## Coding Style & Naming Conventions
- Rust 2021 edition; `rust-toolchain.toml` pins components.
- rustfmt: nightly; settings in `.rustfmt.toml` (tabs for Rust, width 100).
- EditorConfig: `.editorconfig` enforces tabs for `*.rs`, spaces elsewhere.
- Naming: `snake_case` for modules/files, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for consts.
- Keep modules small and cohesive; prefer crate-local `mod tests` plus integration tests under `tests/` where applicable.

## Testing Guidelines
- Prefer unit tests near code; integration tests in `crate/tests/`.
- For runtime/pallet benches enable `--features runtime-benchmarks` or use `scripts/run_benches_for_pallets.sh`.
- Ensure deterministic tests; avoid network or time dependencies.

## Commit & Pull Request Guidelines
- Commits: concise imperative subject; optional scope prefix (e.g., `node:`, `runtimes:`, `pallets:`, `docs:`). Example: `loom: fix spec file build issue`.
- PRs: include summary, rationale, linked issues, and risks; note CI-impacting changes (features, benches).
- Before pushing: run format, clippy, and tests (commands above). Update `docs/` if flags, APIs, or behavior change.

## Security & Configuration Tips
- Do not commit secrets; prefer local env/config. See `scripts/config.toml.example`.
- Use Docker for quick validation: `docker run --rm dhiway/cord --version` or run a dev node with mapped ports as in README.
