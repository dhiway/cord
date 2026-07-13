#!/usr/bin/env bash
set -euo pipefail

fixture_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

solc \
	--evm-version cancun \
	--optimize \
	--bin \
	--abi \
	--overwrite \
	-o "${fixture_dir}/build" \
	"${fixture_dir}/Counter.sol"
