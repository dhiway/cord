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
import type { SdkResult } from "@cord-network/origin-sdk-errors";
import { decodeHostV2, encodeHostV2, HostV2CodecError } from "../../origin-sdk-host/src/internal/v2/codec.ts";
import {
	IDENTITY_V2_OPERATION_CODES,
	IDENTITY_V2_CONTRACTS,
	IDENTITY_V2_ALLOWED_ERRORS,
	IDENTITY_V2_ERRORS,
	createIdentityV2Client,
	createTransactionSigningV2Client,
	identityV2WireFrame,
	identityRecoveryDispositionV2,
	validateIdentityV2ErrorEnvelope,
	type IdentityGrantV2,
	type IdentityV2Bridge,
	type IdentityV2Call,
	type IdentityV2InvocationOptions,
} from "../src/v2.ts";

function createClient(productId: string, bridge: IdentityV2Bridge) {
	return Object.assign(
		createIdentityV2Client(productId, bridge),
		createTransactionSigningV2Client(productId, bridge),
	);
}

const bytes = (length: number, value: number): Uint8Array => new Uint8Array(length).fill(value);
const incarnation = bytes(32, 9);
const options: IdentityV2InvocationOptions = {
	requestId: bytes(16, 11),
	deadlineBlock: 150n,
	finalizedBlock: 100n,
	finalizedHash: bytes(32, 8),
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
	"identity.profile.disclose": { receipt: { commitment: bytes(32, 3), validUntil: 120n, finalized } },
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
		async request(invocation) {
			calls.push(invocation.operation);
			return { success: true, value: outputByOperation[invocation.operation] };
		},
	};
}

async function call(
	client: ReturnType<typeof createClient>,
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
	...(freshConsent.has(operation) ? { operationId: bytes(16, IDENTITY_V2_OPERATION_CODES[operation] % 251) } : {}),
});

type WireRecord = Readonly<Record<number, unknown>>;
const asBigInt = (value: unknown): bigint => typeof value === "bigint" ? value : BigInt(value as number);
function semanticInput(operation: IdentityV2Call, raw: unknown): unknown {
	const value = raw as WireRecord;
	switch (operation) {
		case "identity.account": return { session: value[0] };
		case "identity.profile.read": return { subject: value[0], fields: value[1], ...(value[2] === undefined ? {} : { at: value[2] }) };
		case "identity.profile.disclose": return { audience: value[0], fields: value[1], purpose: value[2], expiresAt: asBigInt(value[3]) };
		case "identity.humanity.status": return { subject: value[0], ...(value[1] === undefined ? {} : { at: value[1] }) };
		case "identity.humanity.prove": return { audience: value[0], challenge: value[1], expiresAt: asBigInt(value[2]), claims: value[3] };
		case "identity.subject.derive": return { productId: value[0], context: value[1], verifierAudience: value[2], ...(value[3] === undefined ? {} : { epoch: value[3] }) };
		case "identity.entitlements.read": return { subject: value[0], scope: value[1], ...(value[2] === undefined ? {} : { at: value[2] }) };
		case "transaction.sign": return { payloadHash: value[0], policyHash: value[1], expiresAt: asBigInt(value[2]) };
	}
}

