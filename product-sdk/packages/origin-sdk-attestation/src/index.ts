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

import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import type { AccountId, Hash32, Versioned } from "@cord-network/origin-sdk-identity";
import { accountId, hash32 } from "@cord-network/origin-sdk-identity";
import { err } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

declare const nativeAttestationType: unique symbol;
export type SchemaId = Hash32 & { readonly [nativeAttestationType]: "SchemaId" };
export type AttestationId = Hash32 & { readonly [nativeAttestationType]: "AttestationId" };
export type SubjectCommitment = Hash32 & { readonly [nativeAttestationType]: "SubjectCommitment" };
export type PayloadCommitment = Hash32 & { readonly [nativeAttestationType]: "PayloadCommitment" };
export type StatusCommitment = Hash32 & { readonly [nativeAttestationType]: "StatusCommitment" };
export type UniquenessCommitment = Hash32 & { readonly [nativeAttestationType]: "UniquenessCommitment" };
export type IssuerSignature = string & { readonly [nativeAttestationType]: "IssuerSignature" };
export type BlockHash = Hash32 & { readonly [nativeAttestationType]: "BlockHash" };
export type BlockNumber = string & { readonly [nativeAttestationType]: "BlockNumber" };
export type DecimalU64 = string & { readonly [nativeAttestationType]: "DecimalU64" };

export interface PageInput { readonly cursor?: number | null; readonly limit?: number; }
export interface PageRequest { readonly cursor: number | null; readonly limit: number; }
export interface IdPage<Id> {
  readonly version: 1;
  readonly items: readonly Id[];
  readonly next_cursor: number | null;
  readonly finalized_hash: BlockHash;
}

const invalidResult = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "attestation",
  domain: "input",
  code: "invalid_input",
  message,
  retryable: false,
}));
function invalidDomainInput(_domain: string, operation: string, message: string): never {
  throw new OriginSdkError({ source: "attestation", domain: operation, code: "invalid_input", message, retryable: false });
}
export function page(input: PageInput = {}): PageRequest {
  const cursor = input.cursor ?? null;
  const limit = input.limit ?? 50;
  if (cursor !== null && (!Number.isSafeInteger(cursor) || cursor < 0 || cursor > 0xffff_ffff))
    throw new TypeError("cursor must be a u32");
  if (!Number.isSafeInteger(limit) || limit < 0) throw new TypeError("limit must be non-negative");
  return { cursor, limit: Math.min(limit, 100) };
}
const nativeHash = <T extends Hash32>(value: string, field: string): T => {
  hash32(value);
  return value as T;
};
export const schemaId = (value: string): SchemaId => nativeHash<SchemaId>(value, "schema");
export const attestationId = (value: string): AttestationId => nativeHash<AttestationId>(value, "attestation");
export const subjectCommitment = (value: string): SubjectCommitment => nativeHash<SubjectCommitment>(value, "subject commitment");
export const payloadCommitment = (value: string): PayloadCommitment => nativeHash<PayloadCommitment>(value, "payload commitment");
export const statusCommitment = (value: string): StatusCommitment => nativeHash<StatusCommitment>(value, "status commitment");
export const uniquenessCommitment = (value: string): UniquenessCommitment => nativeHash<UniquenessCommitment>(value, "uniqueness commitment");
export const blockHash = (value: string): BlockHash => nativeHash<BlockHash>(value, "block hash");
function decimal(value: string | number, bits: 32 | 64, field: string): string {
  const text = String(value);
  const max = bits === 32 ? 0xffff_ffffn : 0xffff_ffff_ffff_ffffn;
  if (!/^(0|[1-9][0-9]*)$/.test(text) || BigInt(text) > max) throw new TypeError(`${field} must be a u${bits}`);
  return text;
}
export const blockNumber = (value: string | number): BlockNumber => decimal(value, 32, "block number") as BlockNumber;
export const decimalU64 = (value: string | number): DecimalU64 => decimal(value, 64, "decimal") as DecimalU64;
export function issuerSignature(value: string): IssuerSignature {
  if (!/^0x(?:[0-9a-f]{128}|[0-9a-f]{130})$/.test(value)) throw new TypeError("issuer signature has an invalid length");
  return value as IssuerSignature;
}

