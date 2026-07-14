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

"""Fail-closed checks for the isolated Origin GRANDPA tuning decision."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/evidence/performance/p1-grandpa-tuning-decision.json"
EVIDENCE_SHA = ROOT / "docs/evidence/performance/p1-grandpa-tuning-decision.sha256"
SDK_PATCH = ROOT / ".omx/worktrees/sdk-grandpa-voting-policy.patch"
CORD_PATCH = ROOT / ".omx/worktrees/cord-origin-grandpa-n1.patch"
RAW = ROOT / "docs/evidence/performance/p1-elastic-lag-diagnostic-attempt1-failed/diagnostic-1-core-1/finality-lag-samples.jsonl"
LOGS = ROOT / "docs/evidence/performance/p1-elastic-lag-diagnostic-attempt1-failed/diagnostic-1-core-1/node-logs"

SDK_PATHS = {
    "cumulus/client/relay-chain-inprocess-interface/src/lib.rs",
    "polkadot/cli/src/command.rs",
    "polkadot/node/service/src/builder/mod.rs",
    "polkadot/node/test/service/src/lib.rs",
    "polkadot/parachain/test-parachains/adder/collator/src/main.rs",
    "polkadot/parachain/test-parachains/undying/collator/src/main.rs",
}
CORD_PATHS = {"origin/base/cli/src/command.rs"}
RELAY_NODES = ("alice", "bob", "charlie", "dave", "eve", "fredie")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def patch_paths(text: str) -> set[str]:
    return set(re.findall(r"^diff --git a/(\S+) b/\S+$", text, flags=re.MULTILINE))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> None:
    evidence = json.loads(EVIDENCE.read_text())
    sdk = SDK_PATCH.read_text()
    cord = CORD_PATCH.read_text()

    require(EVIDENCE_SHA.read_text().split()[0] == digest(EVIDENCE), "decision evidence digest drift")
    require(digest(SDK_PATCH) == evidence["patches"]["sdk_grandpa_voting_policy"]["sha256"], "SDK patch digest drift")
    require(digest(CORD_PATCH) == evidence["patches"]["cord_origin_n1"]["sha256"], "CORD patch digest drift")
    require(patch_paths(sdk) == SDK_PATHS, "SDK patch path expansion")
    require(patch_paths(cord) == CORD_PATHS, "CORD patch path expansion")
    require("DEFAULT_BEFORE_BEST_BLOCK_BY: u32 = 2" in sdk, "SDK default is not exactly 2")
    require("MIN_BEFORE_BEST_BLOCK_BY: u32 = 1" in sdk, "SDK minimum is not exactly 1")
    require("MAX_BEFORE_BEST_BLOCK_BY: u32 = 32" in sdk, "SDK maximum is not exactly 32")
    require("ThreeQuartersOfTheUnfinalizedChain" in sdk, "three-quarters rule removed")
    require(sdk.count("grandpa_voting_rule: Default::default()") == 5, "not all non-builder call sites preserve the default")
    require("GrandpaVotingRuleConfig::new(1)" in cord, "Origin does not explicitly select n=1")
    require(not any("/runtime/" in p or "/primitives/" in p or p.endswith(("Cargo.toml", "Cargo.lock")) for p in SDK_PATHS | CORD_PATHS), "runtime/primitive/dependency change present")

    require(digest(RAW) == evidence["diagnostic"]["raw_sha256"], "raw diagnostic digest drift")
    rows = [json.loads(line) for line in RAW.read_text().splitlines() if line]
    breaches = [row for row in rows if row["phase"] == "measurement" and row["orbis_finality_lag_blocks"] > 15]
    require(len(rows) == 158 and len(breaches) == 58, "diagnostic sample boundary drift")
    require(max(row["orbis_finality_lag_blocks"] for row in rows) == 18, "diagnostic maximum drift")
    require({row["relay_best"] - row["relay_finalized"] for row in breaches} == {3}, "breach/relay correlation drift")

    orbis_alice = (LOGS / "orbis-alice.log").read_text(errors="replace")
    require(orbis_alice.count("Ran out of free WASM instances") == 85, "WASM pool exhaustion count drift")
    for node in RELAY_NODES:
        log = (LOGS / f"{node}.log").read_text(errors="replace")
        require("does not meet the minimal requirements" in log, f"missing hardware failure for {node}")

    require(evidence["campaign_executed"] is False and evidence["performance_claim"] is False, "diagnostic was promoted to a claim")
    require(evidence["probe_sequence"][0]["name"] == "topology-hygiene-only", "probe order drift")
    require(evidence["probe_sequence"][1]["precondition"] == "probe 1 breached", "n=1 probe is not conditional")
    print("grandpa voting policy patch boundary: PASS")


if __name__ == "__main__":
    main()
