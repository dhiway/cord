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
EXPECTED_RUNTIME_KINDS = [
    "initial_duty",
    "initial_checkpoint_finality",
    "fallback_duty",
    "promotion_finality",
    "repaired_eligible_duty",
    "promoted_checkpoint_finality",
]


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
    require(integer(raw.get("schema_version"), "schema_version", 1) == 2,
            "unsupported source artifact schema")
    require(raw.get("test") == "deterministic_failover", "unexpected source test name")

    content_cids = raw.get("content_cids")
    require(isinstance(content_cids, list) and len(content_cids) == 2,
            "two bounded content identifiers are required")
    require(all(isinstance(cid, str) and cid for cid in content_cids),
            "content identifiers must be non-empty strings")
    require(len(set(content_cids)) == 2, "content identifiers must be distinct")

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

    runtime = object_list(raw.get("runtime_observations"), "runtime_observations")
    require(len(runtime) == len(EXPECTED_RUNTIME_KINDS),
            "the complete ordered runtime observation sequence is required")
    runtime_numbers: list[int] = []
    for index, (observation, expected_kind) in enumerate(zip(runtime, EXPECTED_RUNTIME_KINDS)):
        require(set(observation) == {"kind", "finalized_number"},
                f"runtime_observations[{index}] has unknown fields")
        require(observation["kind"] == expected_kind,
                f"runtime_observations[{index}] is not {expected_kind}")
        runtime_numbers.append(integer(observation["finalized_number"],
                                       f"runtime_observations[{index}].finalized_number"))
    require(runtime_numbers == sorted(runtime_numbers) and len(set(runtime_numbers)) == len(runtime_numbers),
            "runtime observations must be strictly increasing")
    failure_block = runtime_numbers[2]
    repaired_block = runtime_numbers[4]
    observed_block = runtime_numbers[5]
    require(failure_block < repaired_block < observed_block,
            "repair eligibility must precede promoted checkpoint finality")
    convergence_blocks = observed_block - failure_block
    require(convergence_blocks <= 200, "recovery exceeded the 200-block bound")

    require(integer(raw.get("runtime_duty_reads"), "runtime_duty_reads", 1) == 7,
            "each provider must discover both checkpoint duties and the promoter must discover fallback")
    checkpoints = object_list(raw.get("checkpoints"), "checkpoints")
    require(len(checkpoints) == 2, "initial and promoted checkpoint observations are required")
    checkpoint_fields = {
        "phase", "duty_id", "primary", "confirmation_providers", "submission_id",
        "submission_record_hash", "mmr_root", "start_seq", "leaf_count",
        "call_args_blake2_256", "before_restart_blake2_256", "after_restart_blake2_256",
        "finality_calls", "finalized_number", "publication_count", "replay_finality_calls",
        "replay_publication_count",
    }
    submission_ids: list[str] = []
    for index, (checkpoint, expected_phase, runtime_index) in enumerate(
            zip(checkpoints, ["initial", "promoted"], [1, 5])):
        require(set(checkpoint) == checkpoint_fields, f"checkpoints[{index}] has unknown fields")
        require(checkpoint["phase"] == expected_phase, f"checkpoints[{index}] has wrong phase")
        for field in ["duty_id", "primary", "submission_id", "submission_record_hash", "mmr_root",
                      "call_args_blake2_256", "before_restart_blake2_256",
                      "after_restart_blake2_256"]:
            require(isinstance(checkpoint[field], str) and HEX_32.fullmatch(checkpoint[field]),
                    f"checkpoints[{index}].{field} is invalid")
        confirmations = checkpoint["confirmation_providers"]
        require(isinstance(confirmations, list) and len(confirmations) == 2,
                f"checkpoints[{index}] must have exactly two replica confirmations")
        require(all(isinstance(provider, str) and HEX_32.fullmatch(provider)
                    for provider in confirmations),
                f"checkpoints[{index}] confirmation providers are invalid")
        require(len(set(confirmations)) == 2 and checkpoint["primary"] not in confirmations,
                f"checkpoints[{index}] does not prove primary-plus-two quorum")
        require(checkpoint["before_restart_blake2_256"] ==
                checkpoint["after_restart_blake2_256"],
                f"checkpoints[{index}] durable record changed across reopen")
        integer(checkpoint["start_seq"], f"checkpoints[{index}].start_seq")
        integer(checkpoint["leaf_count"], f"checkpoints[{index}].leaf_count", 1)
        require(integer(checkpoint["finalized_number"],
                        f"checkpoints[{index}].finalized_number") == runtime_numbers[runtime_index],
                f"checkpoints[{index}] finality is not runtime-observed")
        require(integer(checkpoint["finality_calls"], f"checkpoints[{index}].finality_calls") == 1,
                f"checkpoints[{index}] did not finalize exactly once")
        require(integer(checkpoint["publication_count"],
                        f"checkpoints[{index}].publication_count") == 1,
                f"checkpoints[{index}] did not publish exactly once")
        require(integer(checkpoint["replay_finality_calls"],
                        f"checkpoints[{index}].replay_finality_calls") == 0,
                f"checkpoints[{index}] replay resubmitted finality")
        require(integer(checkpoint["replay_publication_count"],
                        f"checkpoints[{index}].replay_publication_count") == 0,
                f"checkpoints[{index}] replay republished checkpoint")
        submission_ids.append(checkpoint["submission_id"])
    require(len(set(submission_ids)) == 2, "checkpoint submission identifiers must be distinct")
    require(checkpoints[0]["start_seq"] == 0,
            "initial checkpoint must begin the bounded commitment sequence")
    require(checkpoints[1]["start_seq"] ==
            checkpoints[0]["start_seq"] + checkpoints[0]["leaf_count"],
            "promoted checkpoint must be the next non-overlapping bounded commitment")
    require(checkpoints[1]["mmr_root"] == roots[0],
            "promoted checkpoint must finalize the converged provider commitment")

    promotion = raw.get("promotion")
    require(isinstance(promotion, dict) and
            set(promotion) == {"intent_id", "provider", "finalized_number", "finality_calls"},
            "promotion observation is malformed")
    require(all(isinstance(promotion[field], str) and HEX_32.fullmatch(promotion[field])
                for field in ["intent_id", "provider"]), "promotion identifiers are invalid")
    require(promotion["provider"] == checkpoints[1]["primary"],
            "promoted provider is not the new primary")
    require(integer(promotion["finalized_number"], "promotion.finalized_number") == runtime_numbers[3],
            "promotion finality is not runtime-observed")
    require(integer(promotion["finality_calls"], "promotion.finality_calls") == 1,
            "promotion did not finalize exactly once")

    repaired_lengths = raw.get("repaired_read_lengths")
    require(isinstance(repaired_lengths, list) and len(repaired_lengths) == 3,
            "all three repaired read observations are required")
    require(all(integer(length, "repaired_read_lengths[]", 1) > 0 for length in repaired_lengths),
            "every provider must serve the repaired object")
    integer(raw.get("replication_network_requests"), "replication_network_requests", 1)

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
