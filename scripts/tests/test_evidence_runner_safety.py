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

"""Hostile write/input/executable tests for the evidence runner."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from unittest import mock
from pathlib import Path


CORD = Path(__file__).resolve().parents[2]
TARGET = CORD / "target/evidence"
WRITER = "scripts/tests/fixtures/evidence-writer.py"


def init_repo(path: Path, filename: str) -> None:
    path.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=path, check=True)
    subprocess.run(["git", "config", "user.name", "Evidence Test"], cwd=path, check=True)
    subprocess.run(["git", "config", "user.email", "evidence@example.invalid"], cwd=path, check=True)
    (path / filename).write_text("baseline\n", encoding="utf-8")
    subprocess.run(["git", "add", filename], cwd=path, check=True)
    subprocess.run(["git", "commit", "-qm", "baseline"], cwd=path, check=True)


class EvidenceRunnerSafetyTests(unittest.TestCase):
    def setUp(self) -> None:
        TARGET.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.fake_cord = self.base / "cord"
        self.reference = self.base / "reference"
        init_repo(self.fake_cord, "allowed.txt")
        init_repo(self.reference, "reference.txt")
        self.manifest = TARGET / "hostile-repositories.toml"
        self.manifest.write_text(
            "schema_version=1\n"
            f'[[repository]]\nname="cord"\nrole="cord"\npath="{self.fake_cord}"\ncache_artifact_globs=[]\n'
            f'[[repository]]\nname="reference"\nrole="reference"\npath="{self.reference}"\ncache_artifact_globs=[]\n',
            encoding="utf-8",
        )
        self.registry = TARGET / "hostile-registry.toml"
        self.artifact = TARGET / "hostile-assertions.json"
        self.report = TARGET / "hostile-report.json"

    def tearDown(self) -> None:
        for path in (self.manifest, self.registry, self.artifact, self.report):
            path.unlink(missing_ok=True)
        self.temporary.cleanup()

    def write_registry(
        self,
        argv: list[str],
        *,
        input_hash: str | None = None,
        outputs: list[str] | None = None,
        command_inputs: list[str] | None = None,
    ) -> None:
        schema = CORD / "docs/specs/evidence-report-v1.schema.json"
        frozen = input_hash or hashlib.sha256(schema.read_bytes()).hexdigest()
        encoded_argv = ", ".join(json.dumps(item) for item in argv)
        outputs = outputs if outputs is not None else (
            ["target/evidence/hostile-assertions.json"] if "--out" in argv else []
        )
        command_inputs = command_inputs if command_inputs is not None else [WRITER]
        writer_hash = hashlib.sha256((CORD / WRITER).read_bytes()).hexdigest()
        self.registry.write_text(
            "schema_version=1\n"
            "[execution_policy]\n"
            f'repository_manifest="{self.manifest.relative_to(CORD)}"\n'
            'allowed_executables=["python3"]\n'
            f'allowed_python_scripts=["{WRITER}"]\n'
            'allowed_external_output_roots=["../.omx/evidence/origin-orbis-web3-storage"]\n'
            "[[gate]]\nid=\"AC14\"\noutput=\"target/evidence/hostile-report.json\"\n"
            "decidability=\"mechanical\"\nclauses=[\"hostile\"]\n"
            f"enforce_write_set=true\nwrite_paths={json.dumps(['allowed.txt', *outputs])}\n"
            "[[gate.input]]\npath=\"docs/specs/evidence-report-v1.schema.json\"\n"
            f'sha256="{frozen}"\n'
            f'[[gate.input]]\npath="{WRITER}"\nsha256="{writer_hash}"\n'
            "[[gate.command]]\ncwd=\".\"\n"
            f"argv=[{encoded_argv}]\ntest_count=0\n"
            f"outputs={json.dumps(outputs)}\n"
            f"inputs={json.dumps(command_inputs)}\nallowed_writes=[]\n"
            "[[gate.artifact]]\npath=\"target/evidence/hostile-assertions.json\"\n"
            "sha256=\"record\"\nschema=\"fixture-v1\"\n"
            "[[gate.assertion]]\nclause=\"hostile\"\n"
            "source=\"target/evidence/hostile-assertions.json\"\njson_pointer=\"/ok\"\n"
            "operator=\"eq\"\nexpected=true\n",
            encoding="utf-8",
        )

    def execute(self) -> dict:
        environment = dict(os.environ)
        environment.pop("CORD_EVIDENCE_TRACE_FILE", None)
        environment.pop("CORD_EVIDENCE_TRACE_ROOTS", None)
        environment["PYTHONPATH"] = os.pathsep.join(
            value for value in environment.get("PYTHONPATH", "").split(os.pathsep)
            if "evidence-read-tracer" not in value
        )
        result = subprocess.run(
            [
                "python3", "scripts/run-evidence-gate.py", "--registry", str(self.registry),
                "--gate", "AC14", "--schema", "docs/specs/evidence-report-v1.schema.json",
                "--out", "target/evidence/hostile-report.json",
            ], cwd=CORD, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=environment,
        )
        self.assertEqual(result.returncode, 1, result.stderr.decode("utf-8", "replace"))
        return json.loads(self.report.read_text())

    def test_unregistered_input_drift_fails_before_execution(self) -> None:
        self.write_registry(
            ["python3", WRITER, "--out", "target/evidence/hostile-assertions.json"],
            input_hash="0" * 64,
        )
        report = self.execute()
        self.assertTrue(any("stale input hash" in blocker for blocker in report["blockers"]))
        self.assertEqual(report["commands"], [])

    def test_undeclared_cord_output_is_detected(self) -> None:
        extra = self.fake_cord / "undeclared.txt"
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
            "--extra", str(extra),
        ])
        report = self.execute()
        self.assertEqual(report["write_set"]["undeclared_paths"], ["undeclared.txt"])

    def test_external_repository_write_is_detected(self) -> None:
        extra = self.reference / "external.txt"
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
            "--extra", str(extra),
        ])
        report = self.execute()
        self.assertEqual(report["write_set"]["external_deltas"], ["reference"])

    def test_unapproved_executable_is_rejected(self) -> None:
        self.write_registry(["/usr/bin/touch", str(self.fake_cord / "bad.txt")])
        report = self.execute()
        self.assertTrue(any("executable is not approved" in blocker for blocker in report["blockers"]))
        self.assertFalse((self.fake_cord / "bad.txt").exists())

    def test_undeclared_read_is_rejected_before_execution(self) -> None:
        read_path = TARGET / "hostile-read.txt"
        read_path.write_text("authority\n", encoding="utf-8")
        self.addCleanup(read_path.unlink, missing_ok=True)
        self.write_registry([
            "python3", WRITER, "--read", str(read_path.relative_to(CORD)),
            "--out", "target/evidence/hostile-assertions.json",
        ])
        report = self.execute()
        self.assertTrue(any("undeclared read" in blocker for blocker in report["blockers"]))
        self.assertEqual(report["commands"], [])

    def hidden_read_report(self, flag: str) -> dict:
        read_path = TARGET / "hostile-hidden-read.txt"
        read_path.write_text("hidden authority\n", encoding="utf-8")
        self.addCleanup(read_path.unlink, missing_ok=True)
        self.write_registry([
            "python3", WRITER, flag, "--out", "target/evidence/hostile-assertions.json",
        ])
        with mock.patch.dict(os.environ, {"CORD_HOSTILE_READ": str(read_path)}):
            return self.execute()

    def test_hard_coded_read_is_observed_and_rejected(self) -> None:
        report = self.hidden_read_report("--hardcoded-env-read")
        command = report["commands"][0]
        self.assertFalse(command["observed_read_manifest"]["closure_proven"])
        self.assertTrue(any("observed undeclared read" in blocker for blocker in report["blockers"]))

    def test_imported_transitive_read_is_observed_and_rejected(self) -> None:
        report = self.hidden_read_report("--transitive-read")
        manifest = report["commands"][0]["observed_read_manifest"]
        self.assertTrue(any("hostile-hidden-read.txt" in path for path in manifest["undeclared_paths"]))

    def test_child_process_read_and_launch_ancestry_are_observed(self) -> None:
        report = self.hidden_read_report("--child-read")
        manifest = report["commands"][0]["observed_read_manifest"]
        self.assertTrue(any("hostile-hidden-read.txt" in path for path in manifest["undeclared_paths"]))
        self.assertTrue(manifest["subprocess_ancestry"])

    def test_undeclared_generated_ancestor_is_rejected(self) -> None:
        generated = TARGET / "hostile-generated.json"
        generated.write_text('{"value":true}', encoding="utf-8")
        self.addCleanup(generated.unlink, missing_ok=True)
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
        ])
        with self.registry.open("a", encoding="utf-8") as output:
            output.write(
                "[[gate.input]]\npath=\"target/evidence/hostile-generated.json\"\n"
                "sha256=\"record\"\ninput_class=\"generated\"\n"
                "producer_gate=\"missing-producer\"\n"
                "producer_output=\"target/evidence/hostile-generated.json\"\n"
            )
        report = self.execute()
        self.assertTrue(any("no exact producer gate" in blocker for blocker in report["blockers"]))
        self.assertEqual(report["commands"], [])

    def test_sibling_target_write_is_rejected(self) -> None:
        sibling = TARGET / "hostile-sibling.json"
        self.addCleanup(sibling.unlink, missing_ok=True)
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
            "--extra", str(sibling.relative_to(CORD)),
        ])
        report = self.execute()
        self.assertTrue(any("undeclared output" in blocker for blocker in report["blockers"]))

    def test_unexpected_file_in_output_directory_is_rejected(self) -> None:
        output = "target/evidence/hostile-dir/result.json"
        sibling = "target/evidence/hostile-dir/unexpected.json"
        self.addCleanup(lambda: __import__("shutil").rmtree(TARGET / "hostile-dir", ignore_errors=True))
        self.write_registry(
            ["python3", WRITER, "--out", output, "--extra", sibling],
            outputs=[output],
        )
        report = self.execute()
        self.assertTrue(any("undeclared output" in blocker for blocker in report["blockers"]))

    def test_stale_preexisting_output_cannot_satisfy_command(self) -> None:
        self.artifact.write_text('{"ok":true}', encoding="utf-8")
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
            "--skip-output",
        ])
        report = self.execute()
        self.assertTrue(any("did not materialize" in blocker for blocker in report["blockers"]))

    def test_missing_output_fails(self) -> None:
        self.write_registry([
            "python3", WRITER, "--out", "target/evidence/hostile-assertions.json",
            "--skip-output",
        ])
        report = self.execute()
        self.assertTrue(any("did not materialize" in blocker for blocker in report["blockers"]))


if __name__ == "__main__":
    unittest.main()
