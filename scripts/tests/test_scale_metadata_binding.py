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

"""Hostile P1/P3 SCALE portable-binding tests."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path


CORD = Path(__file__).resolve().parents[2]
FIXTURES = CORD / "scripts/tests/fixtures/scale-metadata"


class ScaleMetadataBindingTests(unittest.TestCase):
    def generate(self, registry: str, output: Path) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [
                "python3", "scripts/generate-scale-metadata-binding.py",
                "--metadata-scale", str(FIXTURES / "runtime.scale"),
                "--portable-registry", str(FIXTURES / registry),
                "--logical-types", "docs/specs/checkpoint-scale-logical-types-v1.toml",
                "--runtime", "origin-commons-runtime", "--out", str(output),
            ],
            cwd=CORD,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_complete_binding_is_stable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "binding.json"
            self.assertEqual(self.generate("portable-valid.json", output).returncode, 0)
            generated = json.loads(output.read_text())
            frozen = json.loads((FIXTURES / "binding-valid.json").read_text())
            self.assertEqual(generated, frozen)
            self.assertEqual(len(generated["logical_types"]), 5)

    def test_missing_logical_type_fails_without_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "binding.json"
            result = self.generate("portable-missing.json", output)
            self.assertEqual(result.returncode, 1)
            self.assertFalse(output.exists())
            self.assertIn(b"resolves to 0 portable entries", result.stderr)

    def test_shape_drift_fails_without_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "binding.json"
            result = self.generate("portable-drift.json", output)
            self.assertEqual(result.returncode, 1)
            self.assertFalse(output.exists())
            self.assertIn(b"logical type shape drift", result.stderr)

    def test_p3_descriptor_drift_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "recheck.json"
            result = subprocess.run(
                [
                    "python3", "scripts/validate-scale-metadata-binding.py",
                    "--binding", str(FIXTURES / "binding-valid.json"),
                    "--metadata-scale", str(FIXTURES / "runtime.scale"),
                    "--portable-registry", str(FIXTURES / "portable-valid.json"),
                    "--logical-types", "docs/specs/checkpoint-scale-logical-types-v1.toml",
                    "--descriptor", str(FIXTURES / "descriptor-drift.json"),
                    "--out", str(output),
                ], cwd=CORD, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            self.assertEqual(result.returncode, 1)
            report = json.loads(output.read_text())
            self.assertFalse(report["descriptor_binding_equal"])
            self.assertEqual(report["status"], "blocked")


if __name__ == "__main__":
    unittest.main()
