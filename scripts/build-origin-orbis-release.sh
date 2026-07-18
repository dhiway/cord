#!/usr/bin/env bash
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

set -euo pipefail

readonly SRTOOL_DIGEST="docker.io/paritytech/srtool@sha256:8638a668bd6d29111dc01953fbead6eb08c062e1cc62d3047a245a52b6edb3bf"
readonly SRTOOL_IMAGE_ID="sha256:8638a668bd6d29111dc01953fbead6eb08c062e1cc62d3047a245a52b6edb3bf"
readonly SRTOOL_LOCAL_REPOSITORY="cord-srtool-pinned"
readonly SRTOOL_RUST_TAG="1.93.0"
readonly SRTOOL_LOCAL_IMAGE="$SRTOOL_LOCAL_REPOSITORY:$SRTOOL_RUST_TAG"
readonly PROFILE="release"
# Serial Cargo scheduling prevents an intermittent missing dependency artifact
# under amd64 Docker emulation while retaining fresh no-cache targets.
readonly SRTOOL_CARGO_JOBS="1"
readonly FOUNDATION_PACKAGE="origin-foundation-runtime"
readonly FOUNDATION_DIR="origin/base/runtime"
readonly COMMONS_PACKAGE="origin-commons-runtime"
readonly COMMONS_DIR="origin/orbis/runtime"

root="$(git rev-parse --show-toplevel)"
physical_root="$(cd "$root" && pwd -P)"
if [[ "$(pwd -P)" != "$physical_root" ]]; then
	echo "run this release entrypoint from the CORD workspace root: $physical_root" >&2
	exit 2
fi
readonly output_dir="$root/release-artifacts/origin-orbis"

for command in awk cmp cp dirname docker git mkdir mktemp mv pwd python3 rm rmdir shasum srtool tee; do
	command -v "$command" >/dev/null || {
		echo "missing required command: $command" >&2
		exit 2
	}
done

srtool_version="$(srtool --version)"
if [[ "$srtool_version" != "srtool-cli 0.13.2" ]]; then
	echo "srtool-cli 0.13.2 is required, got: $srtool_version" >&2
	exit 2
fi

if [[ -n "$(git -C "$root" status --porcelain --untracked-files=all)" ]]; then
	echo "canonical release builds require a clean CORD worktree" >&2
	exit 2
fi

if [[ -e "$output_dir" ]]; then
	echo "refusing to replace an existing release evidence directory: $output_dir" >&2
	exit 2
fi

verify_pinned_image() {
	local identity
	identity="$(docker image inspect --format '{{.Id}}|{{.Os}}|{{.Architecture}}' "$SRTOOL_LOCAL_IMAGE")"
	if [[ "$identity" != "$SRTOOL_IMAGE_ID|linux|amd64" ]]; then
		echo "pinned srtool alias mismatch: expected $SRTOOL_IMAGE_ID|linux|amd64, got $identity" >&2
		exit 1
	fi
}

docker pull "$SRTOOL_DIGEST"
actual_image_identity="$(docker image inspect --format '{{.Id}}|{{.Os}}|{{.Architecture}}' "$SRTOOL_DIGEST")"
if [[ "$actual_image_identity" != "$SRTOOL_IMAGE_ID|linux|amd64" ]]; then
	echo "srtool image mismatch: expected $SRTOOL_IMAGE_ID|linux|amd64, got $actual_image_identity" >&2
	exit 1
fi
# srtool-cli 0.13.2 appends the Rust tag itself, so give it a verified local repository name.
docker tag "$SRTOOL_DIGEST" "$SRTOOL_LOCAL_IMAGE"
verify_pinned_image

