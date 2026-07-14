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
import { ORBIS_CANDIDATE_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { FakeHost, type HostRequest } from "../../packages/host/src/fake-host.ts";
import { attestation } from "../../src/attestation.ts";
import { page, type AccountId, type StatusCommitment } from "../../src/types.ts";

const ISSUER = "5GrwvaEF5zXb26Fz9rcQpDWSGJ7c9ZkZ7E3hV8VJtPp8wQnZ" as AccountId;
const STATUS = `0x${"44".repeat(32)}` as StatusCommitment;

function context(scope: string, index: number) {
	return {
		request_id: `attestation-contract-${String(index).padStart(4, "0")}`,
		application_id: "festival",
		network: ORBIS_CANDIDATE_NETWORK_BINDING,
		consent: {
			scopes: [scope],
			expires_at: 2_000,
			nonce: `attestation-consent-${String(index).padStart(4, "0")}`,
		},
	};
}

test("new attestation methods have exact payload and finality contracts", async () => {
	assert.deepEqual(page({ limit: 0 }), { cursor: null, limit: 0 });
	const requests = [
		attestation.schemaCount(context("attestation:schema_count", 1)),
		attestation.attestationCount(context("attestation:attestation_count", 2)),
		attestation.nextIssuanceNonce(context("attestation:next_issuance_nonce", 3), ISSUER),
		attestation.externalStatus(context("attestation:external_status", 4), ISSUER, STATUS),
		attestation.revokeExternalStatus(
			context("attestation:revoke_external_status", 5),
			STATUS,
		),
		attestation.revokeExternalStatusBatch(
			context("attestation:revoke_external_status_batch", 6),
			[STATUS],
		),
	] as const;

	assert.deepEqual(
		requests.map(({ method, finality, payload }) => ({ method, finality, payload })),
		[
			{ method: "schema_count", finality: "finalized", payload: {} },
			{ method: "attestation_count", finality: "finalized", payload: {} },
			{ method: "next_issuance_nonce", finality: "finalized", payload: { issuer: ISSUER } },
			{
				method: "external_status",
				finality: "finalized",
				payload: { issuer: ISSUER, status_commitment: STATUS },
			},
			{
				method: "revoke_external_status",
				finality: "submit-and-finalize",
				payload: { status_commitment: STATUS },
			},
			{
				method: "revoke_external_status_batch",
				finality: "submit-and-finalize",
				payload: { status_commitments: [STATUS] },
			},
		],
	);

	const routed: string[] = [];
	const host = new FakeHost({
		finalizedRead: async ({ request }) => {
			routed.push(`read:${request.method}`);
			return { finalizedHash: `0x${"aa".repeat(32)}` };
		},
		submitAndFinalize: async ({ request }) => {
			routed.push(`write:${request.method}`);
			return { finalizedHash: `0x${"bb".repeat(32)}` };
		},
	});
	host.grant("festival", requests.map(({ method }) => `attestation:${method}`));
	for (const request of requests) {
		const hostRequest = request as HostRequest;
		host.issueConsent(hostRequest.application_id, hostRequest.consent);
		await host.execute(hostRequest);
	}
	assert.deepEqual(routed, [
		"read:schema_count",
		"read:attestation_count",
		"read:next_issuance_nonce",
		"read:external_status",
		"write:revoke_external_status",
		"write:revoke_external_status_batch",
	]);
});
