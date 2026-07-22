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
# Pinned to dhiway/sdk release-v1.24.0 cc190ea8 substrate-wasm-builder metadata_hash:
# execute first-pass Wasm Metadata_metadata_at_version(15) and Core_version, extract System SS58,
# then RFC-78 merkleized_metadata with the chain-spec symbol ORGN and 10 decimals.
root="$(git rev-parse --show-toplevel)"
target="${CARGO_TARGET_DIR:-$root/target/orbis-metadata}"
log="$(mktemp)"
trap 'rm -f "$log"' EXIT
SKIP_PALLET_REVIVE_FIXTURES=1 CARGO_TARGET_DIR="$target" \
  cargo build -p origin-commons-runtime --release --features on-chain-release-build -vv \
  2>&1 | tee "$log" >&2
hash="$(grep -o 'RUNTIME_METADATA_HASH=[^ ]*' "$log" | tail -1 | cut -d= -f2 | tr -d "'\"" || true)"
if [[ -z "$hash" ]]; then
  hash="$(grep -h -o 'RUNTIME_METADATA_HASH=[^ ]*' \
    "$target"/release/build/origin-commons-runtime-*/output 2>/dev/null \
    | tail -1 | cut -d= -f2 | tr -d "'\"" || true)"
fi
test -n "$hash"
printf '%s\n' "$hash"
