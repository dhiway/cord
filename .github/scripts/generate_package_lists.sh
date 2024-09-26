#!/bin/bash

# Files to store the package names
packages_with_feature="packages_with_runtime_benchmarks.txt"
packages_without_feature="packages_without_runtime_benchmarks.txt"

# Empty the files if they already exist
> $packages_with_feature
> $packages_without_feature

# Find Cargo.toml files and check for the feature
for cargo_file in $(find . -name "Cargo.toml"); do
    if grep -q "\[features\]" "$cargo_file"; then
        if grep -q "runtime-benchmarks" "$cargo_file"; then
            package_name=$(basename $(dirname "$cargo_file"))
            echo "$package_name" >> $packages_with_feature
        else
            package_name=$(basename $(dirname "$cargo_file"))
            echo "$package_name" >> $packages_without_feature
        fi
    else
        package_name=$(basename $(dirname "$cargo_file"))
        echo "$package_name" >> $packages_without_feature
    fi
done

# Output the result
echo "Packages with 'runtime-benchmarks' feature are stored in $packages_with_feature"
echo "Packages without 'runtime-benchmarks' feature are stored in $packages_without_feature"
