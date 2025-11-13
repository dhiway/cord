#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
EXAMPLE_ROOT=$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)
REPO_ROOT=$(cd -- "$EXAMPLE_ROOT/.." &>/dev/null && pwd)

echo "Building origin-rs release binary (metadata-synced)..."
cargo build -p origin-rs --release \
  --target-dir "$EXAMPLE_ROOT/target/subxt-example"

echo "Binary available at $EXAMPLE_ROOT/target/subxt-example/release/origin-rs"
