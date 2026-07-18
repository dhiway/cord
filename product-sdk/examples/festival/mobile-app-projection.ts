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
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runFestivalJourney } from "./journey.ts";

const readJson = (path: string): any => JSON.parse(readFileSync(resolve(import.meta.dirname, path), "utf8"));
const digest = (path: string): string => createHash("sha256")
  .update(readFileSync(resolve(import.meta.dirname, path)))
  .digest("hex");

export type MobilePlatform = "ios" | "android";

export async function validatePlatformAppProjection(
  platform: MobilePlatform,
): Promise<Record<string, unknown>> {
  const manifest = readJson(`${platform}-app-projection.manifest.json`);
  const vectors = readJson("mobile-app-vectors.json");
  const hostSchema = readJson("../../../docs/sdk/host/host-request.schema.json");
  const errorSchema = readJson("../../../docs/sdk/native-error.schema.json");
  const routeContract = readJson("../../../docs/sdk/native-route-contract.json");
  const hostV2Registry = readJson("../../../docs/specs/origin-host-registry-v2.operations.json");
  const journey = await runFestivalJourney() as any;

  assert.equal(manifest.schema, "cord.mobile-app-projection.v2");
  assert.equal(manifest.platform, platform);
  assert.equal(manifest.implementation_scope, "app-projection-vectors-only");
  assert.equal(manifest.production_rewrite, false);
  assert.equal(manifest.transport_policy, "origin-sdk-create-app+typed-native-routes");
  assert.equal(manifest.raw_scale, false);
  assert.equal(manifest.contract_abi, false);
  assert.equal(manifest.pallet_or_call_indices, false);

  assert.deepEqual([...manifest.required_request_fields].sort(), [...hostSchema.required].sort());
  assert.deepEqual(
    [...manifest.required_consent_fields].sort(),
    [...hostSchema.properties.consent.required].sort(),
  );
  assert.deepEqual(
    [...manifest.required_network_fields].sort(),
    [...hostSchema.properties.network.required].sort(),
  );
  assert.deepEqual(
    [...manifest.required_error_fields].sort(),
    [...errorSchema.required].sort(),
  );
  assert.deepEqual(manifest.required_event_fields, ["finalized_block_hash", "event_index", "event"]);

  const routeIds = new Set(routeContract.routes.map((route: any) => route.id));
  const hostV2Operations = new Set(hostV2Registry.operations.map((operation: any) => operation.name));
  for (const vector of vectors.vectors) {
    if (vector.surface === "native-route") assert.ok(routeIds.has(vector.route), vector.id);
    else {
      assert.equal(vector.surface, "origin-app", vector.id);
      assert.match(vector.operation, /^app\.(?:identity|signing|names|cloudStorage)\./, vector.id);
      if (vector.operation.startsWith("app.identity.")) assert.ok(hostV2Operations.has(
        vector.operation.endsWith("subjectDerive") ? "identity.subject.derive" : "identity.entitlements.read",
      ));
      if (vector.operation === "app.signing.signTransaction") {
        assert.ok(hostV2Operations.has("transaction.sign"));
      }
    }
  }
  const capabilities = [...new Set(routeContract.routes.map((route: any) => route.capability))].sort();
  const identityRoutes = routeContract.routes.filter((route: any) => route.capability === "identity");
  const sponsoredRoutes = [...routeIds].filter((id: any) => /sponsor|meta.?tx/i.test(id));
  assert.deepEqual(capabilities, ["attestation", "names", "storage", "transaction"]);
  assert.equal(identityRoutes.length, 0);
  assert.equal(sponsoredRoutes.length, 2);
  assert.deepEqual(vectors.forbidden, {
    raw_scale: false,
    abi: false,
    pallet_indices: false,
    call_indices: false,
  });

  const resultKey: Record<string, string> = {
    "scoped-permission-denial": "permission_denial",
    "participant-dot-registration": "dot_register",
    "sponsored-check-in": "sponsored_check_in",
    "sponsored-replay": "sponsored_replay",
    "tampered-sponsored-intent": "tampered_sponsored_intent",
    "sponsor-budget-exhaustion": "sponsor_budget_exhaustion",
    offline: "offline",
    "cancelled-submission": "cancelled",
    reconnect: "offline_reconnect",
    replay: "offline_replay",
    "version-drift": "version_drift",
    "revoked-credential-deny": "revoked_sponsored_denial",
    "revoked-permission-deny": "permission_revoked",
  };
  for (const vector of vectors.vectors) {
    const result = vector.surface === "origin-app"
      ? journey.developer_flow.results[vector.result]
      : journey.results[resultKey[vector.id]];
    assert.ok(result, vector.id);
    assert.equal(result.code, vector.expected.terminal, vector.id);
    if (vector.expected.retryable !== undefined) {
      assert.equal(result.retryable, vector.expected.retryable, vector.id);
    }
    if (vector.expected.finality === "finalized") {
      assert.match(result.finalized_hash, /^0x[0-9a-f]{64}$/);
    }
    if (vector.expected.response) {
      for (const [key, value] of Object.entries(vector.expected.response)) {
        assert.equal(result.response?.[key], value, `${vector.id}.${key}`);
      }
    }
    for (const expectation of [
      "chain_signer",
      "event",
      "meta_tx_event",
      "inner_result",
      "new_request_and_consent",
    ]) {
      if (vector.expected[expectation] !== undefined) {
        assert.equal(
          journey.vector_observations?.[vector.id]?.[expectation],
          vector.expected[expectation],
          `${vector.id}.${expectation}`,
        );
      }
    }
  }
  assert.equal(journey.signer_boundaries.distinct_chain_signers, true);
  assert.deepEqual(
    journey.finalized_events.map((entry: any) => entry.event.event),
    ["name_registered", "sponsored_check_in", "attestation_revoked"],
  );

  return {
    schema: "cord.festival-platform-app-projection-report.v2",
    status: "PASS",
    journey_acceptance: true,
    p6_acceptance: true,
    scope: "ios-android-app-projection-only",
    platform,
    vector_count: vectors.vectors.length,
    request_schema_parity: true,
    consent_schema_parity: true,
    network_schema_parity: true,
    error_schema_parity: true,
    event_schema_parity: true,
    vector_routes_present_in_registry: true,
    app_projection_outcome_parity: true,
    qualification: "The projection covers one createApp journey across unified Identity, Names, Provider/Drive/S3, separate signing, and sealed sponsored routes. The private Host-v2 fixture verifies exact wire bytes. This is not a production mobile implementation or live-network claim.",
    production_evidence_deferred: [
      "Live-chain execution and production-finality observation.",
      "Final E/Q/C SLO and storage-headroom campaigns.",
      "Production Swift or Kotlin rewrites are explicit non-goals.",
    ],
    sealed_route_evidence: {
      capabilities,
      legacy_identity_routes: identityRoutes.length,
      sponsorship_or_metatx_routes: sponsoredRoutes.length,
    },
    inputs: {
      manifest_sha256: digest(`${platform}-app-projection.manifest.json`),
      vectors_sha256: digest("mobile-app-vectors.json"),
      host_request_schema_sha256: digest("../../../docs/sdk/host/host-request.schema.json"),
      native_error_schema_sha256: digest("../../../docs/sdk/native-error.schema.json"),
      native_route_contract_sha256: digest("../../../docs/sdk/native-route-contract.json"),
      host_v2_registry_sha256: digest("../../../docs/specs/origin-host-registry-v2.operations.json"),
    },
    native_only: {
      raw_scale: false,
      contract_abi: false,
      pallet_indices: false,
      call_indices: false,
    },
    deferred: {
      live_chain: true,
      production_finality: true,
      slo: true,
      production_mobile_rewrite: true,
    },
  };
}