test("public Identity-v2 operation codes remain equal to the frozen host registry", () => {
	const registry = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/origin-host-registry-v2.operations.json", import.meta.url),
		"utf8",
	));
	const frozen = Object.fromEntries(registry.operations
		.filter(({ name }: { name: string }) => name.startsWith("identity.") || name === "transaction.sign")
		.map(({ name, code }: { name: string; code: number }) => [name, code]));
	assert.deepEqual(IDENTITY_V2_OPERATION_CODES, frozen);
	for (const [name, contract] of Object.entries(IDENTITY_V2_CONTRACTS)) {
		const row = registry.operations.find((candidate: { name: string }) => candidate.name === name);
		assert.ok(row, name);
		assert.equal(contract.code, row.code, name);
		assert.equal(contract.grantScope, row.grant_scope, name);
		assert.equal(contract.consentMode, row.consent_mode, name);
		assert.equal(contract.operationIdRequired, row.operation_id_required, name);
		assert.equal(contract.request, row.cddl.Request, name);
		assert.equal(contract.result, row.cddl.Result, name);
		assert.equal(contract.error, row.cddl.Error, name);
		assert.deepEqual(row.allowed_errors, IDENTITY_V2_ERRORS);
		assert.deepEqual(row.allowed_errors.map(({ name: error }: { name: string }) => error), IDENTITY_V2_ALLOWED_ERRORS);
	}
	const errors = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/origin-host-registry-v2.errors.json", import.meta.url),
		"utf8",
	));
	assert.deepEqual(IDENTITY_V2_ERRORS, errors.errors
		.filter(({ code }: { code: number }) => (code >= 100 && code <= 117) || (code >= 400 && code <= 413))
		.map(({ code, name, retryable }: { code: number; name: string; retryable: boolean }) => ({ code, name, retryable })));
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

test("semantic Identity DTOs encode and decode every frozen generated Host-v2 frame", () => {
	const vectors = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/origin-host-registry-v2.vectors.json", import.meta.url),
		"utf8",
	));
	for (const [operation, contract] of Object.entries(IDENTITY_V2_CONTRACTS)) {
		const positive = vectors.vectors.find(({ id }: { id: string }) => id === `${contract.code}-positive`);
		const schemaNegative = vectors.vectors.find(({ id }: { id: string }) => id === `${contract.code}-schema-negative`);
		const grantNegative = vectors.vectors.find(({ id }: { id: string }) => id === `${contract.code}-grant-negative`);
		assert.equal(positive.operation, operation);
		assert.equal(positive.cddl.Request, contract.request);
		assert.equal(positive.expected, "accept");
		const wire = Uint8Array.from(Buffer.from(positive.wire_hex, "hex"));
		assert.equal(createHash("sha256").update(wire).digest("hex"), positive.wire_sha256);
		const decoded = decodeHostV2(contract.frame, wire).value as WireRecord;
		assert.deepEqual(decoded[1], bytes(16, 0x11), `${operation} requestId`);
		assert.equal(asBigInt(decoded[7]), 100n, `${operation} deadline`);
		const frame = identityV2WireFrame(
			operation as IdentityV2Call,
			decoded[1] as Uint8Array,
			decoded[2] as string,
			decoded[4] as Uint8Array,
			asBigInt(decoded[7]),
			semanticInput(operation as IdentityV2Call, decoded[8]) as never,
			decoded[5] as Uint8Array | undefined,
		);
		assert.deepEqual(encodeHostV2(contract.frame, frame as never), wire, operation);
		const noncanonical = Uint8Array.of(0xb8, wire[0]! & 0x1f, ...wire.slice(1));
		assert.throws(
			() => decodeHostV2(contract.frame, noncanonical),
			(error) => error instanceof HostV2CodecError && error.code === "WIRE_NON_CANONICAL",
			operation,
		);
		assert.equal(schemaNegative.expected_error, "WIRE_SCHEMA_INVALID");
		assert.equal(grantNegative.expected_error, "GRANT_SCOPE_DENIED");
	}
});

