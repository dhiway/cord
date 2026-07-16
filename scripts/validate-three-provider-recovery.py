#!/usr/bin/env python3
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

"""Derive AC5 assertions from the raw three-provider integration-test artifact."""

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


DEFAULT_SOURCE = Path("target/debug/ac5-three-provider-recovery-v1.json")
HEX_32 = re.compile(r"[0-9a-f]{64}")
REQUIRED_SURFACES = {
    "StreamingStore::read_range_verified",
    "BucketMmrStore::commitment_candidate",
    "replication_worker::select_source",
    "PeerResponder::page/chunk",
    "ReplicationReconciler::reconcile_one",
}


class ValidationError(Exception):
    """Raised when raw test evidence cannot support an AC5 assertion."""


def require(condition: bool, message: str) -> None:
    """Reject an unsupported or malformed evidence claim."""
    if not condition:
        raise ValidationError(message)


def integer(value: Any, field: str, minimum: int = 0) -> int:
    """Return one non-boolean bounded integer field."""
    require(isinstance(value, int) and not isinstance(value, bool), f"{field} must be an integer")
    require(value >= minimum, f"{field} must be at least {minimum}")
    return value


def object_list(value: Any, field: str) -> list[dict[str, Any]]:
    """Return a non-empty list containing only JSON objects."""
    require(isinstance(value, list) and value, f"{field} must be a non-empty list")
    require(all(isinstance(item, dict) for item in value), f"{field} entries must be objects")
    return value


def derive(raw: Any) -> dict[str, Any]:
    """Validate raw observations and derive exactly the four registered assertions."""
    require(isinstance(raw, dict), "source artifact must be a JSON object")
    require(integer(raw.get("schema_version"), "schema_version", 1) == 1,
            "unsupported source artifact schema")
    require(raw.get("test") == "deterministic_failover", "unexpected source test name")

    provider_count = integer(raw.get("provider_count"), "provider_count", 1)
    roots = raw.get("provider_roots")
    require(isinstance(roots, list), "provider_roots must be a list")
    require(provider_count == 3 and len(roots) == provider_count, "exactly three roots are required")
    require(all(isinstance(root, str) and HEX_32.fullmatch(root) for root in roots),
            "provider roots must be canonical lowercase 32-byte hex")
    root_matches = len(set(roots)) == 1
    require(root_matches, "provider MMR roots did not converge")

    sources = object_list(raw.get("eligible_sources"), "eligible_sources")
    source_rows: list[tuple[int, str]] = []
    for index, source in enumerate(sources):
        require(set(source) == {"provider", "order"}, f"eligible_sources[{index}] has unknown fields")
        provider = source["provider"]
        require(isinstance(provider, str) and HEX_32.fullmatch(provider),
                f"eligible_sources[{index}].provider is invalid")
        source_rows.append((integer(source["order"], f"eligible_sources[{index}].order"), provider))
    require(len({order for order, _ in source_rows}) == len(source_rows),
            "eligible source orders must be unique")
    expected_source = min(source_rows)[1]
    selections = raw.get("selection_results")
    require(isinstance(selections, list) and len(selections) >= 2,
            "two topology-order selection observations are required")
    require(all(isinstance(provider, str) and HEX_32.fullmatch(provider) for provider in selections),
            "selection results must be canonical provider identifiers")
    tie_break_matches = all(provider == expected_source for provider in selections)
    require(tie_break_matches, "production source selection was not deterministic")

    reads = object_list(raw.get("corrupt_read_observations"), "corrupt_read_observations")
    corrupt_reads = 0
    for index, read in enumerate(reads):
        require(set(read) == {"result", "bytes_returned"},
                f"corrupt_read_observations[{index}] has unknown fields")
        require(isinstance(read["result"], str),
                f"corrupt_read_observations[{index}].result must be a string")
        returned = integer(read["bytes_returned"],
                           f"corrupt_read_observations[{index}].bytes_returned")
        corrupt_reads += int(returned > 0)
        require(read["result"] == "rejected_integrity_failed" and returned == 0,
                "a corrupt-object read did not fail closed")

    failure_block = integer(raw.get("failure_detected_block"), "failure_detected_block")
    observed_block = integer(raw.get("convergence_observed_block"), "convergence_observed_block")
    recovery_steps = integer(raw.get("recovery_steps"), "recovery_steps", 1)
    require(observed_block >= failure_block, "convergence preceded failure detection")
    convergence_blocks = observed_block - failure_block
    require(convergence_blocks == recovery_steps,
            "convergence block delta was not derived from real reconciler actions")
    require(convergence_blocks <= 200, "recovery exceeded the 200-block bound")
    requests = integer(raw.get("recovery_network_requests"), "recovery_network_requests", 1)
    require(requests <= recovery_steps, "network requests exceeded reconciler actions")

    surfaces = raw.get("production_surfaces")
    require(isinstance(surfaces, list) and all(isinstance(surface, str) for surface in surfaces),
            "production_surfaces must be a string list")
    require(REQUIRED_SURFACES.issubset(set(surfaces)), "required production surfaces are missing")

    return {
        "root_matches": root_matches,
        "tie_break_matches": tie_break_matches,
        "corrupt_reads": corrupt_reads,
        "convergence_blocks": convergence_blocks,
    }


def main() -> int:
    """Load raw evidence, fail closed, and atomically write derived assertions."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        args.out.unlink(missing_ok=True)
        raw = json.loads(args.source.read_text(encoding="utf-8"))
        assertions = derive(raw)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.out.with_suffix(args.out.suffix + ".tmp")
        encoded = json.dumps(assertions, indent=2, sort_keys=True) + "\n"
        temporary.write_text(encoded, encoding="utf-8")
        temporary.replace(args.out)
    except (OSError, json.JSONDecodeError, ValidationError) as error:
        print(f"AC5 validation failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(assertions, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
