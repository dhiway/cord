#!/usr/bin/env bash
set -euo pipefail
# Pinned to dhiway/sdk release-v1.24.0 cc190ea8 substrate-wasm-builder metadata_hash:
# execute first-pass Wasm Metadata_metadata_at_version(15) and Core_version, extract System SS58,
# then RFC-78 merkleized_metadata with symbol ORU and 10 decimals.
root="$(git rev-parse --show-toplevel)"
target="${CARGO_TARGET_DIR:-$root/target/orbis-metadata}"
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
SKIP_PALLET_REVIVE_FIXTURES=1 CARGO_TARGET_DIR="$target" \
  cargo build -p origin-orbis-runtime --release --features on-chain-release-build -vv \
  2>&1 | tee "$log" >&2
hash="$(grep -o 'RUNTIME_METADATA_HASH=[^ ]*' "$log" | tail -1 | cut -d= -f2 | tr -d "'\"")"
test -n "$hash"
printf '%s\n' "$hash"
