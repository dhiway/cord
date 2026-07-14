# Orbis metadata no-hash isolation

This crate must be tested with `env -u RUNTIME_METADATA_HASH`, its own
`CARGO_TARGET_DIR=target/nohash`, and `--features runtime-benchmarks`. The test proves an enabled
`CheckMetadataHash` returns exact `UnknownTransaction::CannotLookup` when no compiled hash exists,
even if the invoking parent environment is contaminated with `RUNTIME_METADATA_HASH`.

The canonical evidence command is:

```sh
env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR=target/nohash cargo test --manifest-path origin/orbis/runtime/metadata-implicit-nohash/Cargo.toml --features runtime-benchmarks tests::enabled_metadata_without_compiled_hash_is_exact_cannot_lookup -- --exact --nocapture
```
