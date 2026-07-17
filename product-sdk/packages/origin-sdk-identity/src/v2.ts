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

import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err } from "@cord-network/origin-sdk-result";

/**
 * Private P3 projection of the frozen `cord.origin.host/2` Identity registry.
 *
 * This file is intentionally not re-exported by the package. P5 makes the public cutover only after
 * every legacy consumer is replaced and the separate-grant contract has passed its privacy gates.
 */

export const IDENTITY_V2_OPERATION_CODES = {
	"identity.account": 1100,
	"identity.profile.read": 1101,
	"identity.profile.disclose": 1102,
	"identity.humanity.status": 1103,
	"identity.humanity.prove": 1104,
	"identity.subject.derive": 1105,
	"identity.entitlements.read": 1106,
	"transaction.sign": 1200,
} as const;

export type IdentityV2Operation = Exclude<keyof typeof IDENTITY_V2_OPERATION_CODES, "transaction.sign">;
export type IdentityV2Call = keyof typeof IDENTITY_V2_OPERATION_CODES;

type Bytes32 = Uint8Array;
type Bytes16 = Uint8Array;

export interface FinalizedIdentityV2 {
	readonly blockNumber: bigint;
	readonly blockHash: Bytes32;
}

export interface IdentityReceiptV2 {
	readonly commitment: Bytes32;
	readonly validUntil: bigint;
	readonly finalized?: FinalizedIdentityV2;
}

export interface IdentityAccountRequestV2 {
	readonly session: string;
}

export interface IdentityAccountResultV2 {
	readonly account: Bytes32;
	readonly sessionExpiresAt: bigint;
	readonly finalized: FinalizedIdentityV2;
}

export interface IdentityProfileReadRequestV2 {
	readonly subject: Bytes32;
	readonly fields: readonly string[];
	readonly at?: Bytes32;
}

export interface IdentityProfileReadResultV2 {
	readonly receipt: IdentityReceiptV2;
}

export interface IdentityProfileDiscloseRequestV2 {
	readonly audience: string;
	readonly fields: readonly string[];
	readonly purpose: string;
	readonly expiresAt: bigint;
}

export interface IdentityProfileDiscloseResultV2 {
	readonly receipt: IdentityReceiptV2;
}

export interface IdentityHumanityStatusRequestV2 {
	readonly subject: Bytes32;
	readonly at?: Bytes32;
}

export interface IdentityHumanityStatusResultV2 {
	readonly status: number;
	readonly freshUntil: bigint;
	readonly finalized: FinalizedIdentityV2;
}

export interface IdentityHumanityProveRequestV2 {
	readonly audience: string;
	readonly challenge: Uint8Array;
	readonly expiresAt: bigint;
	readonly claims: readonly string[];
}

export interface IdentityHumanityProveResultV2 {
	readonly proof: Uint8Array;
	readonly derivedPublicKey: Bytes32;
	readonly proofHash: Bytes32;
	readonly continuity: boolean;
	readonly expiresAt: bigint;
}

export interface IdentitySubjectDeriveRequestV2 {
	readonly productId: string;
	readonly context: string;
	readonly verifierAudience: string;
	readonly epoch?: number;
}

export interface IdentitySubjectDeriveResultV2 {
	readonly subject: Bytes32;
	readonly derivedPublicKey: Bytes32;
	readonly epoch: number;
	readonly recoveryIncarnationHash: Bytes32;
	readonly continuity: boolean;
}

export interface IdentityEntitlementsReadRequestV2 {
	readonly subject: Bytes32;
	readonly scope: string;
	readonly at?: Bytes32;
}

export interface IdentityEntitlementsReadResultV2 {
	readonly allowed: boolean;
	readonly scope: string;
	readonly policyVersion: number;
	readonly expiresAt: bigint;
	readonly freshUntil: bigint;
	readonly finalized: FinalizedIdentityV2;
}

