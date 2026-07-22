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

"""Produce the P1 logical SCALE type to portable-ID binding."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json  # noqa: E402
from scale_metadata_binding import BindingError, produce_binding  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata-scale", required=True, type=Path)
    parser.add_argument("--portable-registry", required=True, type=Path)
    parser.add_argument("--logical-types", required=True, type=Path)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = produce_binding(
            args.metadata_scale, args.portable_registry, args.logical_types, args.runtime
        )
    except (OSError, ValueError) as exception:
        print(f"BLOCKED SCALE metadata binding: {exception}", file=sys.stderr)
        return 1
    atomic_write_json(args.out, report)
    print(f"PASS SCALE metadata binding: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
