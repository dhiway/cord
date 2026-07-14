// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { runFestivalJourney } from "./journey.ts";
import { validateMobileContractHarness } from "./mobile-contract-harness.ts";

const evidence = (name: string): any => JSON.parse(readFileSync(
  resolve(import.meta.dirname, `../../../docs/evidence/verification/p6/${name}`),
  "utf8",
));

test("Festival journey uses sealed typed routes and exercises required failure boundaries", async () => {
  const report = await runFestivalJourney();
  assert.deepEqual(report, evidence("festival-journey.report.json"));
  assert.equal(report.status, "PASS");
  assert.equal(report.journey_acceptance, true);
  assert.equal(report.p6_acceptance, false);
  assert.deepEqual(report.native_only, {
    raw_scale: false,
    contract_abi: false,
    pallet_indices: false,
    call_indices: false,
  });
  assert.equal(report.signer_boundaries.distinct_chain_signers, true);
  assert.deepEqual(report.deferred, {
    live_chain: true,
    production_finality: true,
    slo: true,
    production_mobile_rewrite: true,
  });
});

test("iOS and Android manifests have exact request consent network error and event parity", async () => {
  const report = await validateMobileContractHarness();
  assert.deepEqual(report, evidence("festival-mobile-contract-parity.report.json"));
  assert.equal(report.status, "PASS");
  assert.equal(report.journey_acceptance, true);
  assert.equal(report.p6_acceptance, false);
  assert.equal(report.vector_count, 14);
  assert.equal(report.host_harness_outcome_parity, true);
});
