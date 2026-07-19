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

"""Validate the frozen ledger and the superseding V2 phase amendment."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


FROZEN_LEDGER_SHA256 = "86406be20d40d4439d3612127687e4943f175902d75e93f62a82dbe32e0fe83d"
CAPABILITY_ID = "WSI-PROVIDER-BYTE-PLANE"
AMENDMENT_SHA256 = "ba89e20cb46bc19c9aa96cd1b316a1952f596209e82a939c446db86179e65c2d"
G003_OBLIGATIONS = {
    "checkpoint-provider-7",
    "checkpoint-provider-8",
    "checkpoint-provider-9",
    "checkpoint-provider-10",
    "checkpoint-three-provider-11",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


PACKAGE_SOURCES = {
    "origin-commons-runtime": Path("origin/orbis/runtime/src/tests.rs"),
    "pallet-orbis-storage-provider": Path("origin/orbis/pallets/storage-provider/src/tests.rs"),
}


def validate(ledger_path: Path, amendment_path: Path, verification_path: Path | None = None) -> dict[str, Any]:
    ledger_sha = sha256(ledger_path)
    amendment_sha = sha256(amendment_path)
    ledger = load_json(ledger_path)
    amendment = load_json(amendment_path)
    failures: list[str] = []

    if ledger_sha != FROZEN_LEDGER_SHA256:
        failures.append("frozen P0 capability ledger hash changed")
    rows = ledger.get("rows", [])
    row = next((item for item in rows if item.get("id") == CAPABILITY_ID), None)
    if row is None:
        failures.append(f"frozen ledger lacks {CAPABILITY_ID}")
        ledger_tests: set[str] = set()
    else:
        ledger_tests = {item.get("id") for item in row.get("required_p1_tests", [])}
        if not G003_OBLIGATIONS.issubset(ledger_tests):
            failures.append("frozen capability ledger no longer contains every G003 obligation")

    if amendment_sha != AMENDMENT_SHA256:
        failures.append("V2 amendment hash changed")
    if amendment.get("schema_version") != 2 or amendment.get("classification") != "explicit-superseding-execution-phase-amendment":
        failures.append("V2 amendment identity/classification mismatch")
    if amendment.get("not_a_clarification") is not True:
        failures.append("V2 amendment must remain an explicit amendment, not a clarification")
    supersedes = amendment.get("supersedes", {})
    if supersedes.get("sha256") != "56e894c5adea60a81bea79e5d46848c42bce8002cf10ab44d2b675dce75c082d" or supersedes.get("mutated") is not False:
        failures.append("V2 amendment must preserve and explicitly supersede V1")

    schedule = amendment.get("superseding_schedule", {})
    removed = set(schedule.get("from", {}).get("removed_obligations", []))
    added = set(schedule.get("to", {}).get("added_obligations", []))
    if removed != G003_OBLIGATIONS or added != G003_OBLIGATIONS:
        failures.append("V2 schedule must move exactly the five byte-plane obligations from G002 to G003")
    required_false = (
        "total_requirement_changed", "tests_removed", "quorum_or_finality_weakened",
        "provider_byte_plane_claim_allowed_at_p1", "feature_complete_claim_allowed_before_g003",
    )
    if any(schedule.get(field) is not False for field in required_false):
        failures.append("V2 schedule weakens or overclaims the accepted program")

    scope = amendment.get("g002_p1_scope", {})
    mappings = scope.get("real_test_mappings", [])
    if scope.get("claim") != "canonical-commons-control-plane-source-only" or len(mappings) != 8:
        failures.append("G002 must contain exactly the eight real control-plane mappings")
    mapped_tests: dict[str, str] = {}
    for mapping in mappings:
        if not isinstance(mapping, dict):
            continue
        test = mapping.get("test")
        package = mapping.get("package")
        if not all(isinstance(value, str) and value for value in (test, package)):
            failures.append("invalid G002 real test mapping")
            continue
        source_path = PACKAGE_SOURCES.get(package)
        if source_path is None or not source_path.is_file():
            failures.append(f"unknown or missing mapped package source: {package}")
            continue
        function = test.rsplit("::", 1)[-1]
        text = source_path.read_text(encoding="utf-8")
        if f"fn {function}(" not in text:
            failures.append(f"mapped G002 test is not located in {source_path}: {test}")
            continue
        if test in mapped_tests:
            failures.append(f"G002 test is mapped more than once: {test}")
        mapped_tests[test] = package

    must_not_claim = set(scope.get("must_not_claim", []))
    if not {"provider byte plane complete", "feature complete", "production ready"}.issubset(must_not_claim):
        failures.append("G002 scope does not preserve its mandatory negative claims")

    obligations = amendment.get("g003_p2_required_obligations", [])
    obligation_ids = {item.get("id") for item in obligations if isinstance(item, dict)}
    if obligation_ids != G003_OBLIGATIONS or len(obligations) != len(G003_OBLIGATIONS):
        failures.append("G003 must retain exactly five required provider byte-plane obligations")
    if any(item.get("status") != "missing-required" or item.get("test_mapping") is not None or not item.get("requirement") for item in obligations):
        failures.append("G003 obligations must remain explicit, unmapped, and missing-required")

    completion = amendment.get("completion_law", {})
    if set(completion) != {"before_g003", "after_g003", "whole_program"} or not all(
        isinstance(value, str) and value for value in completion.values()
    ):
        failures.append("V2 completion law is incomplete")

    executed_tests: dict[str, str] = {}
    if verification_path is None:
        failures.append("P1 control-plane execution report is required")
    else:
        try:
            verification = load_json(verification_path)
            executed_tests = verification.get("named_tests", {})
            if not isinstance(executed_tests, dict):
                raise ValueError("named_tests must be an object")
            for test in mapped_tests:
                if executed_tests.get(test) != "passed":
                    failures.append(f"mapped G002 test did not execute successfully: {test}")
            executed_g002 = set(verification.get("g002_control_plane_tests", []))
            if executed_g002 != set(mapped_tests):
                failures.append("verification report must bind exactly the eight V2 G002 tests")
            boundary = verification.get("claim_boundary", {})
            if boundary.get("g003_provider_byte_plane") is not False or boundary.get("feature_complete") is not False or boundary.get("production_ready") is not False:
                failures.append("P1 verification overclaims beyond V2 G002 scope")
        except (OSError, ValueError, json.JSONDecodeError) as exception:
            failures.append(f"cannot validate P1 control-plane execution report: {exception}")

    return {
        "amendment_sha256": amendment_sha,
        "byte_plane_claim_permitted_at_p1": schedule.get("provider_byte_plane_claim_allowed_at_p1"),
        "capability_id": CAPABILITY_ID,
        "failures": failures,
        "failure_count": len(failures),
        "frozen_ledger_sha256": ledger_sha,
        "g002_control_plane_test_count": len(mapped_tests),
        "p1_executed_test_count": sum(
            executed_tests.get(test) == "passed" for test in mapped_tests
        ),
        "p1_mapped_test_count": len(mapped_tests),
        "g003_missing_obligation_count": len(obligations),
        "schema_version": 2,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--amendment", "--addendum", dest="amendment", type=Path, required=True)
    parser.add_argument("--verification", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    report = validate(args.ledger, args.amendment, args.verification)
    payload = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(payload, encoding="utf-8")
    else:
        print(payload, end="")
    return 1 if report["failures"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
