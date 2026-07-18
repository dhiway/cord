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

"""Build Commons and atomically export its complete metadata binding inputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import shutil
from pathlib import Path

sys.dont_write_bytecode = True


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def checksum_entry(checksums: Path, artifact: Path) -> str | None:
    """Return the SHA-256 committed for an artifact by canonical SHA256SUMS."""
    for line in checksums.read_text(encoding="utf-8").splitlines():
        fields = line.split(maxsplit=1)
        if len(fields) != 2:
            continue
        digest, name = fields
        if name.lstrip(" *") == artifact.name:
            return digest
    return None


def verify_canonical_compact(
    canonical_compact: Path, release_inputs: Path, release_checksums: Path
) -> dict[str, str]:
    release_dir = release_inputs.resolve().parent
    if canonical_compact.resolve().parent != release_dir:
        raise ValueError("canonical Commons compact WASM and release inputs must share a release directory")
    if release_checksums.resolve().parent != release_dir:
        raise ValueError("canonical SHA256SUMS and release inputs must share a release directory")
    reproduction = release_dir / "origin_commons_runtime.reproduction.compact.wasm"
    for artifact in (canonical_compact, reproduction, release_inputs):
        if not artifact.is_file():
            raise ValueError(f"canonical release artifact is missing: {artifact}")
        expected = checksum_entry(release_checksums, artifact)
        if expected != sha256(artifact):
            raise ValueError(f"canonical SHA256SUMS does not bind {artifact.name}")
    if canonical_compact.read_bytes() != reproduction.read_bytes():
        raise ValueError("canonical and reproduction Commons compact WASM differ")
    return {
        "canonical_compact_wasm_sha256": sha256(canonical_compact),
        "release_inputs_sha256": sha256(release_inputs),
        "release_sha256sums_sha256": sha256(release_checksums),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata-scale", required=True, type=Path)
    parser.add_argument("--portable-registry", required=True, type=Path)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--compact-wasm", type=Path)
    parser.add_argument("--canonical-compact-wasm", required=True, type=Path)
    parser.add_argument("--release-inputs", required=True, type=Path)
    args = parser.parse_args()

    metadata = args.metadata_scale.resolve()
    registry = args.portable_registry.resolve()
    metadata.parent.mkdir(parents=True, exist_ok=True)
    registry.parent.mkdir(parents=True, exist_ok=True)

    try:
        canonical = verify_canonical_compact(
            args.canonical_compact_wasm,
            args.release_inputs,
            args.release_inputs.resolve().parent / "SHA256SUMS",
        )
    except (OSError, ValueError) as exception:
        print(f"BLOCKED canonical Commons compact WASM: {exception}", file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="commons-metadata-", dir=metadata.parent) as temp:
        temp_root = Path(temp)
        temp_metadata = temp_root / "runtime.metadata.scale"
        temp_registry = temp_root / "runtime.portable-registry.json"
        command = [
            "cargo",
            "run",
            "--locked",
            "--quiet",
            "-p",
            "origin-commons-runtime",
            "--example",
            "export_scale_metadata",
            "--",
            os.fspath(temp_metadata),
            os.fspath(temp_registry),
        ]
        result = subprocess.run(command, check=False)
        if result.returncode != 0:
            print("BLOCKED Commons metadata export: runtime build/export failed", file=sys.stderr)
            return result.returncode
        if not temp_metadata.is_file() or not temp_registry.is_file():
            print("BLOCKED Commons metadata export: producer omitted an output", file=sys.stderr)
            return 1
        os.replace(temp_metadata, metadata)
        os.replace(temp_registry, registry)
        if args.compact_wasm:
            args.compact_wasm.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(args.canonical_compact_wasm, args.compact_wasm)

    print(f"PASS Commons SCALE metadata: {metadata}")
    print(f"PASS Commons portable registry: {registry}")
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(
            json.dumps(
                {
                    "metadata_scale_sha256": hashlib.sha256(metadata.read_bytes()).hexdigest(),
                    "portable_registry_sha256": hashlib.sha256(registry.read_bytes()).hexdigest(),
                    "compact_wasm_sha256": sha256(args.compact_wasm) if args.compact_wasm else None,
                    "canonical_release": canonical,
                    "schema_version": 1,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
