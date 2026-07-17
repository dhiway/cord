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
import { readFileSync } from "node:fs";
import test from "node:test";
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import {
	IDENTITY_V2_OPERATION_CODES,
	createIdentityV2Client,
	identityRecoveryDispositionV2,
	type IdentityGrantV2,
	type IdentityV2Bridge,
	type IdentityV2Call,
	type IdentityV2InvocationOptions,
} from "../src/v2.ts";

const bytes = (length: number, value: number): Uint8Array => new Uint8Array(length).fill(value);
const incarnation = bytes(32, 9);
const options: IdentityV2InvocationOptions = {
	finalizedBlock: 100n,
	currentRecoveryIncarnation: incarnation,
};
const finalized = { blockNumber: 100n, blockHash: bytes(32, 8) };

const inputByOperation = {
	"identity.account": { session: "selected" },
	"identity.profile.read": { subject: bytes(32, 1), fields: ["display"] },
	"identity.profile.disclose": {
		audience: "festival.example",
		fields: ["email"],
		purpose: "ticket delivery",
		expiresAt: 120n,
	},
	"identity.humanity.status": { subject: bytes(32, 2) },
	"identity.humanity.prove": {
		audience: "festival.example",
		challenge: bytes(16, 3),
		expiresAt: 120n,
		claims: ["adult"],
	},
	"identity.subject.derive": {
		productId: "festival",
		context: "attendee",
		verifierAudience: "festival.example",
	},
	"identity.entitlements.read": { subject: bytes(32, 4), scope: "festival.entry" },
	"transaction.sign": { payloadHash: bytes(32, 5), policyHash: bytes(32, 6), expiresAt: 120n },
} as const;

const outputByOperation = {
	"identity.account": { account: bytes(32, 1), sessionExpiresAt: 120n, finalized },
	"identity.profile.read": { receipt: { commitment: bytes(32, 2), validUntil: 120n, finalized } },
	"identity.profile.disclose": { receipt: { commitment: bytes(32, 3), validUntil: 120n } },
	"identity.humanity.status": { status: 1, freshUntil: 120n, finalized },
	"identity.humanity.prove": {
		proof: bytes(64, 4),
		derivedPublicKey: bytes(32, 5),
		proofHash: bytes(32, 6),
		continuity: true,
		expiresAt: 120n,
	},
	"identity.subject.derive": {
		subject: bytes(32, 7),
		derivedPublicKey: bytes(32, 8),
		epoch: 0,
		recoveryIncarnationHash: bytes(32, 9),
		continuity: true,
	},
	"identity.entitlements.read": {
		allowed: true,
		scope: "festival.entry",
		policyVersion: 2,
		expiresAt: 120n,
		freshUntil: 110n,
		finalized,
	},
	"transaction.sign": { transactionHash: bytes(32, 10), finalized },
} as const;

function grant<Operation extends IdentityV2Call>(
	operation: Operation,
	overrides: Partial<IdentityGrantV2<Operation>> = {},
): IdentityGrantV2<Operation> {
	const audience = operation === "identity.profile.disclose" || operation === "identity.humanity.prove"
		|| operation === "identity.subject.derive"
		? "festival.example"
		: undefined;
	return {
		version: 2,
		id: bytes(32, IDENTITY_V2_OPERATION_CODES[operation] % 255),
		productId: "festival",
		scope: operation,
		recoveryIncarnation: incarnation,
		expiresAt: 200n,
		...(audience === undefined ? {} : { audience }),
		...overrides,
	};
}

function successBridge(calls: IdentityV2Call[] = []): IdentityV2Bridge {
	return {
		async request(invocation): Promise<SdkResult<unknown>> {
			calls.push(invocation.operation);
			return { success: true, value: outputByOperation[invocation.operation] };
		},
	};
}

