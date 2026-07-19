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

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
out="$repo_root/docs/evidence/p1/storage-proof/campaign/generated"
mkdir -p "$out"

"$repo_root/target/release/origin" build-spec \
	--chain origin-local --disable-default-bootnode \
	| "$repo_root/scripts/isolate-proof-chainspec.py" \
	> "$out/origin-proof-isolated.json"

"$repo_root/target/release/origin-omni-node" build-spec \
	--chain orbis-local --disable-default-bootnode \
	| "$repo_root/scripts/isolate-proof-chainspec.py" \
	> "$out/orbis-proof-isolated.json"

shasum -a 256 \
	"$out/origin-proof-isolated.json" \
	"$out/orbis-proof-isolated.json" \
	> "$out/SHA256SUMS"
