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

import { ProductSdkError } from "../../core/src/contract.ts";
import {
  attestationEventOutcome,
  type AttestationEvent,
  type AttestationEventKind,
  type AttestationEventSubscription,
  type AttestationOutcome,
  type FinalizedAttestationEvent,
  type IndexPolicy,
  type SchemaStatus,
} from "../../../src/attestation.ts";
import type {
  AccountId,
  AttestationId,
  BlockHash,
  BlockNumber,
  DecimalU64,
  Hash32,
  SchemaId,
  StatusCommitment,
  SubjectCommitment,
} from "../../../src/types.ts";

/** One descriptor-decoded event from a finalized block. */
export interface TypedFinalizedEvent {
  readonly pallet: string;
  readonly event: string;
  readonly data: Readonly<Record<string, unknown>>;
  readonly index: number;
}

/** Finalized event batch supplied by a PAPI/substrate-client adapter. */
export interface TypedFinalizedEventBlock {
  readonly hash: string;
  readonly events: readonly TypedFinalizedEvent[];
}

/** Minimal typed client boundary used by native attestation subscriptions. */
export interface TypedFinalizedEventSource {
  subscribeFinalizedEvents(options: {
    readonly from: string;
    readonly signal: AbortSignal;
  }): AsyncIterable<TypedFinalizedEventBlock>;
}

export interface FinalizedAttestationOutcome {
  readonly event: FinalizedAttestationEvent;
  readonly outcome: AttestationOutcome;
}

const HASH_32 = /^0x[0-9a-f]{64}$/i;
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const U64_MAX = 18_446_744_073_709_551_615n;

function rejected(message: string): never {
  throw new ProductSdkError("runtime_rejected", message);
}

function hash<Kind extends string>(value: unknown, field: string): string & { readonly __kind?: Kind } {
  if (typeof value !== "string" || !HASH_32.test(value)) rejected(`Attestation.${field} is not a hash`);
  return value.toLowerCase();
}

function account(value: unknown, field: string): AccountId {
  if (typeof value !== "string" || value.length < 1 || value.length > 128)
    rejected(`Attestation.${field} is not an account`);
  return value as AccountId;
}

function bool(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") rejected(`Attestation.${field} is not boolean`);
  return value;
}

function decimal(value: unknown, field: string): DecimalU64 {
  const text = typeof value === "bigint" || typeof value === "number" || typeof value === "string"
    ? String(value)
    : "";
  if (!DECIMAL.test(text) || BigInt(text) > U64_MAX)
    rejected(`Attestation.${field} is not an unsigned 64-bit integer`);
  return text as DecimalU64;
}

function blockNumber(value: unknown, field: string): BlockNumber {
  return decimal(value, field) as BlockNumber;
}

function indexPolicy(value: unknown): IndexPolicy {
  if (!["none", "issuer", "subject_and_schema", "issuer_and_subject_schema"].includes(String(value)))
    rejected("Attestation.index_policy is unknown");
  return value as IndexPolicy;
}

function schemaStatus(value: unknown): SchemaStatus {
  if (!["active", "paused", "retired"].includes(String(value)))
    rejected("Attestation.status is unknown");
  return value as SchemaStatus;
}

function nullableAccount(value: unknown, field: string): AccountId | null {
  return value === null || value === undefined ? null : account(value, field);
}