test("client bridge receives exact generated requestId and deadline frames", async () => {
	const seen: IdentityV2Call[] = [];
	const bridge: IdentityV2Bridge = {
		async request(invocation) {
			const production = IDENTITY_V2_CONTRACTS[invocation.operation].frame;
			const canonical = encodeHostV2(production, invocation.wireFrame as never);
			const decoded = decodeHostV2(production, canonical).value as WireRecord;
			assert.deepEqual(decoded[1], options.requestId);
			assert.equal(asBigInt(decoded[7]), options.deadlineBlock);
			assert.equal(decoded[3], invocation.code);
			assert.equal(invocation.finalizedBlock, options.finalizedBlock);
			assert.deepEqual(invocation.finalizedHash, options.finalizedHash);
			seen.push(invocation.operation);
			return { success: true, value: outputByOperation[invocation.operation] };
		},
	};
	const client = createClient("festival", bridge);
	for (const operation of Object.keys(IDENTITY_V2_OPERATION_CODES) as IdentityV2Call[]) {
		assert.equal((await call(client, operation, grant(operation), operationOptions(operation))).success, true, operation);
	}
	assert.deepEqual(seen, Object.keys(IDENTITY_V2_OPERATION_CODES));
});

test("every Identity operation accepts only its exact grant and transaction signing stays separate", async () => {
	const operations = Object.keys(IDENTITY_V2_OPERATION_CODES) as IdentityV2Call[];
	const calls: IdentityV2Call[] = [];
	const identity = createIdentityV2Client("festival", successBridge(calls));
	const signing = createTransactionSigningV2Client("festival", successBridge(calls));
	assert.deepEqual(Object.keys(identity), [
		"account",
		"profileRead",
		"profileDisclose",
		"humanityStatus",
		"humanityProve",
		"subjectDerive",
		"entitlementsRead",
	]);
	assert.deepEqual(Object.keys(signing), ["signTransaction"]);
	const client = createClient("festival", successBridge(calls));
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
	const deniedClient = createClient("festival", successBridge(noCalls));
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
	const client = createClient("festival", leakingBridge);
	const account = await client.account(grant("identity.account"), inputByOperation["identity.account"], options);
	assert.equal(!account.success && account.error.code, "WIRE_SCHEMA_INVALID");
	const entitlements = await client.entitlementsRead(
		grant("identity.entitlements.read"),
		inputByOperation["identity.entitlements.read"],
		options,
	);
	assert.equal(!entitlements.success && entitlements.error.code, "WIRE_SCHEMA_INVALID");
});

test("hostile inputs, grants, results, and bridge errors cannot escape the closed facade", async () => {
	const calls: IdentityV2Call[] = [];
	const client = createClient("festival", successBridge(calls));
	const joinedInput = { ...inputByOperation["identity.account"], profile: { email: "hidden@example" } };
	const inputRejected = await client.account(grant("identity.account"), joinedInput as never, options);
	assert.equal(!inputRejected.success && inputRejected.error.code, "WIRE_SCHEMA_INVALID");
	const joinedGrant = { ...grant("identity.account"), compositeAuthority: true };
	const grantRejected = await client.account(joinedGrant as never, inputByOperation["identity.account"], options);
	assert.equal(!grantRejected.success && grantRejected.error.code, "WIRE_SCHEMA_INVALID");
	assert.equal(calls.length, 0);
	const wrongProduct = await client.subjectDerive(
		grant("identity.subject.derive"),
		{ ...inputByOperation["identity.subject.derive"], productId: "other" },
		options,
	);
	assert.equal(!wrongProduct.success && wrongProduct.error.code, "GRANT_SCOPE_DENIED");
	const expiredDeadline = await client.account(
		grant("identity.account"),
		inputByOperation["identity.account"],
		{ ...options, deadlineBlock: options.finalizedBlock },
	);
	assert.equal(!expiredDeadline.success && expiredDeadline.error.code, "REQUEST_DEADLINE_EXPIRED");

	const unbounded: IdentityV2Bridge = { async request() {
		return { success: true, value: { ...outputByOperation["identity.humanity.prove"], proof: bytes(4_097, 1) } };
	} };
	const resultRejected = await createClient("festival", unbounded).humanityProve(
		grant("identity.humanity.prove"),
		inputByOperation["identity.humanity.prove"],
		operationOptions("identity.humanity.prove"),
	);
	assert.equal(!resultRejected.success && resultRejected.error.code, "WIRE_SCHEMA_INVALID");

	const unknownError: IdentityV2Bridge = { async request() {
		return { success: false, error: { code: 999, name: "legacy_people_error", retryable: false } };
	} };
	const errorRejected = await createClient("festival", unknownError).account(
		grant("identity.account"), inputByOperation["identity.account"], options,
	);
	assert.equal(!errorRejected.success && errorRejected.error.code, "WIRE_SCHEMA_INVALID");
});

