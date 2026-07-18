#!/usr/bin/env python3
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

"""Validate the immutable Drive/S3 wire vectors consumed by native SDK tests."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VECTORS = ROOT / "docs/specs/drive-s3-v1.vectors.json"


def digest(hex_value: str) -> str:
    return hashlib.sha256(bytes.fromhex(hex_value)).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    data = json.loads(VECTORS.read_text(encoding="utf-8"))
    bounds: list[str] = []
    codes: list[str] = []
    states: list[str] = []
    events: list[str] = []
    vectors = data.get("vectors", [])
    if data.get("version") != 1 or len(vectors) != 2:
        bounds.append("expected exactly the v1 Drive and S3 vector pair")
    expected = {
        "drive-manifest-v1": {"type": "DriveManifestV1", "negatives": {("DRIVE_VERSION_CONFLICT", 306), ("DRIVE_ORDER_INVALID", 305)}},
        "s3-object-version-v1": {"type": "S3ObjectVersionV1", "negatives": {("S3_PRECONDITION_FAILED", 323), ("S3_KEY_INVALID", 321)}},
    }
    for row in vectors:
        identifier = row.get("id")
        contract = expected.get(identifier)
        if contract is None:
            bounds.append(f"unexpected vector {identifier!r}")
            continue
        if row.get("type") != contract["type"]:
            bounds.append(f"{identifier}: type drift")
        for field in ("canonical_cbor_hex", "pre_state_cbor_hex", "post_state_cbor_hex", "exact_response_cbor_hex"):
            hash_field = field.replace("_cbor_hex", "_sha256")
            try:
                if digest(row[field]) != row.get(hash_field):
                    states.append(f"{identifier}: {hash_field} drift")
            except (KeyError, ValueError):
                states.append(f"{identifier}: malformed {field}")
        if row.get("expected") != "accept" or row.get("effect_count") != 1 or row.get("event_count") != 1:
            events.append(f"{identifier}: accepted effect/event contract drift")
        actual_negatives: set[tuple[str, int]] = set()
        for negative in row.get("negative_vectors", []):
            code, name = negative.get("expected_error_code"), negative.get("expected_error")
            actual_negatives.add((name, code))
            if negative.get("effect_count") != 0 or negative.get("event_count") != 0:
                events.append(f"{identifier}/{negative.get('id')}: rejected vector emitted effects/events")
            if negative.get("post_state_sha256") != row.get("pre_state_sha256"):
                states.append(f"{identifier}/{negative.get('id')}: rejected vector mutated state")
        if actual_negatives != contract["negatives"]:
            codes.append(f"{identifier}: negative error-code contract drift")
    report = {
        "vector_count": len(vectors),
        "bound_failures": bounds,
        "error_code_failures": codes,
        "state_hash_failures": states,
        "event_hash_failures": events,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return 0 if not any((bounds, codes, states, events)) else 1

if __name__ == "__main__":
    raise SystemExit(main())
