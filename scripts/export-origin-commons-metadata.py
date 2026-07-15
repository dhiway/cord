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
from pathlib import Path

sys.dont_write_bytecode = True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata-scale", required=True, type=Path)
    parser.add_argument("--portable-registry", required=True, type=Path)
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()

    metadata = args.metadata_scale.resolve()
    registry = args.portable_registry.resolve()
    metadata.parent.mkdir(parents=True, exist_ok=True)
    registry.parent.mkdir(parents=True, exist_ok=True)

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

    print(f"PASS Commons SCALE metadata: {metadata}")
    print(f"PASS Commons portable registry: {registry}")
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(
            json.dumps(
                {
                    "metadata_scale_sha256": hashlib.sha256(metadata.read_bytes()).hexdigest(),
                    "portable_registry_sha256": hashlib.sha256(registry.read_bytes()).hexdigest(),
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
