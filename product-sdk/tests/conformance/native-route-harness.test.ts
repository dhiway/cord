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
import { NATIVE_ROUTE_CONTRACT } from "../../packages/descriptors/generated/native-route-contract.ts";
import { ORBIS_CANDIDATE_NETWORK_BINDING } from "../../packages/descriptors/generated/orbis-network-binding.ts";
import { NATIVE_RUNTIME_ROUTE_REGISTRY } from "../../packages/descriptors/src/runtime-route-registry.ts";
import { FakeHost } from "../../packages/host/src/fake-host.ts";

test("every authoritative native route constructs, validates and selects its exact TS dispatch", async () => {
	const selected: string[] = [];
	const host = new FakeHost({
		finalizedRead: async ({ request }) => {
			selected.push(`finalized:${request.capability}:${request.method}`);
			return { finalizedHash: `0x${"ab".repeat(32)}` };
		},
		submitAndFinalize: async ({ request }) => {
			selected.push(`submit-and-finalize:${request.capability}:${request.method}`);
			return { finalizedHash: `0x${"cd".repeat(32)}` };
		},
	});
	for (const [index, route] of NATIVE_ROUTE_CONTRACT.routes.entries()) {
		const scope = route.id;
		host.grant("route-harness", [scope]);
		const activeConsent = { scopes: [scope], expires_at: 2_000, nonce: `route-consent-${String(index).padStart(4, "0")}` };
		const request = (NATIVE_RUNTIME_ROUTE_REGISTRY[scope] as (...args: any[]) => any)({
			request_id: `route-request-${String(index).padStart(4, "0")}`,
			application_id: "route-harness",
			network: { ...ORBIS_CANDIDATE_NETWORK_BINDING },
			consent: activeConsent,
		}, ...structuredClone(route.canonical_arguments));
		host.issueConsent("route-harness", request.consent);
		await host.execute(request);
		assert.deepEqual(request.payload, route.sample_payload, route.id);
		assert.equal(request.method, route.method, route.id);
		assert.equal(request.finality, route.finality, route.id);
		assert.equal(route.parameters.map(({ name }) => name).join(","), Object.keys(request.payload).join(","), route.id);
		assert.ok(route.runtime.target && route.rust.declaration && route.rust.variant && route.typescript.callable, route.id);
	}
	assert.equal(selected.length, NATIVE_ROUTE_CONTRACT.route_count);
	assert.deepEqual(selected, NATIVE_ROUTE_CONTRACT.routes.map((route) => `${route.finality}:${route.id}`));
});
