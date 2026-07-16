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

"""Validate the frozen AC2 content vectors without creating storage effects."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import tempfile
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib


EXPECTED_VECTOR_SHA256 = "8458f96e635568ae84e4234a26378f6fb2133d5ce9877ae1aa985970752899c9"
EXPECTED_BOUNDS_SHA256 = "0dd0a38e5a064e23501f0f446bbef9bd6015fdbaef953bdf0e515f92bb1a2b2c"
EXPECTED_NEGATIVE_CODES = {
    "none-64m-plus-1": "STORAGE_OBJECT_TOO_LARGE",
    "encrypted-plaintext-plus-1": "STORAGE_OBJECT_TOO_LARGE",
    "none-manifest-plus-1": "STORAGE_OBJECT_TOO_LARGE",
    "encrypted-manifest-plus-1": "STORAGE_OBJECT_TOO_LARGE",
    "range-end-past-object": "STORAGE_RANGE_INVALID",
    "duplicate-changed-byte": "STORAGE_IDEMPOTENCY_CONFLICT",
    "corrupt-stored-byte": "STORAGE_INTEGRITY_FAILED",
    "encrypted-nonce-reuse": "ENCRYPTION_NONCE_REUSE",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def varint(value: int) -> bytes:
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        encoded.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(encoded)


def canonical_cid(digest: bytes) -> str:
    binary = varint(1) + varint(0x55) + varint(0xB220) + varint(32) + digest
    return "b" + base64.b32encode(binary).decode("ascii").rstrip("=").lower()


def repeated_digest(length: int) -> bytes:
    digest = hashlib.blake2b(digest_size=32)
    block = bytes([0xA5]) * (1024 * 1024)
    while length:
        take = min(length, len(block))
        digest.update(block[:take])
        length -= take
    return digest.digest()


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as target:
            target.write(payload)
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def validate(root: Path) -> dict[str, Any]:
    vector_path = root / "docs/specs/storage-v1.vectors.json"
    bounds_path = root / "docs/specs/storage-bounds-v1.toml"
    vectors = json.loads(vector_path.read_text(encoding="utf-8"))
    bounds = tomllib.loads(bounds_path.read_text(encoding="utf-8"))
    rows = vectors.get("vectors", [])
    by_id = {row.get("id"): row for row in rows if isinstance(row, dict)}
    vector_errors: list[str] = []
    negative_errors: list[str] = []
    commit_errors: list[str] = []
    hash_errors: list[str] = []

    expected_bounds = {
        "cid_version": 1,
        "cid_multicodec": 0x55,
        "multihash_code": 0xB220,
        "digest_bytes": 32,
        "chunk_bytes": 262_144,
        "max_chunks": 256,
        "max_stored_object_bytes": 67_108_864,
        "max_none_plaintext_bytes": 67_108_864,
        "xchacha_envelope_overhead_bytes": 41,
        "max_encrypted_plaintext_bytes": 67_108_823,
    }
    for key, expected in expected_bounds.items():
        if bounds.get(key) != expected:
            vector_errors.append(f"bound {key} is {bounds.get(key)!r}, expected {expected}")
    if vectors.get("version") != 1 or len(by_id) != len(rows):
        vector_errors.append("vector version or unique identifier set is invalid")
    profile = vectors.get("cid_profile", {})
    if profile != {
        "version": 1,
        "multicodec": 85,
        "multihash": 45600,
        "digest": "blake2b-256",
        "text": "base32lower",
    }:
        vector_errors.append("CID profile differs from the frozen launch profile")

    hash_checked: list[str] = []
    for row in rows:
        identifier = row.get("id", "<missing>")
        stored_len = row.get("stored_len")
        chunk_count = row.get("chunk_count")
        if isinstance(stored_len, int) and stored_len <= bounds["max_stored_object_bytes"]:
            expected_chunks = (stored_len + bounds["chunk_bytes"] - 1) // bounds["chunk_bytes"]
            if chunk_count is not None and chunk_count != expected_chunks:
                vector_errors.append(f"{identifier}: chunk count is not derived from stored bytes")
        if row.get("mode") == "xchacha20poly1305-v1" and isinstance(row.get("plaintext_len"), int):
            if stored_len != row["plaintext_len"] + bounds["xchacha_envelope_overhead_bytes"]:
                vector_errors.append(f"{identifier}: encrypted stored length is not plaintext plus 41")
        digest: bytes | None = None
        if isinstance(row.get("envelope_hex"), str):
            envelope = bytes.fromhex(row["envelope_hex"])
            if len(envelope) != stored_len:
                vector_errors.append(f"{identifier}: literal envelope length drift")
            digest = hashlib.blake2b(envelope, digest_size=32).digest()
        elif row.get("generator") == "repeat-byte-a5" and row.get("mode") != "xchacha20poly1305-v1":
            digest = repeated_digest(row["plaintext_len"])
        if digest is not None:
            hash_checked.append(identifier)
            if digest.hex() != row.get("digest_hex") or canonical_cid(digest) != row.get("cid"):
                hash_errors.append(f"{identifier}: digest or canonical CID mismatch")
        if isinstance(row.get("cid"), str) and not (
            row["cid"].startswith("b") and row["cid"] == row["cid"].lower()
        ):
            vector_errors.append(f"{identifier}: CID is not base32lower")

    for identifier, expected in EXPECTED_NEGATIVE_CODES.items():
        if by_id.get(identifier, {}).get("expected_error") != expected:
            negative_errors.append(f"{identifier}: expected error code drift")
    if by_id.get("none-64m-plus-1", {}).get("pre_encryption_rejection") is not True:
        commit_errors.append("none plus-one is not rejected before content admission")
    if by_id.get("encrypted-plaintext-plus-1", {}).get("pre_encryption_rejection") is not True:
        commit_errors.append("encrypted plus-one is not rejected before encryption")
    if by_id.get("corrupt-stored-byte", {}).get("returned_bytes") != 0:
        commit_errors.append("corrupt stored bytes can escape verification")
    if by_id.get("duplicate-identical", {}).get("effects") != 1 or by_id.get(
        "duplicate-changed-byte", {}
    ).get("effects") != 1:
        commit_errors.append("changed retry can create a second effect")
    if not str(by_id.get("encrypted-nonce-reuse", {}).get("effect", "")).startswith(
        "reject before"
    ):
        commit_errors.append("nonce reuse is not rejected before encryption/addressing")

    if sha256(vector_path) != EXPECTED_VECTOR_SHA256:
        hash_errors.append("storage vector file hash drift")
    if sha256(bounds_path) != EXPECTED_BOUNDS_SHA256:
        hash_errors.append("storage bounds file hash drift")
    return {
        "schema_version": 1,
        "vector_failures": len(vector_errors),
        "negative_code_failures": len(negative_errors),
        "commits": len(commit_errors),
        "hash_failures": len(hash_errors),
        "vector_count": len(rows),
        "negative_vector_count": len(EXPECTED_NEGATIVE_CODES),
        "hash_checked_vectors": hash_checked,
        "provider_boundary": "stored-bytes-only",
        "host_encrypted_plus_one": by_id.get("encrypted-plaintext-plus-1", {}).get("stored_len"),
        "failures": vector_errors + negative_errors + commit_errors + hash_errors,
        "input_sha256": {
            "docs/specs/storage-v1.vectors.json": sha256(vector_path),
            "docs/specs/storage-bounds-v1.toml": sha256(bounds_path),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    report = validate(Path.cwd().resolve())
    atomic_json(args.out, report)
    print(json.dumps(report, sort_keys=True))
    return 0 if not report["failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
