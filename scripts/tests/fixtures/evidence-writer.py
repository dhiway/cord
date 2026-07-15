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

"""Hostile evidence-runner fixture; never used by a product gate."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--extra", type=Path)
    parser.add_argument("--read", type=Path)
    parser.add_argument("--skip-output", action="store_true")
    parser.add_argument("--hardcoded-env-read", action="store_true")
    parser.add_argument("--transitive-read", action="store_true")
    parser.add_argument("--child-read", action="store_true")
    args = parser.parse_args()
    if args.read:
        args.read.read_bytes()
    if args.hardcoded_env_read:
        Path(os.environ["CORD_HOSTILE_READ"]).read_bytes()
    if args.transitive_read:
        from evidence_transitive_reader import read_hidden
        read_hidden()
    if args.child_read:
        subprocess.run(
            [sys.executable, "-c", "import os;open(os.environ['CORD_HOSTILE_READ'],'rb').read()"],
            check=True,
        )
    if not args.skip_output:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps({"ok": True}), encoding="utf-8")
    if args.extra:
        args.extra.parent.mkdir(parents=True, exist_ok=True)
        args.extra.write_text("hostile-write\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