async function call(
	client: ReturnType<typeof createIdentityV2Client>,
	operation: IdentityV2Call,
	operationGrant: IdentityGrantV2<IdentityV2Call> = grant(operation),
	invocationOptions: IdentityV2InvocationOptions = options,
): Promise<SdkResult<unknown>> {
	const input = inputByOperation[operation] as never;
	switch (operation) {
		case "identity.account": return client.account(operationGrant as never, input, invocationOptions);
		case "identity.profile.read": return client.profileRead(operationGrant as never, input, invocationOptions);
		case "identity.profile.disclose": return client.profileDisclose(operationGrant as never, input, invocationOptions);
		case "identity.humanity.status": return client.humanityStatus(operationGrant as never, input, invocationOptions);
		case "identity.humanity.prove": return client.humanityProve(operationGrant as never, input, invocationOptions);
		case "identity.subject.derive": return client.subjectDerive(operationGrant as never, input, invocationOptions);
		case "identity.entitlements.read": return client.entitlementsRead(operationGrant as never, input, invocationOptions);
		case "transaction.sign": return client.signTransaction(operationGrant as never, input, invocationOptions);
	}
}

const freshConsent = new Set<IdentityV2Call>([
	"identity.profile.disclose",
	"identity.humanity.prove",
	"transaction.sign",
]);
const operationOptions = (operation: IdentityV2Call): IdentityV2InvocationOptions => ({
	...options,
	...(freshConsent.has(operation) ? { operationId: bytes(16, 12) } : {}),
});

test("private Identity-v2 operation codes remain equal to the frozen host registry", () => {
	const registry = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/origin-host-registry-v2.operations.json", import.meta.url),
		"utf8",
	));
	const frozen = Object.fromEntries(registry.operations
		.filter(({ name }: { name: string }) => name.startsWith("identity.") || name === "transaction.sign")
		.map(({ name, code }: { name: string; code: number }) => [name, code]));
	assert.deepEqual(IDENTITY_V2_OPERATION_CODES, frozen);
	assert.deepEqual(Object.keys(IDENTITY_V2_OPERATION_CODES), [
		"identity.account",
		"identity.profile.read",
		"identity.profile.disclose",
		"identity.humanity.status",
		"identity.humanity.prove",
		"identity.subject.derive",
		"identity.entitlements.read",
		"transaction.sign",
	]);
	const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
	assert.deepEqual(Object.keys(manifest.exports), ["."]);
});

test("every Identity operation accepts only its exact grant and transaction signing stays separate", async () => {
	const operations = Object.keys(IDENTITY_V2_OPERATION_CODES) as IdentityV2Call[];
	const calls: IdentityV2Call[] = [];
	const client = createIdentityV2Client("festival", successBridge(calls));
	for (let index = 0; index < operations.length; index++) {
		const operation = operations[index]!;
		const wrong = operations[(index + 1) % operations.length]!;
		const denied = await call(client, operation, grant(wrong) as IdentityGrantV2<IdentityV2Call>, operationOptions(operation));
		assert.equal(denied.success, false, `${operation} accepted ${wrong}`);
		assert.equal(!denied.success && denied.error.code, "GRANT_SCOPE_DENIED", operation);
		const accepted = await call(client, operation, grant(operation), operationOptions(operation));
		assert.equal(accepted.success, true, operation);
	}
	assert.deepEqual(calls, operations);
});

test("audience binding and closed result schemas prevent joined authority leakage", async () => {
	const noCalls: IdentityV2Call[] = [];
	const deniedClient = createIdentityV2Client("festival", successBridge(noCalls));
	const denied = await deniedClient.humanityProve(
		grant("identity.humanity.prove", { audience: "other.example" }),
		inputByOperation["identity.humanity.prove"],
		operationOptions("identity.humanity.prove"),
	);
	assert.equal(!denied.success && denied.error.code, "IDENTITY_AUDIENCE_INVALID");
	assert.equal(noCalls.length, 0);

	const leakingBridge: IdentityV2Bridge = {
		async request(invocation) {
			if (invocation.operation === "identity.account") {
				return { success: true, value: { ...outputByOperation["identity.account"], signingAuthority: bytes(32, 1) } };
			}
			return {
				success: true,
				value: { ...outputByOperation["identity.entitlements.read"], profile: { email: "hidden@example" } },
			};
		},
	};
	const client = createIdentityV2Client("festival", leakingBridge);
	const account = await client.account(grant("identity.account"), inputByOperation["identity.account"], options);
	assert.equal(!account.success && account.error.code, "WIRE_SCHEMA_INVALID");
	const entitlements = await client.entitlementsRead(
		grant("identity.entitlements.read"),
		inputByOperation["identity.entitlements.read"],
		options,
	);
	assert.equal(!entitlements.success && entitlements.error.code, "WIRE_SCHEMA_INVALID");
});