cargo_lock_sha256="$(shasum -a 256 "$root/Cargo.lock" | awk '{print $1}')"
source_commit="$(git -C "$root" rev-parse HEAD)"
verify_source_tree() {
	local candidate="$1"
	local label="$2"
	local current_lock current_source
	if [[ "$label" == "reproduction" ]]; then
		if [[ -n "$(git -C "$candidate" status --porcelain --untracked-files=all)" ]]; then
			echo "$label CORD source is not clean" >&2
			exit 1
		fi
	elif ! git -C "$candidate" diff --quiet || ! git -C "$candidate" diff --cached --quiet; then
		echo "$label CORD tracked source changed during canonical release build" >&2
		exit 1
	fi
	current_source="$(git -C "$candidate" rev-parse HEAD)"
	current_lock="$(shasum -a 256 "$candidate/Cargo.lock" | awk '{print $1}')"
	if [[ "$current_source" != "$source_commit" || "$current_lock" != "$cargo_lock_sha256" ]]; then
		echo "$label CORD commit or Cargo.lock differs from the canonical source" >&2
		exit 1
	fi
}
verify_source_identity() {
	verify_source_tree "$root" "primary"
}
verify_source_identity
output_parent="$(dirname "$output_dir")"
staging_dir="$output_dir.tmp.$$"
repro_root="$(mktemp -d /tmp/cord-origin-orbis-reproduction.XXXXXX)"
rmdir "$repro_root"
repro_registered=false
mkdir -p "$output_parent"
rm -rf "$staging_dir"
mkdir "$staging_dir"
cleanup() {
	set +e
	if [[ "$repro_registered" == true ]]; then
		git -C "$root" worktree remove --force "$repro_root" >/dev/null 2>&1
	fi
	rm -rf "$repro_root" "$staging_dir"
}
trap cleanup EXIT

git -C "$root" worktree add --detach "$repro_root" "$source_commit"
repro_registered=true
verify_source_tree "$repro_root" "reproduction"

# Release evidence must not inherit a prior wbuild fingerprint or Wasm blob.
rm -rf \
	"$root/$FOUNDATION_DIR/target/srtool" "$root/$COMMONS_DIR/target/srtool" \
	"$repro_root/$FOUNDATION_DIR/target/srtool" "$repro_root/$COMMONS_DIR/target/srtool"

run_srtool() {
	local source_root="$1"
	local source_label="$2"
	local package="$3"
	local runtime_dir="$4"
	local log="$staging_dir/$package.srtool-$source_label.log"
	local report="$staging_dir/$package.srtool-$source_label.json"

	verify_source_tree "$source_root" "$source_label"
	verify_pinned_image
	CARGO_BUILD_JOBS="$SRTOOL_CARGO_JOBS" CARGO_INCREMENTAL=0 SRTOOL_TAG="$SRTOOL_RUST_TAG" srtool build \
		--engine docker \
		--image "$SRTOOL_LOCAL_REPOSITORY" \
		--app \
		--package "$package" \
		--runtime-dir "$runtime_dir" \
		--no-cache \
		--build-opts=--features=on-chain-release-build \
		--profile "$PROFILE" \
		"$source_root" | tee "$log"

	python3 - "$log" "$report" "$package" <<'PY'
import json
import pathlib
import sys

log_path, output_path, expected_package = map(pathlib.Path, sys.argv[1:])
for line in reversed(log_path.read_text(encoding="utf-8").splitlines()):
    try:
        report = json.loads(line)
    except json.JSONDecodeError:
        continue
    if report.get("pkg") == str(expected_package):
        temporary = output_path.with_suffix(output_path.suffix + ".tmp")
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(output_path)
        break
else:
    raise SystemExit(f"srtool did not emit a JSON report for {expected_package}")
PY
}

run_srtool "$root" primary "$FOUNDATION_PACKAGE" "$FOUNDATION_DIR"
run_srtool "$root" primary "$COMMONS_PACKAGE" "$COMMONS_DIR"
run_srtool "$repro_root" reproduction "$FOUNDATION_PACKAGE" "$FOUNDATION_DIR"
run_srtool "$repro_root" reproduction "$COMMONS_PACKAGE" "$COMMONS_DIR"

