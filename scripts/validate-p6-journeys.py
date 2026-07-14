#!/usr/bin/env python3
"""Validate the deterministic P6 enterprise and Festival journey harnesses."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/evidence/verification/p6"
RAW = EVIDENCE / "raw"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def run(name: str, command: list[str], env: dict[str, str] | None = None) -> None:
    process = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    RAW.mkdir(parents=True, exist_ok=True)
    (RAW / f"{name}.log").write_text(
        f"$ {' '.join(command)}\nexit={process.returncode}\n{process.stdout.rstrip()}\n"
    )
    if process.returncode:
        raise SystemExit(f"{name} failed; see {relative(RAW / f'{name}.log')}")


def main() -> None:
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["SKIP_WASM_BUILD"] = "1"
    lifecycle = (
        "enterprise_journey::"
        "enterprise_identity_attestation_name_and_storage_lifecycle_is_native_and_fail_closed"
    )
    sponsored = (
        "enterprise_journey::"
        "enterprise_sponsored_meta_boundaries_reject_exhaustion_and_version_drift"
    )
    run(
        "enterprise-lifecycle",
        [
            "cargo", "test", "-p", "origin-commons-runtime", lifecycle,
            "--lib", "--locked", "--", "--exact",
        ],
        environment,
    )
    run(
        "enterprise-sponsored-boundaries",
        [
            "cargo", "test", "-p", "origin-commons-runtime", sponsored,
            "--lib", "--locked", "--", "--exact",
        ],
        environment,
    )
    run(
        "enterprise-provider-read-reopen",
        [
            "cargo", "test", "-p", "origin-orbis-provider",
            "storage::tests::commit_read_proof_delete_and_reopen",
            "--locked", "--", "--exact",
        ],
        environment,
    )
    run(
        "enterprise-content-failover",
        [
            "node", "--experimental-strip-types", "--test",
            "--test-name-pattern=gateway and Bitswap sources fail over in exact caller order and verify bytes",
            "product-sdk/tests/content/content.test.ts",
        ],
    )
    run(
        "festival-report",
        [
            "node", "--experimental-strip-types",
            "product-sdk/examples/festival/journey.ts", "--write",
        ],
    )
    run(
        "festival-ios-contract-report",
        [
            "node", "--experimental-strip-types",
            "product-sdk/examples/festival/ios-contract-harness.ts", "--write",
        ],
    )
    run(
        "festival-android-contract-report",
        [
            "node", "--experimental-strip-types",
            "product-sdk/examples/festival/android-contract-harness.ts", "--write",
        ],
    )
    run(
        "festival-mobile-report",
        [
            "node", "--experimental-strip-types",
            "product-sdk/examples/festival/mobile-contract-harness.ts", "--write",
        ],
    )
    run(
        "festival-contract-tests",
        [
            "node", "--experimental-strip-types", "--test",
            "product-sdk/examples/festival/journey.test.ts",
        ],
    )

    festival_path = EVIDENCE / "festival-journey.report.json"
    ios_path = EVIDENCE / "festival-ios-contract-parity.report.json"
    android_path = EVIDENCE / "festival-android-contract-parity.report.json"
    mobile_path = EVIDENCE / "festival-mobile-contract-parity.report.json"
    festival = json.loads(festival_path.read_text())
    ios = json.loads(ios_path.read_text())
    android = json.loads(android_path.read_text())
    mobile = json.loads(mobile_path.read_text())
    assert festival["status"] == "PASS" and festival["journey_acceptance"] is True
    assert ios["status"] == "PASS" and ios["platform"] == "ios"
    assert android["status"] == "PASS" and android["platform"] == "android"
    assert mobile["status"] == "PASS" and mobile["journey_acceptance"] is True
    assert all(report["p6_acceptance"] is False for report in [festival, ios, android, mobile])

    enterprise_path = EVIDENCE / "enterprise-journey.report.json"
    enterprise = {
        "schema": "cord.enterprise-journey-report.v1",
        "status": "PASS",
        "journey_acceptance": True,
        "p6_acceptance": False,
        "runtime": "origin-commons-runtime",
        "source": "origin/orbis/runtime/src/enterprise_journey.rs",
        "assertions": {
            "identity_judgement": "native People registrar judgement",
            "subject": "native Entity subject identifier",
            "attestation": "native schema, issue, live check, revoke and deny",
            "name": "native Orbis Names commit, register and attestation/content resolution",
            "content": "Bulletin commitment/provenance plus Product SDK ordered gateway-to-Bitswap failover with CID byte verification",
            "provider": "inactive-provider rejection, active-provider agreement/checkpoint, and DiskStore committed-byte read/reopen proof",
            "application_storage": "native Drive and S3 records bind the verified content hash",
            "sponsorship": "successful paid outer MetaTx executes native inner attestation and advances participant/sponsor nonces; exhaustion and signed version drift reject",
        },
        "native_only": {
            "raw_scale_product_api": False,
            "contract_abi": False,
            "migrated_domain_revive": False,
            "legacy_state": False,
        },
        "production_evidence_deferred": [
            "Live candidate topology execution and multi-provider network failover remain deferred.",
            "Final E/Q/C SLO and storage-headroom campaigns run after deterministic journey closure.",
            "Production launch approval remains unsigned and blocked.",
        ],
    }
    write_json(enterprise_path, enterprise)

    journeys_path = EVIDENCE / "journeys.json"
    journeys = {
        "schema": "cord.p6-journeys.v1",
        "status": "PASS",
        "journey_acceptance": True,
        "p6_acceptance": False,
        "enterprise": {
            "report": relative(enterprise_path),
            "sha256": sha256(enterprise_path),
        },
        "festival": {
            "report": relative(festival_path),
            "sha256": sha256(festival_path),
        },
        "ios_contract_harness": {
            "report": relative(ios_path),
            "sha256": sha256(ios_path),
        },
        "android_contract_harness": {
            "report": relative(android_path),
            "sha256": sha256(android_path),
        },
        "mobile_contract_parity": {
            "report": relative(mobile_path),
            "sha256": sha256(mobile_path),
        },
        "required_failures": [
            "permission_denial", "cancellation", "sponsor_exhaustion", "version_drift",
            "provider_unavailable", "offline", "reconnect", "replay", "revocation",
            "sponsored_replay", "tampered_sponsored_intent",
        ],
        "remaining_p6_gates": [
            "final mixed E/Q/C campaign and storage/resource headroom",
            "clean-developer diagnosis of at least three injected failures",
            "independent P6 verification and adoption acceptance",
        ],
        "production_app_claim": False,
    }
    write_json(journeys_path, journeys)

    tracked_inputs = [
        ROOT / "scripts/validate-p6-journeys.py",
        ROOT / "origin/orbis/runtime/src/lib.rs",
        ROOT / "origin/orbis/runtime/src/enterprise_journey.rs",
        ROOT / "origin/orbis/provider-node/src/storage.rs",
        ROOT / "product-sdk/examples/festival/journey.ts",
        ROOT / "product-sdk/examples/festival/journey.test.ts",
        ROOT / "product-sdk/examples/festival/mobile-contract-harness.ts",
        ROOT / "product-sdk/examples/festival/ios-contract-harness.ts",
        ROOT / "product-sdk/examples/festival/android-contract-harness.ts",
        ROOT / "product-sdk/examples/festival/mobile-contract-vectors.json",
        ROOT / "product-sdk/examples/festival/ios-contract-harness.manifest.json",
        ROOT / "product-sdk/examples/festival/android-contract-harness.manifest.json",
        ROOT / "product-sdk/examples/festival/p6-journey.manifest.json",
        ROOT / "docs/sdk/native-route-contract.json",
        ROOT / "product-sdk/tests/content/content.test.ts",
    ]
    artifacts = [
        enterprise_path, festival_path, ios_path, android_path, mobile_path, journeys_path,
        *sorted(RAW.glob("*.log")),
    ]
    index = {
        "schema": "cord.p6-journey-evidence-index.v1",
        "command": "python3 scripts/validate-p6-journeys.py",
        "status": "PASS",
        "journey_acceptance": True,
        "p6_acceptance": False,
        "inputs": [
            {"path": relative(path), "sha256": sha256(path)} for path in tracked_inputs
        ],
        "artifacts": [
            {"path": relative(path), "bytes": path.stat().st_size, "sha256": sha256(path)}
            for path in artifacts
        ],
        "stop_condition": "journeys are feature-complete; production/SLO gates remain deferred",
    }
    write_json(EVIDENCE / "index.json", index)
    print(json.dumps({
        "status": "PASS",
        "enterprise": "PASS",
        "festival": "PASS",
        "mobile": "PASS",
        "p6_acceptance": False,
    }, sort_keys=True))


if __name__ == "__main__":
    main()
