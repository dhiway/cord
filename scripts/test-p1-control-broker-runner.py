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

"""Regression checks for the live P1 runner's binary-supported genesis derivation."""

import hashlib
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).parents[1]
SCRIPT = ROOT / "zombienet" / "p1-control-broker" / "run.py"
SPEC = importlib.util.spec_from_file_location("p1_control_broker_runner", SCRIPT)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class GenesisDerivationTests(unittest.TestCase):
    def test_origin_workers_must_be_regular_executable_adjacent_files(self):
        with tempfile.TemporaryDirectory() as directory:
            bundle = pathlib.Path(directory)
            origin = bundle / "origin"
            origin.write_bytes(b"origin")
            origin.chmod(0o755)

            with self.assertRaisesRegex(RuntimeError, "missing adjacent Origin PVF worker"):
                RUNNER.origin_worker_binaries(origin)

            prepare = bundle / "origin-prepare-worker"
            execute = bundle / "origin-execute-worker"
            prepare.write_bytes(b"prepare")
            execute.write_bytes(b"execute")
            prepare.chmod(0o755)
            execute.chmod(0o644)
            with self.assertRaisesRegex(RuntimeError, "not executable"):
                RUNNER.origin_worker_binaries(origin)

            execute.chmod(0o755)
            workers = RUNNER.origin_worker_binaries(origin)
            self.assertEqual(set(workers), set(RUNNER.ORIGIN_WORKER_NAMES))
            self.assertEqual(RUNNER.sha256(workers["origin-prepare-worker"]),
                             hashlib.sha256(b"prepare").hexdigest())
            self.assertEqual(RUNNER.sha256(workers["origin-execute-worker"]),
                             hashlib.sha256(b"execute").hexdigest())

    def test_origin_workers_reject_symlinks_outside_the_sealed_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            bundle = root / "bundle"
            outside = root / "outside"
            bundle.mkdir()
            outside.mkdir()
            origin = bundle / "origin"
            origin.write_bytes(b"origin")
            origin.chmod(0o755)
            for name in RUNNER.ORIGIN_WORKER_NAMES:
                target = outside / name
                target.write_bytes(name.encode())
                target.chmod(0o755)
                (bundle / name).symlink_to(target)

            with self.assertRaisesRegex(RuntimeError, "regular adjacent file"):
                RUNNER.origin_worker_binaries(origin)

    def test_orbis_wrapper_pins_only_embedded_relay_launches(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            captured = root / "captured"
            binary = root / "origin-omni-node"
            binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$CAPTURED"\n')
            binary.chmod(0o755)
            wrapper = RUNNER.write_orbis_command_wrapper(root / "wrapper", binary)

            env = {"CAPTURED": str(captured)}
            subprocess.run([str(wrapper), "build-spec", "--chain", "orbis-local"],
                           check=True, env=env)
            self.assertEqual(captured.read_text().splitlines(),
                             ["build-spec", "--chain", "orbis-local"])

            subprocess.run([
                str(wrapper), "--name", "orbis-alice", "--collator", "--",
                "--chain", "relay.json",
            ], check=True, env=env)
            self.assertEqual(captured.read_text().splitlines()[-3:], [
                "--node-key", RUNNER.INTERNAL_RELAY_NODE_KEYS[0], "--no-mdns",
            ])

            hostile = subprocess.run([
                str(wrapper), "--name", "orbis-bob", "--collator", "--",
                "--node-key", "11" * 32,
            ], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertNotEqual(hostile.returncode, 0)
            self.assertIn(b"pre-existing", hostile.stderr)

    def test_topology_does_not_repeat_any_zombienet_owned_option(self):
        template = ROOT / "zombienet" / "p1-control-broker" / "topology.toml.in"
        RUNNER.validate_template(template)
        text = "\n".join(
            line for line in template.read_text().splitlines() if line.startswith("args = ")
        )
        for forbidden in RUNNER.PROVIDER_OWNED_ARGS:
            self.assertNotIn(forbidden, text)

    def test_topology_pins_the_exact_internal_relay_identity_ledger(self):
        template = ROOT / "zombienet" / "p1-control-broker" / "topology.toml.in"
        lines = [
            line.removeprefix("relay_chain_args = ")
            for line in template.read_text().splitlines()
            if line.startswith("relay_chain_args = ")
        ]
        self.assertEqual([json.loads(line) for line in lines], RUNNER.INTERNAL_RELAY_ARGS)
        self.assertEqual(len(RUNNER.PROVIDER_NODE_KEYS), 8)
        self.assertEqual(len(RUNNER.INTERNAL_RELAY_NODE_KEYS), 2)
        self.assertEqual(RUNNER.NODE_KEYS,
                         RUNNER.PROVIDER_NODE_KEYS + RUNNER.INTERNAL_RELAY_NODE_KEYS)

    def test_template_validator_rejects_internal_relay_identity_drift(self):
        original = (ROOT / "zombienet" / "p1-control-broker" / "topology.toml.in").read_text()
        mutations = {
            "wrong fixed key": original.replace("09" * 32, "11" * 32, 1),
            "missing no-mdns": original.replace(', "--no-mdns"]', "]", 1),
            "provider port injection": original.replace(
                ', "--no-mdns"]', ', "--rpc-port=19999", "--no-mdns"]', 1
            ),
        }
        for name, text in mutations.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                candidate = pathlib.Path(directory) / "topology.toml.in"
                candidate.write_text(text)
                with self.assertRaisesRegex(RuntimeError, "internal relay identity ledger"):
                    RUNNER.validate_template(candidate)

    def test_template_validator_rejects_each_zombienet_owned_option(self):
        original = (ROOT / "zombienet" / "p1-control-broker" / "topology.toml.in").read_text()
        for forbidden in RUNNER.PROVIDER_OWNED_ARGS:
            with self.subTest(forbidden=forbidden), tempfile.TemporaryDirectory() as directory:
                candidate = pathlib.Path(directory) / "topology.toml.in"
                candidate.write_text(original.replace(
                    'args = ["--alice",', f'args = ["{forbidden}", "--alice",', 1
                ))
                with self.assertRaisesRegex(RuntimeError, "Zombienet-owned"):
                    RUNNER.validate_template(candidate)

    def test_topology_stages_only_spec_basenames_from_the_spec_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            specs = root / "deep" / "evidence" / "specs"
            specs.mkdir(parents=True)
            relay = specs / "relay.raw.json"
            orbis = specs / "orbis.raw.json"
            relay.write_text("{}")
            orbis.write_text("{}")
            template = root / "topology.toml.in"
            template.write_text(
                'relay = "__RELAY_SPEC__"\norbis = "__ORBIS_SPEC__"\n'
                'origin = "__ORIGIN_BINARY__"\nbinary = "__ORBIS_BINARY__"\n'
            )
            rendered = RUNNER.render_topology(
                template, relay, orbis, specs, "/sealed/origin", "/sealed/origin-omni-node"
            )

            self.assertIn('relay = "relay.raw.json"', rendered)
            self.assertIn('orbis = "orbis.raw.json"', rendered)
            self.assertNotIn(str(specs), rendered)
            self.assertEqual(RUNNER.topology_spec_locator(relay, specs), relay.name)

    def test_topology_rejects_spec_outside_staged_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            specs = root / "specs"
            specs.mkdir()
            outside = root / "relay.raw.json"
            outside.write_text("{}")
            with self.assertRaisesRegex(RuntimeError, "direct child"):
                RUNNER.topology_spec_locator(outside, specs)

    def test_relay_uses_export_state_then_chain_info_on_one_ephemeral_database(self):
        genesis = "0x" + "12" * 32
        record = {
            "best_hash": genesis,
            "best_number": 0,
            "genesis_hash": genesis,
            "finalized_hash": genesis,
            "finalized_number": 0,
        }
        commands = []

        def fake_run(command, **kwargs):
            commands.append(command)
            if command[1] == "export-state":
                self.assertIs(kwargs["stdout"], subprocess.DEVNULL)
                return subprocess.CompletedProcess(command, 0, stdout=None, stderr=None)
            self.assertEqual(command[1], "chain-info")
            return subprocess.CompletedProcess(
                command, 0, stdout=json.dumps(record).encode(), stderr=None
            )

        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "relay.log"
            with mock.patch.object(RUNNER.subprocess, "run", side_effect=fake_run):
                observed = RUNNER.relay_genesis_hash("origin", "relay.json", log)

        self.assertEqual(observed, genesis)
        self.assertEqual([command[1] for command in commands], ["export-state", "chain-info"])
        self.assertEqual(commands[0][commands[0].index("--base-path") + 1],
                         commands[1][commands[1].index("--base-path") + 1])
        self.assertNotIn("export-genesis-" + "state", {part for command in commands for part in command})

    def test_relay_rejects_uninitialized_chain_info(self):
        zero = "0x" + "00" * 32
        record = {
            "best_hash": zero,
            "best_number": 0,
            "genesis_hash": zero,
            "finalized_hash": zero,
            "finalized_number": 0,
        }

        def fake_run(command, **_kwargs):
            stdout = json.dumps(record).encode() if command[1] == "chain-info" else None
            return subprocess.CompletedProcess(command, 0, stdout=stdout, stderr=None)

        with tempfile.TemporaryDirectory() as directory:
            with mock.patch.object(RUNNER.subprocess, "run", side_effect=fake_run):
                with self.assertRaisesRegex(RuntimeError, "uninitialized genesis hash"):
                    RUNNER.relay_genesis_hash(
                        "origin", "relay.json", pathlib.Path(directory) / "relay.log"
                    )

    def test_orbis_uses_export_genesis_head_and_hashes_only_stdout(self):
        header = bytes(range(101))
        command_record = []

        def fake_run(command, **_kwargs):
            command_record.append(command)
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=("0x" + header.hex()).encode(),
                stderr=b"informational log containing 0xdeadbeef",
            )

        with tempfile.TemporaryDirectory() as directory:
            log = pathlib.Path(directory) / "orbis.log"
            with mock.patch.object(RUNNER.subprocess, "run", side_effect=fake_run):
                observed = RUNNER.parachain_genesis_hash("origin-omni-node", "orbis.json", log)

        expected = "0x" + hashlib.blake2b(header, digest_size=32).hexdigest()
        self.assertEqual(observed, expected)
        self.assertEqual(command_record[0][1], "export-genesis-head")

    def test_runner_source_never_references_unsupported_genesis_state_command(self):
        self.assertNotIn("export-genesis-" + "state", SCRIPT.read_text())

    def test_runner_hashes_both_origin_pvf_workers_into_inputs(self):
        source = SCRIPT.read_text()
        self.assertIn('"origin_prepare_worker_sha256"', source)
        self.assertIn('"origin_execute_worker_sha256"', source)
        self.assertIn('"orbis_command_wrapper_sha256"', source)
        self.assertIn('"launch_command_ledger_sha256"', source)


if __name__ == "__main__":
    unittest.main()
