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

"""Shared deterministic evidence helpers.

``cord-json-canonical-v1`` is the exact canonical representation used by the
evidence framework.  It accepts only null, booleans, integers, strings, arrays
and objects with string keys; floats are rejected.  Object keys are ordered by
their unsigned UTF-8 encoding.  Strings use JSON escaping with UTF-8 characters
left unescaped and the output contains no insignificant whitespace.  The
encoding is UTF-8 with no BOM or trailing newline.  This deliberately small,
integer-only profile is stable across supported Python versions and avoids
claiming full RFC 8785 number serialization.
"""

from __future__ import annotations

import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Any


CANONICALIZATION = "cord-json-canonical-v1"


def _canonical(value: Any) -> str:
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        raise TypeError("cord-json-canonical-v1 forbids floating-point values")
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    if isinstance(value, list):
        return "[" + ",".join(_canonical(item) for item in value) + "]"
    if isinstance(value, dict):
        if not all(isinstance(key, str) for key in value):
            raise TypeError("canonical JSON object keys must be strings")
        keys = sorted(value, key=lambda key: key.encode("utf-8"))
        return "{" + ",".join(
            _canonical(key) + ":" + _canonical(value[key]) for key in keys
        ) + "}"
    raise TypeError(f"unsupported canonical JSON value: {type(value).__name__}")


def canonical_bytes(value: Any) -> bytes:
    """Return the exact ``cord-json-canonical-v1`` encoding."""

    return _canonical(value).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_write_json(path: Path, value: Any) -> None:
    """Durably replace ``path`` with canonical JSON."""

    path.parent.mkdir(parents=True, exist_ok=True)
    payload = canonical_bytes(value)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(payload)
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if temporary.exists():
            temporary.unlink()


def report_hash(report: dict[str, Any]) -> str:
    """Hash a report after omitting its self-referential hash field."""

    without_hash = dict(report)
    without_hash.pop("report_sha256", None)
    return sha256_bytes(canonical_bytes(without_hash))


def json_pointer(document: Any, pointer: str) -> Any:
    """Resolve an RFC 6901 JSON pointer without extensions."""

    if pointer == "":
        return document
    if not pointer.startswith("/"):
        raise ValueError(f"invalid JSON pointer: {pointer}")
    current = document
    for encoded in pointer[1:].split("/"):
        token = encoded.replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            current = current[int(token)]
        elif isinstance(current, dict):
            current = current[token]
        else:
            raise KeyError(pointer)
    return current