test("fresh-consent replay and recovery-incarnation failures match frozen Identity vectors", async () => {
	const vectors = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/identity-v2.vectors.json", import.meta.url),
		"utf8",
	));
	const proofVector = vectors.executable_vectors.find(({ id }: { id: string }) => id === "subject-proof-v2");
	const replayVector = proofVector.negative_vectors.find(({ id }: { id: string }) => id === "subject-proof-replay");
	const audienceVector = proofVector.negative_vectors.find(
		({ id }: { id: string }) => id === "subject-proof-audience-substitution",
	);
	assert.equal(createHash("sha256").update(Buffer.from(proofVector.canonical_cbor_hex, "hex")).digest("hex"), proofVector.canonical_sha256);
	assert.equal(replayVector.expected_error, "IDENTITY_CHALLENGE_REPLAY");
	assert.equal(audienceVector.expected_error, "IDENTITY_AUDIENCE_INVALID");

	const consumed = new Set<string>();
	const bridge: IdentityV2Bridge = {
		async request(invocation) {
			const id = Buffer.from(invocation.operationId ?? []).toString("hex");
			if (consumed.has(id)) {
				return {
					success: false,
					error: new OriginSdkError({
						source: "identity-v2-test",
						domain: "replay",
						code: replayVector.expected_error,
						message: "challenge already consumed",
					}),
				};
			}
			consumed.add(id);
			return { success: true, value: outputByOperation[invocation.operation] };
		},
	};
	const client = createIdentityV2Client("festival", bridge);
	const invocation = operationOptions("identity.humanity.prove");
	assert.equal((await call(client, "identity.humanity.prove", grant("identity.humanity.prove"), invocation)).success, true);
	const replay = await call(client, "identity.humanity.prove", grant("identity.humanity.prove"), invocation);
	assert.equal(!replay.success && replay.error.code, replayVector.expected_error);

	const old = vectors.recovery_vectors.find(({ id }: { id: string }) => id === "old-incarnation-grant");
	const oldGrant = grant("identity.subject.derive", { recoveryIncarnation: bytes(32, 7) });
	const rejected = await call(client, "identity.subject.derive", oldGrant, options);
	assert.equal(!rejected.success && rejected.error.code, old.expected_error);
});

test("only the complete authenticated monotonic same-store recovery preserves continuity", () => {
	const vectors = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/identity-v2.vectors.json", import.meta.url),
		"utf8",
	));
	const expected = new Map(vectors.recovery_vectors
		.filter(({ id }: { id: string }) => ["same-store-restart", "seed-only-backup", "stale-backup"].includes(id))
		.map(({ id, continuity }: { id: string; continuity: boolean }) => [id, continuity]));
	assert.equal(identityRecoveryDispositionV2({
		sameStore: true,
		authenticated: true,
		completeReplayJournal: true,
		monotonic: true,
	}).continuity, expected.get("same-store-restart"));
	for (const [id, evidence] of [
		["seed-only-backup", { sameStore: false, authenticated: true, completeReplayJournal: false, monotonic: false }],
		["stale-backup", { sameStore: true, authenticated: true, completeReplayJournal: false, monotonic: false }],
	] as const) {
		const disposition = identityRecoveryDispositionV2(evidence);
		assert.equal(disposition.continuity, expected.get(id));
		assert.deepEqual(disposition, { continuity: false, action: "install-fresh-root", epoch: 0 });
	}
});
