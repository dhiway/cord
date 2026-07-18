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

"""Hostile tests for the executable P6 observability contract."""

from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path


CORD = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "p6_observability", CORD / "scripts/validate-p6-observability.py"
)
assert SPEC and SPEC.loader
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)
CONTRACT = json.loads(
    (CORD / "docs/operations/origin-orbis-p6-observability.json").read_text()
)
SCENARIOS = json.loads(
    (CORD / "docs/operations/origin-orbis-p6-observability.scenarios.json").read_text()
)
RUNBOOK = (CORD / "docs/operations/origin-orbis-p6-incidents.md").read_text()
PROVIDER_OUTCOMES = {
    "schema": "cord.p1-provider-typed-outcomes.v1",
    "status": "pass",
    "provider_count": 3,
    "active_provider_count": 3,
    "typed_outcome_count": 3,
    "failure_count": 0,
    "typed_outcomes": [
        {
            "phase": "baseline",
            "provider": f"provider-{index}",
            "state": "REPLICA_REPAIRED",
            "action": "resume_verified_reads",
            "retryable": False,
            "byte_plane_ready": True,
            "redacted_counts": {
                "installed_objects": 1,
                "ready_objects": 1,
                "quarantined_objects": 0,
                "duties": 1,
                "initiator_duties": 1,
                "failover_duties": 0,
                "promotion_pending_duties": 0,
                "blocked_duties": 0,
            },
        }
        for index in range(1, 4)
    ],
}


class P6ObservabilityTests(unittest.TestCase):
    def test_current_contract_and_scenarios_pass(self) -> None:
        self.assertEqual(VALIDATOR.contract_failures(CONTRACT, RUNBOOK), [])
        self.assertEqual(VALIDATOR.scenario_failures(CONTRACT, SCENARIOS), [])

    def test_thresholds_are_strict_at_every_boundary(self) -> None:
        healthy = next(row for row in SCENARIOS["scenarios"] if row["id"] == "healthy-boundaries")
        self.assertEqual(VALIDATOR.evaluate_alerts(CONTRACT, healthy), [])
        for code in VALIDATOR.REQUIRED_ALERTS:
            self.assertTrue(any(
                code in VALIDATOR.evaluate_alerts(CONTRACT, row)
                for row in SCENARIOS["scenarios"]
            ), code)

    def test_secret_label_and_threshold_drift_fail_closed(self) -> None:
        contract = copy.deepcopy(CONTRACT)
        contract["metrics"][0]["labels"].append("subject")
        contract["alerts"][0]["threshold"] = 121
        failures = VALIDATOR.contract_failures(contract, RUNBOOK)
        self.assertTrue(any("labels" in failure for failure in failures))
        self.assertTrue(any("threshold" in failure for failure in failures))

    def test_missing_expected_alert_is_detected(self) -> None:
        scenarios = copy.deepcopy(SCENARIOS)
        scenarios["scenarios"][1]["expected_alerts"] = []
        self.assertTrue(VALIDATOR.scenario_failures(CONTRACT, scenarios))

    def test_provider_sources_use_only_typed_redacted_stderr(self) -> None:
        typed, redaction = VALIDATOR.source_failures(CORD)
        self.assertEqual(typed, [])
        self.assertEqual(redaction, [])

    def test_provider_outcomes_are_typed_and_redacted(self) -> None:
        self.assertEqual(VALIDATOR.provider_outcome_failures(PROVIDER_OUTCOMES), [])
        unredacted = copy.deepcopy(PROVIDER_OUTCOMES)
        unredacted["typed_outcomes"][0]["redacted_counts"] = {"cid": 1}
        self.assertTrue(VALIDATOR.provider_outcome_failures(unredacted))

    def test_ac11_and_legacy_generator_claims_are_honest(self) -> None:
        try:
            import tomllib
        except ModuleNotFoundError:
            import tomli as tomllib
        registry = tomllib.loads((CORD / "docs/specs/evidence-gates-v1.toml").read_text())
        self.assertEqual(VALIDATOR.evidence_claim_failures(CORD, registry), [])

    def test_ac11_rejects_legacy_generator_in_any_command_surface(self) -> None:
        try:
            import tomllib
        except ModuleNotFoundError:
            import tomli as tomllib
        registry = tomllib.loads((CORD / "docs/specs/evidence-gates-v1.toml").read_text())
        for field in ("argv", "inputs", "outputs"):
            promoted = copy.deepcopy(registry)
            ac11 = next(gate for gate in promoted["gate"] if gate["id"] == "AC11")
            ac11["command"][0][field].append("scripts/validate-p6-journeys.py")
            self.assertTrue(VALIDATOR.evidence_claim_failures(CORD, promoted), field)


if __name__ == "__main__":
    unittest.main()
