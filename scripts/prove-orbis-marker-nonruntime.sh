#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BASE=f88aa3faa6573582ca690fa3cace58b7f670aa88
MARKER=d89192f79c4979cc475d56ebb7bc84df2c4d4af6
FIXED=/tmp/orbis-v4-equivalence-fixed
TARGET="$ROOT/target/evidence-v4"
OUT=/tmp/orbis-v4-equivalence
mkdir -p "$OUT"
for ITEM in baseline:$BASE marker:$MARKER; do
  NAME=${ITEM%%:*}; COMMIT=${ITEM#*:}
  rm -rf "$FIXED"; mkdir -p "$FIXED"
  git -C "$ROOT" archive "$COMMIT" | tar -x -C "$FIXED"
  (cd "$FIXED" && env -u RUNTIME_METADATA_HASH CARGO_TARGET_DIR="$TARGET" cargo build -p origin-orbis-runtime) >"$OUT/$NAME.log" 2>&1
  cp "$TARGET/debug/wbuild/origin-orbis-runtime/origin_orbis_runtime.wasm" "$OUT/$NAME.wasm"
done
shasum -a 256 "$OUT/baseline.wasm" "$OUT/marker.wasm" | tee "$OUT/sha256.log"
cmp "$OUT/baseline.wasm" "$OUT/marker.wasm"
printf 'cmp_exit_code=0\n' | tee "$OUT/cmp.log"
