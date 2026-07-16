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

"""Materialize AC3 assertions from the frozen checkpoint proof vectors."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True


FROZEN_NEGATIVE_CODES = {
    "checkpoint-wrong-domain": 220,
    "checkpoint-wrong-version": 221,
    "checkpoint-wrong-bucket": 222,
    "checkpoint-wrong-key": 223,
    "checkpoint-stale-nonce": 224,
    "checkpoint-wrong-window": 225,
}


def repository_root() -> Path:
    output = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True
    )
    return Path(output.strip())


def sha256_hex(value: str) -> str:
    return hashlib.sha256(bytes.fromhex(value)).hexdigest()


def validate_hashes(vector: dict[str, Any], failures: list[str]) -> None:
    vector_id = str(vector.get("id", "unknown"))
    for name, value in vector.items():
        if not name.endswith("_hex") or not isinstance(value, str):
            continue
        digest_stem = name.removesuffix("_hex").removesuffix("_cbor")
        digest_name = digest_stem + "_sha256"
        expected = vector.get(digest_name)
        if isinstance(expected, str):
            try:
                observed = sha256_hex(value)
            except ValueError:
                failures.append(f"{vector_id}:{name} is not hexadecimal")
                continue
            if observed != expected:
                failures.append(f"{vector_id}:{digest_name} does not match {name}")


def require_hash_pair(
    vector: dict[str, Any], stem: str, failures: list[str]
) -> None:
    vector_id = str(vector.get("id", "unknown"))
    encoded = vector.get(f"{stem}_cbor_hex")
    expected = vector.get(f"{stem}_sha256")
    if not isinstance(encoded, str) or not isinstance(expected, str):
        failures.append(f"{vector_id}:{stem} frozen hash proof is missing")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    root = repository_root()
    vector_path = root / "docs/specs/checkpoint-v2.vectors.json"
    vectors = json.loads(vector_path.read_text(encoding="utf-8"))
    proof_failures: list[str] = []
    negative_code_failures: list[str] = []

    if vectors.get("version") != 2 or vectors.get("type") != "CheckpointSubmissionV2":
        proof_failures.append("checkpoint vector envelope is not version 2")

    positive = vectors.get("positive", {})
    negative = vectors.get("negative", [])
    if not isinstance(positive, dict) or not isinstance(negative, list):
        proof_failures.append("checkpoint vector body has an invalid shape")
        positive = {}
        negative = []

    validate_hashes(positive, proof_failures)
    for stem in ("canonical", "exact_response", "exact_event"):
        require_hash_pair(positive, stem, proof_failures)
    signed_message = positive.get("signed_message_hex")
    expected_digest = positive.get("digest_hex")
    if isinstance(signed_message, str) and isinstance(expected_digest, str):
        try:
            observed_digest = hashlib.blake2b(
                bytes.fromhex(signed_message), digest_size=32
            ).hexdigest()
        except ValueError:
            observed_digest = ""
        if observed_digest != expected_digest:
            proof_failures.append("checkpoint signed-message BLAKE2b-256 digest drifted")
    else:
        proof_failures.append("checkpoint signed-message digest proof is missing")

    rows: dict[str, dict[str, Any]] = {}
    for item in negative:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            proof_failures.append("checkpoint negative vector has an invalid shape")
            continue
        vector_id = item["id"]
        if vector_id in rows:
            proof_failures.append(f"duplicate checkpoint vector: {vector_id}")
            continue
        rows[vector_id] = item
        validate_hashes(item, proof_failures)
        for stem in ("canonical", "exact_response"):
            require_hash_pair(item, stem, proof_failures)

    for vector_id, expected_code in FROZEN_NEGATIVE_CODES.items():
        vector = rows.get(vector_id)
        if vector is None:
            negative_code_failures.append(f"missing {vector_id}")
            continue
        if vector.get("expected_error_code") != expected_code:
            negative_code_failures.append(f"{vector_id} does not use code {expected_code}")
        if (
            vector.get("pre_state_sha256") != vector.get("post_state_sha256")
            or vector.get("effect_count") != 0
            or vector.get("event_count") != 0
        ):
            proof_failures.append(f"{vector_id} is not a zero-effect rejection")

    equivocation = rows.get("checkpoint-equivocation", {})
    evidence = equivocation.get("effect_count", 0)
    suspensions = equivocation.get("event_count", 0)
    if not isinstance(evidence, int) or not isinstance(suspensions, int):
        proof_failures.append("equivocation evidence counters are not integers")
        evidence = 0
        suspensions = 0
    if equivocation.get("pre_state_sha256") == equivocation.get("post_state_sha256"):
        proof_failures.append("equivocation does not preserve a state transition")
    if not isinstance(equivocation.get("exact_event_cbor_hex"), str):
        proof_failures.append("equivocation evidence event proof is missing")
    else:
        require_hash_pair(equivocation, "exact_event", proof_failures)

    report = {
        "proof_failures": len(proof_failures),
        "negative_code_failures": len(negative_code_failures),
        "evidence": evidence,
        "suspensions": suspensions,
        "proof_failure_details": proof_failures,
        "negative_code_failure_details": negative_code_failures,
        "source": str(vector_path.relative_to(root)),
        "source_sha256": hashlib.sha256(vector_path.read_bytes()).hexdigest(),
    }
    output = args.out if args.out.is_absolute() else root / args.out
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 1 if proof_failures or negative_code_failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
