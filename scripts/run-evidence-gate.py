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

"""Execute one evidence gate from literal argv and emit a hashed report."""

from __future__ import annotations

import argparse
import fnmatch
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

from evidence_common import (
    atomic_write_json,
    canonical_bytes,
    json_pointer,
    report_hash,
    sha256_bytes,
    sha256_file,
)


OPERATORS = {"eq", "zero", "gte", "lte", "subset", "sha256_eq", "matches_schema"}
SHELL_PROGRAMS = {"sh", "bash", "zsh", "fish", "dash", "cmd", "powershell", "pwsh"}
WORKSPACE_PRUNE = {".git", "node_modules", "__pycache__"}


def load_script_module(name: str, path: Path):
    specification = importlib.util.spec_from_file_location(name, path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load evidence control module: {path}")
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def capture_repositories(root: Path, manifest_path: str) -> list[dict[str, Any]]:
    snapshot_module = load_script_module(
        "cord_snapshot_repositories", root / "scripts/snapshot-repositories.py"
    )
    manifest = tomllib.loads((root / manifest_path).read_text(encoding="utf-8"))
    return [snapshot_module.snapshot_repo(entry) for entry in manifest.get("repository", [])]


def repository_write_set(
    root: Path,
    before: list[dict[str, Any]],
    after: list[dict[str, Any]],
    allowed_paths: list[str],
) -> dict[str, Any]:
    boundary_module = load_script_module(
        "cord_validate_repository_boundary", root / "scripts/validate-repository-boundary.py"
    )
    old = {row["name"]: row for row in before}
    new = {row["name"]: row for row in after}
    external_deltas = []
    cord_changed: set[str] = set()
    violations = []
    if old.keys() != new.keys():
        violations.append("repository set changed during gate")
    for name in sorted(old.keys() & new.keys()):
        if old[name].get("role") == "cord":
            cord_changed.update(boundary_module.changed_paths(old[name], new[name]))
            if old[name].get("head") != new[name].get("head"):
                violations.append("gate command changed CORD HEAD")
        elif old[name] != new[name]:
            external_deltas.append(name)
    undeclared = sorted(
        path for path in cord_changed
        if not any(fnmatch.fnmatchcase(path, pattern) for pattern in allowed_paths)
    )
    if external_deltas:
        violations.append("gate command changed an external repository")
    if undeclared:
        violations.append("gate command wrote undeclared CORD paths")
    return {
        "before_sha256": sha256_bytes(canonical_bytes(before)),
        "after_sha256": sha256_bytes(canonical_bytes(after)),
        "external_deltas": external_deltas,
        "cord_changed_paths": sorted(cord_changed),
        "undeclared_paths": undeclared,
        "allowed_paths": allowed_paths,
        "violations": violations,
    }


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def command_output(argv: list[str]) -> str:
    result = subprocess.run(argv, check=False, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.stdout.decode("utf-8", "replace").strip()


def hash_path(path: Path) -> str:
    if path.is_file():
        return sha256_file(path)
    if not path.is_dir():
        raise FileNotFoundError(path)
    records = []
    result = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=path,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode == 0:
        for encoded in result.stdout.split(b"\0"):
            if not encoded:
                continue
            relative = encoded.decode("utf-8", "strict")
            absolute = path / relative
            if absolute.is_file():
                records.append({"path": relative, "sha256": sha256_file(absolute)})
        if records:
            records.sort(key=lambda record: record["path"])
            return sha256_bytes(canonical_bytes(records))
    for current, directories, files in os.walk(path):
        directories[:] = sorted(
            name for name in directories
            if name not in WORKSPACE_PRUNE and not (
                Path(current).resolve() == path.resolve() and name in {"target"}
            )
        )
        for name in sorted(files):
            absolute = Path(current) / name
            relative = absolute.relative_to(path).as_posix()
            records.append({"path": relative, "sha256": sha256_file(absolute)})
    records.sort(key=lambda record: record["path"])
    return sha256_bytes(canonical_bytes(records))


def canonical_release_hash(root: Path, declared: dict[str, Any]) -> str:
    """Validate a release member against its adjacent canonical SHA256SUMS.

    Canonical release outputs are intentionally generated outside the Git tree.
    Their immutable commitment is SHA256SUMS, while the consumer also verifies
    semantic provenance (source, lockfile, pinned srtool and two clean runs).
    """
    path = root / declared["path"]
    attestation = declared.get("attestation")
    if not isinstance(attestation, str):
        raise ValueError("canonical release input has no SHA256SUMS attestation")
    checksum_path = root / attestation
    entries: dict[str, str] = {}
    for line in checksum_path.read_text(encoding="utf-8").splitlines():
        fields = line.split(maxsplit=1)
        if len(fields) == 2:
            entries[fields[1].lstrip(" *")] = fields[0]
    actual = sha256_file(path)
    if entries.get(path.name) != actual:
        raise ValueError(f"canonical release attestation does not bind {declared['path']}")
    return actual


def workspace_state(
    root: Path, external_roots: list[str], output_paths: list[Path]
) -> dict[str, list[int | str]]:
    state: dict[str, list[int | str]] = {}
    tracked = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    for encoded in tracked.split(b"\0"):
        if not encoded:
            continue
        relative = encoded.decode("utf-8", "strict")
        if relative.startswith(("target/debug/", "target/release/", "docs/evidence/performance/")):
            continue
        path = root / relative
        if path.is_file():
            stat = path.stat()
            state[str(path.resolve())] = [stat.st_size, stat.st_mtime_ns]

    monitor_roots = {path.parent for path in output_paths if root in path.parents}
    for base in sorted(monitor_roots):
        if base == root / "target":
            candidates = [path for path in base.iterdir() if path.is_file()] if base.is_dir() else []
            for path in candidates:
                stat = path.stat()
                state[str(path.resolve())] = [stat.st_size, stat.st_mtime_ns]
            continue
        if not base.exists():
            continue
        for current, directories, files in os.walk(base):
            directories[:] = sorted(name for name in directories if name not in WORKSPACE_PRUNE)
            for name in sorted(files):
                path = Path(current) / name
                stat = path.stat()
                state[str(path.resolve())] = [stat.st_size, stat.st_mtime_ns]

    for base in [Path(path).resolve() for path in external_roots]:
        if not base.exists():
            continue
        for current, directories, files in os.walk(base):
            current_path = Path(current)
            relative_parts = current_path.relative_to(base).parts
            directories[:] = sorted(
                name for name in directories
                if name not in WORKSPACE_PRUNE
            )
            for name in sorted(files):
                path = current_path / name
                try:
                    stat = path.stat()
                except FileNotFoundError:
                    continue
                state[str(path.resolve())] = [stat.st_size, stat.st_mtime_ns]
    return state


def remove_output(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink()
    elif path.is_dir():
        shutil.rmtree(path)


def changed_files(before: dict[str, Any], after: dict[str, Any]) -> list[str]:
    return sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))


def repository_snapshot_receipt(repositories: list[dict[str, Any]]) -> dict[str, Any]:
    rows = []
    for repository in repositories:
        row = {
            "name": repository.get("name"),
            "role": repository.get("role"),
            "path": repository.get("path"),
            "head": repository.get("head"),
            "tracked_tree_root": repository.get("tracked_tree_root"),
            "untracked_tree_root": repository.get("untracked_tree_root"),
            "worktree_diff_sha256": repository.get("worktree_diff_sha256"),
            "cached_diff_sha256": repository.get("cached_diff_sha256"),
            "index_sha256": repository.get("index_sha256"),
        }
        row["receipt_sha256"] = sha256_bytes(canonical_bytes(row))
        rows.append(row)
    rows.sort(key=lambda row: str(row["name"]))
    return {
        "repositories": rows,
        "sha256": sha256_bytes(canonical_bytes(rows)),
    }


def read_trace(path: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    reads: dict[tuple[str, str], dict[str, Any]] = {}
    subprocesses = []
    if not path.is_file():
        return [], []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("event") == "read" and isinstance(row.get("path"), str):
            observed_path = Path(row["path"])
            # Python raises its audit event before attempting open(2).  Failed
            # probes, directory descriptors, and already-removed atomic output
            # temporaries are not content reads and cannot affect the command.
            if not observed_path.is_file():
                continue
            key = (row["path"], str(row.get("provenance", "unknown")))
            reads[key] = {
                "path": row["path"],
                "provenance": key[1],
                "sha256": sha256_file(observed_path),
            }
        elif row.get("event") == "subprocess":
            subprocesses.append({
                "pid": row.get("pid"),
                "ppid": row.get("ppid"),
                "executable": row.get("executable"),
                "argv": row.get("argv", []),
            })
    return sorted(reads.values(), key=lambda row: (row["path"], row["provenance"])), subprocesses


def declared_read_path(
    path: Path,
    root: Path,
    read_inputs: list[str],
    produced_outputs: dict[str, dict[str, Any]],
    declared_outputs: list[str],
    registry: Path,
    schema: Path,
    tracer: Path,
    allowed_patterns: list[str],
    snapshot_bound_files: set[Path],
) -> bool:
    allowed = [tracer.resolve()]
    for value in read_inputs:
        if value == "@registry":
            allowed.append(registry.resolve())
        elif value == "@schema":
            allowed.append(schema.resolve())
        else:
            allowed.append((root / value).resolve())
    allowed.extend((root / value).resolve() for value in produced_outputs)
    allowed.extend((root / value).resolve() for value in declared_outputs)
    absolute = path.resolve()
    if any(candidate in (absolute, *absolute.parents) for candidate in allowed):
        return True
    for value in declared_outputs:
        output = (root / value).resolve()
        # Atomic writers commonly create an O_RDWR temporary sibling before
        # replacing the declared output.  The audit hook conservatively reports
        # that descriptor as a read, so close only the exact tempfile shape and
        # its parent-directory probe to the declared output.
        if absolute == output.parent or (
            absolute.parent == output.parent
            and absolute.name.startswith(f".{output.name}.")
            and absolute.name.endswith(".tmp")
        ):
            return True
    # Commands which intentionally walk every repository must opt into the
    # complete pre-command snapshot.  Membership is exact (tracked/untracked
    # files only) and the aggregate content root is included in the report.
    # Without this token, hidden CORD and reference reads remain undeclared.
    if "@repository_snapshot" in read_inputs and absolute in snapshot_bound_files:
        return True
    try:
        relative = absolute.relative_to(root).as_posix()
    except ValueError:
        return False
    return any(fnmatch.fnmatchcase(relative, pattern) for pattern in allowed_patterns)


def schema_errors(
    value: Any,
    schema: dict[str, Any],
    pointer: str = "",
    root_schema: dict[str, Any] | None = None,
) -> list[str]:
    """Validate the JSON Schema subset used by EvidenceReportV1."""

    root_schema = root_schema or schema
    reference = schema.get("$ref")
    if isinstance(reference, str) and reference.startswith("#/"):
        target: Any = root_schema
        for token in reference[2:].split("/"):
            target = target[token.replace("~1", "/").replace("~0", "~")]
        return schema_errors(value, target, pointer, root_schema)
    errors: list[str] = []
    if "const" in schema and value != schema["const"]:
        errors.append(f"{pointer}: expected const {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{pointer}: not in enum")
    expected_type = schema.get("type")
    types = expected_type if isinstance(expected_type, list) else [expected_type] if expected_type else []
    type_ok = not types or any(
        (kind == "object" and isinstance(value, dict))
        or (kind == "array" and isinstance(value, list))
        or (kind == "string" and isinstance(value, str))
        or (kind == "integer" and isinstance(value, int) and not isinstance(value, bool))
        or (kind == "boolean" and isinstance(value, bool))
        or (kind == "null" and value is None)
        for kind in types
    )
    if not type_ok:
        return [f"{pointer}: wrong type, expected {types}"]
    if isinstance(value, dict):
        required = schema.get("required", [])
        errors.extend(f"{pointer}/{key}: required" for key in required if key not in value)
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties")
        if additional is False:
            errors.extend(f"{pointer}/{key}: additional property" for key in value if key not in properties)
        elif isinstance(additional, dict):
            for key in value:
                if key not in properties:
                    errors.extend(schema_errors(value[key], additional, f"{pointer}/{key}", root_schema))
        for key, child in properties.items():
            if key in value:
                errors.extend(schema_errors(value[key], child, f"{pointer}/{key}", root_schema))
    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{pointer}: too few items")
        if "maxItems" in schema and len(value) > schema["maxItems"]:
            errors.append(f"{pointer}: too many items")
        if schema.get("uniqueItems") and len({json.dumps(item, sort_keys=True) for item in value}) != len(value):
            errors.append(f"{pointer}: items are not unique")
        if "items" in schema:
            for index, item in enumerate(value):
                errors.extend(schema_errors(item, schema["items"], f"{pointer}/{index}", root_schema))
    if isinstance(value, str):
        if "pattern" in schema and re.fullmatch(schema["pattern"], value) is None:
            errors.append(f"{pointer}: pattern mismatch")
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{pointer}: too short")
    if isinstance(value, int) and "minimum" in schema and value < schema["minimum"]:
        errors.append(f"{pointer}: below minimum")
    return errors


def validate_assertion(specification: dict[str, Any], root: Path) -> dict[str, Any]:
    source = root / specification["source"]
    operator = specification["operator"]
    expected = specification.get("expected")
    actual: Any = None
    passed = False
    error = None
    try:
        document = json.loads(source.read_text(encoding="utf-8"))
        actual = json_pointer(document, specification["json_pointer"])
        if operator == "eq":
            passed = actual == expected
        elif operator == "zero":
            passed = actual == 0
        elif operator == "gte":
            passed = actual >= expected
        elif operator == "lte":
            passed = actual <= expected
        elif operator == "subset":
            passed = set(actual).issubset(set(expected))
        elif operator == "sha256_eq":
            passed = actual == expected and isinstance(actual, str) and re.fullmatch(r"[0-9a-f]{64}", actual) is not None
        elif operator == "matches_schema":
            schema_path = root / str(expected)
            schema = json.loads(schema_path.read_text(encoding="utf-8"))
            failures = schema_errors(actual, schema)
            passed = not failures
            actual = {"valid": passed, "schema_sha256": sha256_file(schema_path)}
            expected = True
        else:
            raise ValueError(f"unsupported operator: {operator}")
    except (OSError, KeyError, ValueError, TypeError, json.JSONDecodeError) as exception:
        error = str(exception)
    result = {
        "json_pointer": specification["json_pointer"],
        "operator": operator,
        "expected": expected,
        "actual": actual,
        "pass": passed,
    }
    if error:
        result["error"] = error
    return result


def load_gate(registry: dict[str, Any], gate_id: str) -> dict[str, Any]:
    matches = [gate for gate in registry.get("gate", []) if gate.get("id") == gate_id]
    if len(matches) != 1:
        raise ValueError(f"gate {gate_id!r} must occur exactly once")
    return matches[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--registry", required=True, type=Path)
    parser.add_argument("--gate", required=True)
    parser.add_argument("--schema", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    root = Path.cwd().resolve()
    registry = tomllib.loads(args.registry.read_text(encoding="utf-8"))
    gate = load_gate(registry, args.gate)
    policy = registry.get("execution_policy", {})
    if os.path.normpath(gate["output"]) != os.path.normpath(str(args.out)):
        raise SystemExit(f"output must exactly match registry: {gate['output']}")

    started = utc_now()
    blockers: list[str] = []
    before_repositories: list[dict[str, Any]] = []
    if gate.get("enforce_write_set", False):
        try:
            before_repositories = capture_repositories(root, policy["repository_manifest"])
        except (OSError, KeyError, ValueError, RuntimeError) as exception:
            blockers.append(f"cannot capture before write-set: {exception}")
    repository_snapshot = repository_snapshot_receipt(before_repositories)
    snapshot_bound_files = {
        (Path(repository["path"]) / file_record["path"]).resolve()
        for repository in before_repositories
        for collection in ("tracked_files", "untracked_files")
        for file_record in repository.get(collection, [])
        if isinstance(file_record, dict) and isinstance(file_record.get("path"), str)
    }
    tracer_path = root / "scripts/evidence-read-tracer/sitecustomize.py"
    input_hashes: dict[str, str] = {
        "@registry": sha256_file(args.registry),
        "@schema": sha256_file(args.schema),
        "@runner": sha256_file(Path(__file__)),
        "@repository_manifest": sha256_file(root / policy["repository_manifest"]),
        "@read_tracer": sha256_file(tracer_path),
        "@repository_snapshot": repository_snapshot["sha256"],
    }
    input_specs = {declared["path"]: declared for declared in gate.get("input", [])}
    input_ancestry: dict[str, dict[str, Any]] = {}
    for declared in gate.get("input", []):
        path = root / declared["path"]
        try:
            actual_hash = hash_path(path)
            input_hashes[declared["path"]] = actual_hash
            input_class = declared.get("input_class")
            if input_class == "canonical_release":
                actual_hash = canonical_release_hash(root, declared)
                input_hashes[declared["path"]] = actual_hash
                input_ancestry[declared["path"]] = {
                    # EvidenceReportV1 records all input ancestry with this
                    # common shape. Here SHA256SUMS is the external producer
                    # receipt rather than a prior in-repository gate report.
                    "producer_gate": "CANONICAL_RELEASE",
                    "producer_output": declared["path"],
                    "producer_report": declared["attestation"],
                    "producer_report_sha256": sha256_file(root / declared["attestation"]),
                    "artifact_sha256": actual_hash,
                }
            elif declared["sha256"] == "record" and input_class != "generated":
                blockers.append(f"unfrozen input is not an approved generated input: {declared['path']}")
            elif input_class == "generated":
                producer_id = declared.get("producer_gate")
                producer_output = declared.get("producer_output")
                producer_gates = [row for row in registry.get("gate", []) if row.get("id") == producer_id]
                if len(producer_gates) != 1:
                    blockers.append(f"generated input has no exact producer gate: {declared['path']}")
                else:
                    producer_report_path = root / producer_gates[0]["output"]
                    try:
                        producer_report = json.loads(producer_report_path.read_text(encoding="utf-8"))
                        artifact = next(
                            row for row in producer_report.get("artifacts", [])
                            if row.get("path") == producer_output == declared["path"]
                        )
                        receipt_valid = (
                            producer_report.get("gate_id") == producer_id
                            and producer_report.get("status") == "pass"
                            and producer_report.get("report_sha256") == report_hash(producer_report)
                            and artifact.get("sha256") == actual_hash
                            and not schema_errors(
                                producer_report,
                                json.loads(args.schema.read_text(encoding="utf-8")),
                            )
                        )
                        if not receipt_valid:
                            raise ValueError("producer report/artifact hash receipt is invalid")
                        input_ancestry[declared["path"]] = {
                            "producer_gate": producer_id,
                            "producer_output": producer_output,
                            "producer_report": producer_gates[0]["output"],
                            "producer_report_sha256": producer_report["report_sha256"],
                            "artifact_sha256": artifact["sha256"],
                        }
                    except (OSError, ValueError, StopIteration, KeyError, json.JSONDecodeError) as exception:
                        blockers.append(f"generated input ancestry is unverified: {declared['path']}: {exception}")
            elif declared["sha256"] != "record" and actual_hash != declared["sha256"]:
                blockers.append(f"stale input hash: {declared['path']}")
        except OSError as exception:
            blockers.append(f"missing input {declared['path']}: {exception}")

    for declared in gate.get("artifact", []):
        artifact_path = (root / declared["path"]).resolve()
        if root not in (artifact_path, *artifact_path.parents):
            blockers.append(f"artifact path escapes CORD root: {declared['path']}")
        elif artifact_path.exists():
            artifact_path.unlink()

    commands = []
    produced_outputs: dict[str, dict[str, Any]] = {}
    if not blockers:
        for command in gate.get("command", []):
            argv = command.get("argv")
            cwd = command.get("cwd", ".")
            if not isinstance(argv, list) or not argv or not all(isinstance(item, str) and item for item in argv):
                blockers.append("command argv must be a non-empty literal string array")
                break
            if Path(argv[0]).name in SHELL_PROGRAMS:
                blockers.append(f"shell command is forbidden: {argv[0]}")
                break
            executable = argv[0]
            if executable not in policy.get("allowed_executables", []):
                blockers.append(f"executable is not approved: {argv[0]}")
                break
            if Path(executable).name.startswith("python"):
                if len(argv) < 2 or argv[1] not in policy.get("allowed_python_scripts", []):
                    blockers.append(f"Python entry point is not approved: {argv[1] if len(argv) > 1 else ''}")
                    break
            declared_outputs = command.get("outputs")
            if not isinstance(declared_outputs, list):
                blockers.append(f"command has no explicit outputs declaration: {argv!r}")
                break
            for output in declared_outputs:
                output_path = (root / output).resolve()
                in_cord = root in (output_path, *output_path.parents)
                allowed_external = any(
                    Path(external).resolve() in (output_path, *output_path.parents)
                    for external in policy.get("allowed_external_output_roots", [])
                )
                if not in_cord and not allowed_external:
                    blockers.append(f"declared output escapes approved roots: {output}")
                    break
            if blockers:
                break
            read_inputs = command.get("inputs")
            allowed_writes = command.get("allowed_writes")
            allowed_reads = command.get("allowed_reads", [])
            if not isinstance(read_inputs, list) or not isinstance(allowed_writes, list) or not isinstance(allowed_reads, list):
                blockers.append(f"command lacks explicit inputs/allowed_writes: {argv!r}")
                break
            undeclared_reads = []
            for value in argv[1:]:
                candidate = (root / value).resolve()
                if (candidate.is_file() or candidate.is_dir()) and value not in declared_outputs:
                    declared_value = (
                        "@registry" if value == "docs/specs/evidence-gates-v1.toml"
                        else "@schema" if value == "docs/specs/evidence-report-v1.schema.json"
                        else value
                    )
                    if declared_value not in read_inputs:
                        undeclared_reads.append(value)
            if undeclared_reads:
                blockers.append(f"argv contains undeclared read input(s): {sorted(set(undeclared_reads))}")
                break
            command_read_hashes: dict[str, str] = {}
            command_ancestry: dict[str, Any] = {}
            for value in read_inputs:
                if value == "@registry":
                    command_read_hashes[value] = sha256_file(args.registry)
                    continue
                if value == "@schema":
                    command_read_hashes[value] = sha256_file(args.schema)
                    continue
                if value == "@repository_snapshot":
                    command_read_hashes[value] = repository_snapshot["sha256"]
                    continue
                if value in produced_outputs:
                    path = root / value
                    try:
                        command_read_hashes[value] = hash_path(path)
                        command_ancestry[value] = produced_outputs[value]
                    except OSError as exception:
                        blockers.append(f"missing generated command input {value}: {exception}")
                    continue
                if value not in input_specs:
                    blockers.append(f"command read is absent from frozen gate inputs: {value}")
                    continue
                command_read_hashes[value] = input_hashes.get(value, "")
            if blockers:
                break
            command_root = (root / cwd).resolve()
            if root not in (command_root, *command_root.parents):
                blockers.append(f"command cwd escapes CORD root: {cwd}")
                break
            output_paths = [(root / output).resolve() for output in declared_outputs]
            for output_path in output_paths:
                remove_output(output_path)
            before_command = workspace_state(
                root, policy.get("allowed_external_output_roots", []), output_paths
            )
            trace_descriptor, trace_name = tempfile.mkstemp(prefix="cord-evidence-read-", suffix=".jsonl")
            os.close(trace_descriptor)
            trace_path = Path(trace_name)
            trace_roots = {str(root)}
            trace_roots.update(
                str(Path(row["path"]).resolve())
                for row in repository_snapshot["repositories"] if isinstance(row.get("path"), str)
            )
            environment = dict(os.environ)
            environment["CORD_EVIDENCE_TRACE_FILE"] = str(trace_path)
            environment["CORD_EVIDENCE_TRACE_ROOTS"] = json.dumps(sorted(trace_roots))
            environment["PYTHONDONTWRITEBYTECODE"] = "1"
            tracer_directory = str(tracer_path.parent)
            environment["PYTHONPATH"] = tracer_directory + (
                os.pathsep + environment["PYTHONPATH"] if environment.get("PYTHONPATH") else ""
            )
            try:
                result = subprocess.run(
                    argv,
                    cwd=command_root,
                    check=False,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    env=environment,
                )
                observed_reads, subprocess_ancestry = read_trace(trace_path)
            finally:
                trace_path.unlink(missing_ok=True)
            undeclared_observed = sorted({
                row["path"] for row in observed_reads
                if not declared_read_path(
                    Path(row["path"]), root, read_inputs, produced_outputs,
                    declared_outputs, args.registry, args.schema, tracer_path, allowed_reads,
                    snapshot_bound_files,
                )
            })
            after_command = workspace_state(
                root, policy.get("allowed_external_output_roots", []), output_paths
            )
            command_changed = changed_files(before_command, after_command)
            unexpected_changes = []
            for changed in command_changed:
                changed_path = Path(changed)
                declared = any(
                    changed_path == output_path
                    or (output_path.is_dir() and output_path in changed_path.parents)
                    for output_path in output_paths
                )
                try:
                    relative = changed_path.relative_to(root).as_posix()
                except ValueError:
                    relative = changed
                cache_allowed = any(fnmatch.fnmatchcase(relative, pattern) for pattern in allowed_writes)
                if not declared and not cache_allowed:
                    unexpected_changes.append(relative)
            command_record = {
                    "argv": argv,
                    "cwd": cwd,
                    "exit_code": result.returncode,
                    "stdout_sha256": sha256_bytes(result.stdout),
                    "stderr_sha256": sha256_bytes(result.stderr),
                    "test_count": int(command.get("test_count", 0)),
                    "declared_outputs": declared_outputs,
                    "read_inputs": command_read_hashes,
                    "generated_input_ancestry": command_ancestry,
                    "observed_read_manifest": {
                        "reads": observed_reads,
                        "undeclared_paths": undeclared_observed,
                        "closure_proven": not undeclared_observed,
                        "allowed_read_patterns": allowed_reads,
                        "repository_snapshot_sha256": repository_snapshot["sha256"],
                        "subprocess_ancestry": subprocess_ancestry,
                    },
                    "write_set": {
                        "changed_paths": [
                            Path(path).relative_to(root).as_posix()
                            if root in Path(path).parents else path
                            for path in command_changed
                        ],
                        "allowed_cache_patterns": allowed_writes,
                        "unexpected_paths": unexpected_changes,
                    },
                }
            commands.append(command_record)
            if undeclared_observed:
                blockers.append(f"command observed undeclared read(s): {undeclared_observed!r}")
                break
            if unexpected_changes:
                blockers.append(f"command wrote undeclared output(s): {unexpected_changes!r}")
                break
            if result.returncode != 0:
                blockers.append(f"command failed ({result.returncode}): {argv!r}")
                break
            missing_outputs = [output for output in declared_outputs if not (root / output).is_file()]
            if missing_outputs:
                blockers.append(
                    f"command did not materialize declared output(s): {missing_outputs!r}"
                )
                break
            command_index = len(commands) - 1
            for output in declared_outputs:
                produced_outputs[output] = {
                    "command_index": command_index,
                    "sha256": hash_path(root / output),
                }

    write_set = {
        "before_sha256": sha256_bytes(canonical_bytes(before_repositories)),
        "after_sha256": sha256_bytes(canonical_bytes(before_repositories)),
        "external_deltas": [],
        "cord_changed_paths": [],
        "undeclared_paths": [],
        "allowed_paths": gate.get("write_paths", []),
        "violations": [],
    }
    if gate.get("enforce_write_set", False) and before_repositories:
        try:
            after_repositories = capture_repositories(root, policy["repository_manifest"])
            write_set = repository_write_set(
                root, before_repositories, after_repositories, gate.get("write_paths", [])
            )
            blockers.extend(write_set["violations"])
        except (OSError, KeyError, ValueError, RuntimeError) as exception:
            blockers.append(f"cannot capture after write-set: {exception}")

    artifacts = []
    for declared in gate.get("artifact", []):
        path = root / declared["path"]
        if not path.is_file():
            blockers.append(f"missing artifact: {declared['path']}")
            continue
        actual_hash = sha256_file(path)
        expected_hash = declared.get("sha256")
        if expected_hash and expected_hash != "record" and actual_hash != expected_hash:
            blockers.append(f"artifact hash mismatch: {declared['path']}")
        artifacts.append(
            {
                "path": declared["path"],
                "sha256": actual_hash,
                "schema": declared.get("schema"),
            }
        )

    assertions = [validate_assertion(specification, root) for specification in gate.get("assertion", [])]
    if any(not assertion["pass"] for assertion in assertions):
        blockers.append("one or more typed assertions failed")

    toolchains = {
        "python": sys.version.split()[0],
        "rustc": command_output(["rustc", "--version"]),
        "cargo": command_output(["cargo", "--version"]),
        "node": command_output(["node", "--version"]),
        "npm": command_output(["npm", "--version"]),
    }
    cord_head = command_output(["git", "rev-parse", "HEAD"])
    report = {
        "schema_version": 1,
        "gate_id": args.gate,
        "status": "pass" if not blockers else "blocked",
        "cord_head": cord_head,
        "started_at": started,
        "finished_at": utc_now(),
        "registry_sha256": sha256_file(args.registry),
        "input_hashes": input_hashes,
        "input_ancestry": input_ancestry,
        "repository_snapshot": repository_snapshot,
        "toolchains": toolchains,
        "commands": commands,
        "artifacts": artifacts,
        "assertions": assertions,
        "blockers": blockers,
        "write_set": write_set,
        "report_sha256": "",
    }
    if "stop_literal" in gate:
        report["stop_literal"] = gate["stop_literal"]
    if "p8_authorized" in gate:
        report["p8_authorized"] = gate["p8_authorized"]
    report["report_sha256"] = report_hash(report)
    schema = json.loads(args.schema.read_text(encoding="utf-8"))
    errors = schema_errors(report, schema)
    if errors:
        print("report schema validation failed:\n" + "\n".join(errors), file=sys.stderr)
        return 2
    atomic_write_json(args.out, report)
    print(f"{report['status'].upper()} {args.gate}: {args.out}")
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
