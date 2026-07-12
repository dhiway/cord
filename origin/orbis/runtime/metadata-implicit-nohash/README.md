# Orbis metadata implicit no-hash harness

This crate intentionally has no build script and must be compiled without a metadata hash. It uses
`bulletin_pallets_common::resolve_metadata_implicit`, the same no-std helper as the Orbis runtime.
Keep its artifacts isolated from runtime builds:

```sh
env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR=target/nohash \
  cargo test -p orbis-metadata-implicit-nohash --features runtime-benchmarks
env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR=target/nohash \
  cargo bench -p orbis-metadata-implicit-nohash --features runtime-benchmarks --no-run
```

The enabled extension must return exactly `UnknownTransaction::CannotLookup`.
