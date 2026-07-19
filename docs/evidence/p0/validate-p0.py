#!/usr/bin/env python3
"""Validate the complete, independently approved P0 evidence bundle.

Before Architect approval, exit zero proves internal consistency while the P0 gate
remains blocked.  After approval, the authoritative index may claim the P0 gate only
when the hash-bound approval record, its reviewed indexes, plan, payload, validator
and scope all validate.  Production activation remains a separate downstream gate.
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

ROOT = Path(__file__).resolve().parents[3]
INDEX = ROOT / "docs/evidence/p0/index.json"
APPROVAL = ROOT / "docs/evidence/p0/architect-approval.json"
REVALIDATION_CANDIDATE = ROOT / "docs/evidence/p0/scope-revalidation-candidate.json"
PLAN = ROOT.parent / ".omx/plans/origin-orbis-emerging-app-platform.md"
SUPERSEDED_APPROVAL_SHA256 = "f35d428b92fb936052bebe76239103856a1f1aa57d50e38d27f0fa52e439d3d3"
SUPERSEDED_PLAN_SHA256 = "75e78cbbdfd94e7cd6442982d95e6c376288b13daf2fd4d36a98178cb8bd1bce"
SUPERSEDED_REVIEW_ID = "019f59f2-330b-7881-a4ea-7788af0a4313"
REVALIDATION_REVIEWER = "native-agent:/root/p0_scope_revalidation_architect"


def sha256(path: Path) -> str:
	return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str], errors: list[str], allowed: tuple[int, ...] = (0,)) -> None:
	result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
	label = " ".join(command)
	if result.returncode not in allowed:
		errors.append(f"{label}: exit {result.returncode}: {(result.stdout + result.stderr).strip()}")
	else:
		state = "PASS" if result.returncode == 0 else "BLOCKED"
		print(f"{state} child: {label}")


def main() -> int:
	parser = argparse.ArgumentParser()
	parser.add_argument(
		"--runtime-test",
		action="store_true",
		help="also run the frozen Orbis transaction-policy construction test",
	)
	args = parser.parse_args()
	errors: list[str] = []

	try:
		index = json.loads(INDEX.read_text())
	except Exception as error:
		print(f"ERROR: cannot read {INDEX.relative_to(ROOT)}: {error}")
		return 1

	if index.get("schema_version") != 2:
		errors.append("authoritative P0 index schema_version must be 2")
	if index.get("phase") != "P0":
		errors.append("authoritative index phase must be P0")
	if "p0_executable_prerequisite" not in index.get("blockers", {}):
		errors.append("P0 executable-prerequisite blocker class is missing")
	blocker_ids = {
		item.get("id")
		for group in index.get("blockers", {}).values()
		for item in group
	}
	approval_ref = index.get("architect_approval")
	if approval_ref is None:
		if index.get("status") != "blocked" or index.get("phase_gate_claim") is not False:
			errors.append("unapproved P0 index must remain blocked and make no phase-gate claim")
		if "P0-ARCHITECT" not in blocker_ids:
			errors.append("unapproved P0 index is missing P0-ARCHITECT")
	else:
		if index.get("status") != "pass" or index.get("phase_gate_claim") is not True:
			errors.append("approved P0 index must be pass with phase_gate_claim=true")
		if index.get("blockers", {}).get("p0_ratification"):
			errors.append("approved P0 index retains a P0 ratification blocker")
		if index.get("blockers", {}).get("p0_executable_prerequisite"):
			errors.append("approved P0 index retains an executable-prerequisite blocker")
		if "P0-ARCHITECT" in blocker_ids:
			errors.append("approved P0 index retains P0-ARCHITECT")
		validate_architect_approval(index, approval_ref, errors)

	artifacts = index.get("artifacts", [])
	paths = [item.get("path") for item in artifacts]
	if len(paths) != len(set(paths)):
		errors.append("authoritative P0 index contains duplicate artifact paths")
	required = {
		"docs/architecture/origin-orbis-runtime-alignment.csv",
		"docs/architecture/contract-to-native-migration.csv",
		"docs/evidence/source-ledger.csv",
		"docs/evidence/p0-contract-native/evidence-index.json",
		"docs/evidence/p0-contract-native/repository-scope.report.json",
		"docs/evidence/verification/p0/index.json",
		"docs/evidence/verification/p0/contracts-report.json",
		"docs/sdk/compatibility-manifest.json",
		"docs/sdk/contract-to-native-map.json",
		"docs/sdk/signed-extension-manifest.json",
		"docs/sdk/host/fake-host-scenarios.json",
		"product-sdk/package.json",
		"product-sdk/package-lock.json",
		"product-sdk/packages/descriptors/generated/orbis-descriptor.json",
		"docs/evidence/performance/service-slo-manifest.json",
		"docs/evidence/performance/service-slo-manifest.schema.json",
		"docs/evidence/verification/p0/ratification-envelope.json",
		"docs/orbis-completion-manifest.toml",
		"origin-rs/src/product_sdk/contract.rs",
		"origin-rs/src/product_sdk/eqc.rs",
		"origin-rs/src/product_sdk/host.rs",
		"origin-rs/tests/product_sdk_p0.rs",
	}
	if approval_ref is not None:
		required.add("docs/evidence/p0/architect-approval.json")
	missing = sorted(required - set(paths))
	if missing:
		errors.append(f"authoritative P0 index misses required artifacts: {missing}")
	for item in artifacts:
		rel = item.get("path", "")
		path = ROOT / rel
		if not path.is_file():
			errors.append(f"indexed artifact is missing: {rel}")
			continue
		if sha256(path) != item.get("sha256"):
			errors.append(f"indexed artifact hash drift: {rel}")

	for rel in index.get("json_artifacts", []):
		try:
			json.loads((ROOT / rel).read_text())
		except Exception as error:
			errors.append(f"invalid JSON {rel}: {error}")
	for rel in index.get("csv_artifacts", []):
		try:
			with (ROOT / rel).open(newline="") as source:
				rows = list(csv.DictReader(source))
			if not rows or None in rows[0]:
				raise ValueError("empty CSV or malformed columns")
		except Exception as error:
			errors.append(f"invalid CSV {rel}: {error}")
	try:
		repository_scope = json.loads(
			(ROOT / "docs/evidence/p0-contract-native/repository-scope.report.json").read_text()
		)
		if repository_scope.get("status") != "pass":
			errors.append("repository scope report is not pass")
		if repository_scope.get("external_modified") != 0:
			errors.append("repository scope external_modified is not zero")
	except Exception as error:
		errors.append(f"invalid repository scope report: {error}")

	commands = [
		([sys.executable, "docs/evidence/p0/validate-runtime-alignment.py"], (0,)),
		([sys.executable, "scripts/validate-contract-native-census.py"], (0, 2)),
		([sys.executable, "scripts/validate-p0-provenance.py"], (0, 2)),
		([sys.executable, "docs/evidence/verification/p0/validate_contracts.py"], (0,)),
		([
			"cargo",
			"test",
			"--manifest-path",
			"origin-rs/Cargo.toml",
			"--test",
			"product_sdk_p0",
		], (0,)),
	]
	for command, allowed in commands:
		run(command, errors, allowed)
	if args.runtime_test:
		run(
			[
				"cargo",
				"test",
				"-p",
				"origin-orbis-runtime",
				"tests::transaction_policy_construction_surfaces_share_the_frozen_slots",
				"--",
				"--exact",
			],
			errors,
		)

	if errors:
		for error in errors:
			print(f"ERROR: {error}")
		return 1
	gate = "CLEAR (production activation remains downstream BLOCKED)" if approval_ref else (
		"BLOCKED (see authoritative index blockers)"
	)
	print(f"PASS: {len(artifacts)} P0 artifacts are internally consistent; PHASE GATE: {gate}")
	return 0


def validate_architect_approval(index: dict, approval_ref: dict, errors: list[str]) -> None:
	"""Fail closed unless the independent Architect record binds the reviewed P0 bundle."""
	if approval_ref.get("path") != "docs/evidence/p0/architect-approval.json":
		errors.append("architect approval path is not canonical")
		return
	if not APPROVAL.is_file():
		errors.append("architect approval artifact is missing")
		return
	approval_hash = sha256(APPROVAL)
	if approval_ref.get("sha256") != approval_hash:
		errors.append("architect approval artifact hash drift")
	try:
		approval = json.loads(APPROVAL.read_text())
	except Exception as error:
		errors.append(f"invalid architect approval JSON: {error}")
		return

	expected_scalars = {
		"schema_version": 1,
		"scope": "p0-origin-orbis-foundation-integrated-architect-review",
		"review_scope": "cord-only-plan-scope-revalidation",
		"verdict": "CLEAR",
		"agent_role": "architect",
		"CODEX_THREAD_ID": REVALIDATION_REVIEWER,
		"review_thread_id": REVALIDATION_REVIEWER,
		"review_agent_task": "/root/p0_scope_revalidation_architect",
		"branch": "sm-update-sub-0x63",
		"head": "439a1b62da11175129ad5390186230a64d569810",
		"sdk_revision": "cc190ea83c590b6a14a6b9771ab02c81618dc118",
		"plan_path": ".omx/plans/origin-orbis-emerging-app-platform.md",
		"plan_sha256": "86a985809b36bc4645715fd673cd84fd99c93f5b0ab5df1e0b121a57058afa75",
		"ratification_payload_sha256": "18a7fe95a300632121d0f448cb7bf3e1bc0e6776e254bc26bc1437a5f19c790a",
		"production_activation_ready": False,
		"reviewed_top_index_sha256": sha256(REVALIDATION_CANDIDATE),
		"superseded_approval_sha256": SUPERSEDED_APPROVAL_SHA256,
		"superseded_plan_sha256": SUPERSEDED_PLAN_SHA256,
		"verification_index_sha256": sha256(ROOT / "docs/evidence/verification/p0/index.json"),
		"semantic_index_sha256": sha256(ROOT / "docs/evidence/p0-contract-native/evidence-index.json"),
		"runtime_alignment_sha256": sha256(
			ROOT / "docs/architecture/origin-orbis-runtime-alignment.csv"
		),
		"approval_validator_sha256": sha256(Path(__file__)),
	}
	for field, expected in expected_scalars.items():
		if approval.get(field) != expected:
			errors.append(f"architect approval {field} drift")
	expected_artifact_hashes = {
		"completion_manifest": sha256(ROOT / "docs/orbis-completion-manifest.toml"),
		"contract_census_index": sha256(
			ROOT / "docs/evidence/p0-contract-native/evidence-index.json"
		),
		"product_sdk_descriptor": sha256(
			ROOT / "product-sdk/packages/descriptors/generated/orbis-descriptor.json"
		),
		"ratification_envelope": sha256(
			ROOT / "docs/evidence/verification/p0/ratification-envelope.json"
		),
		"rust_product_contract": sha256(ROOT / "origin-rs/src/product_sdk/contract.rs"),
		"rust_product_eqc": sha256(ROOT / "origin-rs/src/product_sdk/eqc.rs"),
		"rust_product_host": sha256(ROOT / "origin-rs/src/product_sdk/host.rs"),
		"rust_product_tests": sha256(ROOT / "origin-rs/tests/product_sdk_p0.rs"),
		"semantic_approval": sha256(
			ROOT / "docs/evidence/p0-contract-native/architect-semantic-disposition-approval.json"
		),
		"slice2_gate": sha256(ROOT / "docs/evidence/orbis-v5/GATE-6-SLICE2-EVIDENCE.json"),
		"repository_scope": sha256(
			ROOT / "docs/evidence/p0-contract-native/repository-scope.report.json"
		),
		"source_ledger": sha256(ROOT / "docs/evidence/source-ledger.csv"),
		"provenance_generator": sha256(ROOT / "scripts/generate-p0-provenance.py"),
		"provenance_validator": sha256(ROOT / "scripts/validate-p0-provenance.py"),
		"provenance_scope_tests": sha256(ROOT / "scripts/test-p0-provenance-scope.py"),
	}
	if approval.get("reviewed_artifact_hashes") != expected_artifact_hashes:
		errors.append("architect approval reviewed artifact hashes drift")
	if not PLAN.is_file() or sha256(PLAN) != approval.get("plan_sha256"):
		errors.append("architect approval plan hash drift")
	if sha256(ROOT / "docs/evidence/verification/p0/p0-ratification.payload.json") != approval.get(
		"ratification_payload_sha256"
	):
		errors.append("architect approval ratification payload drift")
	if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", str(approval.get("reviewed_at"))):
		errors.append("architect approval reviewed_at is invalid")
	required_exclusions = {
		"no P1-P7 implementation approval",
		"no production activation",
		"no final genesis approval",
		"no performance campaign claim",
		"no legacy data migration or backward compatibility",
		"no final native-cutover cleanup verdict",
	}
	if set(approval.get("scope_exclusions", [])) != required_exclusions:
		errors.append("architect approval scope exclusions drift")
	commands = approval.get("commands", [])
	if len(commands) < 13 or any(
		item.get("exit_code") != 0 or item.get("result") != "PASS" for item in commands
	):
		errors.append("architect approval command evidence is incomplete")
	required_scope_commands = {
		"python3 scripts/validate-p0-provenance.py --workspace-root "
		+ str(ROOT.parent)
		+ " --scope-only",
		"python3 scripts/test-p0-provenance-scope.py --source-workspace " + str(ROOT.parent),
		"python3 docs/evidence/p0/validate-p0.py",
	}
	command_names = {item.get("command") for item in commands}
	if not required_scope_commands.issubset(command_names):
		errors.append("architect approval is missing CORD-only scope revalidation commands")
	invariants = approval.get("verified_invariants", {})
	expected_invariants = {
		"alignment_rows": 207,
		"source_ledger_rows": 91,
		"contract_census_rows": 132,
		"semantic_design_entries": 2780,
		"semantic_design_coverage_percent": 100.0,
		"ratification_signatures": 5,
		"origin_spec_version": 9901,
		"orbis_spec_version": 29,
		"orbis_transaction_version": 8,
		"orbis_para_id": 1006,
		"block_processing_velocity": 3,
		"unincluded_segment_capacity": 12,
	}
	if invariants != expected_invariants:
		errors.append("architect approval invariant summary drift")


if __name__ == "__main__":
	raise SystemExit(main())