export type SchemaStatus = "active" | "paused" | "retired";
export type IndexPolicy = "none" | "issuer" | "subject_and_schema" | "issuer_and_subject_schema";
export const DELEGATED_ISSUE_DOMAIN = "cord:orbis:delegated-attestation:v1" as const;
export const DELEGATED_REVOKE_DOMAIN = "cord:orbis:delegated-revocation:v1" as const;
export const EXTERNAL_STATUS_DOMAIN = "cord:orbis:external-status:v1" as const;

declare const attestationType: unique symbol;
export type SchemaDefinition = string & { readonly [attestationType]: "SchemaDefinition" };

export function schemaDefinition(value: string): SchemaDefinition {
  const bytes = new TextEncoder().encode(value).length;
  if (bytes < 1 || bytes > 16 * 1024) {
    invalidDomainInput(
      "attestation",
      "schema_definition",
      "schema definition must contain 1-16384 UTF-8 bytes",
    );
  }
  return value as SchemaDefinition;
}

export interface SchemaView {
  readonly schema: SchemaId;
  readonly creator: AccountId;
  readonly definition: SchemaDefinition;
  readonly definition_commitment: Hash32;
  readonly status: SchemaStatus;
  readonly revocable: boolean;
  readonly unique: boolean;
  readonly index_policy: IndexPolicy;
  readonly authorized_issuers: readonly AccountId[];
  readonly created_at: BlockNumber;
}

export interface AttestationInput {
  readonly schema: SchemaId;
  readonly subject_commitment: SubjectCommitment;
  readonly payload_commitment: PayloadCommitment;
  readonly status_commitment: StatusCommitment;
  readonly parent: AttestationId | null;
  readonly expiry: BlockNumber | null;
  readonly uniqueness_commitment: UniquenessCommitment | null;
  readonly revocable: boolean;
}

export interface AttestationView extends AttestationInput {
  readonly attestation: AttestationId;
	readonly issuer: AccountId;
	readonly issuance_nonce: DecimalU64;
	readonly issued_at: BlockNumber;
  readonly revoked_at: BlockNumber | null;
  readonly revoked_by: AccountId | null;
}

export interface ExternalStatusView {
  readonly key: Hash32;
  readonly issuer: AccountId;
  readonly status_commitment: StatusCommitment;
  readonly revoked_at: BlockNumber;
}

export type AttestationEventKind =
  | "schema_created"
  | "schema_status_changed"
  | "attestation_issued"
  | "delegated_intent_consumed"
  | "delegated_revocation_consumed"
  | "attestation_revoked"
  | "external_status_revoked"
  | "emergency_pause_changed";

export type AttestationEvent =
  | {
      readonly event: "schema_created";
      readonly data: {
        readonly schema: SchemaId;
        readonly creator: AccountId;
        readonly definition_commitment: Hash32;
        readonly revocable: boolean;
        readonly unique: boolean;
        readonly index_policy: IndexPolicy;
      };
    }
  | {
      readonly event: "schema_status_changed";
      readonly data: { readonly schema: SchemaId; readonly status: SchemaStatus; readonly forced: boolean };
    }
  | {
      readonly event: "attestation_issued";
      readonly data: {
        readonly attestation: AttestationId;
        readonly schema: SchemaId;
        readonly issuer: AccountId;
        readonly subject_commitment: SubjectCommitment;
      };
    }
  | {
      readonly event: "delegated_intent_consumed";
      readonly data: {
        readonly issuer: AccountId;
        readonly delegate: AccountId;
        readonly nonce: DecimalU64;
        readonly attestation: AttestationId;
      };
    }
  | {
      readonly event: "delegated_revocation_consumed";
      readonly data: {
        readonly revoker: AccountId;
        readonly delegate: AccountId;
        readonly nonce: DecimalU64;
        readonly attestation: AttestationId;
      };
    }
  | {
      readonly event: "attestation_revoked";
      readonly data: {
        readonly attestation: AttestationId;
        readonly by: AccountId | null;
        readonly forced: boolean;
      };
    }
  | {
      readonly event: "external_status_revoked";
      readonly data: {
        readonly key: Hash32;
        readonly issuer: AccountId;
        readonly status_commitment: StatusCommitment;
        readonly revoked_at: BlockNumber;
      };
    }
  | { readonly event: "emergency_pause_changed"; readonly data: { readonly paused: boolean } };