foundation_wbuild="$root/$FOUNDATION_DIR/target/srtool/$PROFILE/wbuild/$FOUNDATION_PACKAGE"
commons_target="$root/$COMMONS_DIR/target/srtool"
commons_wbuild="$commons_target/$PROFILE/wbuild/$COMMONS_PACKAGE"
repro_foundation_wbuild="$repro_root/$FOUNDATION_DIR/target/srtool/$PROFILE/wbuild/$FOUNDATION_PACKAGE"
repro_commons_wbuild="$repro_root/$COMMONS_DIR/target/srtool/$PROFILE/wbuild/$COMMONS_PACKAGE"
foundation_compact="$foundation_wbuild/origin_foundation_runtime.compact.wasm"
foundation_compressed="$foundation_wbuild/origin_foundation_runtime.compact.compressed.wasm"
commons_compact="$commons_wbuild/origin_commons_runtime.compact.wasm"
commons_compressed="$commons_wbuild/origin_commons_runtime.compact.compressed.wasm"
repro_foundation_compact="$repro_foundation_wbuild/origin_foundation_runtime.compact.wasm"
repro_foundation_compressed="$repro_foundation_wbuild/origin_foundation_runtime.compact.compressed.wasm"
repro_commons_compact="$repro_commons_wbuild/origin_commons_runtime.compact.wasm"
repro_commons_compressed="$repro_commons_wbuild/origin_commons_runtime.compact.compressed.wasm"

for artifact in \
	"$foundation_compact" "$foundation_compressed" \
	"$commons_compact" "$commons_compressed" \
	"$repro_foundation_compact" "$repro_foundation_compressed" \
	"$repro_commons_compact" "$repro_commons_compressed"; do
	[[ -s "$artifact" ]] || {
		echo "missing srtool artifact: $artifact" >&2
		exit 1
	}
done

# Two independent clean sources mounted at the same /build path must be byte-identical.
cmp "$foundation_compact" "$repro_foundation_compact"
cmp "$foundation_compressed" "$repro_foundation_compressed"
cmp "$commons_compact" "$repro_commons_compact"
cmp "$commons_compressed" "$repro_commons_compressed"

# Freeze both runs before a node build can mutate either srtool target.
cp "$foundation_compact" "$staging_dir/origin_foundation_runtime.compact.wasm"
cp "$foundation_compressed" "$staging_dir/origin_foundation_runtime.compact.compressed.wasm"
cp "$commons_compact" "$staging_dir/origin_commons_runtime.compact.wasm"
cp "$commons_compressed" "$staging_dir/origin_commons_runtime.compact.compressed.wasm"
cp "$repro_foundation_compact" \
	"$staging_dir/origin_foundation_runtime.reproduction.compact.wasm"
cp "$repro_foundation_compressed" \
	"$staging_dir/origin_foundation_runtime.reproduction.compact.compressed.wasm"
cp "$repro_commons_compact" \
	"$staging_dir/origin_commons_runtime.reproduction.compact.wasm"
cp "$repro_commons_compressed" \
	"$staging_dir/origin_commons_runtime.reproduction.compact.compressed.wasm"

foundation_canonical_compact="$staging_dir/origin_foundation_runtime.compact.wasm"
foundation_canonical_compressed="$staging_dir/origin_foundation_runtime.compact.compressed.wasm"
commons_canonical_compact="$staging_dir/origin_commons_runtime.compact.wasm"
commons_canonical_compressed="$staging_dir/origin_commons_runtime.compact.compressed.wasm"

# Source B is no longer needed after its artifacts and reports are frozen.
git -C "$root" worktree remove --force "$repro_root"
repro_registered=false
rm -rf "$repro_root"

