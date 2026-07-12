#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BASE=f88aa3faa6573582ca690fa3cace58b7f670aa88
MARKER=28a00e425d86092eddd08a7bfb10c09fa467450e
FIXED=/tmp/orbis-v4-equivalence-fixed
TARGET=/tmp/orbis-v4-equivalence-target
OUT=/tmp/orbis-v4-equivalence-output
PROOF="$ROOT/docs/evidence/orbis-v4/marker-nonruntime-equivalence.json"
TOOLCHAIN=1.93.0
LOCK=/tmp/orbis-v4-equivalence.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "another Orbis equivalence proof owns $LOCK" >&2
  exit 73
fi
trap 'rm -rf "$LOCK"' EXIT HUP INT TERM
rm -rf "$OUT"
mkdir -p "$OUT"
for ITEM in baseline:$BASE marker:$MARKER; do
  NAME=${ITEM%%:*}; COMMIT=${ITEM#*:}
  # Deliberately remove all source and target state before each build. Any poison is discarded.
  rm -rf "$FIXED" "$TARGET"
  mkdir -p "$FIXED" "$TARGET"
  git -C "$ROOT" archive "$COMMIT" | tar -x -C "$FIXED"
  (
    cd "$FIXED"
    env -u RUNTIME_METADATA_HASH \
      CARGO_INCREMENTAL=0 SOURCE_DATE_EPOCH=0 TZ=UTC LC_ALL=C \
      CARGO_TARGET_DIR="$TARGET" \
      cargo +"$TOOLCHAIN" build --locked --frozen -p origin-orbis-runtime
  ) >"$OUT/$NAME.log" 2>&1
  cp "$TARGET/debug/wbuild/origin-orbis-runtime/origin_orbis_runtime.wasm" "$OUT/$NAME.wasm"
done
python3 - "$ROOT" "$OUT" "$PROOF" "$BASE" "$MARKER" "$TOOLCHAIN" "${1:-}" <<'PY'
import hashlib,json,pathlib,subprocess,sys
root,out,proof,base,marker,toolchain,mode=sys.argv[1:]
root=pathlib.Path(root); out=pathlib.Path(out); proof=pathlib.Path(proof)
b=(out/'baseline.wasm').read_bytes(); m=(out/'marker.wasm').read_bytes()
if b!=m: raise SystemExit('cold baseline/marker compact runtime Wasm mismatch')
def git_show(commit,path): return subprocess.check_output(['git','-C',str(root),'show',f'{commit}:{path}'])
actual={
 'runtime_baseline_commit':base,
 'evidence_marker_commit':marker,
 'toolchain':subprocess.check_output(['rustc',f'+{toolchain}','--version'],text=True).strip()+'; '+subprocess.check_output(['cargo',f'+{toolchain}','--version'],text=True).strip(),
 'cargo_lock_sha256':hashlib.sha256(git_show(base,'Cargo.lock')).hexdigest(),
 'marker_cargo_lock_sha256':hashlib.sha256(git_show(marker,'Cargo.lock')).hexdigest(),
 'sdk_revision':'cc190ea8',
 'features':'default',
 'deterministic_env':'env -u RUNTIME_METADATA_HASH; CARGO_INCREMENTAL=0; SOURCE_DATE_EPOCH=0; TZ=UTC; LC_ALL=C',
 'build_command':'cargo +1.93.0 build --locked --frozen -p origin-orbis-runtime',
 'baseline_wasm_sha256':hashlib.sha256(b).hexdigest(),
 'marker_wasm_sha256':hashlib.sha256(m).hexdigest(),
 'wasm_size_bytes':len(b),
 'cmp_exit_code':0,
 'cold_target_removed_before_each_build':True,
}
committed=json.loads(proof.read_text())
if mode=='--write':
 committed.update(actual)
 proof.write_text(json.dumps(committed,sort_keys=True,indent=2)+'\n')
 print(json.dumps(actual,sort_keys=True))
 raise SystemExit(0)
if mode!='--check': raise SystemExit('usage: prove-orbis-marker-nonruntime.sh --check|--write')
for key,value in actual.items():
 if committed.get(key)!=value: raise SystemExit(f'committed proof mismatch {key}: {committed.get(key)!r} != {value!r}')
print(json.dumps(actual,sort_keys=True))
PY