export type AttestationOutcome =
  | { readonly outcome: "schema_available"; readonly data: { readonly schema: SchemaId } }
  | {
      readonly outcome: "schema_status_changed";
      readonly data: { readonly schema: SchemaId; readonly status: SchemaStatus };
    }
  | {
      readonly outcome: "attestation_available";
      readonly data: { readonly attestation: AttestationId };
    }
  | {
      readonly outcome: "delegation_consumed";
      readonly data: {
        readonly account: AccountId;
        readonly nonce: DecimalU64;
        readonly attestation: AttestationId;
      };
    }
  | {
      readonly outcome: "attestation_revoked";
      readonly data: { readonly attestation: AttestationId };
    }
  | { readonly outcome: "external_status_revoked"; readonly data: { readonly key: Hash32 } }
  | { readonly outcome: "emergency_pause_changed"; readonly data: { readonly paused: boolean } };

export interface FinalizedAttestationEvent {
  readonly finalized_block_hash: BlockHash;
  readonly event_index: number;
  readonly event: AttestationEvent;
}

export interface AttestationEventSubscription {
  readonly finality: "finalized";
  readonly from_finalized_block: BlockHash;
  readonly kinds: readonly AttestationEventKind[];
}

export function attestationEventOutcome(event: AttestationEvent): AttestationOutcome {
  switch (event.event) {
    case "schema_created":
      return { outcome: "schema_available", data: { schema: event.data.schema } };
    case "schema_status_changed":
      return {
        outcome: "schema_status_changed",
        data: { schema: event.data.schema, status: event.data.status },
      };
    case "attestation_issued":
      return { outcome: "attestation_available", data: { attestation: event.data.attestation } };
    case "delegated_intent_consumed":
      return {
        outcome: "delegation_consumed",
        data: { account: event.data.issuer, nonce: event.data.nonce, attestation: event.data.attestation },
      };
    case "delegated_revocation_consumed":
      return {
        outcome: "delegation_consumed",
        data: { account: event.data.revoker, nonce: event.data.nonce, attestation: event.data.attestation },
      };
    case "attestation_revoked":
      return { outcome: "attestation_revoked", data: { attestation: event.data.attestation } };
    case "external_status_revoked":
      return { outcome: "external_status_revoked", data: { key: event.data.key } };
    case "emergency_pause_changed":
      return { outcome: "emergency_pause_changed", data: { paused: event.data.paused } };
  }
}

export function attestationEventSubscription(
  from_finalized_block: BlockHash,
  kinds: readonly AttestationEventKind[],
): AttestationEventSubscription {
  if (kinds.length < 1 || kinds.length > 8 || new Set(kinds).size !== kinds.length) {
    invalidDomainInput(
      "attestation",
      "event_subscription",
      "subscription requires 1-8 unique event kinds",
    );
  }
  return { finality: "finalized", from_finalized_block, kinds: [...kinds] };
}

export interface AttestationLiveStatus {
  readonly version: 1;
  readonly exists: boolean;
  readonly live: boolean;
  readonly evaluated_at: BlockNumber;
  readonly expiry: BlockNumber | null;
  readonly revoked_at: BlockNumber | null;
}

