#!/bin/bash

# Runs all benchmarks for all pallets, for a given runtime, provided by $1
# Should be run on a reference machine to gain accurate benchmarks

while getopts 'bfp:v' flag; do
  case "${flag}" in
  b)
    # Skip build.
    skip_build='true'
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
  echo "[+] Compiling benchmarks..."
  cargo build --profile=production --locked --features=runtime-benchmarks --bin cord
fi

# The executable to use.
CORD=./target/production/cord

# Load all pallet names in an array.
PALLETS=($(
  $CORD benchmark pallet --list --chain="dev" |
    tail -n+2 |
    cut -d',' -f1 |
    sort |
    uniq
))

echo "[+] Benchmarking ${#PALLETS[@]} pallets for CORD runtime"

# Define the error file.
ERR_FILE="benchmarking_errors.txt"
# Delete the error file before each run.
rm -f $ERR_FILE

# Benchmark each pallet.
for PALLET in "${PALLETS[@]}"; do
  echo "[+] Benchmarking $PALLET for CORD runtime"

  output_file=""
  if [[ $PALLET == *"::"* ]]; then
    # translates e.g. "pallet_foo::bar" to "pallet_foo_bar"
    output_file="${PALLET//::/_}.rs"
  fi

  OUTPUT=$(
    $CORD benchmark pallet \
      --chain=dev \
      --steps=50 \
      --repeat=20 \
      --pallet="$PALLET" \
      --extrinsic="*" \
      --wasm-execution=compiled \
      --heap-pages=4096 \
      --header=./HEADER-GPL3 \
      --output="./runtime/src/weights/${output_file}" 2>&1
  )
  if [ $? -ne 0 ]; then
    echo "$OUTPUT" >>"$ERR_FILE"
    echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
  fi
done

# Update the block and extrinsic overhead weights.
echo "[+] Benchmarking block and extrinsic overheads..."
OUTPUT=$(
  $CORD benchmark overhead \
    --chain="dev" \
    --wasm-execution=compiled \
    --weight-path="runtime/constants/src/weights/" \
    --warmup=10 \
    --repeat=100 \
    --header=./HEADER-GPL3
)
if [ $? -ne 0 ]; then
  echo "$OUTPUT" >>"$ERR_FILE"
  echo "[-] Failed to benchmark the block and extrinsic overheads. Error written to $ERR_FILE; continuing..."
fi

echo "[+] Benchmarking the machine..."
OUTPUT=$(
  $CORD benchmark machine --chain=dev 2>&1
)
if [ $? -ne 0 ]; then
  # Do not write the error to the error file since it is not a benchmarking error.
  echo "[-] Failed the machine benchmark:\n$OUTPUT"
fi

# Check if the error file exists.
if [ -f "$ERR_FILE" ]; then
  echo "[-] Some benchmarks failed. See: $ERR_FILE"
else
  echo "[+] All benchmarks passed."
fi
