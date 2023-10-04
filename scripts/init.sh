#!/usr/bin/env bash
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