test("results bind profile finality, requested entitlement scope, and proof freshness", async () => {
	const response = async (
		operation: IdentityV2Call,
		value: unknown,
	): Promise<SdkResult<unknown>> => {
		const bridge: IdentityV2Bridge = { async request() { return { success: true, value }; } };
		return call(createClient("festival", bridge), operation, grant(operation), operationOptions(operation));
	};
	const noFinality = await response("identity.profile.read", {
		receipt: { commitment: bytes(32, 2), validUntil: 120n },
	});
	assert.equal(!noFinality.success && noFinality.error.code, "WIRE_SCHEMA_INVALID");
	const staleAccount = await response("identity.account", {
		...outputByOperation["identity.account"], sessionExpiresAt: 100n,
	});
	assert.equal(!staleAccount.success && staleAccount.error.code, "WIRE_SCHEMA_INVALID");
	const crossSnapshotAccount = await response("identity.account", {
		...outputByOperation["identity.account"],
		finalized: { blockNumber: 99n, blockHash: bytes(32, 7) },
	});
	assert.equal(!crossSnapshotAccount.success && crossSnapshotAccount.error.code, "WIRE_SCHEMA_INVALID");
	const wrongCurrentHash = await response("identity.account", {
		...outputByOperation["identity.account"],
		finalized: { blockNumber: 100n, blockHash: bytes(32, 7) },
	});
	assert.equal(!wrongCurrentHash.success && wrongCurrentHash.error.code, "WIRE_SCHEMA_INVALID");
	const crossSnapshotProfile = await response("identity.profile.read", {
		receipt: {
			commitment: bytes(32, 2), validUntil: 120n,
			finalized: { blockNumber: 99n, blockHash: bytes(32, 7) },
		},
	});
	assert.equal(!crossSnapshotProfile.success && crossSnapshotProfile.error.code, "WIRE_SCHEMA_INVALID");
	const staleProfile = await response("identity.profile.disclose", {
		receipt: { commitment: bytes(32, 2), validUntil: 100n, finalized },
	});
	assert.equal(!staleProfile.success && staleProfile.error.code, "WIRE_SCHEMA_INVALID");
	const overlongDisclosure = await response("identity.profile.disclose", {
		receipt: { commitment: bytes(32, 2), validUntil: 121n, finalized },
	});
	assert.equal(!overlongDisclosure.success && overlongDisclosure.error.code, "WIRE_SCHEMA_INVALID");
	const staleHumanity = await response("identity.humanity.status", {
		...outputByOperation["identity.humanity.status"], freshUntil: 100n,
	});
	assert.equal(!staleHumanity.success && staleHumanity.error.code, "WIRE_SCHEMA_INVALID");
	const historicalHash = bytes(32, 9);
	const historicalProfileBridge: IdentityV2Bridge = { async request() {
		return { success: true, value: {
			receipt: {
				commitment: bytes(32, 2), validUntil: 110n,
				finalized: { blockNumber: 99n, blockHash: historicalHash },
			},
		} };
	} };
	const historicalProfile = await createClient("festival", historicalProfileBridge).profileRead(
		grant("identity.profile.read"),
		{ ...inputByOperation["identity.profile.read"], at: historicalHash },
		operationOptions("identity.profile.read"),
	);
	assert.equal(historicalProfile.success, true);
	const futureHistoricalBridge: IdentityV2Bridge = { async request() {
		return { success: true, value: {
			...outputByOperation["identity.humanity.status"],
			finalized: { blockNumber: 101n, blockHash: historicalHash },
		} };
	} };
	const futureHistorical = await createClient("festival", futureHistoricalBridge).humanityStatus(
		grant("identity.humanity.status"),
		{ ...inputByOperation["identity.humanity.status"], at: historicalHash },
		operationOptions("identity.humanity.status"),
	);
	assert.equal(!futureHistorical.success && futureHistorical.error.code, "WIRE_SCHEMA_INVALID");
	const wrongScope = await response("identity.entitlements.read", {
		...outputByOperation["identity.entitlements.read"], scope: "other",
	});
	assert.equal(!wrongScope.success && wrongScope.error.code, "WIRE_SCHEMA_INVALID");
	const staleEntitlement = await response("identity.entitlements.read", {
		...outputByOperation["identity.entitlements.read"], freshUntil: 100n,
	});
	assert.equal(!staleEntitlement.success && staleEntitlement.error.code, "WIRE_SCHEMA_INVALID");
	const crossSnapshotEntitlement = await response("identity.entitlements.read", {
		...outputByOperation["identity.entitlements.read"],
		finalized: { blockNumber: 99n, blockHash: bytes(32, 7) },
	});
	assert.equal(!crossSnapshotEntitlement.success && crossSnapshotEntitlement.error.code, "WIRE_SCHEMA_INVALID");
	const overlongProof = await response("identity.humanity.prove", {
		...outputByOperation["identity.humanity.prove"], expiresAt: 121n,
	});
	assert.equal(!overlongProof.success && overlongProof.error.code, "WIRE_SCHEMA_INVALID");
	const crossSnapshotTransaction = await response("transaction.sign", {
		...outputByOperation["transaction.sign"],
		finalized: { blockNumber: 99n, blockHash: bytes(32, 7) },
	});
	assert.equal(!crossSnapshotTransaction.success && crossSnapshotTransaction.error.code, "WIRE_SCHEMA_INVALID");
});

