#!/bin/bash

# Define the input text with packages
input_text=$(cat << EOF
authority-membership = { path = "runtimes/common/authorities", default-features = false }
identifier = { package = "cord-identifier", path = "primitives/identifier", default-features = false }
cord-node-cli = { path = "node/cli", default-features = false }
cord-primitives = { path = "primitives/cord", default-features = false }
network-membership = { path = "primitives/network-membership", default-features = false }
cord-braid-runtime = { path = "runtimes/braid", default-features = false }
cord-braid-runtime-constants = { path = "runtimes/braid/constants", default-features = false }
cord-runtime-common = { path = "runtimes/common", default-features = false }
cord-loom-runtime = { path = "runtimes/loom", default-features = false }
cord-loom-runtime-constants = { path = "runtimes/loom/constants", default-features = false }
cord-weave-runtime = { path = "runtimes/weave", default-features = false }
cord-weave-runtime-constants = { path = "runtimes/weave/constants", default-features = false }
cord-node-service = { path = "node/service", default-features = false }
cord-node-inspect = { path = "node/inspect", default-features = false }
cord-node-rpc = { path = "node/rpc", default-features = false }
cord-node-testing = { path = "node/testing", default-features = false }
cord-test-utils = { path = "test-utils", default-features = false }
cord-service-test = { path = "test-utils/service", default-features = false }
cord-test-runtime = { path = "test-utils/runtime", default-features = false }
cord-test-client = { path = "test-utils/client", default-features = false }
cord-test-runtime-client = { path = "test-utils/runtime/client", default-features = false }
cord-test-runtime-transaction-pool = { path = "test-utils/runtime/transaction-pool", default-features = false }
cord-cli-test-utils = { path = "test-utils/cli", default-features = false }
cord-utilities = { path = "utilities", default-features = false }
pallet-membership = { path = 'pallets/membership/', default-features = false }
pallet-config = { path = 'pallets/config/', default-features = false }
pallet-did = { path = 'pallets/did', default-features = false }
pallet-did-name = { path = 'pallets/did-name', default-features = false }
pallet-schema = { path = 'pallets/schema', default-features = false }
pallet-chain-space = { path = 'pallets/chain-space', default-features = false }
pallet-statement = { path = 'pallets/statement', default-features = false }
pallet-network-membership = { path = 'pallets/network-membership', default-features = false }
pallet-runtime-upgrade = { path = 'pallets/runtime-upgrade', default-features = false }
pallet-identity = { path = 'pallets/identity', default-features = false }
pallet-offences = { path = 'pallets/offences', default-features = false }
pallet-node-authorization = { path = "pallets/node-authorization", default-features = false }
pallet-network-score = { path = 'pallets/network-score', default-features = false }
pallet-session-benchmarking = { path = 'pallets/session-benchmarking', default-features = false }
pallet-assets-runtime-api = { path = "runtimes/common/api/assets", default-features = false }
pallet-did-runtime-api = { path = "runtimes/common/api/did", default-features = false }
pallet-transaction-weight-runtime-api = { path = "runtimes/common/api/weight", default-features = false }
pallet-registries = { path = "pallets/registries", default-features = false }
pallet-entries = { path = "pallets/entries", default-features = false }
EOF
)

# Define output files
packages_with_feature="packages_with_runtime_benchmarks.txt"
packages_without_feature="packages_without_runtime_benchmarks.txt"

# Clear existing files
> $packages_with_feature
> $packages_without_feature

# Process the input text
echo "$input_text" | while read -r line; do
    # Extract the package name
    package_name=$(echo "$line" | cut -d '=' -f 1 | tr -d ' ')

    # Check if it contains 'runtime-benchmarks' (adjust according to your context)
    if echo "$line" | grep -q "runtime-benchmarks"; then
        echo "$package_name" >> $packages_with_feature
    else
        echo "$package_name" >> $packages_without_feature
    fi
done

# Output the results
echo "Packages with 'runtime-benchmarks' feature are stored in $packages_with_feature"
echo "Packages without 'runtime-benchmarks' feature are stored in $packages_without_feature"
