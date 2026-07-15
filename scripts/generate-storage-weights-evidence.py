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

"""Generate Commons storage pallet weights and a machine-checkable receipt."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import socket
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Sequence


PALLET_OUTPUTS = {
    "pallet_orbis_storage_provider": Path(
        "origin/orbis/pallets/storage-provider/src/weights.rs"
    ),
    "pallet_orbis_drive": Path("origin/orbis/pallets/drive/src/weights.rs"),
    "pallet_orbis_s3": Path("origin/orbis/pallets/s3/src/weights.rs"),
}
HEADER_MARKER = b"// This file is part of CORD"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def output(command: Sequence[str], cwd: Path) -> str:
    return subprocess.check_output(command, cwd=cwd, text=True).strip()


def run_logged(command: Sequence[str], cwd: Path, log_path: Path, env: dict[str, str]) -> None:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            list(command),
            cwd=cwd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        assert process.stdout is not None
        for line in process.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            log.write(line)
            log.flush()
        return_code = process.wait()
    if return_code:
        raise subprocess.CalledProcessError(return_code, list(command))


def normalize_and_check_header(path: Path, header: bytes) -> None:
    data = path.read_bytes()
    duplicate = header + b"\n" + header
    if data.startswith(duplicate):
        data = header + data[len(duplicate) :]
        path.write_bytes(data)
    if not data.startswith(header):
        raise RuntimeError(f"generated file does not start with exact HEADER-GPL3: {path}")
    if data.count(HEADER_MARKER) != 1:
        raise RuntimeError(f"generated file does not contain exactly one CORD header: {path}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--node-package", required=True)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--pallet", action="append", required=True, dest="pallets")
    parser.add_argument("--steps", type=int, required=True)
    parser.add_argument("--repeat", type=int, required=True)
    parser.add_argument("--limits", type=Path, required=True)
    parser.add_argument("--weights-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    return parser.parse_args()


def generated_coverage(path: Path) -> tuple[list[str], list[str], list[str]]:
    text = path.read_text()
    trait_start = text.index("pub trait WeightInfo {")
    generic_start = text.index("impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {")
    fallback_start = text.index("impl WeightInfo for () {")
    trait = text[trait_start:generic_start]
    generic = text[generic_start:fallback_start]
    fallback = text[fallback_start:]
    pattern = re.compile(r"(?m)^\s*fn\s+([a-z0-9_]+)\s*\(")
    return pattern.findall(trait), pattern.findall(generic), pattern.findall(fallback)


def check_generated_weights(
    root: Path, benchmark_records: list[dict[str, object]], header: bytes
) -> dict[str, object]:
    checks: list[dict[str, object]] = []
    for record in benchmark_records:
        pallet = str(record["pallet"])
        generated = root / str(record["generated"]["path"])
        normalize_and_check_header(generated, header)
        text = generated.read_text()
        trait, generic, fallback = generated_coverage(generated)
        measured = list(record["raw_json"]["benchmarks"])
        if trait != generic or trait != fallback or set(trait) != set(measured):
            raise RuntimeError(
                f"generated coverage mismatch for {pallet}: "
                f"trait={trait}, generic={generic}, fallback={fallback}, measured={measured}"
            )
        generic_text = text[
            text.index("impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {") :
            text.index("impl WeightInfo for () {")
        ]
        fallback_text = text[text.index("impl WeightInfo for () {") :]
        forbidden = ["Weight::zero()", "Weight::from_parts(0, 0)"]
        for needle in forbidden:
            if needle in generic_text or needle in fallback_text:
                raise RuntimeError(f"zero/fallback weight found in {generated}: {needle}")
        if "STEPS: `50`, REPEAT: `20`" not in text or "WASM-EXECUTION: `Compiled`" not in text:
            raise RuntimeError(f"wrong benchmark mode in {generated}")
        checks.append(
            {
                "pallet": pallet,
                "measured_benchmarks": measured,
                "measured_count": len(measured),
                "trait_count": len(trait),
                "generic_implementation_count": len(generic),
                "nonzero_fallback_implementation_count": len(fallback),
                "exact_header": True,
                "compiled_wasm_steps_50_repeat_20": True,
                "zero_weight_absent": True,
            }
        )
    runtime = (root / "origin/orbis/runtime/src/lib.rs").read_text()
    bindings = {
        "pallet_orbis_storage_provider": "type WeightInfo = pallet_orbis_storage_provider::weights::SubstrateWeight<Runtime>;",
        "pallet_orbis_drive": "type WeightInfo = pallet_orbis_drive::weights::SubstrateWeight<Runtime>;",
        "pallet_orbis_s3": "type WeightInfo = pallet_orbis_s3::weights::SubstrateWeight<Runtime>;",
    }
    for pallet, binding in bindings.items():
        if binding not in runtime:
            raise RuntimeError(f"runtime does not bind generated weights for {pallet}")
    pallet_source = (root / "origin/orbis/pallets/storage-provider/src/lib.rs").read_text()
    promotion_pattern = re.compile(
        r"#\[pallet::weight\(T::WeightInfo::promote_checkpoint_fallback\(\s*"
        r"BucketAgreements::<T>::decode_len\(payload\.bucket_id\)\.unwrap_or\(0\) as u32,?\s*"
        r"\)\)\]",
        re.MULTILINE,
    )
    if not promotion_pattern.search(pallet_source):
        raise RuntimeError("fallback promotion dispatch does not use its generated weight")
    return {
        "generated_coverage": checks,
        "runtime_generated_weight_bindings": bindings,
        "promotion_dispatch_uses_generated_weight": True,
        "fallback_zero_weight_absent": True,
    }


def main() -> int:
    args = parse_args()
    root = Path(output(["git", "rev-parse", "--show-toplevel"], Path.cwd()))
    if args.steps != 50 or args.repeat != 20:
        raise RuntimeError("P1 requires exactly --steps 50 --repeat 20")
    if len(args.pallets) != len(set(args.pallets)):
        raise RuntimeError("duplicate --pallet is not allowed")
    if set(args.pallets) != set(PALLET_OUTPUTS):
        raise RuntimeError(f"P1 requires exactly {sorted(PALLET_OUTPUTS)}")
    limits = (root / args.limits).resolve()
    if not limits.is_file():
        raise RuntimeError(f"missing limits file: {limits}")
    header_path = root / "HEADER-GPL3"
    template_path = root / ".maintain/frame-weight-template.hbs"
    header = header_path.read_bytes()
    script_data = Path(__file__).read_bytes()
    python_header = b"#!/usr/bin/env python3\n" + b"\n".join(
        (b"#" + line[2:]) if line.startswith(b"//") else line
        for line in header.split(b"\n")
    )
    if not script_data.startswith(python_header) or script_data.count(
        HEADER_MARKER.replace(b"//", b"#")
    ) != 1:
        raise RuntimeError("producer does not carry exactly HEADER-GPL3")

    evidence_path = (root / args.out).resolve()
    work = evidence_path.parent / "p1-weights-work"
    work.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["SKIP_PALLET_REVIVE_FIXTURES"] = "1"
    env.pop("RUNTIME_METADATA_HASH", None)
    build_argv = [
        "cargo",
        "build",
        "--release",
        "--locked",
        "-p",
        args.node_package,
        "--features",
        "runtime-benchmarks",
    ]
    started_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    receipt: dict[str, object] = {
        "schema_version": 1,
        "status": "running",
        "producer": {
            "path": str(Path(__file__).resolve().relative_to(root)),
            "sha256": sha256(Path(__file__).resolve()),
            "argv": sys.argv,
        },
        "source": {
            "commit": output(["git", "rev-parse", "HEAD"], root),
            "tree": output(["git", "rev-parse", "HEAD^{tree}"], root),
            "branch": output(["git", "branch", "--show-current"], root),
        },
        "parameters": {
            "node_package": args.node_package,
            "runtime": args.runtime,
            "pallets": args.pallets,
            "steps": args.steps,
            "repeat": args.repeat,
            "chain": "orbis-dev",
            "wasm_execution": "compiled",
            "limits_path": str(args.limits),
            "limits_sha256": sha256(limits),
            "requested_weights_dir": str(args.weights_dir),
        },
        "machine": {
            "hostname": socket.gethostname(),
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "python": sys.version,
            "rustc": output(["rustc", "--version", "--verbose"], root),
            "cargo": output(["cargo", "--version"], root),
        },
        "started_at": started_at,
        "build": {"argv": build_argv, "log": str((work / "build.log").relative_to(root))},
        "benchmarks": [],
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.write_text(json.dumps(receipt, indent=2) + "\n")
    try:
        run_logged(build_argv, root, work / "build.log", env)
        node_path = root / "target" / "release" / args.node_package
        runtime_stem = args.runtime.replace("-", "_")
        runtime_wasm = (
            root
            / "target"
            / "release"
            / "wbuild"
            / args.runtime
            / f"{runtime_stem}.compact.compressed.wasm"
        )
        if not node_path.is_file() or not runtime_wasm.is_file():
            raise RuntimeError("release native node or compiled Commons runtime Wasm is missing")
        receipt["artifacts"] = {
            "native_node": {"path": str(node_path.relative_to(root)), "sha256": sha256(node_path)},
            "runtime_wasm": {
                "path": str(runtime_wasm.relative_to(root)),
                "sha256": sha256(runtime_wasm),
            },
            "header": {"path": "HEADER-GPL3", "sha256": sha256(header_path)},
            "template": {
                "path": ".maintain/frame-weight-template.hbs",
                "sha256": sha256(template_path),
            },
        }
        benchmark_records: list[dict[str, object]] = []
        for pallet in args.pallets:
            canonical_output = root / PALLET_OUTPUTS[pallet]
            raw_json = work / f"{pallet}.json"
            log = work / f"{pallet}.log"
            benchmark_argv = [
                str(node_path),
                "benchmark",
                "pallet",
                "--chain=orbis-dev",
                "--wasm-execution=compiled",
                "--pallets",
                pallet,
                "--extrinsic=*",
                "--steps",
                str(args.steps),
                "--repeat",
                str(args.repeat),
                "--min-duration",
                "0",
                "--output-analysis=max",
                "--output-pov-analysis=max",
                "--header",
                str(header_path),
                "--template",
                str(template_path),
                "--json-file",
                str(raw_json),
                "--output",
                str(canonical_output),
            ]
            run_logged(benchmark_argv, root, log, env)
            normalize_and_check_header(canonical_output, header)
            parsed = json.loads(raw_json.read_text())
            if not parsed or {row.get("pallet") for row in parsed} != {pallet}:
                raise RuntimeError(f"invalid benchmark JSON coverage for {pallet}")
            benchmark_records.append(
                {
                    "pallet": pallet,
                    "argv": benchmark_argv,
                    "log": str(log.relative_to(root)),
                    "raw_json": {
                        "path": str(raw_json.relative_to(root)),
                        "sha256": sha256(raw_json),
                        "benchmarks": [row["benchmark"] for row in parsed],
                    },
                    "generated": {
                        "path": str(canonical_output.relative_to(root)),
                        "sha256": sha256(canonical_output),
                    },
                }
            )
            receipt["benchmarks"] = benchmark_records
            evidence_path.write_text(json.dumps(receipt, indent=2) + "\n")
        checks = check_generated_weights(root, benchmark_records, header)
        limits_text = limits.read_text()
        bounded_limits = {}
        for name in ["max_duties_per_block", "max_reconciliation_records_per_tick"]:
            match = re.search(rf"(?m)^{name}\s*=\s*(\d+)\s*$", limits_text)
            if match is None:
                raise RuntimeError(f"missing required bounded limit: {name}")
            bounded_limits[name] = int(match.group(1))
        if bounded_limits != {
            "max_duties_per_block": 256,
            "max_reconciliation_records_per_tick": 128,
        }:
            raise RuntimeError(f"unexpected storage bounds: {bounded_limits}")
        gate_argv = [
            "cargo",
            "test",
            "--locked",
            "-p",
            args.runtime,
            "tests::commons_storage_control_worst_case_weights_fit_the_runtime_block_budget",
            "--",
            "--exact",
            "--nocapture",
        ]
        gate_log = work / "storage-weight-gate.log"
        run_logged(gate_argv, root, gate_log, env)
        checks["bounded_limits"] = bounded_limits
        checks["compiled_runtime_gate"] = {
            "argv": gate_argv,
            "log": str(gate_log.relative_to(root)),
            "log_sha256": sha256(gate_log),
            "result": "pass",
            "assertions": [
                "all generated dispatch and hook weights are nonzero",
                "0/1/limit-1/limit sequences are monotonic where the function domain permits",
                "256-release, 128-reconciliation and 128-challenge hook maxima fit RuntimeBlockWeights",
                "each hook plus the maximum provider dispatch fits the aggregate block budget",
                "provider replica/agreement/proof, Drive and S3 component limits are monotonic and bounded",
            ],
        }
        receipt["checks"] = checks
        receipt["benchmarks"] = benchmark_records
        receipt["status"] = "pass"
        receipt["finished_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        evidence_path.write_text(json.dumps(receipt, indent=2) + "\n")
    except Exception as error:
        receipt["status"] = "failed"
        receipt["failure"] = {"type": type(error).__name__, "message": str(error)}
        receipt["finished_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        evidence_path.write_text(json.dumps(receipt, indent=2) + "\n")
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
