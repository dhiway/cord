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
import test from "node:test";
import { runFestivalJourney } from "./journey.ts";
import { validateMobileAppProjection } from "./mobile-app-projection.ts";

test("Festival journey uses sealed typed routes and exercises required failure boundaries", async () => {
  const report = await runFestivalJourney();
  assert.deepEqual(report, await runFestivalJourney());
  assert.equal(report.status, "PASS");
  assert.equal(report.journey_acceptance, true);
  assert.equal(report.p6_acceptance, true);
  const developer = report.developer_flow as any;
  assert.equal(developer.status, "PASS");
  assert.deepEqual(developer.identity_operations, [
    "identity.subject.derive", "identity.entitlements.read", "transaction.sign",
  ]);
  assert.equal(developer.results.provider_unavailable.retryable, true);
  assert.equal(developer.results.object_write_recovered.code, "success");
  assert.deepEqual(report.native_only, {
    raw_scale: false,
    contract_abi: false,
    pallet_indices: false,
    call_indices: false,
  });
  assert.equal((report.signer_boundaries as any).distinct_chain_signers, true);
  assert.deepEqual(report.deferred, {
    live_chain: true,
    production_finality: true,
    slo: true,
    production_mobile_rewrite: true,
  });
});

test("iOS and Android projections cover the complete createApp journey", async () => {
  const report = await validateMobileAppProjection();
  assert.deepEqual(report, await validateMobileAppProjection());
  assert.equal(report.status, "PASS");
  assert.equal(report.journey_acceptance, true);
  assert.equal(report.p6_acceptance, true);
  assert.equal(report.vector_count, 29);
  assert.equal(report.app_projection_outcome_parity, true);
});