export async function validateMobileAppProjection(): Promise<Record<string, unknown>> {
  const iosManifest = readJson("ios-app-projection.manifest.json");
  const androidManifest = readJson("android-app-projection.manifest.json");
  const withoutPlatform = ({ platform: _platform, ...value }: any) => value;
  assert.deepEqual(withoutPlatform(iosManifest), withoutPlatform(androidManifest));
  const ios = await validatePlatformAppProjection("ios");
  const android = await validatePlatformAppProjection("android");
  const withoutPlatformReport = ({ platform: _platform, inputs, ...value }: any) => ({
    ...value,
    inputs: { ...inputs, manifest_sha256: undefined },
  });
  assert.deepEqual(withoutPlatformReport(ios), withoutPlatformReport(android));
  return {
    schema: "cord.festival-mobile-app-projection-report.v2",
    status: "PASS",
    journey_acceptance: true,
    p6_acceptance: true,
    scope: "ios-android-app-projection-only",
    platforms: { ios, android },
    vector_count: (ios as any).vector_count,
    app_projection_outcome_parity: true,
    qualification: "iOS and Android consume the same deterministic createApp and sealed-route vectors; these CORD-owned projections are not production Swift or Kotlin applications.",
  };
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  const report = await validateMobileAppProjection();
  if (process.argv.includes("--write")) {
    const path = resolve(import.meta.dirname, "../../../target/evidence/p6/festival-mobile-app-projection.report.json");
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `${JSON.stringify(report, null, 2)}\n`);
    process.stdout.write(`${path}\n`);
  } else {
    process.stdout.write(`${JSON.stringify(report)}\n`);
  }
}