# Reuse both srtool targets in the same pinned image and fixed /build mount. This makes each
# production node embed the exact canonical wbuild output instead of rebuilding under a host path.
verify_source_identity
verify_pinned_image
docker run --rm \
	--volume "$root:/build" \
	--volume "$staging_dir:/release-out" \
	--workdir /build \
	--entrypoint /bin/bash \
	"$SRTOOL_LOCAL_IMAGE" -lc "
		set -euo pipefail
		rustup override set 1.93.0
		# rocksdb's bindgen/clang-sys needs a libclang.so filename. The pinned
		# image's llvm-14 compatibility link is broken, but its verified Debian
		# library is present. Create an ephemeral in-container discovery link;
		# do not alter the image or use a host toolchain.
		test -r /usr/lib/x86_64-linux-gnu/libclang-14.so.14.0.0
		mkdir -p /tmp/cord-srtool-libclang
		ln -sfn /usr/lib/x86_64-linux-gnu/libclang-14.so.14.0.0 /tmp/cord-srtool-libclang/libclang.so
		test -r /tmp/cord-srtool-libclang/libclang.so
		export LIBCLANG_PATH=/tmp/cord-srtool-libclang
		export CLANG_PATH=/usr/bin/clang-14
		cargo build --locked --profile $PROFILE \
			--package origin \
			--no-default-features \
			--features on-chain-release-build \
			--target-dir /build/$FOUNDATION_DIR/target/srtool
		/build/$FOUNDATION_DIR/target/srtool/$PROFILE/origin \
			build-spec --chain origin-local --raw --disable-default-bootnode \
			> /release-out/origin-local.raw.json
		cp /build/$FOUNDATION_DIR/target/srtool/$PROFILE/origin \
			/release-out/origin

		cargo build --locked --profile $PROFILE \
			--package origin-omni-node \
			--no-default-features \
			--features on-chain-release-build \
			--target-dir /build/$COMMONS_DIR/target/srtool
		/build/$COMMONS_DIR/target/srtool/$PROFILE/origin-omni-node \
			build-spec --chain orbis-dev --raw --disable-default-bootnode \
			> /release-out/orbis-dev.raw.json
		cp /build/$COMMONS_DIR/target/srtool/$PROFILE/origin-omni-node \
			/release-out/origin-omni-node
	"

python3 - \
	"$staging_dir/origin-local.raw.json" "$staging_dir/origin-local-code.compact.compressed.wasm" \
	"$staging_dir/orbis-dev.raw.json" "$staging_dir/orbis-dev-code.compact.compressed.wasm" <<'PY'
import json
import pathlib
import sys

for spec_name, code_name in zip(sys.argv[1::2], sys.argv[2::2]):
    spec_path = pathlib.Path(spec_name)
    code_path = pathlib.Path(code_name)
    spec = json.loads(spec_path.read_text(encoding="utf-8"))
    encoded = spec["genesis"]["raw"]["top"]["0x3a636f6465"]
    if not encoded.startswith("0x"):
        raise SystemExit(f"raw :code in {spec_path} is not 0x-prefixed")
    code_path.write_bytes(bytes.fromhex(encoded[2:]))
PY

cmp "$staging_dir/origin-local-code.compact.compressed.wasm" "$foundation_canonical_compressed"
cmp "$staging_dir/orbis-dev-code.compact.compressed.wasm" "$commons_canonical_compressed"

verify_source_identity
verify_pinned_image
docker run --rm \
	--volume "$root:/build" \
	--volume "$staging_dir:/release-out" \
	--workdir /build \
	--entrypoint subwasm \
	"$SRTOOL_LOCAL_IMAGE" decompress \
	/release-out/origin-local-code.compact.compressed.wasm \
	/release-out/origin-local-code.compact.wasm

verify_source_identity
verify_pinned_image
docker run --rm \
	--volume "$root:/build" \
	--volume "$staging_dir:/release-out" \
	--workdir /build \
	--entrypoint subwasm \
	"$SRTOOL_LOCAL_IMAGE" decompress \
	/release-out/orbis-dev-code.compact.compressed.wasm \
	/release-out/orbis-dev-code.compact.wasm

cmp "$staging_dir/origin-local-code.compact.wasm" "$foundation_canonical_compact"
cmp "$staging_dir/orbis-dev-code.compact.wasm" "$commons_canonical_compact"

