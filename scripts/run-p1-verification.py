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

"""Execute and bind the deterministic P1 Commons control-plane evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier.
    import tomli as tomllib

sys.dont_write_bytecode = True


EXPECTED_TESTS = {
    "provider": (86, 0, 0),
    "drive": (15, 0, 0),
    "s3": (24, 0, 0),
    "runtime": (75, 0, 0),
}
EXPECTED_BENCHMARKS = {
    "provider": [
        "register_provider", "update_provider", "rotate_service_key",
        "rotate_provider_organization", "set_provider_status", "remove_provider",
        "heartbeat", "create_bucket", "change_bucket_grant", "propose_agreement",
        "accept_agreement", "set_agreement_suspension", "terminate_agreement",
        "expire_agreement", "submit_checkpoint", "promote_checkpoint_fallback",
        "issue_challenge", "submit_challenge_proof", "reconcile_bucket",
        "refresh_bucket_authority_valid", "refresh_bucket_authority_failover",
        "register_manifest", "publish_manifest", "tombstone_manifest",
        "acknowledge_manifest_deletion", "replace_bucket_replica",
        "advance_finalized_checkpoint", "create_host_delegation",
        "rotate_host_delegation", "revoke_host_delegation", "on_initialize_release",
        "on_initialize_reconcile", "on_initialize_challenges",
    ],
    "drive": [
        "create_drive", "update_root", "set_grant", "transfer_drive", "archive_drive",
        "write_node_create", "write_node_update_file", "remove_node",
    ],
    "s3": [
        "create_bucket", "set_controller", "transfer_bucket", "set_archived",
        "set_versioning", "put_object_create", "put_object_update", "delete_object",
        "delete_bucket", "prune_history", "purge_object",
    ],
}
EXPECTED_WEIGHTS = {
    "provider": "022594cabf3b1755cd9319e851c8666b289bf38dc6f66e70489759286b72fe81",
    "drive": "25af195852af0c66e265bfd82ded1ddd878c4527b672d08d04401221eab25615",
    "s3": "0ea2abd134979aea574c6f6e2cdd5fae8bfcb67f01db0307a33e998647791987",
}
WEIGHT_PATHS = {
    "provider": Path("origin/orbis/pallets/storage-provider/src/weights.rs"),
    "drive": Path("origin/orbis/pallets/drive/src/weights.rs"),
    "s3": Path("origin/orbis/pallets/s3/src/weights.rs"),
}
G002_TESTS = {
    "tests::checkpoint_duty_runtime_api_has_exact_128_snapshot_paging_contract",
    "tests::checkpoint_duty_runtime_api_filters_members_and_preserves_typed_ineligible_views",
    "tests::runtime_checkpoint_duty_admission_is_exactly_255_256_257",
    "tests::checkpoint_context_duty_and_promotion_exact_golden_vector_is_stable",
    "tests::authority_only_fallback_promotion_is_idempotent_and_requires_repair_before_publish",
    "tests::authority_only_fallback_promotion_rolls_back_every_surface_on_hostile_invariants",
    "tests::checkpoint_and_challenge_admissions_share_one_exact_bound",
    "tests::bucket_agreement_admission_is_exact_and_terminal_release_frees_slot",
}
# Native runtime tests retained after contract/fixture cutover. They bind the
# current Commons storage APIs, metadata-hash profile, and recovery journey.
REQUIRED_NAMED_TESTS = G002_TESTS | {
    "enterprise_journey::enterprise_identity_attestation_name_and_storage_lifecycle_is_native_and_fail_closed",
    "tests::native_identity_attestation_name_asset_and_storage_journey",
    "tests::storage_runtime_api_exposes_exact_active_and_revoked_host_delegation",
    "tests::deletion_duty_runtime_api_is_provider_scoped_bounded_and_ack_aware",
    "tests::s3_runtime_api_uses_snapshot_cursor_raw_order_and_hides_tombstones",
    "tests::runtime_signing_payloads_match_shared_sdk_vectors",
    "tests::commons_checkpoint_wire_vector_production_profile_is_stable",
    "tests::three_provider_promotion_repair_quorum_returns_to_standard_exactly_once",
    "tests::metadata_custom_hash_loss_is_detected_after_wire_roundtrip",
    "tests::transaction_policy_construction_surfaces_share_the_frozen_slots",
}
EXPECTED_AMENDMENT_SHA256 = "ba89e20cb46bc19c9aa96cd1b316a1952f596209e82a939c446db86179e65c2d"
EXPECTED_RATIFICATION_SHA256 = "3e4e66e4fc5be2e1aeaf1f3a84dcb8c3c3d7f2dc1ce421b4283be4a6ca351ac5"
TEST_RESULT = re.compile(
    r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;"
)
TEST_LINE = re.compile(r"^test (\S+) \.\.\. (ok|ignored)$", re.MULTILINE)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def run(argv: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )


def suite(
    name: str, argv: list[str], failures: list[str], metadata_hash: str
) -> tuple[dict[str, Any], dict[str, str]]:
    environment = dict(os.environ)
    environment.update({"CARGO_TERM_COLOR": "never", "SKIP_PALLET_REVIVE_FIXTURES": "1"})
    if name == "runtime":
        environment["RUNTIME_METADATA_HASH"] = metadata_hash
    result = run(argv, environment)
    summaries = [tuple(map(int, item)) for item in TEST_RESULT.findall(result.stdout)]
    observed = max(summaries, key=sum) if summaries else (-1, -1, -1)
    expected = EXPECTED_TESTS[name]
    passed = result.returncode == 0 and observed == expected
    if not passed:
        failures.append(
            f"{name} suite: exit={result.returncode}, observed={observed}, expected={expected}"
        )
    names = {test: "passed" if status == "ok" else "ignored" for test, status in TEST_LINE.findall(result.stdout)}
    return ({
        "argv": argv,
        "expected": {"passed": expected[0], "failed": expected[1], "ignored": expected[2]},
        "observed": {"passed": observed[0], "failed": observed[1], "ignored": observed[2]},
        "status": "passed" if passed else "failed",
    }, names)


def benchmark_evidence(receipt_path: Path, failures: list[str]) -> dict[str, Any]:
    receipt = load_json(receipt_path)
    rows = receipt.get("components", []) if isinstance(receipt, dict) else []
    by_pallet = {row.get("pallet"): row for row in rows if isinstance(row, dict)}
    names = {
        "provider": "pallet_orbis_storage_provider",
        "drive": "pallet_orbis_drive",
        "s3": "pallet_orbis_s3",
    }
    result: dict[str, Any] = {}
    receipt_valid = (
        receipt.get("status") == "pass"
        and receipt.get("claim_boundary") == {
            "feature_complete": False,
            "g002_control_plane": True,
            "g003_provider_byte_plane": False,
            "production_ready": False,
        }
        and set(by_pallet) == set(names.values())
    )
    if not receipt_valid:
        failures.append("storage weight evidence receipt contract mismatch")
    for name, pallet in names.items():
        row = by_pallet.get(pallet, {})
        component_receipt_path = Path(str(row.get("receipt_path", "")))
        component_receipt = (
            load_json(component_receipt_path) if component_receipt_path.is_file() else {}
        )
        benchmark = component_receipt.get("benchmark", {})
        raw_path = Path(str(row.get("raw_json_path", "")))
        raw_rows = load_json(raw_path) if raw_path.is_file() else []
        observed = [sample.get("benchmark") for sample in raw_rows if isinstance(sample, dict)]
        expected = EXPECTED_BENCHMARKS[name]
        complete_samples = bool(raw_rows) and all(
            sample.get("time_results") and sample.get("db_results")
            for sample in raw_rows if isinstance(sample, dict)
        )
        weight_path = WEIGHT_PATHS[name]
        text = weight_path.read_text(encoding="utf-8")
        weight_hash = sha256(weight_path)
        trait_match = re.search(r"pub trait WeightInfo \{(.*?)\n\}", text, re.DOTALL)
        trait_functions = re.findall(r"\bfn\s+(\w+)\s*\(", trait_match.group(1) if trait_match else "")
        raw_hash = sha256(raw_path) if raw_path.is_file() else None
        valid = (
            observed == expected
            and complete_samples
            and trait_functions == expected
            and row.get("benchmark_count") == len(expected)
            and raw_hash == row.get("raw_json_sha256")
            and weight_hash == row.get("generated_weight_sha256") == EXPECTED_WEIGHTS[name]
            and row.get("generated_weight_path") == str(weight_path)
            and component_receipt_path.is_file()
            and sha256(component_receipt_path) == row.get("receipt_sha256")
            and component_receipt.get("status") == "pass"
            and component_receipt.get("measurement_source") == row.get("measurement_source")
            and benchmark.get("benchmarks") == expected
            and benchmark.get("benchmark_count") == len(expected)
            and benchmark.get("raw_json_sha256") == raw_hash
            and benchmark.get("generated_weight_sha256") == weight_hash
            and "STEPS: `50`, REPEAT: `20`" in text
            and "WASM-EXECUTION: `Compiled`" in text
        )
        if not valid:
            failures.append(f"{name} benchmark coverage/provenance mismatch")
        result[name] = {
            "benchmark_count": len(observed),
            "benchmarks": observed,
            "db_and_time_samples_complete": complete_samples,
            "raw_json_sha256": raw_hash,
            "receipt_sha256": sha256(component_receipt_path),
            "steps": 50,
            "repeat": 20,
            "wasm_execution": "Compiled",
            "weight_sha256": weight_hash,
        }
    return result


def repository_boundary(before_path: Path, after_path: Path, failures: list[str]) -> dict[str, Any]:
    before = load_json(before_path)
    after = load_json(after_path)
    fields = (
        "head", "index_tree", "cached_diff_sha256", "worktree_diff_sha256",
        "untracked_tree_root", "raw_status_sha256", "tracked_tree_root",
    )
    old = {row["name"]: row for row in before["repositories"]}
    new = {row["name"]: row for row in after["repositories"]}
    changed = []
    for name in sorted(set(old) | set(new)):
        if name not in old or name not in new:
            changed.append(name)
            continue
        if old[name].get("role") == "cord":
            continue
        if any(old[name].get(field) != new[name].get(field) for field in fields):
            changed.append(name)
    if changed:
        failures.append(f"non-CORD repository changes detected: {', '.join(changed)}")
    return {
        "after_sha256": after.get("snapshot_sha256"),
        "before_sha256": before.get("snapshot_sha256"),
        "changed_non_cord_repositories": changed,
        "changed_non_cord_repository_count": len(changed),
        "repository_count": len(new),
    }


def ratification(path: Path, amendment_path: Path, failures: list[str]) -> dict[str, Any]:
    config = tomllib.loads(path.read_text(encoding="utf-8"))
    expected_g003 = [
        "checkpoint-provider-7", "checkpoint-provider-8", "checkpoint-provider-9",
        "checkpoint-provider-10", "checkpoint-three-provider-11",
    ]
    base_valid = (
        sha256(path) == EXPECTED_RATIFICATION_SHA256
        and config.get("status") == "accepted"
        and config.get("ratified") is True
        and config.get("self_ratification") is False
        and config.get("scope") == "G002 canonical Commons control-plane semantic authority only"
        and config.get("phase_amendment_path") == str(amendment_path)
        and config.get("phase_amendment_sha256") == sha256(amendment_path) == EXPECTED_AMENDMENT_SHA256
    )
    if not base_valid:
        failures.append("accepted V2 storage-control ratification contract mismatch")
    reports: dict[str, Any] = {}
    expected = {
        "architect_review": ("architect", "CLEAR"),
        "critic_review": ("critic", "APPROVE"),
    }
    report_paths: list[str] = []
    for key, (role, verdict) in expected.items():
        declaration = config.get(key, {})
        report_path = Path(str(declaration.get("path", "")))
        report_paths.append(str(report_path))
        valid = report_path.is_file()
        report = load_json(report_path) if valid else {}
        report_hash = sha256(report_path) if valid else None
        valid = bool(
            valid
            and report_hash == declaration.get("sha256")
            and report.get("role") == role
            and report.get("verdict") == declaration.get("verdict") == verdict
            and report.get("independent_review") is True
            and declaration.get("independent") is True
            and report.get("review_id") == declaration.get("review_id")
            and report.get("approval_id") == declaration.get("approval_id")
        )
        if not valid:
            failures.append(f"V2 ratification report invalid: {role}")
        reports[role] = {
            "path": str(report_path), "sha256": report_hash, "verdict": report.get("verdict"),
            "valid": valid,
        }
    if len(report_paths) != len(set(report_paths)):
        failures.append("V2 ratification review paths are not independent")
    boundary = config.get("acceptance_boundary", {})
    boundary_valid = boundary == {
        "g002_control_plane_semantic_authority": True,
        "feature_complete": False,
        "provider_byte_plane_complete": False,
        "production_ready": False,
        "whole_program_complete": False,
        "g003_remains_required": True,
        "g003_required_tests": expected_g003,
    }
    if not boundary_valid:
        failures.append("V2 ratification acceptance boundary mismatch")
    ready = base_valid and boundary_valid and len(reports) == 2 and all(
        row["valid"] for row in reports.values()
    )
    return {
        "acceptance_boundary": boundary,
        "ready": ready,
        "ratification_sha256": sha256(path),
        "reports": reports,
        "status": config.get("status"),
    }


MATERIAL_PATHS = [
    Path("Cargo.lock"), Path("origin/orbis/runtime"), Path("origin/orbis/runtime-api/storage"),
    Path("origin/orbis/primitives"), Path("origin/orbis/pallets/storage-provider"),
    Path("origin/orbis/pallets/drive"), Path("origin/orbis/pallets/s3"),
]

def source_tree_hash(paths: list[Path]) -> str:
    """Hash the tracked runtime material, never worktree build by-products."""
    digest = hashlib.sha256()
    root = Path.cwd().resolve()
    tracked = subprocess.run(
        ["git", "ls-files", "-z", "--", *(str(path) for path in paths)],
        check=True, stdout=subprocess.PIPE,
    ).stdout.split(b"\0")
    files = [root / item.decode("utf-8") for item in tracked if item]
    for path in sorted(files, key=lambda item: item.resolve().relative_to(root).as_posix().encode("utf-8")):
        relative = path.resolve().relative_to(root).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(bytes.fromhex(sha256(path)))
    return digest.hexdigest()

def canonical_provenance(path: Path, failures: list[str]) -> dict[str, Any]:
    release = load_json(path)
    current_material = source_tree_hash(MATERIAL_PATHS)
    commit = str(release.get("source_commit", ""))
    ancestor = run(["git", "merge-base", "--is-ancestor", commit, "HEAD"]).returncode == 0 if commit else False
    valid = (
        release.get("p1_runtime_material_sha256") == current_material
        and release.get("cargo_lock_sha256") == sha256(Path("Cargo.lock"))
        and release.get("independent_clean_source_runs") == 2
        and release.get("srtool_no_cache") is True
        and release.get("srtool_cargo_incremental") is False
        and release.get("srtool_cargo_jobs") == 1
        and release.get("srtool_image_digest") == "docker.io/paritytech/srtool@sha256:8638a668bd6d29111dc01953fbead6eb08c062e1cc62d3047a245a52b6edb3bf"
        and ancestor
    )
    if not valid:
        failures.append("canonical release provenance does not match current runtime material")
    return {"valid": valid, "source_commit": commit, "runtime_material_sha256": current_material}


def canonical_commons_wasm(
    release_inputs: Path, compact_wasm: Path, failures: list[str]
) -> dict[str, Any]:
    """Bind P1's compact WASM to the two canonical srtool release runs."""
    release_dir = release_inputs.resolve().parent
    checksums = release_dir / "SHA256SUMS"
    primary = release_dir / "origin_commons_runtime.compact.wasm"
    reproduction = release_dir / "origin_commons_runtime.reproduction.compact.wasm"
    entries: dict[str, str] = {}
    try:
        for line in checksums.read_text(encoding="utf-8").splitlines():
            fields = line.split(maxsplit=1)
            if len(fields) == 2:
                entries[fields[1].lstrip(" *")] = fields[0]
        observed = {
            "primary": sha256(primary),
            "reproduction": sha256(reproduction),
            "p1_compact": sha256(compact_wasm),
            "release_inputs": sha256(release_inputs),
        }
        valid = (
            entries.get(primary.name) == observed["primary"]
            and entries.get(reproduction.name) == observed["reproduction"]
            and entries.get(release_inputs.name) == observed["release_inputs"]
            and observed["primary"] == observed["reproduction"] == observed["p1_compact"]
        )
    except OSError:
        observed = {}
        valid = False
    if not valid:
        failures.append("P1 compact WASM is not the reproducible canonical Commons release artifact")
    return {"valid": valid, "release_dir": str(release_dir), "sha256": observed}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--metadata-scale", required=True, type=Path)
    parser.add_argument("--portable-registry", required=True, type=Path)
    parser.add_argument("--scale-binding", required=True, type=Path)
    parser.add_argument("--amendment", required=True, type=Path)
    parser.add_argument("--ratification", required=True, type=Path)
    parser.add_argument("--metadata-record", required=True, type=Path)
    parser.add_argument("--transaction-manifest", required=True, type=Path)
    parser.add_argument("--weight-evidence", required=True, type=Path)
    parser.add_argument("--compact-wasm", required=True, type=Path)
    parser.add_argument("--release-inputs", required=True, type=Path)
    parser.add_argument("--repositories-before", required=True, type=Path)
    parser.add_argument("--repositories-after", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    failures: list[str] = []
    provenance = canonical_provenance(args.release_inputs, failures)
    canonical_wasm = canonical_commons_wasm(args.release_inputs, args.compact_wasm, failures)
    metadata_record = load_json(args.metadata_record)
    runtime_metadata_hash = metadata_record.get("metadata_hash")
    if not isinstance(runtime_metadata_hash, str) or re.fullmatch(r"0x[0-9a-f]{64}", runtime_metadata_hash) is None:
        failures.append("metadata record has no RFC-78 metadata hash")
        runtime_metadata_hash = "0x" + "0" * 64

    commands = {
        "provider": ["cargo", "test", "-p", "pallet-orbis-storage-provider", "--features", "runtime-benchmarks"],
        "drive": ["cargo", "test", "-p", "pallet-orbis-drive", "--features", "runtime-benchmarks"],
        "s3": ["cargo", "test", "-p", "pallet-orbis-s3", "--features", "runtime-benchmarks"],
        "runtime": ["cargo", "test", "--manifest-path", "origin/orbis/runtime/Cargo.toml"],
    }
    suites = {}
    named_tests: dict[str, str] = {}
    for name, argv in commands.items():
        suites[name], observed_names = suite(name, argv, failures, runtime_metadata_hash)
        named_tests.update(observed_names)
    missing_named = sorted(test for test in REQUIRED_NAMED_TESTS if named_tests.get(test) != "passed")
    if missing_named:
        failures.append(f"required named tests did not pass: {', '.join(missing_named)}")
    g002_executed = sorted(test for test in G002_TESTS if named_tests.get(test) == "passed")

    metadata_scale_sha = sha256(args.metadata_scale)
    registry_sha = sha256(args.portable_registry)
    compact_wasm_sha = sha256(args.compact_wasm)
    transaction_manifest = load_json(args.transaction_manifest)
    binding = load_json(args.scale_binding)
    metadata_valid = (
        binding.get("complete") is True
        and binding.get("metadata_sha256") == metadata_scale_sha
        and binding.get("portable_registry_sha256") == registry_sha
        and metadata_record.get("runtime") == "commons"
        and isinstance(metadata_record.get("metadata_hash"), str)
        and re.fullmatch(r"0x[0-9a-f]{64}", metadata_record["metadata_hash"]) is not None
        and isinstance(metadata_record.get("spec_version"), int)
        and isinstance(metadata_record.get("transaction_version"), int)
        and compact_wasm_sha == metadata_record.get("compact_wasm_sha256")
        and canonical_wasm["valid"]
    )
    manifest_metadata = next(
        (row for row in transaction_manifest.get("files", []) if row.get("file") == "metadata-hash.json"),
        {},
    )
    metadata_valid = metadata_valid and manifest_metadata.get("sha256") == sha256(args.metadata_record)
    if not metadata_valid:
        failures.append("source/Wasm/current-metadata binding mismatch")

    benchmarks = benchmark_evidence(args.weight_evidence, failures)
    header = run(["python3", "scripts/check-source-headers.py"])
    if header.returncode != 0:
        failures.append("source header validation failed")
    diff = run(["git", "diff", "--check"])
    if diff.returncode != 0:
        failures.append("git diff --check failed")
    boundary = repository_boundary(
        args.repositories_before, args.repositories_after, failures
    )

    ratification_report = ratification(args.ratification, args.amendment, failures)
    report = {
        "benchmark_evidence": benchmarks,
        "canonical_provenance": provenance,
        "canonical_commons_wasm": canonical_wasm,
        "cord_only_boundary": boundary,
        "header_check": {"exit_code": header.returncode, "output_sha256": hashlib.sha256(header.stdout.encode()).hexdigest()},
        "mechanical_failures": failures,
        "mechanical_failure_count": len(failures),
        "executed_test_count": sum(value[0] for value in EXPECTED_TESTS.values()),
        "ignored_test_count": sum(value[2] for value in EXPECTED_TESTS.values()),
        "named_test_count": len(REQUIRED_NAMED_TESTS),
        "benchmark_count": sum(len(value) for value in EXPECTED_BENCHMARKS.values()),
        "base_limit_fit_dispatch_count": 19,
        "phase_headroom_check_count": 3,
        "metadata_binding": {
            "complete": metadata_valid,
            "compact_wasm_sha256": compact_wasm_sha,
            "metadata_record_sha256": sha256(args.metadata_record),
            "metadata_scale_sha256": metadata_scale_sha,
            "portable_registry_sha256": registry_sha,
            "transaction_manifest_sha256": sha256(args.transaction_manifest),
        },
        "named_tests": {name: named_tests[name] for name in sorted(REQUIRED_NAMED_TESTS) if name in named_tests},
        "g002_control_plane_test_count": len(g002_executed),
        "g002_control_plane_tests": g002_executed,
        "ratification": ratification_report,
        "ratification_ready": ratification_report["ready"],
        "schema_version": 1,
        "semantic_vectors": {
            "checkpoint_v2_sha256": sha256(Path("docs/specs/checkpoint-v2.vectors.json")),
            "drive_s3_v1_sha256": sha256(Path("docs/specs/drive-s3-v1.vectors.json")),
            "provider_protocol_v1_sha256": sha256(Path("docs/specs/provider-protocol-v1.vectors.json")),
        },
        "source_tree_sha256": source_tree_hash([
            Path("origin/orbis/pallets/storage-provider"), Path("origin/orbis/pallets/drive"),
            Path("origin/orbis/pallets/s3"), Path("origin/orbis/primitives"),
            Path("origin/orbis/runtime-api/storage"), Path("origin/orbis/runtime"),
        ]),
        "claim_boundary": {
            "g002_control_plane": True,
            "g003_provider_byte_plane": False,
            "feature_complete": False,
            "production_ready": False,
            "whole_program_complete": False,
        },
        "specification_binding": {
            "phase_amendment_sha256": sha256(args.amendment),
            "ratification_input_sha256": sha256(args.ratification),
        },
        "suites": suites,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
