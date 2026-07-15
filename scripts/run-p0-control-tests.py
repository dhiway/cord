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

"""Run the P0-bound B6/B7 hostile control suites and emit an exact receipt."""

from __future__ import annotations

import argparse
import json
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from evidence_common import atomic_write_json


MODULES = (
    "scripts.tests.test_storage_identity_deletion",
    "scripts.tests.test_evidence_runner_safety",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    suite = unittest.TestLoader().loadTestsFromNames(MODULES)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {
        "schema_version": 1,
        "modules": list(MODULES),
        "tests_run": result.testsRun,
        "failures": len(result.failures),
        "errors": len(result.errors),
        "skipped": len(result.skipped),
        "status": "pass" if result.wasSuccessful() else "fail",
    }
    atomic_write_json(args.out, report)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
