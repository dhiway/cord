#!/usr/bin/env python3
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
        payload = json.dumps({"core_version": {"specName": "origin", "specVersion": 9902}})
        info = module.parse_subwasm(payload)
        module.validate_runtime_info(info, spec_name="origin", spec_version=9902)

    def test_rejects_wrong_spec_version(self):
        with self.assertRaisesRegex(RuntimeError, "expected 30"):
            module.validate_runtime_info(
                {"runtime": {"spec_name": "orbis", "spec_version": 29}},
                spec_name="orbis",
                spec_version=30,
            )

    def test_command_hash_is_argument_boundary_sensitive(self):
        self.assertNotEqual(module.command_sha256(["ab", "c"]), module.command_sha256(["a", "bc"]))


if __name__ == "__main__":
    unittest.main()
