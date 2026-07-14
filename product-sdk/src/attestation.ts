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

import { finalizedRead, submitAndFinalize, type HostRequest, type RequestContext } from "./host.ts";
import { invalidDomainInput } from "./errors.ts";
import {
  page,
  type AccountId,
  type AttestationId,
  type BlockNumber,
  type IdPage,
  type PageInput,
  type PayloadCommitment,
  type SchemaId,
  type StatusCommitment,
  type SubjectCommitment,
  type UniquenessCommitment,
  type Versioned,
  type IssuerSignature,
  type Hash32,
  type BlockHash,
  type DecimalU64,
} from "./types.ts";

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

export const attestation = {
  schemaById(context: RequestContext, schema: SchemaId) {
    return finalizedRead("attestation", context, "attestation", "schema_by_id", { schema });
  },

  attestationById(context: RequestContext, attestation: AttestationId) {
    return finalizedRead("attestation", context, "attestation", "attestation_by_id", { attestation });
  },

  liveStatus(context: RequestContext, attestation: AttestationId) {
    return finalizedRead("attestation", context, "attestation", "attestation_live_status", { attestation });
  },

  creatorSchemas(context: RequestContext, creator: AccountId, input?: PageInput) {
    return finalizedRead("attestation", context, "attestation", "creator_schemas", {
      creator,
      ...page(input),
    });
  },

  issuerAttestations(context: RequestContext, issuer: AccountId, input?: PageInput) {
    return finalizedRead("attestation", context, "attestation", "issuer_attestations", {
      issuer,
      ...page(input),
    });
  },

  subjectSchemaAttestations(
    context: RequestContext,
    subject_commitment: SubjectCommitment,
    schema: SchemaId,
    input?: PageInput,
  ) {
    return finalizedRead("attestation", context, "attestation", "subject_schema_attestations", {
      subject_commitment,
      schema,
      ...page(input),
    });
  },

  nextDelegatedNonce(context: RequestContext, issuer: AccountId) {
    return finalizedRead("attestation", context, "attestation", "next_delegated_nonce", { issuer });
  },

  schemaCount(context: RequestContext) {
    return finalizedRead("attestation", context, "attestation", "schema_count", {});
  },

  attestationCount(context: RequestContext) {
    return finalizedRead("attestation", context, "attestation", "attestation_count", {});
  },

  nextIssuanceNonce(context: RequestContext, issuer: AccountId) {
    return finalizedRead("attestation", context, "attestation", "next_issuance_nonce", { issuer });
  },

  externalStatus(
    context: RequestContext,
    issuer: AccountId,
    status_commitment: StatusCommitment,
  ) {
    return finalizedRead("attestation", context, "attestation", "external_status", {
      issuer,
      status_commitment,
    });
  },

  createSchema(
    context: RequestContext,
    definition: SchemaDefinition,
    authorized_issuers: readonly AccountId[],
    revocable: boolean,
    unique: boolean,
    index_policy: IndexPolicy,
  ) {
    if (authorized_issuers.length > 64 || new Set(authorized_issuers).size !== authorized_issuers.length) {
      invalidDomainInput(
        "attestation",
        "create_schema",
        "authorized issuers must be unique and contain at most 64 accounts",
      );
    }
    return submitAndFinalize("attestation", context, "attestation", "create_schema", {
      definition,
      authorized_issuers: [...authorized_issuers],
      revocable,
      unique,
      index_policy,
    });
  },

  setSchemaStatus(context: RequestContext, schema: SchemaId, status: SchemaStatus) {
    return submitAndFinalize("attestation", context, "attestation", "set_schema_status", {
      schema,
      status,
    });
  },

  issue(context: RequestContext, input: AttestationInput) {
    return submitAndFinalize("attestation", context, "attestation", "issue", { ...input });
  },

  issueDelegated(context: RequestContext, intent: DelegatedIssueIntent, signature: IssuerSignature) {
    return submitAndFinalize("attestation", context, "attestation", "issue_delegated", {
      intent: { ...intent },
      signature,
    });
  },

  issueBatch(context: RequestContext, items: readonly AttestationInput[]) {
    if (items.length < 1 || items.length > 64) {
      invalidDomainInput("attestation", "issue_batch", "batch must contain 1-64 attestations");
    }
    return submitAndFinalize("attestation", context, "attestation", "issue_batch", {
      items: items.map((item) => ({ ...item })),
    });
  },

  revoke(context: RequestContext, attestation: AttestationId) {
    return submitAndFinalize("attestation", context, "attestation", "revoke", { attestation });
  },

  revokeDelegated(
    context: RequestContext,
    intent: DelegatedRevokeIntent,
    signature: IssuerSignature,
  ) {
    return submitAndFinalize("attestation", context, "attestation", "revoke_delegated", {
      intent: { ...intent },
      signature,
    });
  },

  issueDelegatedBatch(context: RequestContext, items: readonly SignedDelegatedIssue[]) {
    ensureBatch(items.length, "issue_delegated_batch");
    return submitAndFinalize("attestation", context, "attestation", "issue_delegated_batch", {
      items: items.map(({ intent, signature }) => ({ intent: { ...intent }, signature })),
    });
  },

  revokeBatch(context: RequestContext, attestations: readonly AttestationId[]) {
    ensureBatch(attestations.length, "revoke_batch");
    return submitAndFinalize("attestation", context, "attestation", "revoke_batch", {
      attestations: [...attestations],
    });
  },

  revokeDelegatedBatch(context: RequestContext, items: readonly SignedDelegatedRevoke[]) {
    ensureBatch(items.length, "revoke_delegated_batch");
    return submitAndFinalize("attestation", context, "attestation", "revoke_delegated_batch", {
      items: items.map(({ intent, signature }) => ({ intent: { ...intent }, signature })),
    });
  },

  revokeExternalStatus(context: RequestContext, status_commitment: StatusCommitment) {
    return submitAndFinalize("attestation", context, "attestation", "revoke_external_status", {
      status_commitment,
    });
  },

  revokeExternalStatusBatch(
    context: RequestContext,
    status_commitments: readonly StatusCommitment[],
  ) {
    ensureBatch(status_commitments.length, "revoke_external_status_batch");
    return submitAndFinalize(
      "attestation",
      context,
      "attestation",
      "revoke_external_status_batch",
      { status_commitments: [...status_commitments] },
    );
  },

  setEmergencyPause(context: RequestContext, paused: boolean) {
    return submitAndFinalize("attestation", context, "attestation", "set_emergency_pause", { paused });
  },

  forceSchemaStatus(context: RequestContext, schema: SchemaId, status: SchemaStatus) {
    return submitAndFinalize("attestation", context, "attestation", "force_schema_status", {
      schema,
      status,
    });
  },

  forceRevoke(context: RequestContext, attestation: AttestationId) {
    return submitAndFinalize("attestation", context, "attestation", "force_revoke", { attestation });
  },
} as const;

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

export type SchemaByIdRequest = ReturnType<typeof attestation.schemaById>;
export type AttestationByIdRequest = ReturnType<typeof attestation.attestationById>;
export type SchemaByIdResponse = Versioned<SchemaView>;
export type AttestationByIdResponse = Versioned<AttestationView>;
export type AttestationIdPage = IdPage<AttestationId>;
export type SchemaIdPage = IdPage<SchemaId>;

// Compile-time compatibility assertion for the public HostRequest envelope.
const _hostRequestShape: HostRequest = null as unknown as SchemaByIdRequest;
void _hostRequestShape;
