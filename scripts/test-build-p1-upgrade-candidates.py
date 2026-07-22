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

"""Unit tests for the P1 upgrade-candidate builder (no compilation or network)."""

import importlib.util
import json
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("build-p1-upgrade-candidates.py")
spec = importlib.util.spec_from_file_location("p1_upgrade_candidates", SCRIPT)
module = importlib.util.module_from_spec(spec)
assert spec.loader
spec.loader.exec_module(module)


class CandidateBuilderTests(unittest.TestCase):
    def test_checked_in_sources_keep_production_and_candidate_versions(self):
        module.assert_version_sources()

    def test_subwasm_nested_runtime_version(self):
        payload = json.dumps({"core_version": {"specName": "foundation", "specVersion": 9902}})
        info = module.parse_subwasm(payload)
        module.validate_runtime_info(info, spec_name="foundation", spec_version=9902)

    def test_subwasm_commons_runtime_version(self):
        payload = json.dumps({"core_version": {"specName": "commons", "specVersion": 32}})
        info = module.parse_subwasm(payload)
        module.validate_runtime_info(info, spec_name="commons", spec_version=32)

    def test_rejects_wrong_spec_version(self):
        with self.assertRaisesRegex(RuntimeError, "expected 32"):
            module.validate_runtime_info(
                {"runtime": {"spec_name": "commons", "spec_version": 31}},
                spec_name="commons",
                spec_version=32,
            )

    def test_command_hash_is_argument_boundary_sensitive(self):
        self.assertNotEqual(module.command_sha256(["ab", "c"]), module.command_sha256(["a", "bc"]))


if __name__ == "__main__":
    unittest.main()
