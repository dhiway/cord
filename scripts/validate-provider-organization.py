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

"""Validate the source-backed AC4 provider-organization contract.

The accompanying focused pallet test exercises atomic rejection and the ordered
rotation event.  This validator deliberately checks the actual pallet and test
sources rather than treating a successful filtered Cargo invocation as proof:
the former registry referred to a test name which did not exist.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any


INVALID_AUTHORITY_ERRORS = (
    "OrganizationUnknown",
    "AttestationInvalid",
    "AttestationExpired",
    "SlaInvalid",
    "ServiceKeyInvalid",
)
PALLET = Path("origin/orbis/pallets/storage-provider/src/lib.rs")
TESTS = Path("origin/orbis/pallets/storage-provider/src/tests.rs")
CONTRACT_TEST = "provider_organization_contract_rejects_atomically_and_emits_ordered_rotation"
ADMISSION_TEST = "organization_admission_rejects_every_invalid_authority_case_without_state"


def root() -> Path:
    return Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as target:
            json.dump(value, target, indent=2, sort_keys=True)
            target.write("\n")
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    repository = root()
    pallet_path, tests_path = repository / PALLET, repository / TESTS
    pallet = pallet_path.read_text(encoding="utf-8")
    tests = tests_path.read_text(encoding="utf-8")
    invalid_failures: list[str] = []
    event_failures: list[str] = []
    humanity_reads: list[str] = []
    hash_failures: list[str] = []

    for name in INVALID_AUTHORITY_ERRORS:
        if f"ProviderAuthorityError::{name}" not in pallet:
            invalid_failures.append(f"pallet does not map ProviderAuthorityError::{name}")
        if f"ProviderAuthorityError::{name}" not in tests:
            invalid_failures.append(f"admission test does not cover ProviderAuthorityError::{name}")
    for marker in (CONTRACT_TEST, ADMISSION_TEST, "assert_noop!", "Providers::<Test>::get(1)"):
        if marker not in tests:
            invalid_failures.append(f"missing atomic invalid-case test marker: {marker}")

    for marker in (
        "pub fn rotate_provider_organization(",
        "organization.rotation_predecessor == Some(predecessor)",
        "Event::ProviderOrganizationRotated { provider, predecessor }",
    ):
        if marker not in pallet:
            event_failures.append(f"missing organization rotation invariant: {marker}")
    for marker in (
        "System::events().len()",
        "ProviderOrganizationRotated",
        "events_before + 1",
    ):
        if marker not in tests:
            event_failures.append(f"rotation test does not prove ordered event marker: {marker}")

    # Commons authority is injected through ProviderAuthority.  The storage pallet
    # must not import or call a humanity/personhood runtime surface directly.
    prohibited = ("Humanity", "Personhood", "PeopleLite", "pallet_humanity", "pallet_personhood")
    for token in prohibited:
        if token in pallet:
            humanity_reads.append(token)
    if "T::ProviderAuthority::validate" not in pallet:
        humanity_reads.append("missing injected ProviderAuthority validation boundary")

    evidence_paths = {str(PALLET): sha256(pallet_path), str(TESTS): sha256(tests_path)}
    if any(len(value) != 64 or set(value) - set("0123456789abcdef") for value in evidence_paths.values()):
        hash_failures.append("invalid SHA-256 evidence encoding")
    if "T::Hashing::hash_of(&record.organization)" not in pallet:
        hash_failures.append("rotation predecessor is not derived from the prior organization hash")

    report: dict[str, Any] = {
        "schema_version": 1,
        "invalid_case_failures": len(invalid_failures),
        "event_order_matches": not event_failures,
        "humanity_reads": len(humanity_reads),
        "evidence_hash_failures": len(hash_failures),
        "invalid_case_failure_details": invalid_failures,
        "event_order_failure_details": event_failures,
        "humanity_read_details": humanity_reads,
        "evidence_hash_failure_details": hash_failures,
        "evidence_sha256": evidence_paths,
        "contract_test": CONTRACT_TEST,
        "authority_boundary": "ProviderAuthority",
    }
    output = args.out if args.out.is_absolute() else repository / args.out
    atomic_json(output, report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if invalid_failures or event_failures or humanity_reads or hash_failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
