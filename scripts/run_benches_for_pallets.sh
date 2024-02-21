#!/usr/bin/env bash
# This file is part of CORD – https://cord.network
#
# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later
#
# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

while getopts 'bfp:v' flag; do
  case "${flag}" in
  b)
    # Skip build.
    skip_build='true'
    ;;
  f)
    # Fail if any sub-command in a pipe fails, not just the last one.
    set -o pipefail
    # Fail on undeclared variables.
    set -u
    # Fail if any sub-command fails.
    set -e
    # Fail on traps.
    set -E
    ;;
  p)
    # Start at pallet
    start_pallet="${OPTARG}"
    ;;
  v)
    # Echo all executed commands.
    set -x
    ;;
  *)
    # Exit early.
    echo "Bad options. Check Script."
    exit 1
    ;;
  esac
done

if [ "$skip_build" != true ]; then
  echo "[+] Compiling CORD benchmarks..."
  cargo build --profile=production --locked --features=runtime-benchmarks --bin cord
fi

# The executable to use.
CORD=./target/production/cord

# Manually exclude some pallets.
PALLETS=(
  "pallet_chain_space"
  "pallet_collective"
  "pallet_did"
  "pallet_did_name"
  "pallet_identity"
  "pallet_membership"
  "pallet_network_membership"
  "pallet_network_score"
  "pallet_schema"
  "pallet_statement"
  "pallet_asset"
)

echo "[+] Benchmarking ${#PALLETS[@]} pallets."

# Define the error file.
ERR_FILE="benchmarking_errors.txt"
# Delete the error file before each run.
rm -f $ERR_FILE

# Benchmark each pallet.
for PALLET in "${PALLETS[@]}"; do
  # If `-p` is used, skip benchmarks until the start pallet.
  if [ ! -z "$start_pallet" ] && [ "$start_pallet" != "$PALLET" ]; then
    echo "[+] Skipping ${PALLET}..."
    continue
  else
    unset start_pallet
  fi

  case $PALLET in
  pallet_chain_space)
    FOLDER="chain-space"
    ;;
  pallet_collective)
    FOLDER="collective"
    ;;
  pallet_did)
    FOLDER="did"
    ;;
  pallet_did_name)
    FOLDER="did-name"
    ;;
  pallet_identity)
    FOLDER="identity"
    ;;
  pallet_membership)
    FOLDER="membership"
    ;;
  pallet_network_membership)
    FOLDER="network-membership"
    ;;
  pallet_network_score)
    FOLDER="network-score"
    ;;
  pallet_schema)
    FOLDER="schema"
    ;;
  pallet_statement)
    FOLDER="statement"
    ;;
  pallet_asset)
    FOLDER="asset"
    ;;

  *)
    # Exit early.
    echo "Bad pallet option. Check Script."
    exit 1
    ;;
  esac

  WEIGHT_FILE="./pallets/${FOLDER}/src/weights.rs"
  echo "[+] Benchmarking $PALLET with weight file $WEIGHT_FILE"

  OUTPUT=$(
    $CORD benchmark pallet \
      --chain=dev \
      --steps=50 \
      --repeat=20 \
      --pallet="$PALLET" \
      --extrinsic="*" \
      --wasm-execution=compiled \
      --heap-pages=4096 \
      --output="$WEIGHT_FILE" \
      --header="./HEADER-GPL3" \
      --template=./.maintain/frame-weight-template.hbs 2>&1
  )
  if [ $? -ne 0 ]; then
    echo "$OUTPUT" >>"$ERR_FILE"
    echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
  fi
done

# Check if the error file exists.
if [ -f "$ERR_FILE" ]; then
  echo "[-] Some benchmarks failed. See: $ERR_FILE"
  exit 1
else
  echo "[+] All benchmarks passed."
  exit 0
fi