test("all exact numeric error envelopes are operation-scoped and tuple-closed", () => {
	const operations = Object.keys(IDENTITY_V2_OPERATION_CODES) as IdentityV2Call[];
	for (const operation of operations) {
		for (const frozen of IDENTITY_V2_ERRORS) {
			assert.deepEqual(validateIdentityV2ErrorEnvelope(operation, frozen), frozen);
			assert.throws(
				() => validateIdentityV2ErrorEnvelope(operation, { ...frozen, retryable: !frozen.retryable }),
				/exact operation registry/,
			);
		}
	}
	assert.throws(() => validateIdentityV2ErrorEnvelope("identity.account", {
		code: 400,
		name: "IDENTITY_AUDIENCE_INVALID",
		retryable: false,
		compositeAuthority: true,
	}), /unknown fields/);
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
					error: {
						code: replayVector.expected_error_code,
						name: replayVector.expected_error,
						retryable: false,
						details: { message: "challenge already consumed" },
					},
				};
			}
			consumed.add(id);
			return { success: true, value: outputByOperation[invocation.operation] };
		},
	};
	const client = createClient("festival", bridge);
	const invocation = operationOptions("identity.humanity.prove");
	assert.equal((await call(client, "identity.humanity.prove", grant("identity.humanity.prove"), invocation)).success, true);
	const replay = await call(client, "identity.humanity.prove", grant("identity.humanity.prove"), invocation);
	assert.equal(!replay.success && replay.error.code, replayVector.expected_error);

	const old = vectors.recovery_vectors.find(({ id }: { id: string }) => id === "old-incarnation-grant");
	const oldGrant = grant("identity.subject.derive", { recoveryIncarnation: bytes(32, 7) });
	const rejected = await call(client, "identity.subject.derive", oldGrant, options);
	assert.equal(!rejected.success && rejected.error.code, old.expected_error);
});

