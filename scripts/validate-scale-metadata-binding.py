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

"""Recheck the P1 binding against P3 runtime metadata and product descriptor."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json  # noqa: E402
from scale_metadata_binding import BindingError, produce_binding, verify_binding_hash  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binding", required=True, type=Path)
    parser.add_argument("--metadata-scale", required=True, type=Path)
    parser.add_argument("--portable-registry", required=True, type=Path)
    parser.add_argument("--logical-types", required=True, type=Path)
    parser.add_argument("--descriptor", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    failures = []
    try:
        binding = json.loads(args.binding.read_text(encoding="utf-8"))
        current = produce_binding(
            args.metadata_scale,
            args.portable_registry,
            args.logical_types,
            "origin-commons-runtime",
        )
        descriptor = json.loads(args.descriptor.read_text(encoding="utf-8"))
        binding_hash_valid = verify_binding_hash(binding)
        metadata_hash_matches = binding.get("metadata_sha256") == current["metadata_sha256"]
        binding_equal = binding.get("logical_types") == current["logical_types"]
        descriptor_equal = descriptor.get("runtime_metadata_binding") == {
            "metadata_sha256": current["metadata_sha256"],
            "logical_types": current["logical_types"],
        }
        if not binding_hash_valid:
            failures.append("P1 binding hash is invalid")
        if not metadata_hash_matches or not binding_equal:
            failures.append("P1 binding drifted from P3 runtime metadata")
        if not descriptor_equal:
            failures.append("P3 product descriptor binding is absent or unequal")
    except (OSError, ValueError, KeyError, json.JSONDecodeError, BindingError) as exception:
        binding_hash_valid = metadata_hash_matches = binding_equal = descriptor_equal = False
        failures.append(str(exception))
    report = {
        "schema_version": 1,
        "status": "pass" if not failures else "blocked",
        "binding_hash_valid": binding_hash_valid,
        "metadata_hash_matches": metadata_hash_matches,
        "binding_equal": binding_equal,
        "descriptor_binding_equal": descriptor_equal,
        "missing_logical_types": 0 if binding_equal else 1,
        "drifted_logical_types": 0 if binding_equal else 1,
        "failures": len(failures),
        "failure_details": failures,
    }
    atomic_write_json(args.out, report)
    print(f"{report['status'].upper()} P3 SCALE metadata binding recheck")
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
