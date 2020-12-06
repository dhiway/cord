#!/usr/bin/env bash
BASEDIR=$(dirname "$0")
set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup install nightly-2020-10-06
   rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2020-10-06
rustup override set nightly-2020-10-06 --path $BASEDIR/..