export interface DelegatedIssueIntent extends AttestationInput {
  readonly genesis_hash: BlockHash;
  readonly spec_version: number;
  readonly action: "issue";
  readonly issuer: AccountId;
  readonly delegate: AccountId;
  readonly nonce: DecimalU64;
  readonly deadline: BlockNumber;
}

export interface DelegatedRevokeIntent {
  readonly genesis_hash: BlockHash;
  readonly spec_version: number;
  readonly action: "revoke";
  readonly revoker: AccountId;
  readonly delegate: AccountId;
  readonly attestation: AttestationId;
  readonly nonce: DecimalU64;
  readonly deadline: BlockNumber;
}

export interface SignedDelegatedIssue {
  readonly intent: DelegatedIssueIntent;
  readonly signature: IssuerSignature;
}

export interface SignedDelegatedRevoke {
  readonly intent: DelegatedRevokeIntent;
  readonly signature: IssuerSignature;
}

export type DelegatedSignatureScheme = "sr25519" | "ed25519" | "ecdsa";
export const SUPPORTED_DELEGATED_SIGNATURE_SCHEMES = ["sr25519", "ed25519", "ecdsa"] as const;

/** Exact `(domain, intent)` SCALE bytes verified by the native runtime. */
export function delegatedIssueSigningPayload(intent: DelegatedIssueIntent): Uint8Array {
	return concatBytes(
		scaleByteString(DELEGATED_ISSUE_DOMAIN),
		hashBytes(intent.genesis_hash, "genesis_hash"),
		u32Bytes(intent.spec_version, "spec_version"),
		Uint8Array.of(0),
		ss58AccountBytes(intent.issuer, "issuer"),
		ss58AccountBytes(intent.delegate, "delegate"),
		hashBytes(intent.schema, "schema"),
		hashBytes(intent.subject_commitment, "subject_commitment"),
		hashBytes(intent.payload_commitment, "payload_commitment"),
		hashBytes(intent.status_commitment, "status_commitment"),
		optionBytes(intent.parent, (value) => hashBytes(value, "parent")),
		optionBytes(intent.expiry, (value) => u32DecimalBytes(value, "expiry")),
		optionBytes(intent.uniqueness_commitment, (value) =>
			hashBytes(value, "uniqueness_commitment"),
		),
		Uint8Array.of(intent.revocable ? 1 : 0),
		u64DecimalBytes(intent.nonce, "nonce"),
		u32DecimalBytes(intent.deadline, "deadline"),
	);
}

/** Exact `(domain, intent)` SCALE bytes verified by the native runtime. */
export function delegatedRevokeSigningPayload(intent: DelegatedRevokeIntent): Uint8Array {
	return concatBytes(
		scaleByteString(DELEGATED_REVOKE_DOMAIN),
		hashBytes(intent.genesis_hash, "genesis_hash"),
		u32Bytes(intent.spec_version, "spec_version"),
		Uint8Array.of(1),
		ss58AccountBytes(intent.revoker, "revoker"),
		ss58AccountBytes(intent.delegate, "delegate"),
		hashBytes(intent.attestation, "attestation"),
		u64DecimalBytes(intent.nonce, "nonce"),
		u32DecimalBytes(intent.deadline, "deadline"),
	);
}

