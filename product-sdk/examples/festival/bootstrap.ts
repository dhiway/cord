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
import { ProductSdkError } from "../../packages/core/src/contract.ts";
import { ORBIS_CANDIDATE_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { FakeHost, type HostRequest, type HostTransportResult } from "../../packages/host/src/fake-host.ts";

interface ReferenceManifest {
  schema: "cord.reference-app.v1";
  application_id: string;
  host_permissions: Array<{
    capability: string;
    method: string;
    finality: "finalized" | "submit-and-finalize";
  }>;
  consent: { explicit: boolean; expiring: boolean; revocable: boolean; single_use_nonce: boolean };
  transport: {
    finalized_reads_only: boolean;
    submit_and_finalize_only: boolean;
    cancellation_required: boolean;
    runtime_identity_fail_closed: boolean;
  };
  native_cutover: {
    migrated_domain_contract_calls: unknown[];
    deployment_addresses: unknown[];
    generated_contract_bindings: unknown[];
    raw_runtime_encoding: boolean;
    pallet_or_call_indices: boolean;
  };
}

const manifest = JSON.parse(readFileSync(
  resolve(import.meta.dirname, "reference-app.manifest.json"),
  "utf8",
)) as ReferenceManifest;
const finalizedHash = `0x${"ab".repeat(32)}`;
const extrinsicHash = `0x${"cd".repeat(32)}`;
const terminal: string[] = [];
let finalizedReads = 0;
let submissions = 0;

const host = new FakeHost({
  now: () => 1_000,
  finalizedRead: async (): Promise<HostTransportResult> => {
    finalizedReads++;
    return { finalizedHash, response: { active: true } };
  },
  submitAndFinalize: async ({ request }, signal): Promise<HostTransportResult> => {
    submissions++;
    if (request.request_id === "festival-cancel-000001") {
      await new Promise<never>((_, reject) =>
        signal.addEventListener("abort", () => reject(new ProductSdkError("cancelled", "cancelled")), { once: true }));
    }
    return {
      finalizedHash,
      extrinsicHash,
      lifecycle: {
        version: 1,
        intent_id: request.request_id,
        state: "finalized",
        block_hash: finalizedHash,
        extrinsic_hash: extrinsicHash,
      },
    };
  },
  onTerminal: (requestId, outcome) => terminal.push(`${requestId}:${outcome}`),
});

const grantedMethods = manifest.host_permissions.map(({ capability, method }) => `${capability}:${method}`);
host.grant(manifest.application_id, grantedMethods);

function request(
  requestId: string,
  capability: string,
  method: string,
  finality: HostRequest["finality"],
  payload: HostRequest["payload"],
  expiresAt = 2_000,
): HostRequest {
  return {
    version: 1,
    request_id: requestId,
    application_id: manifest.application_id,
    capability,
    method,
    network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
    finality,
    payload,
    consent: {
      scope: [`${capability}:${method}`],
      expires_at: expiresAt,
      nonce: `consent-${requestId}`,
    },
  };
}

function withHostConsent(value: HostRequest): HostRequest {
  host.issueConsent(value.application_id, value.consent);
  return value;
}

async function errorCode(operation: Promise<unknown>): Promise<string> {
  try {
    await operation;
    return "success";
  } catch (error) {
    assert.ok(error instanceof ProductSdkError);
    return error.code;
  }
}

assert.equal(manifest.schema, "cord.reference-app.v1");
assert.ok(manifest.consent.explicit && manifest.consent.expiring && manifest.consent.revocable);
assert.ok(manifest.consent.single_use_nonce);
assert.ok(manifest.transport.finalized_reads_only && manifest.transport.submit_and_finalize_only);
assert.ok(manifest.transport.cancellation_required && manifest.transport.runtime_identity_fail_closed);
assert.deepEqual(manifest.native_cutover.migrated_domain_contract_calls, []);
assert.deepEqual(manifest.native_cutover.deployment_addresses, []);
assert.deepEqual(manifest.native_cutover.generated_contract_bindings, []);
assert.equal(manifest.native_cutover.raw_runtime_encoding, false);
assert.equal(manifest.native_cutover.pallet_or_call_indices, false);

await host.execute(withHostConsent(request(
  "festival-read-00000001",
  "attestation",
  "attestation_live_status",
  "finalized",
  { attestation: `0x${"11".repeat(32)}` },
)));

assert.equal(await errorCode(host.execute(withHostConsent(request(
  "festival-ungranted-001",
  "attestation",
  "schema_by_id",
  "finalized",
  { schema: `0x${"22".repeat(32)}` },
)))), "permission_denied");

assert.equal(await errorCode(host.execute(request(
  "festival-no-consent-01",
  "attestation",
  "attestation_live_status",
  "finalized",
  { attestation: `0x${"11".repeat(32)}` },
))), "permission_denied");

assert.equal(await errorCode(host.execute(withHostConsent(request(
  "festival-expired-0001",
  "attestation",
  "attestation_live_status",
  "finalized",
  { attestation: `0x${"11".repeat(32)}` },
  999,
)))), "consent_expired");

const consentRevoked = withHostConsent(request(
  "festival-consent-revoke",
  "attestation",
  "attestation_live_status",
  "finalized",
  { attestation: `0x${"11".repeat(32)}` },
));
host.revokeConsent(consentRevoked.consent.nonce);
assert.equal(await errorCode(host.execute(consentRevoked)), "permission_revoked");

await host.execute(withHostConsent(request(
  "festival-submit-000001",
  "names",
  "commit",
  "submit-and-finalize",
  { commitment: `0x${"33".repeat(32)}` },
)));
const cancellable = host.execute(withHostConsent(request(
  "festival-cancel-000001",
  "names",
  "commit",
  "submit-and-finalize",
  { commitment: `0x${"44".repeat(32)}` },
)));
await new Promise((resolveTick) => setTimeout(resolveTick, 0));
host.cancel("festival-cancel-000001");
assert.equal(await errorCode(cancellable), "cancelled");

host.revoke(manifest.application_id);
assert.equal(await errorCode(host.execute(withHostConsent(request(
  "festival-revoked-0001",
  "attestation",
  "attestation_live_status",
  "finalized",
  { attestation: `0x${"11".repeat(32)}` },
)))), "permission_revoked");

assert.equal(finalizedReads, 1);
assert.equal(submissions, 2);
assert.equal(terminal.length, 8);
process.stdout.write(`${JSON.stringify({
  schema: "cord.reference-app-bootstrap.v1",
  application_id: manifest.application_id,
  status: "pass",
  checks: {
    scoped_permission: true,
    host_owned_active_consent: true,
    missing_consent_rejected: true,
    consent_revocation_rejected: true,
    expiry_rejected: true,
    revoke_before_sign: true,
    cancellation_propagated: true,
    caller_transport_routes: { finalized_reads: finalizedReads, submissions },
    migrated_domain_native_only: true,
  },
})}\n`);
