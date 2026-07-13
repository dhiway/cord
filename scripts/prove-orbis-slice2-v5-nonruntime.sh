#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BASE=317a3a5b3dda0964958e08b94c683b1cc1b8e387
MARKER=ce60319f85a13b29e143a4d15189616f38abe777
FIXED=/tmp/orbis-v5-slice2-equivalence-fixed
TARGET=/tmp/orbis-v5-slice2-equivalence-target
OUT=/tmp/orbis-v5-slice2-equivalence-output
PROOF="$ROOT/docs/evidence/orbis-v5/marker-nonruntime-equivalence.json"
TOOLCHAIN=1.93.0
LOCK=/tmp/orbis-v5-slice2-equivalence.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "another Orbis v5 equivalence proof owns $LOCK" >&2
  exit 73
fi
trap 'rm -rf "$LOCK"' EXIT HUP INT TERM
rm -rf "$OUT"
mkdir -p "$OUT"
# Poison both reusable paths. Each cold build must delete the poison before extracting/building.
rm -rf "$FIXED" "$TARGET"
mkdir -p "$FIXED" "$TARGET"
printf poison >"$FIXED/POISON"
printf poison >"$TARGET/POISON"
for ITEM in baseline:$BASE marker:$MARKER; do
  NAME=${ITEM%%:*}; COMMIT=${ITEM#*:}
  rm -rf "$FIXED" "$TARGET"
  mkdir -p "$FIXED" "$TARGET"
  test ! -e "$FIXED/POISON"
  test ! -e "$TARGET/POISON"
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
python3 - "$ROOT" "$OUT" "$PROOF" "$BASE" "$MARKER" "$TOOLCHAIN" "$LOCK" "${1:-}" <<'PY'
import hashlib,json,pathlib,subprocess,sys
root,out,proof,base,marker,toolchain,lock,mode=sys.argv[1:]
root=pathlib.Path(root); out=pathlib.Path(out); proof=pathlib.Path(proof)
def git(*args,text=False): return subprocess.check_output(['git','-C',str(root),*args],text=text)
def show(commit,path): return git('show',f'{commit}:{path}')
b=(out/'baseline.wasm').read_bytes(); m=(out/'marker.wasm').read_bytes()
if b!=m: raise SystemExit('cold A/marker compact runtime Wasm mismatch')
diff=git('diff','--name-only',base,marker,text=True).splitlines()
allowed=[
 'docs/evidence/orbis-v5/S2-BENCHMARK-01.json',
 'docs/evidence/orbis-v5/S2-BENCHMARK-01.out',
 'docs/evidence/orbis-v5/S2-FIXTURES-01.json',
 'docs/evidence/orbis-v5/S2-FIXTURES-01.out',
 'docs/evidence/orbis-v5/S2-MIGRATIONS-01.json',
 'docs/evidence/orbis-v5/S2-MIGRATIONS-01.out',
 'docs/evidence/orbis-v5/S2-PAYOUT-01.json',
 'docs/evidence/orbis-v5/S2-PAYOUT-01.out',
 'docs/evidence/orbis-v5/S2-SURFACES-01.json',
 'docs/evidence/orbis-v5/S2-SURFACES-01.out',
 'docs/evidence/orbis-v5/architect-delta-approval.md',
 'docs/evidence/orbis-v5/architect-product-clear.md',
 'docs/evidence/orbis-v5/critic-delta-approval.md',
 'docs/evidence/orbis-v5/critic-product-clear.md',
 'docs/evidence/orbis-v5/marker-nonruntime-equivalence.json',
 'docs/evidence/orbis-v5/verification-report.json',
 'docs/evidence/orbis-v5/verification-report.sha256',
 'docs/orbis-completion-manifest.toml',
 'origin/orbis/evidence_inventory_v5.rs',
 'origin/orbis/evidence_markers_v5.rs',
 'origin/orbis/pallets/score/src/tests.rs',
 'origin/orbis/runtime/src/meta_v6_fixtures.rs',
 'origin/orbis/runtime/src/tests.rs',
 'scripts/prove-orbis-slice2-v5-nonruntime.sh',
 'scripts/test-verify-orbis-completion-v5.py',
 'scripts/verify-orbis-completion-v5.py',
]
if diff!=allowed: raise SystemExit('unexpected A-to-marker source diff: '+repr(diff))
actual={
 'schema':'orbis-slice2-v5-cold-equivalence-v1',
 'runtime_baseline_commit':base,
 'evidence_marker_commit':marker,
 'toolchain':subprocess.check_output(['rustc',f'+{toolchain}','--version'],text=True).strip()+'; '+subprocess.check_output(['cargo',f'+{toolchain}','--version'],text=True).strip(),
 'cargo_lock_sha256':hashlib.sha256(show(base,'Cargo.lock')).hexdigest(),
 'marker_cargo_lock_sha256':hashlib.sha256(show(marker,'Cargo.lock')).hexdigest(),
 'cargo_lock_equal':show(base,'Cargo.lock')==show(marker,'Cargo.lock'),
 'features':'default',
 'deterministic_env':'env -u RUNTIME_METADATA_HASH; CARGO_INCREMENTAL=0; SOURCE_DATE_EPOCH=0; TZ=UTC; LC_ALL=C',
 'build_command':'cargo +1.93.0 build --locked --frozen -p origin-orbis-runtime',
 'same_source_path':'/tmp/orbis-v5-slice2-equivalence-fixed',
 'same_target_path':'/tmp/orbis-v5-slice2-equivalence-target',
 'exclusive_lock_path':lock,
 'cold_target_removed_before_each_build':True,
 'poison_control_removed':True,
 'source_diff':diff,
 'source_diff_sha256':hashlib.sha256(('\n'.join(diff)+'\n').encode()).hexdigest(),
 'baseline_wasm_sha256':hashlib.sha256(b).hexdigest(),
 'marker_wasm_sha256':hashlib.sha256(m).hexdigest(),
 'wasm_size_bytes':len(b),
 'cmp_exit_code':0,
}
committed=json.loads(proof.read_text())
if mode=='--write':
 proof.write_text(json.dumps(actual,sort_keys=True,indent=2)+'\n')
 print(json.dumps(actual,sort_keys=True)); raise SystemExit(0)
if mode!='--check': raise SystemExit('usage: prove-orbis-slice2-v5-nonruntime.sh --check|--write')
if committed!=actual: raise SystemExit('committed cold equivalence proof drift')
print(json.dumps(actual,sort_keys=True))
PY