verify_source_identity
python3 - "$staging_dir/release-inputs.json.tmp" "$source_commit" "$cargo_lock_sha256" \
	"$SRTOOL_DIGEST" "$SRTOOL_IMAGE_ID" "$srtool_version" "$SRTOOL_RUST_TAG" <<'PY'
import json
import pathlib
import sys

output, commit, cargo_lock_sha256, image_digest, image_id, srtool_version, rust_tag = sys.argv[1:]
def material_hash(root):
    import hashlib
    roots = ["Cargo.lock", "origin/orbis/runtime", "origin/orbis/runtime-api/storage", "origin/orbis/primitives", "origin/orbis/pallets/storage-provider", "origin/orbis/pallets/drive", "origin/orbis/pallets/s3"]
    digest = hashlib.sha256()
    files = []
    for relative in roots:
        candidate = pathlib.Path(root, relative)
        files.extend([candidate] if candidate.is_file() else [p for p in candidate.rglob("*") if p.is_file()])
    for path in sorted(files, key=lambda p: p.relative_to(root).as_posix().encode()):
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big")); digest.update(relative)
        digest.update(bytes.fromhex(hashlib.sha256(path.read_bytes()).hexdigest()))
    return digest.hexdigest()
report = {
    "p1_runtime_material_sha256": material_hash(pathlib.Path.cwd()),
    "cargo_lock_sha256": cargo_lock_sha256,
    "fresh_srtool_target": True,
    "independent_clean_source_runs": 2,
    "source_commit": commit,
    "srtool_architecture": "amd64",
    "srtool_build_options": "--features=on-chain-release-build",
    "srtool_cli": srtool_version,
    "srtool_image_digest": image_digest,
    "srtool_image_id": image_id,
    "srtool_os": "linux",
    "srtool_no_cache": True,
    "srtool_cargo_incremental": False,
    "srtool_cargo_jobs": 1,
    "srtool_profile": "release",
    "srtool_rust_tag": rust_tag,
    "runtime_feature": "on-chain-release-build",
    "workspace_mount": "/build",
}
pathlib.Path(output).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
mv "$staging_dir/release-inputs.json.tmp" "$staging_dir/release-inputs.json"

(
	cd "$staging_dir"
	shasum -a 256 \
		origin_foundation_runtime.compact.wasm \
		origin_foundation_runtime.compact.compressed.wasm \
		origin_commons_runtime.compact.wasm \
		origin_commons_runtime.compact.compressed.wasm \
		origin_foundation_runtime.reproduction.compact.wasm \
		origin_foundation_runtime.reproduction.compact.compressed.wasm \
		origin_commons_runtime.reproduction.compact.wasm \
		origin_commons_runtime.reproduction.compact.compressed.wasm \
		origin-local-code.compact.compressed.wasm \
		origin-local-code.compact.wasm \
		orbis-dev-code.compact.compressed.wasm \
		orbis-dev-code.compact.wasm \
		origin-local.raw.json \
		orbis-dev.raw.json \
		origin-foundation-runtime.srtool-primary.json \
		origin-foundation-runtime.srtool-primary.log \
		origin-foundation-runtime.srtool-reproduction.json \
		origin-foundation-runtime.srtool-reproduction.log \
		origin-commons-runtime.srtool-primary.json \
		origin-commons-runtime.srtool-primary.log \
		origin-commons-runtime.srtool-reproduction.json \
		origin-commons-runtime.srtool-reproduction.log \
		origin \
		origin-omni-node \
		release-inputs.json >SHA256SUMS.tmp
	mv SHA256SUMS.tmp SHA256SUMS
)

mv "$staging_dir" "$output_dir"
staging_dir=""
trap - EXIT

printf 'Canonical Origin/Orbis release artifacts: %s\n' "$output_dir"
printf 'Foundation and Commons compressed/decompressed node :code identity: PASS\n'
