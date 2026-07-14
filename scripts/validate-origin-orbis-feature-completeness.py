#!/usr/bin/env python3
"""Validate the current CORD-owned Origin/Orbis native feature boundary.

This is deliberately distinct from production readiness. It proves that the current branch has no
unclassified or still-planned product implementation row while retaining explicit P1/P6/P7
validation and release blockers.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "docs/evidence/verification/p5/feature-completeness.report.json"
ALIGNMENT = ROOT / "docs/architecture/origin-foundation-commons-runtime-alignment.csv"
MANIFEST = ROOT / "docs/orbis-completion-manifest.toml"
CENSUS = ROOT / "docs/architecture/contract-to-native-migration.csv"
GENESIS = ROOT / "docs/genesis/origin-orbis-clean-genesis-manifest.json"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def runtime_pallets(path: Path) -> set[tuple[str, int]]:
    source = path.read_text()
    marker = "construct_runtime! {" if "construct_runtime! {" in source else "construct_runtime!("
    start = source.index(marker)
    end = source.index("\n);", start)
    block = source[start:end]
    return {
        (match.group(1), int(match.group(3)))
        for match in re.finditer(
            r"^\s*([A-Za-z][A-Za-z0-9]*):\s*([A-Za-z0-9_:<>]+)\s*=\s*(\d+),",
            block,
            re.MULTILINE,
        )
    }


def validate() -> tuple[dict, list[str]]:
    errors: list[str] = []

    def require(condition: bool, message: str) -> None:
        if not condition:
            errors.append(message)

    require(git("branch", "--show-current") == "sm-update-sub-0x63", "wrong branch")
    lock = (ROOT / "Cargo.lock").read_text()
    sdk_revisions = set(
        re.findall(
            r"git\+https://github.com/dhiway/sdk\?branch=release-v1\.24\.0#([0-9a-f]{40})",
            lock,
        )
    )
    require(
        sdk_revisions == {"cc190ea83c590b6a14a6b9771ab02c81618dc118"},
        f"SDK revision drift: {sorted(sdk_revisions)}",
    )

    scope_process = subprocess.run(
        [sys.executable, "scripts/validate-p0-provenance.py", "--scope-only"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    try:
        scope = json.loads(scope_process.stdout)
    except json.JSONDecodeError:
        scope = {"status": "fail", "errors": ["scope validator returned invalid JSON"]}
    require(scope_process.returncode == 0, "CORD-only repository scope validation failed")
    require(scope.get("status") == "pass", "CORD-only repository scope is not pass")
    require(scope.get("external_modified") == 0, "an external reference repository is modified")

    try:
        genesis = json.loads(GENESIS.read_text())
    except (OSError, json.JSONDecodeError) as error:
        genesis = {}
        errors.append(f"clean-genesis manifest cannot be read: {error}")
    clean_break = genesis.get("clean_break", {})
    legacy_flags = {
        key: value for key, value in clean_break.items() if key.startswith("legacy_")
    }
    clean_genesis_policy = (
        bool(legacy_flags)
        and all(value is False for value in legacy_flags.values())
        and clean_break.get("old_network_dependency") is False
    )
    require(bool(legacy_flags), "clean-genesis legacy policy is absent")
    require(all(value is False for value in legacy_flags.values()), "legacy genesis input is enabled")
    require(
        clean_break.get("old_network_dependency") is False,
        "clean genesis retains an old-network dependency",
    )

    try:
        with CENSUS.open(newline="") as handle:
            census_rows = list(csv.DictReader(handle))
    except OSError as error:
        census_rows = []
        errors.append(f"contract-to-native census cannot be read: {error}")
    require(bool(census_rows), "contract-to-native census is empty")
    require(
        all(row.get("deployment_state_input") == "none" for row in census_rows),
        "contract census contains a deployment-state input",
    )
    require(
        all(
            row.get("compatibility_or_data_migration") == "forbidden-clean-genesis"
            for row in census_rows
        ),
        "contract census admits compatibility or data migration",
    )
    clean_census_policy = bool(census_rows) and all(
        row.get("deployment_state_input") == "none"
        and row.get("compatibility_or_data_migration") == "forbidden-clean-genesis"
        for row in census_rows
    )

    with ALIGNMENT.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
    ids = [row["id"] for row in rows]
    require(len(ids) == len(set(ids)), "duplicate runtime-alignment ID")
    require(
        set(row["status"] for row in rows) <= {"present", "gap", "excluded", "frozen"},
        "unknown runtime-alignment status",
    )
    by_id = {row["id"]: row for row in rows}
    for row_id in (
        "ORIGIN-PAL-221",
        "ORBIS-PAL-105",
        "ORBIS-PAL-116",
        "ORBIS-PAL-120",
        "ORBIS-PAL-121",
        "ORBIS-PAL-122",
        "ORBIS-PAL-221",
        "NODE-PROOF-PROVIDER",
        "MANIFEST-CURRENT",
    ):
        require(by_id.get(row_id, {}).get("status") == "present", f"{row_id} is not present")

    runtime_paths = {
        "Origin": ROOT / "origin/base/runtime/src/lib.rs",
        "Orbis": ROOT / "origin/orbis/runtime/src/lib.rs",
    }
    pallet_counts: dict[str, int] = {}
    for owner, path in runtime_paths.items():
        expected = runtime_pallets(path)
        actual = {
            (row["capability"], int(row["index_or_version"]))
            for row in rows
            if row["owner"] == owner and row["layer"] == "pallet"
        }
        require(actual == expected, f"{owner} pallet inventory drift: missing={sorted(expected-actual)} extra={sorted(actual-expected)}")
        require(len({index for _, index in actual}) == len(actual), f"{owner} pallet index collision")
        pallet_counts[owner.lower()] = len(actual)

    permitted_validation_gaps = {"GAP-XCM-E2E", "GAP-PROOF-E2E", "GAP-WEIGHTS"}
    gap_ids = {row["id"] for row in rows if row["status"] == "gap"}
    require(gap_ids == permitted_validation_gaps, f"feature or unclassified alignment gaps: {sorted(gap_ids)}")
    for row in rows:
        if row["id"] in permitted_validation_gaps:
            require(row["blocker"].startswith(("P1/", "P7")), f"{row['id']} lacks a later validation/release gate")

    manifest_text = MANIFEST.read_text()

    def scalar(key: str) -> str | int | None:
        matches = re.findall(
            rf'^' + re.escape(key) + r'\s*=\s*(?:"([^"]*)"|(\d+))\s*$',
            manifest_text,
            re.MULTILINE,
        )
        require(len(matches) == 1, f"manifest scalar {key} is absent or duplicated")
        if len(matches) != 1:
            return None
        string, integer = matches[0]
        return string if string else int(integer)

    def table_rows(table: str) -> list[dict[str, str | int]]:
        blocks = re.findall(
            rf'^\[\[' + re.escape(table) + r'\]\]\n(.*?)(?=^\[\[|^\[[^\[]|\Z)',
            manifest_text,
            re.MULTILINE | re.DOTALL,
        )
        parsed: list[dict[str, str | int]] = []
        for block in blocks:
            row: dict[str, str | int] = {}
            for key, string, integer in re.findall(
                r'^([A-Za-z0-9_]+)\s*=\s*(?:"([^"]*)"|(\d+))\s*$',
                block,
                re.MULTILINE,
            ):
                row[key] = string if not integer else int(integer)
            parsed.append(row)
        return parsed

    manifest_version = scalar("manifest_version")
    current_state = scalar("current_state")
    require(manifest_version == 21, "current completion manifest is not version 21")
    require(
        current_state == "p2-p6-native-feature-implementation-present-production-validation-deferred",
        "manifest current-state drift",
    )
    review_statuses = {
        key: scalar(key)
        for key in (
            "transition_architect_status",
            "transition_critic_status",
            "product_architect_status",
            "product_critic_status",
            "evidence_architect_status",
            "evidence_critic_status",
        )
    }
    candidate_ratified = all(value == "clear" for value in review_statuses.values())
    adr = (ROOT / "docs/adr/0017-native-feature-source-of-truth.md").read_text()
    require("## Status\n\nCandidate" in adr, "ADR 0017 improperly claims ratification")
    table_names = set(re.findall(r'^\[\[([A-Za-z0-9_]+)\]\]$', manifest_text, re.MULTILINE))
    planned = [
        f"{table}:{row.get('id', row.get('name', '?'))}"
        for table in table_names
        for row in table_rows(table)
        if row.get("state") == "planned"
    ]
    require(not planned, f"planned implementation rows remain: {planned}")

    runtime_rows = {str(row["id"]): row for row in table_rows("runtime_pallet")}
    require(runtime_rows.get("PAL-098", {}).get("state") == "excluded", "Game is not excluded")
    require(runtime_rows.get("PAL-221", {}).get("state") == "present", "CoretimeControl is absent from manifest")
    require(runtime_rows.get("PAL-221", {}).get("index") == 221, "CoretimeControl index drift")

    web3_rows = [
        row for row in table_rows("node_surface") if str(row.get("id", "")).startswith("WEB3-")
    ]
    require(len(web3_rows) == 17, f"Web3 reference inventory drift: {len(web3_rows)}")
    require(all(row.get("state") == "excluded" for row in web3_rows), "external Web3 code remains planned")
    require(
        all("reference-only" in str(row.get("evidence", "")) for row in web3_rows),
        "external Web3 exclusion lacks reference-only evidence",
    )

    benchmark_rows = {row["id"]: row for row in table_rows("benchmark")}
    for row_id in ("BENCH-38", "BENCH-39", "BENCH-58"):
        require(
            str(benchmark_rows.get(row_id, {}).get("state", "")).startswith("present"),
            f"{row_id} is not candidate-present",
        )
    orbis_runtime = runtime_paths["Orbis"].read_text()
    orbis_runtime_cargo = (ROOT / "origin/orbis/runtime/Cargo.toml").read_text()
    require("[pallet_origin_token, Token]" in orbis_runtime, "Token benchmark is not runtime-wired")
    require("[pallet_origin_feeless, Feeless]" in orbis_runtime, "Feeless benchmark is not runtime-wired")
    require(
        '"pallet-origin-token/runtime-benchmarks"' in orbis_runtime_cargo,
        "Token runtime-benchmarks feature is not propagated",
    )
    require(
        '"pallet-origin-feeless/runtime-benchmarks"' in orbis_runtime_cargo,
        "Feeless runtime-benchmarks feature is not propagated",
    )
    origin_runtime = runtime_paths["Origin"].read_text()
    origin_runtime_cargo = (ROOT / "origin/base/runtime/Cargo.toml").read_text()
    origin_runtime_constants = (ROOT / "origin/base/runtime/constants/src/lib.rs").read_text()
    orbis_coretime = (ROOT / "origin/orbis/runtime/src/coretime.rs").read_text()
    control_benchmarks = (ROOT / "origin/pallets/coretime-control/src/benchmarking.rs").read_text()
    for call in (
        "request_core_count",
        "submit_request",
        "acknowledge",
        "retry_request",
        "set_transport_hold",
        "release_held",
    ):
        require(f"fn {call}" in control_benchmarks, f"CoretimeControl benchmark missing: {call}")
    require(
        control_benchmarks.count("T::MaxTrackedRequests::get()") >= 4,
        "CoretimeControl benchmarks do not exercise bounded worst-case ledgers",
    )
    require(
        "HeldRequestIds::<T>" in control_benchmarks,
        "CoretimeControl held-order benchmark coverage is absent",
    )
    require(
        "[pallet_coretime_control, CoretimeControl]" in origin_runtime,
        "Origin CoretimeControl benchmark is not runtime-wired",
    )
    require(
        "[pallet_coretime_control, CoretimeControl]" in orbis_runtime,
        "Orbis CoretimeControl benchmark is not runtime-wired",
    )
    require(
        '"pallet-coretime-control/runtime-benchmarks"' in origin_runtime_cargo,
        "Origin CoretimeControl runtime-benchmarks feature is not propagated",
    )
    require(
        '"pallet-coretime-control/runtime-benchmarks"' in orbis_runtime_cargo,
        "Orbis CoretimeControl runtime-benchmarks feature is not propagated",
    )

    origin_control_config = re.search(
        r"impl pallet_coretime_control::Config for Runtime \{(.*?)\n\}",
        origin_runtime,
        re.DOTALL,
    )
    orbis_control_config = re.search(
        r"impl pallet_coretime_control::Config for Runtime \{(.*?)\n\}",
        orbis_coretime,
        re.DOTALL,
    )
    require(origin_control_config is not None, "Origin CoretimeControl configuration is absent")
    require(orbis_control_config is not None, "Orbis CoretimeControl configuration is absent")
    origin_config = origin_control_config.group(1) if origin_control_config else ""
    orbis_config = orbis_control_config.group(1) if orbis_control_config else ""
    for required in (
        "type RequestOrigin = frame_support::traits::NeverEnsureOrigin<()>;",
        "type BrokerOrigin = EnsureOrbisBroker;",
        "type ReceiptOrigin = frame_support::traits::NeverEnsureOrigin<()>;",
        "type TransportControlOrigin = frame_support::traits::NeverEnsureOrigin<()>;",
        "type TransportControlEnabled = frame_support::traits::ConstBool<false>;",
    ):
        require(required in origin_config, f"Origin CoretimeControl security wiring drift: {required}")
    require(
        "parachains_origin::Origin::Parachain(id)) if id == ORBIS_ID.into()" in origin_runtime,
        "Origin CoretimeControl Broker origin is not restricted to Orbis para 1006",
    )
    require("pub const ORBIS_ID: u32 = 1006;" in origin_runtime_constants, "Origin Orbis para ID drift")
    for required in (
        "type RequestOrigin = EnsureRoot<AccountId>;",
        "type BrokerOrigin = frame_support::traits::NeverEnsureOrigin<()>;",
        "type ReceiptOrigin = EnsureRoot<AccountId>;",
        "type TransportControlOrigin = EnsureRoot<AccountId>;",
        'frame_support::traits::ConstBool<{ cfg!(feature = "fast-runtime") }>;',
    ):
        require(required in orbis_config, f"Orbis CoretimeControl security wiring drift: {required}")

    control_source = (ROOT / "origin/pallets/coretime-control/src/lib.rs").read_text()
    require("const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);" in control_source, "CoretimeControl storage version missing")
    require("#[pallet::storage_version(STORAGE_VERSION)]" in control_source, "CoretimeControl storage version not attached")
    require("fn on_runtime_upgrade" not in control_source, "CoretimeControl contains a predecessor migration hook")
    for method in ("request_core_count", "submit_request", "retry_request"):
        require(
            f"fn {method}() -> Weight;" in control_source,
            f"CoretimeControl generated WeightInfo method mismatch: {method}",
        )
        require(
            f"T::WeightInfo::{method}()" in control_source,
            f"CoretimeControl call does not use generated weight method: {method}",
        )
    require(
        "HeldRequestIds::<T>::try_mutate" in control_source
        and ".find(|candidate| HeldOutbound" not in control_source,
        "CoretimeControl held release is not bounded to the explicit held-order queue",
    )
    require(
        "RequestAlreadyTerminal" in control_source
        and "Self::untrack_held(id);" in control_source
        and "Self::untrack_held(completed);" in control_source,
        "CoretimeControl terminal/pruned requests can survive in the held FIFO",
    )
    require(
        re.search(
            r"recorded_status,\s*ReceiptStatus::Accepted\s*\|\s*ReceiptStatus::Duplicate\s*\|\s*ReceiptStatus::Conflict",
            control_source,
        )
        is not None,
        "CoretimeControl Duplicate receipts are not terminally untracked",
    )
    feeless_weights = (ROOT / "origin/pallets/feeless/src/weights.rs").read_text()
    require(
        feeless_weights.count("reads_writes(1, 2)") == 2,
        "Feeless removal weight does not cover the usage cleanup write",
    )
    migration_rows = {row["id"]: row for row in table_rows("migration")}
    control_migration = migration_rows.get("MIG-pallet-coretime-control", {})
    require(control_migration.get("storage_version") == 1, "CoretimeControl manifest storage version drift")
    require(
        control_migration.get("state") == "not-applicable-clean-genesis-current-schema",
        "CoretimeControl clean-genesis migration disposition drift",
    )

    inputs = [
        "Cargo.lock",
        "docs/adr/0017-native-feature-source-of-truth.md",
        "docs/architecture/origin-foundation-commons-runtime-alignment.csv",
        "docs/architecture/contract-to-native-migration.csv",
        "docs/evidence/p0-contract-native/repository-scope.report.json",
        "docs/evidence/source-ledger.csv",
        "docs/genesis/origin-orbis-clean-genesis-manifest.json",
        "docs/orbis-completion-manifest.toml",
        "docs/orbis-native-capability-matrix.md",
        "origin/base/runtime/Cargo.toml",
        "origin/base/runtime/constants/src/lib.rs",
        "origin/base/runtime/src/lib.rs",
        "origin/orbis/node/src/main.rs",
        "origin/pallets/feeless/src/benchmarking.rs",
        "origin/pallets/feeless/src/weights.rs",
        "origin/pallets/token/src/benchmarking.rs",
        "origin/orbis/runtime/Cargo.toml",
        "origin/orbis/runtime/src/coretime.rs",
        "origin/orbis/runtime/src/lib.rs",
        "origin/pallets/coretime-control/Cargo.toml",
        "origin/pallets/coretime-control/src/benchmarking.rs",
        "origin/pallets/coretime-control/src/lib.rs",
        "origin/pallets/coretime-control/src/tests.rs",
        "scripts/validate-p0-provenance.py",
    ]
    for relative in inputs:
        require((ROOT / relative).is_file(), f"missing feature input: {relative}")

    report = {
        "schema": "cord.origin-orbis-feature-completeness.v1",
        "scope": "CORD-owned clean-break native implementation",
        "status": "pass" if not errors else "fail",
        "feature_complete": not errors,
        "candidate_ratified": candidate_ratified,
        "review_statuses": review_statuses,
        "production_ready": False,
        "production_activation": False,
        "checks": {
            "sdk_revision": next(iter(sdk_revisions), None),
            "origin_pallets": pallet_counts.get("origin", 0),
            "orbis_pallets": pallet_counts.get("orbis", 0),
            "stable_coretime_control_index": 221,
            "manifest_version": manifest_version,
            "planned_implementation_rows": len(planned),
            "external_web3_rows_excluded": len(web3_rows),
            "remaining_feature_or_unclassified_gaps": sorted(gap_ids - permitted_validation_gaps),
            "remaining_validation_or_release_gaps": sorted(gap_ids & permitted_validation_gaps),
            "legacy_data_migration": not (clean_genesis_policy and clean_census_policy),
            "backward_compatibility": not (clean_genesis_policy and clean_census_policy),
            "external_repository_edits": scope.get("external_modified", -1) != 0,
            "external_repositories_checked": scope.get("external_repositories", 0),
        },
        "deferred_production_gates": [
            "live Broker/Coretime XCM lifecycle and session receipts",
            "proof-retention production campaign",
            "final E/Q/C SLO and independent resource-headroom verdict",
            "final generated weights and deterministic release artifacts",
            "soak, recovery, security review, and explicit activation approvals",
        ],
        "inputs": [
            {"path": relative, "sha256": sha256(ROOT / relative)}
            for relative in inputs
            if (ROOT / relative).is_file()
        ],
        "errors": errors,
    }
    return report, errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    report, errors = validate()
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.check:
        if not REPORT.is_file() or REPORT.read_text() != encoded:
            errors.append("feature-completeness report is stale; run with --write")
    if args.write:
        REPORT.parent.mkdir(parents=True, exist_ok=True)
        REPORT.write_text(encoded)
    print(encoded, end="")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