export interface TransactionSignRequestV2 {
	readonly payloadHash: Bytes32;
	readonly policyHash: Bytes32;
	readonly expiresAt: bigint;
}

export interface TransactionSignResultV2 {
	readonly transactionHash: Bytes32;
	readonly finalized: FinalizedIdentityV2;
}

export interface IdentityV2MethodMap {
	readonly "identity.account": {
		readonly input: IdentityAccountRequestV2;
		readonly output: IdentityAccountResultV2;
	};
	readonly "identity.profile.read": {
		readonly input: IdentityProfileReadRequestV2;
		readonly output: IdentityProfileReadResultV2;
	};
	readonly "identity.profile.disclose": {
		readonly input: IdentityProfileDiscloseRequestV2;
		readonly output: IdentityProfileDiscloseResultV2;
	};
	readonly "identity.humanity.status": {
		readonly input: IdentityHumanityStatusRequestV2;
		readonly output: IdentityHumanityStatusResultV2;
	};
	readonly "identity.humanity.prove": {
		readonly input: IdentityHumanityProveRequestV2;
		readonly output: IdentityHumanityProveResultV2;
	};
	readonly "identity.subject.derive": {
		readonly input: IdentitySubjectDeriveRequestV2;
		readonly output: IdentitySubjectDeriveResultV2;
	};
	readonly "identity.entitlements.read": {
		readonly input: IdentityEntitlementsReadRequestV2;
		readonly output: IdentityEntitlementsReadResultV2;
	};
	readonly "transaction.sign": {
		readonly input: TransactionSignRequestV2;
		readonly output: TransactionSignResultV2;
	};
}

export interface IdentityGrantV2<Operation extends IdentityV2Call> {
	readonly version: 2;
	readonly id: Bytes32;
	readonly productId: string;
	readonly scope: Operation;
	readonly recoveryIncarnation: Bytes32;
	readonly expiresAt: bigint;
	readonly audience?: string;
	readonly revoked?: boolean;
}

export interface IdentityInvocationV2<Operation extends IdentityV2Call> {
	readonly protocol: "cord.origin.host/2";
	readonly code: (typeof IDENTITY_V2_OPERATION_CODES)[Operation];
	readonly operation: Operation;
	readonly productId: string;
	readonly grantId: Bytes32;
	readonly recoveryIncarnation: Bytes32;
	readonly input: IdentityV2MethodMap[Operation]["input"];
	readonly operationId?: Bytes16;
}

export interface IdentityV2Bridge {
	request<Operation extends IdentityV2Call>(
		invocation: IdentityInvocationV2<Operation>,
		signal?: AbortSignal,
	): Promise<SdkResult<unknown>>;
}

export interface IdentityV2InvocationOptions {
	readonly finalizedBlock: bigint;
	readonly currentRecoveryIncarnation: Bytes32;
	readonly operationId?: Bytes16;
}

export interface IdentityRecoveryEvidenceV2 {
	readonly sameStore: boolean;
	readonly authenticated: boolean;
	readonly completeReplayJournal: boolean;
	readonly monotonic: boolean;
}

export type IdentityRecoveryDispositionV2 =
	| { readonly continuity: true; readonly action: "restore-complete-store" }
	| { readonly continuity: false; readonly action: "install-fresh-root"; readonly epoch: 0 };

/** Only a complete, authenticated, monotonic same-store restart may preserve subject continuity. */
export function identityRecoveryDispositionV2(
	evidence: IdentityRecoveryEvidenceV2,
): IdentityRecoveryDispositionV2 {
	if (evidence.sameStore && evidence.authenticated && evidence.completeReplayJournal && evidence.monotonic) {
		return { continuity: true, action: "restore-complete-store" };
	}
	return { continuity: false, action: "install-fresh-root", epoch: 0 };
}

