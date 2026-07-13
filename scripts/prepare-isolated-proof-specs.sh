#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
out="$repo_root/docs/evidence/p1/storage-proof/campaign/generated"
mkdir -p "$out"

"$repo_root/target/release/origin" build-spec \
	--chain origin-local --disable-default-bootnode \
	| "$repo_root/scripts/isolate-proof-chainspec.py" \
	> "$out/origin-proof-isolated.json"

"$repo_root/target/release/origin-orbis" build-spec \
	--chain orbis-local --disable-default-bootnode \
	| "$repo_root/scripts/isolate-proof-chainspec.py" \
	> "$out/orbis-proof-isolated.json"

shasum -a 256 \
	"$out/origin-proof-isolated.json" \
	"$out/orbis-proof-isolated.json" \
	> "$out/SHA256SUMS"
