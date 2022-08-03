#!/usr/bin/env bash
set -e

echo "*** Initializing WASM build environment"

rustup default 1.60.0
rustup update 1.60.0
rustup update nightly-2022-05-11

rustup target add wasm32-unknown-unknown --toolchain nightly-2022-05-11

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc || \
	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force