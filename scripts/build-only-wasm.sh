#!/usr/bin/env sh
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

set -e

PROJECT_ROOT=$(git rev-parse --show-toplevel)

if [ "$#" -lt 1 ]; then
  echo "You need to pass the name of the crate you want to compile!"
  exit 1
fi

WASM_BUILDER_RUNNER="$PROJECT_ROOT/target/release/wbuild-runner/$1"

fl_cargo() {
  cargo "$@"
}

if [ -z "$2" ]; then
  export WASM_TARGET_DIRECTORY=$(pwd)
else
  export WASM_TARGET_DIRECTORY=$2
fi

if [ -d $WASM_BUILDER_RUNNER ]; then
  export DEBUG=false
  export OUT_DIR="$PROJECT_ROOT/target/release/build"
  fl_cargo run --release --manifest-path="$WASM_BUILDER_RUNNER/Cargo.toml" |
    grep -vE "cargo:rerun-if-|Executing build command"
else
  fl_cargo build --release -p $1
fi
