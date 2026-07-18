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

"""Focused tests for the Origin/Commons evidence boundary."""

from __future__ import annotations

import json
import hashlib
import importlib.util
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True


CORD = Path(__file__).resolve().parents[2]
SCRIPTS = CORD / "scripts"
PYTHON = Path("/opt/homebrew/bin/python3")
if not PYTHON.exists():
    PYTHON = Path(shutil.which("python3") or sys.executable)
sys.path.insert(0, str(SCRIPTS))

from evidence_common import atomic_write_json, canonical_bytes, report_hash  # noqa: E402


def load_script(name: str):
    specification = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    assert specification and specification.loader
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def run(*argv: str, cwd: Path = CORD, expected: int = 0) -> subprocess.CompletedProcess[bytes]:
    command = (str(PYTHON), *argv[1:]) if argv and argv[0] == "python3" else argv
    result = subprocess.run(command, cwd=cwd, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != expected:
        raise AssertionError(
            f"{command!r} returned {result.returncode}, expected {expected}: "
            f"{result.stderr.decode('utf-8', 'replace')}"
        )
    return result


def init_repo(path: Path, name: str) -> None:
    path.mkdir()
    run("git", "init", "-q", cwd=path)
    run("git", "config", "user.name", "Test", cwd=path)
    run("git", "config", "user.email", "test@example.invalid", cwd=path)
    (path / name).write_text("one\n", encoding="utf-8")
    run("git", "add", name, cwd=path)
    run("git", "commit", "-qm", "initial", cwd=path)


class CanonicalJsonTests(unittest.TestCase):
    def test_canonical_order_and_report_field_omission(self) -> None:
        self.assertEqual(canonical_bytes({"z": 1, "a": "é"}), b'{"a":"\xc3\xa9","z":1}')
        first = {"schema_version": 1, "report_sha256": "old", "value": [True, None, -2]}
        second = dict(first, report_sha256="different")
        self.assertEqual(report_hash(first), report_hash(second))

    def test_floats_are_rejected(self) -> None:
        with self.assertRaises(TypeError):
            canonical_bytes({"not_allowed": 1.5})

    def test_atomic_write_has_no_temporary_residue(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "report.json"
            atomic_write_json(output, {"ok": True})
            self.assertEqual(json.loads(output.read_text()), {"ok": True})
            self.assertEqual(list(output.parent.glob(".report.json.*.tmp")), [])


class RepositoryBoundaryTests(unittest.TestCase):
    def test_cord_topology_change_keeps_content_scope_and_external_equality_strict(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            cord = base / "cord"
            reference = base / "reference"
            init_repo(cord, "allowed.txt")
            init_repo(reference, "reference.txt")
            manifest = base / "repos.toml"
            manifest.write_text(
                "schema_version=1\n"
                f'[[repository]]\nname="cord"\nrole="cord"\npath="{cord}"\ncache_artifact_globs=[]\n'
                f'[[repository]]\nname="reference"\nrole="reference"\npath="{reference}"\ncache_artifact_globs=[]\n',
                encoding="utf-8",
            )
            before = base / "before.json"
            after = base / "after.json"
            assertion = base / "assertion.json"
            run("python3", str(SCRIPTS / "snapshot-repositories.py"), "--manifest", str(manifest), "--out", str(before))
            (cord / "allowed.txt").write_text("allowed change\n", encoding="utf-8")
            run("git", "add", "allowed.txt", cwd=cord)
            run("git", "commit", "-qm", "allowed", cwd=cord)
            commit = run("git", "rev-parse", "HEAD", cwd=cord).stdout.decode("ascii").strip()
            run("python3", str(SCRIPTS / "snapshot-repositories.py"), "--manifest", str(manifest), "--out", str(after))
            # Model a compose-style detached execution checkout without weakening
            # content comparison or the external repository baseline.
            value = json.loads(after.read_text())
            cord_row = next(row for row in value["repositories"] if row["role"] == "cord")
            cord_row.update({"path": "/isolated/cord", "branch": None, "detached": True, "index_tree": "topology-only"})
            unsigned = dict(value)
            unsigned.pop("snapshot_sha256", None)
            value["snapshot_sha256"] = hashlib.sha256(json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            after.write_text(json.dumps(value), encoding="utf-8")
            slices = base / "slices.toml"
            slices.write_text(
                'schema_version=1\n[[slice]]\nid="test"\nenabled=true\n'
                f'path_globs=["allowed.txt"]\ncommit_shas=["{commit}"]\n',
                encoding="utf-8",
            )
            run("python3", str(SCRIPTS / "validate-repository-boundary.py"), "--baseline", str(before), "--current", str(after), "--allow-cord", str(slices), "--out", str(assertion))
            report = json.loads(assertion.read_text())
            self.assertEqual(report["status"], "pass", report)
            self.assertEqual(report["external_content_deltas"], 0)

    def test_dirty_reference_content_change_is_detected_without_repo_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            cord = base / "cord"
            reference = base / "reference"
            init_repo(cord, "allowed.txt")
            init_repo(reference, "dirty.txt")
            (reference / "dirty.txt").write_text("existing dirt\n", encoding="utf-8")
            manifest = base / "repos.toml"
            manifest.write_text(
                "schema_version=1\n"
                f'[[repository]]\nname="cord"\nrole="cord"\npath="{cord}"\ncache_artifact_globs=[]\n'
                f'[[repository]]\nname="reference"\nrole="reference"\npath="{reference}"\ncache_artifact_globs=[]\n',
                encoding="utf-8",
            )
            slices = base / "slices.toml"
            slices.write_text(
                'schema_version=1\n[[slice]]\nid="test"\nenabled=true\n'
                'path_globs=["allowed.txt"]\ncommit_shas=[]\n',
                encoding="utf-8",
            )
            before = base / "before.json"
            after = base / "after.json"
            assertion = base / "assertion.json"
            status_before = run("git", "status", "--porcelain=v2", "-z", cwd=reference).stdout
            run("python3", str(SCRIPTS / "snapshot-repositories.py"), "--manifest", str(manifest), "--out", str(before))
            self.assertEqual(run("git", "status", "--porcelain=v2", "-z", cwd=reference).stdout, status_before)
            (cord / "allowed.txt").write_text("allowed change\n", encoding="utf-8")
            (reference / "dirty.txt").write_text("changed dirt bytes\n", encoding="utf-8")
            run("python3", str(SCRIPTS / "snapshot-repositories.py"), "--manifest", str(manifest), "--out", str(after))
            run(
                "python3", str(SCRIPTS / "validate-repository-boundary.py"),
                "--baseline", str(before), "--current", str(after),
                "--allow-cord", str(slices), "--out", str(assertion), expected=1,
            )
            report = json.loads(assertion.read_text())
            self.assertEqual(report["external_content_deltas"], 1)
            self.assertEqual(report["cord_undeclared_path_count"], 0)


class RegistryTests(unittest.TestCase):
    def test_shell_mediated_argv_is_not_decidable(self) -> None:
        module = load_script("validate-ac-decidability")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.json"
            source.write_text("{}", encoding="utf-8")
            import hashlib

            gate = {
                "id": "AC1",
                "decidability": "mechanical",
                "clauses": ["only"],
                "command": [{"argv": ["sh", "-c", "true"]}],
                "input": [{"path": "input.json", "sha256": hashlib.sha256(b"{}").hexdigest()}],
                "artifact": [{"path": "out.json", "sha256": "record", "schema": "v1"}],
                "assertion": [
                    {
                        "clause": "only",
                        "source": "out.json",
                        "json_pointer": "/ok",
                        "operator": "eq",
                        "expected": True,
                    }
                ],
            }
            self.assertIn("AC1: shell-mediated command is forbidden", module.validate_gate(gate, root))

    def test_registry_has_thirteen_mechanical_rows(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "decidability.json"
            run(
                "python3", "scripts/validate-ac-decidability.py",
                "--registry", "docs/specs/evidence-gates-v1.toml",
                "--schema", "docs/specs/evidence-report-v1.schema.json",
                "--minimum-mechanical", "13", "--out", str(report),
            )
            value = json.loads(report.read_text())
            self.assertEqual(value["status"], "pass")
            self.assertEqual(value["mechanical"], 13)
            self.assertEqual(value["blocked_rows"], ["AC11"])


if __name__ == "__main__":
    unittest.main()
