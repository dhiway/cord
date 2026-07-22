#!/usr/bin/env bash
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

set -eu

# Check that cargo and grep are installed - otherwise abort.
command -v cargo >/dev/null 2>&1 || {
  echo >&2 "cargo is required but not installed. Aborting."
  exit 1
}
command -v grep >/dev/null 2>&1 || {
  echo >&2 "grep is required but not installed. Aborting."
  exit 1
}

# Enter the workspace root folder.
cd "$1"
echo "Workspace root is $PWD"

function main() {
  feature_does_not_imply 'default' 'runtime-benchmarks'
  feature_does_not_imply 'std' 'runtime-benchmarks'
  feature_does_not_imply 'default' 'try-runtime'
  feature_does_not_imply 'std' 'try-runtime'
}

# Accepts two feature names as arguments.
# Checks that the first feature does not imply the second one.
function feature_does_not_imply() {
  ENABLED=$1
  STAYS_DISABLED=$2
  echo "📏 Checking that $ENABLED does not imply $STAYS_DISABLED ..."

  # Check if the forbidden feature is enabled anywhere in the workspace.
  # But only check "normal" dependencies, so no "dev" or "build" dependencies.
  if cargo tree --no-default-features --locked --workspace -e features,normal --features "$ENABLED" | grep -qF "feature \"$STAYS_DISABLED\""; then
    echo "❌ $ENABLED implies $STAYS_DISABLED in the workspace"
  else
    echo "✅ $ENABLED does not imply $STAYS_DISABLED in the workspace"
    return
  fi

  # Find all Cargo.toml files but exclude the root one since we already know that it is broken.
  CARGOS=$(find . -name Cargo.toml -not -path ./Cargo.toml)
  NUM_CRATES=$(echo "$CARGOS" | wc -l)
  FAILED=0
  PASSED=0
  echo "🔍 Checking all $NUM_CRATES crates - this takes some time."

  for CARGO in $CARGOS; do
    OUTPUT=$(cargo tree --no-default-features --locked --offline -e features,normal --features $ENABLED --manifest-path $CARGO 2>&1 || true)

    if echo "$OUTPUT" | grep -qF "not supported for packages in this workspace"; then
      # This case just means that the pallet does not support the
      # requested feature which is fine.
      PASSED=$((PASSED + 1))
    elif echo "$OUTPUT" | grep -qF "feature \"$STAYS_DISABLED\""; then
      echo "❌ Violation in $CARGO by dependency:"
      # Best effort hint for which dependency needs to be fixed.
      echo "$OUTPUT" | grep -wF "feature \"$STAYS_DISABLED\"" | head -n 1
      FAILED=$((FAILED + 1))
    else
      PASSED=$((PASSED + 1))
    fi
  done

  echo "Checked $NUM_CRATES crates in total of which $FAILED failed and $PASSED passed."
  echo "Exiting with code 1"
  exit 1
}

main "$@"