/** Decode one exact native `Attestation` pallet event into the stable product DTO. */
export function decodeAttestationEvent(native: TypedFinalizedEvent): AttestationEvent | null {
  if (native.pallet !== "Attestation") return null;
  if (!native.data || typeof native.data !== "object" || Array.isArray(native.data))
    rejected(`Attestation.${native.event} data is not an object`);
  const data = native.data;
  switch (native.event) {
    case "SchemaCreated":
      return {
        event: "schema_created",
        data: {
          schema: hash(data.schema, "schema") as SchemaId,
          creator: account(data.creator, "creator"),
          definition_commitment: hash(data.definition_commitment, "definition_commitment") as Hash32,
          revocable: bool(data.revocable, "revocable"),
          unique: bool(data.unique, "unique"),
          index_policy: indexPolicy(data.index_policy),
        },
      };
    case "SchemaStatusChanged":
      return {
        event: "schema_status_changed",
        data: {
          schema: hash(data.schema, "schema") as SchemaId,
          status: schemaStatus(data.status),
          forced: bool(data.forced, "forced"),
        },
      };
    case "AttestationIssued":
      return {
        event: "attestation_issued",
        data: {
          attestation: hash(data.attestation, "attestation") as AttestationId,
          schema: hash(data.schema, "schema") as SchemaId,
          issuer: account(data.issuer, "issuer"),
          subject_commitment: hash(data.subject_commitment, "subject_commitment") as SubjectCommitment,
        },
      };
    case "DelegatedIntentConsumed":
      return {
        event: "delegated_intent_consumed",
        data: {
          issuer: account(data.issuer, "issuer"),
          delegate: account(data.delegate, "delegate"),
          nonce: decimal(data.nonce, "nonce"),
          attestation: hash(data.attestation, "attestation") as AttestationId,
        },
      };
    case "DelegatedRevocationConsumed":
      return {
        event: "delegated_revocation_consumed",
        data: {
          revoker: account(data.revoker, "revoker"),
          delegate: account(data.delegate, "delegate"),
          nonce: decimal(data.nonce, "nonce"),
          attestation: hash(data.attestation, "attestation") as AttestationId,
        },
      };
    case "AttestationRevoked":
      return {
        event: "attestation_revoked",
        data: {
          attestation: hash(data.attestation, "attestation") as AttestationId,
          by: nullableAccount(data.by, "by"),
          forced: bool(data.forced, "forced"),
        },
      };
    case "ExternalStatusRevoked":
      return {
        event: "external_status_revoked",
        data: {
          key: hash(data.key, "key") as Hash32,
          issuer: account(data.issuer, "issuer"),
          status_commitment: hash(data.status_commitment, "status_commitment") as StatusCommitment,
          revoked_at: blockNumber(data.revoked_at, "revoked_at"),
        },
      };
    case "EmergencyPauseChanged":
      return {
        event: "emergency_pause_changed",
        data: { paused: bool(data.paused, "paused") },
      };
    default:
      throw new ProductSdkError(
        "unsupported_runtime",
        `unknown native Attestation event ${native.event}`,
      );
  }
}

/**
 * Subscribe from the requested finalized anchor, filter native attestation events, and yield
 * transport-neutral event/outcome pairs. Replay and reconnect policy are owned by P6/P7.
 */
export async function* subscribeAttestationEvents(
  source: TypedFinalizedEventSource,
  subscription: AttestationEventSubscription,
  signal: AbortSignal = new AbortController().signal,
): AsyncGenerator<FinalizedAttestationOutcome> {
  const wanted = new Set<AttestationEventKind>(subscription.kinds);
  const blocks = source.subscribeFinalizedEvents({
    from: subscription.from_finalized_block,
    signal,
  });
  for await (const block of blocks) {
    if (signal.aborted) throw new ProductSdkError("cancelled", "attestation subscription cancelled");
    const finalizedBlockHash = hash(block.hash, "finalized_block_hash") as BlockHash;
    for (const native of block.events) {
      if (!Number.isSafeInteger(native.index) || native.index < 0 || native.index > 0xffff_ffff)
        rejected("Attestation event index is not a u32");
      const event = decodeAttestationEvent(native);
      if (!event || !wanted.has(event.event)) continue;
      const finalized: FinalizedAttestationEvent = {
        finalized_block_hash: finalizedBlockHash,
        event_index: native.index,
        event,
      };
      yield { event: finalized, outcome: attestationEventOutcome(event) };
    }
  }
}
