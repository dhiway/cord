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
	"${fixture_dir}/Counter.sol" \
	"${fixture_dir}/IdentityAssetAudit.sol"

rm -f "${fixture_dir}/build/IERC20.abi" "${fixture_dir}/build/IERC20.bin"
