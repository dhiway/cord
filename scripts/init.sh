#!/usr/bin/env bash
BASEDIR=$(realpath $(dirname "$0"))

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup install nightly-2022-07-24
   rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2022-07-24
rustup target add wasm32-unknown-unknown --toolchain stable
rustup override set nightly-2022-07-24 --path $BASEDIR/..

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc || \
	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force