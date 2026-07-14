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
