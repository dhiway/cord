# Orbis metadata no-hash isolation

This crate must be tested with `env -u RUNTIME_METADATA_HASH`, its own
`CARGO_TARGET_DIR=target/nohash`, and `--features runtime-benchmarks`. The test proves an enabled
`CheckMetadataHash` returns exact `UnknownTransaction::CannotLookup` when no compiled hash exists,
even if the invoking parent environment is contaminated with `RUNTIME_METADATA_HASH`.
