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

"""Generate the SDK-consumable descriptor for the current Commons metadata binding."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from evidence_common import atomic_write_json  # noqa: E402
from scale_metadata_binding import BindingError, verify_binding_hash  # noqa: E402


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument("--binding", required=True, type=Path)
	parser.add_argument("--out", required=True, type=Path)
	args = parser.parse_args()

	try:
		binding = json.loads(args.binding.read_text(encoding="utf-8"))
		if not verify_binding_hash(binding):
			raise BindingError("invalid SCALE metadata binding hash")
		logical_types = binding["logical_types"]
		metadata_sha256 = binding["metadata_sha256"]
	except (OSError, ValueError, KeyError, json.JSONDecodeError, BindingError) as exception:
		print(f"BLOCKED Commons runtime descriptor: {exception}", file=sys.stderr)
		return 1

	descriptor = {
		"schema_version": 1,
		"runtime": "origin-commons-runtime",
		"runtime_metadata_binding": {
			"metadata_sha256": metadata_sha256,
			"logical_types": logical_types,
		},
	}
	atomic_write_json(args.out, descriptor)
	print(f"PASS Commons runtime descriptor: {args.out}")
	return 0


if __name__ == "__main__":
	raise SystemExit(main())
