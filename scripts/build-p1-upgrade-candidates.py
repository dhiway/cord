#!/usr/bin/env python3
"""Build hash-bound, higher-spec P1 Origin/Orbis runtime upgrade candidates.

The production runtime versions remain unchanged. Candidate versions exist only behind the
explicit ``p1-upgrade-candidate`` feature; Orbis is always built with ``fast-runtime`` as well.
An authoritative manifest is emitted only from a clean Git worktree and only after ``subwasm``
proves that the produced Wasm has the exact expected runtime identity and higher spec version.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ORIGIN_MANIFEST = ROOT / "origin/base/runtime/Cargo.toml"
ORIGIN_SOURCE = ROOT / "origin/base/runtime/src/lib.rs"
ORBIS_MANIFEST = ROOT / "origin/orbis/runtime/Cargo.toml"
ORBIS_SOURCE = ROOT / "origin/orbis/runtime/src/lib.rs"
LOCK = ROOT / "Cargo.lock"
SCHEMA = "cord.p1-upgrade-candidates.v1"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_sha256(command: list[str]) -> str:
    return hashlib.sha256("\0".join(command).encode()).hexdigest()


def capture(command: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None) -> str:
    completed = subprocess.run(
        command, cwd=cwd, env=env, text=True, capture_output=True, check=False
    )
    if completed.returncode:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {command!r}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed.stdout.strip()


def git_identity(root: Path = ROOT) -> dict[str, Any]:
    head = capture(["git", "rev-parse", "HEAD"], cwd=root)
    status = capture(["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=root)
    return {"commit": head, "clean": not bool(status), "porcelain": status.splitlines()}


def assert_version_sources() -> None:
    checks = (
        (ORIGIN_MANIFEST, r'^p1-upgrade-candidate\s*=\s*\[\]\s*$', "Origin feature"),
        (ORBIS_MANIFEST, r'^p1-upgrade-candidate\s*=\s*\[\]\s*$', "Orbis feature"),
        (ORIGIN_SOURCE, r'spec_version:\s*9901\s*,', "Origin production spec 9901"),
        (ORIGIN_SOURCE, r'spec_version:\s*9902\s*,', "Origin candidate spec 9902"),
        (ORBIS_SOURCE, r'spec_version:\s*29\s*,', "Orbis production spec 29"),
        (ORBIS_SOURCE, r'spec_version:\s*30\s*,', "Orbis candidate spec 30"),
        (
            ORBIS_SOURCE,
            r'Period:\s*u32\s*=\s*if\s+cfg!\(feature\s*=\s*"fast-runtime"\)\s*'
            r'\{\s*2\s*\*\s*MINUTES\s*\}\s*else\s*\{\s*6\s*\*\s*HOURS\s*\}',
            "Orbis fast two-minute / production six-hour session period",
        ),
    )
    for path, pattern, label in checks:
        if not re.search(pattern, path.read_text(encoding="utf-8"), re.MULTILINE):
            raise RuntimeError(f"missing {label} in {path.relative_to(ROOT)}")
    orbis = ORBIS_SOURCE.read_text(encoding="utf-8")
    if '#[cfg(feature = "p1-upgrade-candidate")]' not in orbis:
        raise RuntimeError("Orbis candidate version is not feature-gated")
    origin = ORIGIN_SOURCE.read_text(encoding="utf-8")
    if '#[cfg(feature = "p1-upgrade-candidate")]' not in origin:
        raise RuntimeError("Origin candidate version is not feature-gated")


def find_wasm(target: Path, stem: str) -> Path:
    preferred = sorted(target.glob(f"**/{stem}.compact.compressed.wasm"))
    candidates = preferred or sorted(target.glob(f"**/{stem}.compact.wasm"))
    if len(candidates) != 1:
        raise RuntimeError(f"expected one {stem} candidate Wasm below {target}, found {candidates}")
    path = candidates[0]
    if path.stat().st_size < 8 or path.read_bytes()[:4] not in (b"\x00asm", b"\x1f\x8b\x08\x00"):
        # compact.compressed runtimes may use zstd rather than gzip; subwasm remains authoritative.
        if not path.name.endswith(".compressed.wasm"):
            raise RuntimeError(f"candidate is not a Wasm or recognized compressed runtime: {path}")
    return path


def parse_subwasm(payload: str) -> dict[str, Any]:
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"subwasm did not return JSON: {error}") from error
    if not isinstance(value, dict):
        raise RuntimeError("subwasm JSON root is not an object")
    return value


def nested_first(value: Any, keys: set[str]) -> Any:
    if isinstance(value, dict):
        for key, child in value.items():
            normalized = re.sub(r"[^a-z0-9]", "", str(key).lower())
            if normalized in keys:
                return child
        for child in value.values():
            found = nested_first(child, keys)
            if found is not None:
                return found
    elif isinstance(value, list):
        for child in value:
            found = nested_first(child, keys)
            if found is not None:
                return found
    return None


def validate_runtime_info(info: dict[str, Any], *, spec_name: str, spec_version: int) -> None:
    observed_name = nested_first(info, {"specname"})
    observed_version = nested_first(info, {"specversion"})
    if str(observed_name) != spec_name:
        raise RuntimeError(f"candidate spec name is {observed_name!r}, expected {spec_name!r}")
    try:
        numeric_version = int(observed_version)
    except (TypeError, ValueError) as error:
        raise RuntimeError(f"candidate spec version is invalid: {observed_version!r}") from error
    if numeric_version != spec_version:
        raise RuntimeError(f"candidate spec version is {numeric_version}, expected {spec_version}")


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true", help="perform the two cargo builds")
    parser.add_argument("--output", type=Path, required=True, help="absent or empty artifact directory")
    parser.add_argument("--target-root", type=Path, required=True, help="dedicated build target root")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--subwasm", default="subwasm")
    parser.add_argument(
        "--reuse-target",
        action="store_true",
        help="allow a non-empty dedicated Cargo target; Cargo fingerprints remain authoritative",
    )
    args = parser.parse_args()

    output = args.output.resolve()
    target_root = args.target_root.resolve()
    if output.exists() and any(output.iterdir()):
        raise RuntimeError(f"output must be absent or empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    manifest_path = output / "manifest.json"
    manifest: dict[str, Any] = {
        "schema": SCHEMA,
        "status": "missing" if not args.execute else "failed",
        "authoritative": False,
        "production_versions_unchanged": True,
        "candidates": {},
    }
    try:
        assert_version_sources()
        identity = git_identity()
        manifest["source"] = {
            **identity,
            "files": {
                str(path.relative_to(ROOT)): sha256(path)
                for path in (ROOT / "Cargo.toml", LOCK, ORIGIN_MANIFEST, ORIGIN_SOURCE, ORBIS_MANIFEST, ORBIS_SOURCE)
            },
        }
        manifest["toolchain"] = {
            "cargo": capture([args.cargo, "--version"]),
            "rustc": capture(["rustc", "--version", "--verbose"]),
            "subwasm_path": shutil.which(args.subwasm),
        }
        if not args.execute:
            manifest.update({"status": "prepared", "reason": "--execute was not supplied"})
            return 2
        if not identity["clean"]:
            raise RuntimeError("authoritative candidate builds require a clean Git worktree")
        if not manifest["toolchain"]["subwasm_path"]:
            raise RuntimeError(f"subwasm executable not found: {args.subwasm}")
        target_nonempty = target_root.exists() and any(target_root.iterdir())
        if target_nonempty and not args.reuse_target:
            raise RuntimeError(f"target root must be absent or empty: {target_root}")
        target_root.mkdir(parents=True, exist_ok=True)
        manifest["target_reused"] = bool(target_nonempty)
        artifacts = output / "artifacts"
        artifacts.mkdir()
        builds = (
            {
                "name": "origin",
                "package": "origin-runtime",
                "features": "p1-upgrade-candidate,on-chain-release-build",
                "target": target_root / "origin",
                "stem": "origin_runtime",
                "spec_name": "origin",
                "current_spec_version": 9901,
                "candidate_spec_version": 9902,
                "destination": artifacts / "origin-runtime-v9902.compact.compressed.wasm",
            },
            {
                "name": "orbis-fast",
                "package": "origin-orbis-runtime",
                "features": "fast-runtime,p1-upgrade-candidate,on-chain-release-build",
                "target": target_root / "orbis-fast",
                "stem": "origin_orbis_runtime",
                "spec_name": "orbis",
                "current_spec_version": 29,
                "candidate_spec_version": 30,
                "destination": artifacts / "orbis-fast-runtime-v30.compact.compressed.wasm",
            },
        )
        for build in builds:
            command = [
                args.cargo,
                "build",
                "--release",
                "--locked",
                "--offline",
                "-p",
                build["package"],
                "--features",
                build["features"],
            ]
            environment = os.environ.copy()
            environment["CARGO_TARGET_DIR"] = str(build["target"])
            environment["WASM_BUILD_WORKSPACE_HINT"] = str(ROOT)
            log = output / f"build-{build['name']}.log"
            with log.open("wb") as handle:
                completed = subprocess.run(command, cwd=ROOT, env=environment, stdout=handle, stderr=subprocess.STDOUT)
            if completed.returncode:
                raise RuntimeError(f"candidate build failed ({completed.returncode}); see {log}")
            wasm = find_wasm(build["target"], build["stem"])
            shutil.copyfile(wasm, build["destination"])
            subwasm_environment = os.environ.copy()
            # clap's NO_COLOR parser requires a boolean word, while many shells export NO_COLOR=1.
            subwasm_environment["NO_COLOR"] = "true"
            info_text = capture(
                [args.subwasm, "info", "--json", "--no-color", str(build["destination"])],
                env=subwasm_environment,
            )
            info = parse_subwasm(info_text)
            validate_runtime_info(
                info, spec_name=build["spec_name"], spec_version=build["candidate_spec_version"]
            )
            info_path = output / f"subwasm-{build['name']}.json"
            write_json(info_path, info)
            manifest["candidates"][build["name"]] = {
                "artifact": str(build["destination"].relative_to(output)),
                "sha256": sha256(build["destination"]),
                "bytes": build["destination"].stat().st_size,
                "spec_name": build["spec_name"],
                "current_spec_version": build["current_spec_version"],
                "candidate_spec_version": build["candidate_spec_version"],
                "higher_spec": build["candidate_spec_version"] > build["current_spec_version"],
                "features": build["features"].split(","),
                "command": command,
                "command_sha256": command_sha256(command),
                "build_environment": {
                    "CARGO_TARGET_DIR": str(build["target"]),
                    "WASM_BUILD_WORKSPACE_HINT": str(ROOT),
                },
                "build_log": log.name,
                "build_log_sha256": sha256(log),
                "subwasm": info_path.name,
                "subwasm_sha256": sha256(info_path),
            }
        if not all(candidate["higher_spec"] for candidate in manifest["candidates"].values()):
            raise RuntimeError("a candidate spec version is not higher than production")
        manifest.update({"status": "pass", "authoritative": True})
        return 0
    except Exception as error:
        manifest.update({"status": "failed", "authoritative": False, "error": str(error)})
        print(f"P1 upgrade-candidate build failed closed: {error}", file=sys.stderr)
        return 1
    finally:
        write_json(manifest_path, manifest)
        sidecar = output / "manifest.sha256"
        sidecar.write_text(f"{sha256(manifest_path)}  {manifest_path.name}\n", encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
