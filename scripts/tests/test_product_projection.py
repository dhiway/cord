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

"""Hostile tests for unified Identity product projection and internal disposition."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


CORD = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "product_projection", CORD / "scripts/validate-product-projection.py"
)
assert SPEC and SPEC.loader
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class ProductProjectionTests(unittest.TestCase):
    def test_identity_authority_evidence_binds_durable_replay_state_and_restart_test(self) -> None:
        authority = (
            CORD / "origin-rs/src/product_sdk/host_v2/identity_authority.rs"
        ).read_text()
        self.assertEqual(
            VALIDATOR.identity_authority_evidence(authority)["same_store_replay_failures"], 0
        )
        for marker in (
            "challenges: BTreeSet<[u8; 32]>",
            "state.challenges.insert(challenge);",
            "if let Err(error) = self.persist(&next)",
            "fn signing_lost_success_and_identity_consent_replay_survive_restart()",
        ):
            with self.subTest(marker=marker):
                tampered = authority.replace(marker, "REMOVED_MARKER", 1)
                self.assertGreater(
                    VALIDATOR.identity_authority_evidence(tampered)[
                        "same_store_replay_failures"
                    ],
                    0,
                )

    def test_identity_authority_evidence_binds_non_live_recovery_assignments_and_tests(self) -> None:
        authority = (
            CORD / "origin-rs/src/product_sdk/host_v2/identity_authority.rs"
        ).read_text()
        evidence = VALIDATOR.identity_authority_evidence(authority)
        self.assertEqual(evidence["recovery_failures"], 0)
        self.assertEqual(evidence["non_live_continuity_true"], 0)
        for marker in (
            "state.epoch = 0;",
            "state.continuity = false;",
            "fn unproven_recovery_is_fresh_idempotent_and_restart_safe_without_grant_inheritance()",
            "fn entropy_failure_persists_a_fail_closed_recovery_barrier_and_retry_can_finish()",
        ):
            with self.subTest(marker=marker):
                tampered = authority.replace(marker, "REMOVED_MARKER", 1)
                tampered_evidence = VALIDATOR.identity_authority_evidence(tampered)
                self.assertGreater(
                    tampered_evidence["recovery_failures"]
                    + tampered_evidence["non_live_continuity_true"],
                    0,
                )

    def test_ratified_internal_disposition_has_no_public_projection(self) -> None:
        report = VALIDATOR.disposition_report(
            CORD, CORD / "docs/specs/web3-storage-disposition-v1.toml"
        )
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["public_old_taxonomy_count"], 0)
        self.assertEqual(report["internal_projection_count"], 0)
        self.assertEqual(report["metadata_filter_shim_count"], 0)
        self.assertGreater(report["distinct_invariant_count"], 0)

    def test_old_taxonomy_in_public_file_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "public.ts"
            path.write_text('export const capability = "personhood.read";\n', encoding="utf-8")
            findings = VALIDATOR.line_findings(root, [path], VALIDATOR.OLD_TAXONOMY)
        self.assertEqual(len(findings), 1)

    def test_metadata_string_hiding_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "origin-rs/src/hidden.rs"
            path.parent.mkdir(parents=True)
            path.write_text('const PALLET: &str = concat!("People", "Lite");\n', encoding="utf-8")
            findings = VALIDATOR.metadata_shims(root)
        self.assertEqual(len(findings), 1)

    def test_delete_branch_cannot_reuse_internal_retain_ledger(self) -> None:
        source = (CORD / "docs/specs/web3-storage-disposition-v1.toml").read_text()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".toml") as ledger:
            ledger.write(source.replace('decision = "internal-retain"', 'decision = "delete"'))
            ledger.flush()
            errors, _ = VALIDATOR.validate_disposition(CORD, Path(ledger.name))
        self.assertTrue(any("internal-retain" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
