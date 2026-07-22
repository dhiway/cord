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

"""Validate the normative per-symbol Orbis Names map and its non-normative CSV summary."""
import collections
import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAP = ROOT / "docs/sdk/contract-to-native-map.json"
CSV = ROOT / "docs/architecture/contract-to-native-migration.csv"
DISPOSITIONS = {"adopt-semantic", "intentional-change"}
BANNED = (
    "Dotns calls/runtime API/events/errors",
    "Dotns registration/policy",
    "Dotns reservation/deposit",
    "eligibility query plus",
    "bounded batch calls",
    "Dotns owner-record",
    "dotns.names",
    "dotns.records",
    "dotns.roles",
)


def main():
    data = json.loads(MAP.read_text())
    errors = []
    exact = []
    by_source = collections.defaultdict(list)
    for row in data["entries"]:
        # Historical contract identifiers are immutable provenance; native targets are Orbis Names.
        if not row.get("source_id", "").startswith("dotns:"):
            continue
        by_source[row["source_id"]].append(row)
        label = f"{row['source_id']}::{row['source_symbol_kind']}::{row['source_symbol']}"
        if row.get("disposition") in DISPOSITIONS:
            exact.append(row)
            for field in (
                "native_target",
                "rust_sdk",
                "typescript_sdk",
                "implementation_evidence",
                "semantic_evidence",
            ):
                if not row.get(field):
                    errors.append(f"{label}: missing {field}")
            surfaces = " ".join(
                str(row.get(field, ""))
                for field in ("native_target", "rust_sdk", "typescript_sdk")
            )
            if any(token in surfaces for token in BANNED):
                errors.append(f"{label}: generic target")
            if row.get("runtime_owner") != "Dotns":
                errors.append(f"{label}: historical retained row lost its source mapping owner")
        elif row.get("disposition") == "retired":
            if any(row.get(field) != "none" for field in ("runtime_owner", "rust_sdk", "typescript_sdk")):
                errors.append(f"{label}: retired row exposes native/SDK ownership")
        for field in ("implementation_evidence", "semantic_evidence", "vector_evidence"):
            if field in row and not (ROOT / row[field]).is_file():
                errors.append(f"{label}: missing {field} file {row[field]}")
        for evidence in row.get("vector_test_evidence", []):
            if not (ROOT / evidence).is_file():
                errors.append(f"{label}: missing vector test {evidence}")

    if any(token in MAP.read_text() for token in BANNED):
        errors.append("map still contains a generic Orbis Names target phrase")

    with CSV.open(newline="") as handle:
        csv_rows = [
        row for row in csv.DictReader(handle) if row["source_id"].startswith("dotns:")
        ]
    csv_by_source = {row["source_id"]: row for row in csv_rows}
    if set(csv_by_source) != set(by_source):
        errors.append("CSV/JSON Orbis Names source census differs")

    pointer = "normative per-symbol mapping: docs/sdk/contract-to-native-map.json"
    for source_id, row in csv_by_source.items():
        entries = by_source.get(source_id, [])
        counts = collections.Counter(entry["disposition"] for entry in entries)
        expected_disposition = next(iter(counts)) if len(counts) == 1 else "mixed-per-symbol"
        expected_state = ";".join(f"{key}:{counts[key]}" for key in sorted(counts))
        retained = any(entry["disposition"] in DISPOSITIONS for entry in entries)
        if row["disposition"] != expected_disposition:
            errors.append(f"{source_id}: CSV disposition conflicts with JSON")
        if row["symbol_disposition_state"] != expected_state:
            errors.append(f"{source_id}: CSV symbol counts conflict with JSON")
        if row["target_pallet"] != ("Dotns" if retained else "none"):
            errors.append(f"{source_id}: CSV target owner conflicts with JSON")
        for field in ("target_native_surface", "rust_sdk_surface", "typescript_sdk_surface"):
            if row[field] != pointer:
                errors.append(f"{source_id}: CSV {field} must defer to normative JSON")
        for field in ("repository", "commit", "path", "blob_sha256", "license_spdx", "semantic_evidence"):
            if not row[field]:
                errors.append(f"{source_id}: CSV lost provenance field {field}")

    if any(token in CSV.read_text() for token in BANNED):
        errors.append("CSV still contains a conflicting generic Orbis Names target phrase")
    if errors:
        raise SystemExit("\n".join(errors))
    print(
        f"validated {len(exact)} historical source rows mapped to Orbis Names and {len(csv_rows)} "
        "JSON-deferred CSV sources; no conflicts or generic retained targets"
    )


if __name__ == "__main__":
    main()
