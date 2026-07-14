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

BASEDIR=$(realpath $(dirname "$0"))

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ]; then
  rustup install nightly-2023-05-22
  rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2023-05-22

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc ||
  cargo +nightly-2023-05-22 install --git https://github.com/alexcrichton/wasm-gc --force