test("fresh-consent replay state commits atomically only after a valid durable result", async () => {
	let retryableFailure = true;
	const bridge: IdentityV2Bridge = { async request(invocation) {
		if (retryableFailure) {
			retryableFailure = false;
			return { success: false, error: { code: 114, name: "HOST_OUTBOX_FULL", retryable: true } };
		}
		return { success: true, value: outputByOperation[invocation.operation] };
	} };
	const client = createClient("festival", bridge);
	const original = inputByOperation["identity.humanity.prove"];
	const firstOptions = operationOptions("identity.humanity.prove");
	const retry = await client.humanityProve(
		grant("identity.humanity.prove"), original, firstOptions,
	);
	assert.equal(!retry.success && retry.error.retryable, true);
	assert.equal((await client.humanityProve(
		grant("identity.humanity.prove"), original, firstOptions,
	)).success, true, "a retryable rejection must not burn operationId or challenge");

	const secondOptions = { ...firstOptions, operationId: bytes(16, 77) };
	const consumedChallenge = await client.humanityProve(
		grant("identity.humanity.prove"), original, secondOptions,
	);
	assert.equal(!consumedChallenge.success && consumedChallenge.error.code, "IDENTITY_CHALLENGE_REPLAY");
	const freshChallenge = { ...original, challenge: bytes(16, 78) };
	assert.equal((await client.humanityProve(
		grant("identity.humanity.prove"), freshChallenge, secondOptions,
	)).success, true, "challenge rejection must not partially burn the new operationId");
});

test("concurrent fresh-consent calls reserve challenge and operation ID before bridge effects", async () => {
	let bridgeEffects = 0;
	let release: (() => void) | undefined;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	const bridge: IdentityV2Bridge = { async request(invocation) {
		bridgeEffects += 1;
		await gate;
		return { success: true, value: outputByOperation[invocation.operation] };
	} };
	const client = createClient("festival", bridge);
	const invocation = operationOptions("identity.humanity.prove");
	const first = client.humanityProve(
		grant("identity.humanity.prove"),
		inputByOperation["identity.humanity.prove"],
		invocation,
	);
	await Promise.resolve();
	const concurrent = await client.humanityProve(
		grant("identity.humanity.prove"),
		inputByOperation["identity.humanity.prove"],
		invocation,
	);
	assert.equal(!concurrent.success && concurrent.error.code, "IDENTITY_CHALLENGE_REPLAY");
	assert.equal(bridgeEffects, 1, "only the lease holder may produce a bridge effect");
	release?.();
	assert.equal((await first).success, true);
	assert.equal(bridgeEffects, 1);
});

test("all executable Identity vectors retain canonical, response, state, and effect commitments", () => {
	const vectors = JSON.parse(readFileSync(
		new URL("../../../../docs/specs/identity-v2.vectors.json", import.meta.url),
		"utf8",
	));
	for (const vector of vectors.executable_vectors) {
		const digest = (hex: string) => createHash("sha256").update(Buffer.from(hex, "hex")).digest("hex");
		assert.equal(digest(vector.canonical_cbor_hex), vector.canonical_sha256, vector.id);
		assert.equal(digest(vector.exact_response_cbor_hex), vector.exact_response_sha256, vector.id);
		assert.equal(digest(vector.pre_state_cbor_hex), vector.pre_state_sha256, vector.id);
		assert.equal(digest(vector.post_state_cbor_hex), vector.post_state_sha256, vector.id);
		assert.equal(vector.effect_count, 1, vector.id);
		assert.equal(vector.event_count, 1, vector.id);
		assert.equal(vector.noncanonical_expected_error, "WIRE_NON_CANONICAL", vector.id);
		assert.notEqual(vector.noncanonical_cbor_hex, vector.canonical_cbor_hex, vector.id);
		for (const negative of vector.negative_vectors) {
			assert.equal(negative.effect_count === 0 || negative.expected === "byte-identical-receipt", true, negative.id);
		}
	}
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