export function signingPayloadHex(payload: Uint8Array): string {
	return `0x${Array.from(payload, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function ensureBatch(length: number, operation: string): void {
  if (length < 1 || length > 64) {
    invalidDomainInput("attestation", operation, "batch must contain 1-64 items");
  }
}

const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function concatBytes(...parts: readonly Uint8Array[]): Uint8Array {
	const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
	let offset = 0;
	for (const part of parts) {
		output.set(part, offset);
		offset += part.length;
	}
	return output;
}

function scaleByteString(value: string): Uint8Array {
	const bytes = new TextEncoder().encode(value);
	if (bytes.length >= 64) invalidDomainInput("attestation", "signing_payload", "domain is too long");
	return concatBytes(Uint8Array.of(bytes.length << 2), bytes);
}

function hashBytes(value: string, field: string): Uint8Array {
	if (!/^0x[0-9a-fA-F]{64}$/.test(value)) {
		invalidDomainInput("attestation", "signing_payload", `${field} must be a 32-byte hash`);
	}
	return Uint8Array.from(value.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
}

function u32Bytes(value: number, field: string): Uint8Array {
	if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
		invalidDomainInput("attestation", "signing_payload", `${field} must be a u32`);
	}
	return Uint8Array.of(value, value >>> 8, value >>> 16, value >>> 24);
}

function u32DecimalBytes(value: string, field: string): Uint8Array {
	const integer = BigInt(value);
	if (integer < 0n || integer > 0xffff_ffffn) {
		invalidDomainInput("attestation", "signing_payload", `${field} must fit the runtime u32`);
	}
	return u32Bytes(Number(integer), field);
}

function u64DecimalBytes(value: string, field: string): Uint8Array {
	let integer = BigInt(value);
	if (integer < 0n || integer > 0xffff_ffff_ffff_ffffn) {
		invalidDomainInput("attestation", "signing_payload", `${field} must be a u64`);
	}
	const output = new Uint8Array(8);
	for (let index = 0; index < output.length; index += 1) {
		output[index] = Number(integer & 0xffn);
		integer >>= 8n;
	}
	return output;
}

function optionBytes<T>(value: T | null, encode: (value: T) => Uint8Array): Uint8Array {
	return value === null ? Uint8Array.of(0) : concatBytes(Uint8Array.of(1), encode(value));
}

function ss58AccountBytes(value: string, field: string): Uint8Array {
	let integer = 0n;
	for (const character of value) {
		const digit = BASE58_ALPHABET.indexOf(character);
		if (digit < 0) {
			invalidDomainInput("attestation", "signing_payload", `${field} must be an SS58 account`);
		}
		integer = integer * 58n + BigInt(digit);
	}
	const decoded: number[] = [];
	while (integer > 0n) {
		decoded.push(Number(integer & 0xffn));
		integer >>= 8n;
	}
	decoded.reverse();
	for (const character of value) {
		if (character !== "1") break;
		decoded.unshift(0);
	}
	const bytes = Uint8Array.from(decoded);
	if (bytes.length < 35 || bytes[0]! >= 128) {
		invalidDomainInput("attestation", "signing_payload", `${field} must encode AccountId32`);
	}
	const prefixLength = (bytes[0]! & 0x40) === 0 ? 1 : 2;
	if (bytes.length !== prefixLength + 34) {
		invalidDomainInput("attestation", "signing_payload", `${field} must encode AccountId32`);
	}
	return bytes.slice(prefixLength, prefixLength + 32);
}

export interface CreateSchemaInput {
  readonly definition: SchemaDefinition;
  readonly authorized_issuers: readonly AccountId[];
  readonly revocable: boolean;
  readonly unique: boolean;
  readonly index_policy: IndexPolicy;
}

export interface AttestationRuntimeAdapter {
  schemaById(at: `0x${string}`, schema: SchemaId, signal?: AbortSignal): Promise<Versioned<SchemaView>>;
  attestationById(at: `0x${string}`, attestation: AttestationId, signal?: AbortSignal): Promise<Versioned<AttestationView>>;
  liveStatus(at: `0x${string}`, attestation: AttestationId, signal?: AbortSignal): Promise<AttestationLiveStatus>;
  creatorSchemas(at: `0x${string}`, creator: AccountId, request: PageRequest, signal?: AbortSignal): Promise<IdPage<SchemaId>>;
  issuerAttestations(at: `0x${string}`, issuer: AccountId, request: PageRequest, signal?: AbortSignal): Promise<IdPage<AttestationId>>;
  subjectSchemaAttestations(at: `0x${string}`, subject: SubjectCommitment, schema: SchemaId, request: PageRequest, signal?: AbortSignal): Promise<IdPage<AttestationId>>;
  nextDelegatedNonce(at: `0x${string}`, issuer: AccountId, signal?: AbortSignal): Promise<DecimalU64>;
  schemaCount(at: `0x${string}`, signal?: AbortSignal): Promise<DecimalU64>;
  attestationCount(at: `0x${string}`, signal?: AbortSignal): Promise<DecimalU64>;
  nextIssuanceNonce(at: `0x${string}`, issuer: AccountId, signal?: AbortSignal): Promise<DecimalU64>;
  externalStatus(at: `0x${string}`, issuer: AccountId, status: StatusCommitment, signal?: AbortSignal): Promise<Versioned<ExternalStatusView>>;
  createSchema(at: `0x${string}`, input: CreateSchemaInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  setSchemaStatus(at: `0x${string}`, schema: SchemaId, status: SchemaStatus, signal?: AbortSignal): Promise<PreparedTransaction>;
  issue(at: `0x${string}`, input: AttestationInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  issueDelegated(at: `0x${string}`, input: SignedDelegatedIssue, signal?: AbortSignal): Promise<PreparedTransaction>;
  issueBatch(at: `0x${string}`, items: readonly AttestationInput[], signal?: AbortSignal): Promise<PreparedTransaction>;
  revoke(at: `0x${string}`, attestation: AttestationId, signal?: AbortSignal): Promise<PreparedTransaction>;
  revokeDelegated(at: `0x${string}`, input: SignedDelegatedRevoke, signal?: AbortSignal): Promise<PreparedTransaction>;
  issueDelegatedBatch(at: `0x${string}`, items: readonly SignedDelegatedIssue[], signal?: AbortSignal): Promise<PreparedTransaction>;
  revokeBatch(at: `0x${string}`, attestations: readonly AttestationId[], signal?: AbortSignal): Promise<PreparedTransaction>;
  revokeDelegatedBatch(at: `0x${string}`, items: readonly SignedDelegatedRevoke[], signal?: AbortSignal): Promise<PreparedTransaction>;
  revokeExternalStatus(at: `0x${string}`, status: StatusCommitment, signal?: AbortSignal): Promise<PreparedTransaction>;
  revokeExternalStatusBatch(at: `0x${string}`, statuses: readonly StatusCommitment[], signal?: AbortSignal): Promise<PreparedTransaction>;
}

export interface AttestationClient {
  schemaById(schema: SchemaId, signal?: AbortSignal): Promise<SdkResult<Versioned<SchemaView>>>;
  attestationById(attestation: AttestationId, signal?: AbortSignal): Promise<SdkResult<Versioned<AttestationView>>>;
  liveStatus(attestation: AttestationId, signal?: AbortSignal): Promise<SdkResult<AttestationLiveStatus>>;
  creatorSchemas(creator: AccountId, input?: PageInput, signal?: AbortSignal): Promise<SdkResult<IdPage<SchemaId>>>;
  issuerAttestations(issuer: AccountId, input?: PageInput, signal?: AbortSignal): Promise<SdkResult<IdPage<AttestationId>>>;
  subjectSchemaAttestations(subject: SubjectCommitment, schema: SchemaId, input?: PageInput, signal?: AbortSignal): Promise<SdkResult<IdPage<AttestationId>>>;
  nextDelegatedNonce(issuer: AccountId, signal?: AbortSignal): Promise<SdkResult<DecimalU64>>;
  schemaCount(signal?: AbortSignal): Promise<SdkResult<DecimalU64>>;
  attestationCount(signal?: AbortSignal): Promise<SdkResult<DecimalU64>>;
  nextIssuanceNonce(issuer: AccountId, signal?: AbortSignal): Promise<SdkResult<DecimalU64>>;
  externalStatus(issuer: AccountId, status: StatusCommitment, signal?: AbortSignal): Promise<SdkResult<Versioned<ExternalStatusView>>>;
  prepareCreateSchema(input: CreateSchemaInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetSchemaStatus(schema: SchemaId, status: SchemaStatus, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareIssue(input: AttestationInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareIssueDelegated(input: SignedDelegatedIssue, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareIssueBatch(items: readonly AttestationInput[], signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevoke(attestation: AttestationId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevokeDelegated(input: SignedDelegatedRevoke, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareIssueDelegatedBatch(items: readonly SignedDelegatedIssue[], signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevokeBatch(attestations: readonly AttestationId[], signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevokeDelegatedBatch(items: readonly SignedDelegatedRevoke[], signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevokeExternalStatus(status: StatusCommitment, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRevokeExternalStatusBatch(statuses: readonly StatusCommitment[], signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
}

export const ATTESTATION_ADMIN_EXCLUSIONS = [
  { target: "Attestation.set_emergency_pause", reason: "runtime safety administration" },
  { target: "Attestation.force_schema_status", reason: "forced runtime administration" },
  { target: "Attestation.force_revoke", reason: "forced runtime administration" },
] as const;

export const ATTESTATION_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  runtimeApi: "AttestationApi.v1",
  pallet: "Attestation",
  reads: ["schema_by_id", "attestation_by_id", "attestation_live_status", "creator_schemas", "issuer_attestations", "subject_schema_attestations", "next_delegated_nonce", "schema_count", "attestation_count", "next_issuance_nonce", "external_status"],
  transactions: ["create_schema", "set_schema_status", "issue", "issue_delegated", "issue_batch", "revoke", "revoke_delegated", "issue_delegated_batch", "revoke_batch", "revoke_delegated_batch", "revoke_external_status", "revoke_external_status_batch"],
} as const;

export function createAttestationClient(chain: CommonsChainClient, runtime: AttestationRuntimeAdapter): AttestationClient {
  const prepare = (builder: (at: `0x${string}`) => Promise<PreparedTransaction>, signal?: AbortSignal) =>
    prepareAtFinalized(chain, ({ block }) => builder(block.hash), signal);
  const batch = (items: readonly unknown[], operation: string): SdkResult<PreparedTransaction> | undefined => {
    try { ensureBatch(items.length, operation); return undefined; }
    catch (error) { return invalidResult<PreparedTransaction>(error instanceof Error ? error.message : "invalid batch"); }
  };
  const request = (input?: PageInput): SdkResult<PageRequest> => {
    try { return { success: true, value: page(input) }; }
    catch (error) { return invalidResult(error instanceof Error ? error.message : "invalid page"); }
  };
  const accountFailure = <T>(value: AccountId): SdkResult<T> | undefined => {
    try { accountId(value); return undefined; }
    catch (error) { return invalidResult<T>(error instanceof Error ? error.message : "invalid account"); }
  };
  return {
    schemaById: (schema, signal) => chain.readFinalized((at) => runtime.schemaById(at, schema, signal), signal),
    attestationById: (attestation, signal) => chain.readFinalized((at) => runtime.attestationById(at, attestation, signal), signal),
    liveStatus: (attestation, signal) => chain.readFinalized((at) => runtime.liveStatus(at, attestation, signal), signal),
    async creatorSchemas(creator, input, signal) { const failure=accountFailure<IdPage<SchemaId>>(creator); if(failure)return failure; const value=request(input); return value.success?chain.readFinalized((at)=>runtime.creatorSchemas(at,creator,value.value,signal),signal):value; },
    async issuerAttestations(issuer, input, signal) { const failure=accountFailure<IdPage<AttestationId>>(issuer); if(failure)return failure; const value=request(input); return value.success?chain.readFinalized((at)=>runtime.issuerAttestations(at,issuer,value.value,signal),signal):value; },
    async subjectSchemaAttestations(subject, schema, input, signal) { const value=request(input); return value.success?chain.readFinalized((at)=>runtime.subjectSchemaAttestations(at,subject,schema,value.value,signal),signal):value; },
    async nextDelegatedNonce(issuer, signal) { const failure=accountFailure<DecimalU64>(issuer); return failure??chain.readFinalized((at)=>runtime.nextDelegatedNonce(at,issuer,signal),signal); },
    schemaCount: (signal) => chain.readFinalized((at) => runtime.schemaCount(at, signal), signal),
    attestationCount: (signal) => chain.readFinalized((at) => runtime.attestationCount(at, signal), signal),
    async nextIssuanceNonce(issuer, signal) { const failure=accountFailure<DecimalU64>(issuer); return failure??chain.readFinalized((at)=>runtime.nextIssuanceNonce(at,issuer,signal),signal); },
    async externalStatus(issuer, status, signal) { const failure=accountFailure<Versioned<ExternalStatusView>>(issuer); return failure??chain.readFinalized((at)=>runtime.externalStatus(at,issuer,status,signal),signal); },
    async prepareCreateSchema(input, signal) {
      try { schemaDefinition(input.definition); if (input.authorized_issuers.length > 64 || new Set(input.authorized_issuers).size !== input.authorized_issuers.length) throw new TypeError("authorized issuers must be unique and contain at most 64 accounts"); }
      catch (error) { return invalidResult(error instanceof Error ? error.message : "invalid schema"); }
      return prepare((at) => runtime.createSchema(at, { ...input, authorized_issuers: [...input.authorized_issuers] }, signal), signal);
    },
    prepareSetSchemaStatus: (schema, status, signal) => prepare((at) => runtime.setSchemaStatus(at, schema, status, signal), signal),
    prepareIssue: (input, signal) => prepare((at) => runtime.issue(at, { ...input }, signal), signal),
    prepareIssueDelegated: (input, signal) => prepare((at) => runtime.issueDelegated(at, { intent: { ...input.intent }, signature: input.signature }, signal), signal),
    async prepareIssueBatch(items, signal) { const failure=batch(items,"issue_batch"); return failure??prepare((at)=>runtime.issueBatch(at,items.map((item)=>({...item})),signal),signal); },
    prepareRevoke: (attestation, signal) => prepare((at) => runtime.revoke(at, attestation, signal), signal),
    prepareRevokeDelegated: (input, signal) => prepare((at) => runtime.revokeDelegated(at, { intent: { ...input.intent }, signature: input.signature }, signal), signal),
    async prepareIssueDelegatedBatch(items, signal) { const failure=batch(items,"issue_delegated_batch"); return failure??prepare((at)=>runtime.issueDelegatedBatch(at,items.map(({intent,signature})=>({intent:{...intent},signature})),signal),signal); },
    async prepareRevokeBatch(items, signal) { const failure=batch(items,"revoke_batch"); return failure??prepare((at)=>runtime.revokeBatch(at,[...items],signal),signal); },
    async prepareRevokeDelegatedBatch(items, signal) { const failure=batch(items,"revoke_delegated_batch"); return failure??prepare((at)=>runtime.revokeDelegatedBatch(at,items.map(({intent,signature})=>({intent:{...intent},signature})),signal),signal); },
    prepareRevokeExternalStatus: (status, signal) => prepare((at) => runtime.revokeExternalStatus(at, status, signal), signal),
    async prepareRevokeExternalStatusBatch(items, signal) { const failure=batch(items,"revoke_external_status_batch"); return failure??prepare((at)=>runtime.revokeExternalStatusBatch(at,[...items],signal),signal); },
  };
}

export type SchemaByIdResponse = Versioned<SchemaView>;
export type AttestationByIdResponse = Versioned<AttestationView>;
export type AttestationIdPage = IdPage<AttestationId>;
export type SchemaIdPage = IdPage<SchemaId>;
