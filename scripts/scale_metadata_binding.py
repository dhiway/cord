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

"""Fail-closed SCALE metadata binding primitives."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib

from evidence_common import canonical_bytes, sha256_bytes, sha256_file


class BindingError(ValueError):
    """Raised when metadata cannot produce the complete frozen binding."""


def load_logical_types(path: Path) -> list[dict[str, Any]]:
    document = tomllib.loads(path.read_text(encoding="utf-8"))
    rows = document.get("logical_type", [])
    names = [row.get("name") for row in rows]
    paths = [row.get("path") for row in rows]
    if len(rows) != 5 or len(set(names)) != 5 or len(set(paths)) != 5:
        raise BindingError("logical type contract must contain five unique names and paths")
    result = []
    for row in rows:
        try:
            shape = json.loads(row["shape"])
        except (KeyError, json.JSONDecodeError) as exception:
            raise BindingError(f"invalid logical shape for {row.get('name')}: {exception}") from exception
        result.append({"name": row["name"], "path": row["path"], "shape": shape})
    return result


def produce_binding(
    metadata_scale: Path,
    portable_registry: Path,
    logical_types_path: Path,
    runtime: str,
) -> dict[str, Any]:
    if runtime != "origin-commons-runtime":
        raise BindingError("runtime must be origin-commons-runtime")
    metadata_hash = sha256_file(metadata_scale)
    registry = json.loads(portable_registry.read_text(encoding="utf-8"))
    if registry.get("metadata_sha256") != metadata_hash:
        raise BindingError("portable registry metadata hash does not match complete SCALE metadata")
    type_rows = registry.get("types")
    if not isinstance(type_rows, list):
        raise BindingError("portable registry types must be an array")
    by_path: dict[str, list[dict[str, Any]]] = {}
    for row in type_rows:
        if not isinstance(row, dict) or not isinstance(row.get("path"), str):
            raise BindingError("portable type entry is malformed")
        by_path.setdefault(row["path"], []).append(row)

    bindings = []
    portable_ids = set()
    for logical in load_logical_types(logical_types_path):
        matches = by_path.get(logical["path"], [])
        if len(matches) != 1:
            raise BindingError(
                f"logical type {logical['name']} resolves to {len(matches)} portable entries"
            )
        match = matches[0]
        portable_id = match.get("id")
        if not isinstance(portable_id, int) or portable_id < 0:
            raise BindingError(f"logical type {logical['name']} has an invalid portable ID")
        if portable_id in portable_ids:
            raise BindingError(f"portable ID {portable_id} is reused by required logical types")
        portable_ids.add(portable_id)
        expected_shape = canonical_bytes(logical["shape"])
        actual_shape = canonical_bytes(match.get("shape"))
        if actual_shape != expected_shape:
            raise BindingError(f"logical type shape drift: {logical['name']}")
        bindings.append(
            {
                "logical_name": logical["name"],
                "type_path": logical["path"],
                "portable_id": portable_id,
                "shape_sha256": sha256_bytes(expected_shape),
            }
        )
    bindings.sort(key=lambda row: row["logical_name"].encode("utf-8"))
    report = {
        "schema_version": 1,
        "runtime": runtime,
        "metadata_sha256": metadata_hash,
        "portable_registry_sha256": sha256_file(portable_registry),
        "logical_types_spec_sha256": sha256_file(logical_types_path),
        "logical_types": bindings,
        "complete": True,
    }
    report["binding_sha256"] = sha256_bytes(canonical_bytes(report))
    return report


def verify_binding_hash(binding: dict[str, Any]) -> bool:
    unsigned = dict(binding)
    expected = unsigned.pop("binding_sha256", None)
    return isinstance(expected, str) and expected == sha256_bytes(canonical_bytes(unsigned))