export interface IdentityV2Client {
	account(
		grant: IdentityGrantV2<"identity.account">,
		input: IdentityAccountRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityAccountResultV2>>;
	profileRead(
		grant: IdentityGrantV2<"identity.profile.read">,
		input: IdentityProfileReadRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityProfileReadResultV2>>;
	profileDisclose(
		grant: IdentityGrantV2<"identity.profile.disclose">,
		input: IdentityProfileDiscloseRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityProfileDiscloseResultV2>>;
	humanityStatus(
		grant: IdentityGrantV2<"identity.humanity.status">,
		input: IdentityHumanityStatusRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityHumanityStatusResultV2>>;
	humanityProve(
		grant: IdentityGrantV2<"identity.humanity.prove">,
		input: IdentityHumanityProveRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityHumanityProveResultV2>>;
	subjectDerive(
		grant: IdentityGrantV2<"identity.subject.derive">,
		input: IdentitySubjectDeriveRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentitySubjectDeriveResultV2>>;
	entitlementsRead(
		grant: IdentityGrantV2<"identity.entitlements.read">,
		input: IdentityEntitlementsReadRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityEntitlementsReadResultV2>>;
	signTransaction(
		grant: IdentityGrantV2<"transaction.sign">,
		input: TransactionSignRequestV2,
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<TransactionSignResultV2>>;
}

const FRESH_CONSENT = new Set<IdentityV2Call>([
	"identity.profile.disclose",
	"identity.humanity.prove",
	"transaction.sign",
]);
const utf8 = new TextEncoder();

function identityError<T>(code: string, message: string): SdkResult<T> {
	return err(new OriginSdkError({
		source: "identity-v2",
		domain: "host-contract",
		code,
		message,
		retryable: false,
	}));
}

function bytes(value: unknown, length: number, label: string): Uint8Array {
	if (!(value instanceof Uint8Array) || value.length !== length) {
		throw new TypeError(`${label} must contain exactly ${length} bytes`);
	}
	return value.slice();
}

function uint(value: unknown, maximum: bigint, label: string): bigint {
	if (typeof value !== "bigint" || value < 0n || value > maximum) {
		throw new TypeError(`${label} must be an unsigned integer`);
	}
	return value;
}

function u32(value: unknown, label: string): number {
	if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 0xffff_ffff) {
		throw new TypeError(`${label} must be a u32`);
	}
	return value as number;
}

function text(value: unknown, maximum: number, label: string): string {
	if (typeof value !== "string" || value.normalize("NFC") !== value) {
		throw new TypeError(`${label} must be NFC text`);
	}
	const size = utf8.encode(value).length;
	if (size < 1 || size > maximum) throw new TypeError(`${label} must contain 1-${maximum} UTF-8 bytes`);
	return value;
}

function stringList(value: unknown, minimum: number, maximum: number, label: string): readonly string[] {
	if (!Array.isArray(value) || value.length < minimum || value.length > maximum) {
		throw new TypeError(`${label} must contain ${minimum}-${maximum} values`);
	}
	return value.map((item, index) => text(item, 128, `${label}[${index}]`));
}

function record(value: unknown, keys: readonly string[], label: string): Record<string, unknown> {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		throw new TypeError(`${label} must be a record`);
	}
	const actual = Object.keys(value).sort();
	const allowed = [...keys].sort();
	if (actual.length !== allowed.length || actual.some((key, index) => key !== allowed[index])) {
		throw new TypeError(`${label} contains joined or unknown fields`);
	}
	return value as Record<string, unknown>;
}

function finalized(value: unknown): FinalizedIdentityV2 {
	const item = record(value, ["blockNumber", "blockHash"], "finalized identity");
	return {
		blockNumber: uint(item.blockNumber, 0xffff_ffff_ffff_ffffn, "finalized block number"),
		blockHash: bytes(item.blockHash, 32, "finalized block hash"),
	};
}

function receipt(value: unknown): IdentityReceiptV2 {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		throw new TypeError("identity receipt must be a record");
	}
	const candidate = value as Record<string, unknown>;
	const keys = candidate.finalized === undefined
		? ["commitment", "validUntil"]
		: ["commitment", "validUntil", "finalized"];
	const item = record(value, keys, "identity receipt");
	return {
		commitment: bytes(item.commitment, 32, "identity receipt commitment"),
		validUntil: uint(item.validUntil, 0xffff_ffff_ffff_ffffn, "identity receipt validity"),
		...(item.finalized === undefined ? {} : { finalized: finalized(item.finalized) }),
	};
}

function validateInput<Operation extends IdentityV2Call>(
	operation: Operation,
	input: IdentityV2MethodMap[Operation]["input"],
): void {
	switch (operation) {
		case "identity.account":
			text((input as IdentityAccountRequestV2).session, 128, "session");
			break;
		case "identity.profile.read": {
			const value = input as IdentityProfileReadRequestV2;
			bytes(value.subject, 32, "subject");
			stringList(value.fields, 1, 64, "profile fields");
			if (value.at !== undefined) bytes(value.at, 32, "profile finalized hash");
			break;
		}
		case "identity.profile.disclose": {
			const value = input as IdentityProfileDiscloseRequestV2;
			text(value.audience, 256, "disclosure audience");
			stringList(value.fields, 1, 64, "disclosure fields");
			text(value.purpose, 256, "disclosure purpose");
			uint(value.expiresAt, 0xffff_ffff_ffff_ffffn, "disclosure expiry");
			break;
		}
		case "identity.humanity.status": {
			const value = input as IdentityHumanityStatusRequestV2;
			bytes(value.subject, 32, "humanity subject");
			if (value.at !== undefined) bytes(value.at, 32, "humanity finalized hash");
			break;
		}
		case "identity.humanity.prove": {
			const value = input as IdentityHumanityProveRequestV2;
			text(value.audience, 256, "proof audience");
			if (!(value.challenge instanceof Uint8Array) || value.challenge.length < 16 || value.challenge.length > 64) {
				throw new TypeError("proof challenge must contain 16-64 bytes");
			}
			uint(value.expiresAt, 0xffff_ffff_ffff_ffffn, "proof expiry");
			stringList(value.claims, 0, 64, "proof claims");
			break;
		}
		case "identity.subject.derive": {
			const value = input as IdentitySubjectDeriveRequestV2;
			text(value.productId, 128, "subject product");
			text(value.context, 256, "subject context");
			text(value.verifierAudience, 256, "subject verifier audience");
			if (value.epoch !== undefined) u32(value.epoch, "subject epoch");
			break;
		}
		case "identity.entitlements.read": {
			const value = input as IdentityEntitlementsReadRequestV2;
			bytes(value.subject, 32, "entitlement subject");
			text(value.scope, 256, "entitlement scope");
			if (value.at !== undefined) bytes(value.at, 32, "entitlement finalized hash");
			break;
		}
		case "transaction.sign": {
			const value = input as TransactionSignRequestV2;
			bytes(value.payloadHash, 32, "transaction payload hash");
			bytes(value.policyHash, 32, "transaction policy hash");
			uint(value.expiresAt, 0xffff_ffff_ffff_ffffn, "transaction expiry");
			break;
		}
	}
}

function audienceOf(operation: IdentityV2Call, input: IdentityV2MethodMap[IdentityV2Call]["input"]): string | undefined {
	switch (operation) {
		case "identity.profile.disclose": return (input as IdentityProfileDiscloseRequestV2).audience;
		case "identity.humanity.prove": return (input as IdentityHumanityProveRequestV2).audience;
		case "identity.subject.derive": return (input as IdentitySubjectDeriveRequestV2).verifierAudience;
		default: return undefined;
	}
}

function validateResult<Operation extends IdentityV2Call>(
	operation: Operation,
	value: unknown,
): IdentityV2MethodMap[Operation]["output"] {
	let output: unknown;
	switch (operation) {
		case "identity.account": {
			const item = record(value, ["account", "sessionExpiresAt", "finalized"], "identity.account result");
			output = {
				account: bytes(item.account, 32, "account"),
				sessionExpiresAt: uint(item.sessionExpiresAt, 0xffff_ffff_ffff_ffffn, "session expiry"),
				finalized: finalized(item.finalized),
			};
			break;
		}
		case "identity.profile.read":
		case "identity.profile.disclose": {
			const item = record(value, ["receipt"], `${operation} result`);
			output = { receipt: receipt(item.receipt) };
			break;
		}
		case "identity.humanity.status": {
			const item = record(value, ["status", "freshUntil", "finalized"], "identity.humanity.status result");
			const status = u32(item.status, "humanity status");
			if (status > 0xffff) throw new TypeError("humanity status must be a u16");
			output = {
				status,
				freshUntil: uint(item.freshUntil, 0xffff_ffff_ffff_ffffn, "humanity freshness"),
				finalized: finalized(item.finalized),
			};
			break;
		}
		case "identity.humanity.prove": {
			const item = record(
				value,
				["proof", "derivedPublicKey", "proofHash", "continuity", "expiresAt"],
				"identity.humanity.prove result",
			);
			if (!(item.proof instanceof Uint8Array) || item.proof.length < 64 || item.proof.length > 4_096) {
				throw new TypeError("humanity proof must contain 64-4096 bytes");
			}
			if (typeof item.continuity !== "boolean") throw new TypeError("humanity continuity must be boolean");
			output = {
				proof: item.proof.slice(),
				derivedPublicKey: bytes(item.derivedPublicKey, 32, "humanity proof key"),
				proofHash: bytes(item.proofHash, 32, "humanity proof hash"),
				continuity: item.continuity,
				expiresAt: uint(item.expiresAt, 0xffff_ffff_ffff_ffffn, "humanity proof expiry"),
			};
			break;
		}
		case "identity.subject.derive": {
			const item = record(
				value,
				["subject", "derivedPublicKey", "epoch", "recoveryIncarnationHash", "continuity"],
				"identity.subject.derive result",
			);
			if (typeof item.continuity !== "boolean") throw new TypeError("subject continuity must be boolean");
			output = {
				subject: bytes(item.subject, 32, "derived subject"),
				derivedPublicKey: bytes(item.derivedPublicKey, 32, "derived subject key"),
				epoch: u32(item.epoch, "subject epoch"),
				recoveryIncarnationHash: bytes(item.recoveryIncarnationHash, 32, "recovery incarnation hash"),
				continuity: item.continuity,
			};
			break;
		}
		case "identity.entitlements.read": {
			const item = record(
				value,
				["allowed", "scope", "policyVersion", "expiresAt", "freshUntil", "finalized"],
				"identity.entitlements.read result",
			);
			if (typeof item.allowed !== "boolean") throw new TypeError("entitlement decision must be boolean");
			output = {
				allowed: item.allowed,
				scope: text(item.scope, 256, "entitlement result scope"),
				policyVersion: u32(item.policyVersion, "entitlement policy version"),
				expiresAt: uint(item.expiresAt, 0xffff_ffff_ffff_ffffn, "entitlement expiry"),
				freshUntil: uint(item.freshUntil, 0xffff_ffff_ffff_ffffn, "entitlement freshness"),
				finalized: finalized(item.finalized),
			};
			break;
		}
		case "transaction.sign": {
			const item = record(value, ["transactionHash", "finalized"], "transaction.sign result");
			output = {
				transactionHash: bytes(item.transactionHash, 32, "transaction hash"),
				finalized: finalized(item.finalized),
			};
			break;
		}
	}
	return output as IdentityV2MethodMap[Operation]["output"];
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function createIdentityV2Client(productId: string, bridge: IdentityV2Bridge): IdentityV2Client {
	text(productId, 128, "product id");

	async function invoke<Operation extends IdentityV2Call>(
		operation: Operation,
		grant: IdentityGrantV2<Operation>,
		input: IdentityV2MethodMap[Operation]["input"],
		options: IdentityV2InvocationOptions,
		signal?: AbortSignal,
	): Promise<SdkResult<IdentityV2MethodMap[Operation]["output"]>> {
		if (signal?.aborted) return identityError("REQUEST_CANCELLED", "Identity request was cancelled");
		try {
			validateInput(operation, input);
			bytes(grant.id, 32, "grant id");
			bytes(grant.recoveryIncarnation, 32, "grant recovery incarnation");
			bytes(options.currentRecoveryIncarnation, 32, "current recovery incarnation");
		} catch (error) {
			return identityError("WIRE_SCHEMA_INVALID", error instanceof Error ? error.message : "Invalid identity request");
		}
		if (grant.version !== 2 || grant.scope !== operation || grant.productId !== productId) {
			return identityError("GRANT_SCOPE_DENIED", "Identity grant does not match the exact operation and product");
		}
		if (grant.revoked === true) return identityError("GRANT_REVOKED", "Identity grant was revoked");
		if (grant.expiresAt <= options.finalizedBlock) return identityError("GRANT_EXPIRED", "Identity grant expired");
		if (!equalBytes(grant.recoveryIncarnation, options.currentRecoveryIncarnation)) {
			return identityError("IDENTITY_OLD_INCARNATION", "Identity grant belongs to an old recovery incarnation");
		}
		const audience = audienceOf(operation, input as IdentityV2MethodMap[IdentityV2Call]["input"]);
		if (audience !== undefined && grant.audience !== audience) {
			return identityError("IDENTITY_AUDIENCE_INVALID", "Identity grant audience does not match the request");
		}
		if (FRESH_CONSENT.has(operation)) {
			try { bytes(options.operationId, 16, "fresh-consent operation id"); }
			catch (error) {
				return identityError("SIGNING_CONSENT_REQUIRED", error instanceof Error ? error.message : "Fresh consent required");
			}
		} else if (options.operationId !== undefined) {
			return identityError("WIRE_SCHEMA_INVALID", "Read-only Identity operation cannot carry an operation id");
		}

		const response = await bridge.request({
			protocol: "cord.origin.host/2",
			code: IDENTITY_V2_OPERATION_CODES[operation],
			operation,
			productId,
			grantId: grant.id.slice(),
			recoveryIncarnation: grant.recoveryIncarnation.slice(),
			input,
			...(options.operationId === undefined ? {} : { operationId: options.operationId.slice() }),
		}, signal);
		if (!response.success) return response;
		try {
			return { success: true, value: validateResult(operation, response.value) };
		} catch (error) {
			return identityError("WIRE_SCHEMA_INVALID", error instanceof Error ? error.message : "Invalid identity response");
		}
	}

	return {
		account: (grant, input, options, signal) => invoke("identity.account", grant, input, options, signal),
		profileRead: (grant, input, options, signal) => invoke("identity.profile.read", grant, input, options, signal),
		profileDisclose: (grant, input, options, signal) =>
			invoke("identity.profile.disclose", grant, input, options, signal),
		humanityStatus: (grant, input, options, signal) =>
			invoke("identity.humanity.status", grant, input, options, signal),
		humanityProve: (grant, input, options, signal) =>
			invoke("identity.humanity.prove", grant, input, options, signal),
		subjectDerive: (grant, input, options, signal) =>
			invoke("identity.subject.derive", grant, input, options, signal),
		entitlementsRead: (grant, input, options, signal) =>
			invoke("identity.entitlements.read", grant, input, options, signal),
		signTransaction: (grant, input, options, signal) =>
			invoke("transaction.sign", grant, input, options, signal),
	};
}
