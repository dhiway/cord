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

"""Check the clean-break scope contract used for the feature-complete stop gate."""
from __future__ import annotations
import argparse
import json
import sys
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
from pathlib import Path


def read_json(path: Path, findings: list[str], label: str) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        return value if isinstance(value, dict) else {}
    except (OSError, json.JSONDecodeError) as error:
        findings.append(f"{label} is unreadable: {error}")
        return {}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", required=True, type=Path)
    parser.add_argument("--deletion", required=True, type=Path)
    parser.add_argument("--repositories", required=True, type=Path)
    parser.add_argument("--p7", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    economics: list[str] = []
    duplicate: list[str] = []
    migration: list[str] = []
    fallback: list[str] = []
    mobile: list[str] = []
    external: list[str] = []
    stop: list[str] = []
    p8: list[str] = []
    try:
        ledger = tomllib.loads(args.ledger.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        ledger = {}
        duplicate.append(f"disposition ledger is unreadable: {error}")
    try:
        deletion = tomllib.loads(args.deletion.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        deletion = {}
        migration.append(f"deletion manifest is unreadable: {error}")
    # The ratified ledger is the authority for retained internal functionality;
    # it must not enable a public token/stake surface or a metadata shim.
    if ledger.get("decision") != "internal-retain" or ledger.get("status") != "ratified":
        duplicate.append("lightweight-humanity disposition is not ratified internal retention")
    if ledger.get("metadata_filter_shim") is not False:
        fallback.append("metadata filter shim is enabled")
    if ledger.get("separate_signing") != "transaction.sign":
        economics.append("identity signing authority is not transaction.sign")
    if deletion.get("policy") != "disposition-driven-clean-break":
        migration.append("deletion policy is not clean-break")
    obsolete = deletion.get("obsolete", [])
    if not obsolete:
        duplicate.append("obsolete-surface inventory is empty")
    # A replacement is complete only if every cleanup row is a terminal
    # deletion receipt or an independently present canonical replacement.
    items = deletion.get("item", [])
    invalid = [str(row.get("id", "?")) for row in items
               if row.get("status") not in {"deleted", "replacement-active"}]
    if invalid:
        migration.append("non-terminal clean-break items: " + ", ".join(invalid))
    active = [row for row in items if row.get("status") == "replacement-active"]
    if not active:
        duplicate.append("replacement-active inventory is empty")
    root = Path.cwd()
    for row in active:
        identifier = str(row.get("id", "?"))
        path = root / str(row.get("path", ""))
        symbol = str(row.get("symbol", ""))
        locator = str(row.get("surface_locator", ""))
        if not path.is_file():
            duplicate.append(f"{identifier}: replacement path is absent: {path}")
            continue
        source = path.read_text(encoding="utf-8")
        if locator == "http-route":
            # The route token is sufficient here: API verb dispatch may be
            # macro-generated, but the canonical path must remain concrete.
            route = symbol.split(" ", 1)[-1]
            present = route in source
        elif locator == "file":
            present = True
        elif locator == "rust-export":
            name = symbol.removeprefix("rust-export::")
            present = f"pub fn {name}" in source or f"pub(crate) fn {name}" in source
        else:
            present = symbol in source
        if not present:
            duplicate.append(f"{identifier}: active replacement marker is absent: {symbol}")
    repositories = read_json(args.repositories, external, "repository boundary report")
    if repositories.get("status") != "pass":
        external.append("repository boundary report is not pass")
    for key in ("external_modified", "external_modifications", "modified_external_repositories"):
        value = repositories.get(key)
        if isinstance(value, int) and value != 0:
            external.append(f"{key}={value}")
    phase = read_json(args.p7, stop, "P7 report")
    if phase.get("status") != "pass":
        stop.append("P7 is not pass")
    # P7 is intentionally a feature-completeness boundary, not production
    # readiness; a P8 authorization here would violate that stop condition.
    if phase.get("gate_id") not in (None, "P7"):
        stop.append("P7 report gate_id drift")
    if phase.get("p8_authorized") is True:
        p8.append("P8 authorization is present")
    report = {
        "economics_findings": economics,
        "duplicate_state_findings": duplicate,
        "migration_compat_findings": migration,
        "fallback_findings": fallback,
        "production_mobile_edits": mobile,
        "external_repo_findings": external,
        "stop_condition_findings": stop,
        "p8_authorization_findings": p8,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return 0 if not any(report.values()) else 1

if __name__ == "__main__":
    raise SystemExit(main())
