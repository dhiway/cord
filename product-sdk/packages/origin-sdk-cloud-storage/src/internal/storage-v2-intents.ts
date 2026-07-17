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

/**
 * Private replacement-first facade for the frozen `cord.origin.host/2` storage registry.
 * This module is deliberately absent from the package entrypoint until the P4 authority cutover.
 */


import { validateStorageV2EventEnvelope, validateStorageV2Payload } from "./storage-v2-validation.ts";

export const STORAGE_V2_PROTOCOL = "cord.origin.host/2" as const;
export const STORAGE_V2_MAJOR = 2 as const;
export const STORAGE_V2_MINOR = 1 as const;
export const STORAGE_V2_REGISTRY_SHA256 =
  "42e4e9660d7e2c26f15565a2448f9d2b384cb5c73321422a3b9ae9107f364b57" as const;

export type Bytes16 = Uint8Array & { readonly __storageV2Bytes16: unique symbol };
export type Bytes32 = Uint8Array & { readonly __storageV2Bytes32: unique symbol };
export type RequestId = Bytes16;
export type OperationId = Bytes16;
export type GrantId = Bytes32;
export type BucketId = Bytes32;
export type ProviderId = Bytes32;
export type Subject = Bytes32;
export type Hash32 = Bytes32;
export type KeyId = Bytes32;
export type ContentId = string;

export type StorageV2Operation =
  | "storage.bucket.create" | "storage.bucket.get" | "storage.bucket.grant" | "storage.bucket.revoke"
  | "storage.object.put" | "storage.object.get" | "storage.object.range" | "storage.object.delete"
  | "storage.object.status" | "storage.checkpoint.status" | "storage.checkpoint.subscribe"
  | "storage.replica.status" | "storage.replica.subscribe" | "storage.deletion.status"
  | "storage.deletion.subscribe" | "storage.drive.read" | "storage.drive.commit"
  | "storage.drive.share" | "storage.s3.put" | "storage.s3.get" | "storage.s3.list"
  | "storage.s3.delete" | "storage.publish" | "storage.resolve" | "storage.keys.export"
  | "storage.keys.import";

export type StorageV2GrantScope =
  | "public" | "storage.bucket.admin" | "storage.bucket.read" | "storage.bucket.reader"
  | "storage.bucket.writer" | "storage.publish" | "storage.keys.export" | "storage.keys.import";
export type StorageV2ResumeMode =
  | "none" | "chain-idempotent" | "provider-token" | "verified-offset" | "cursor-256"
  | "per-object" | "cursor-versioned";
export type StorageV2Cancellation = "pre-effect-only" | "stop-stream-or-terminal";

export interface StorageV2OperationContract {
  readonly code: number;
  readonly grantScope: StorageV2GrantScope;
  readonly resume: StorageV2ResumeMode;
  readonly cancellation: StorageV2Cancellation;
  readonly operationIdRequired: boolean;
  readonly stateChanging: boolean;
}

export const STORAGE_V2_OPERATIONS = {
  "storage.bucket.create": [1000, "storage.bucket.admin", "chain-idempotent", "pre-effect-only", true, true],
  "storage.bucket.get": [1001, "storage.bucket.read", "none", "stop-stream-or-terminal", false, false],
  "storage.bucket.grant": [1002, "storage.bucket.admin", "chain-idempotent", "pre-effect-only", true, true],
  "storage.bucket.revoke": [1003, "storage.bucket.admin", "chain-idempotent", "pre-effect-only", true, true],
  "storage.object.put": [1010, "storage.bucket.writer", "provider-token", "pre-effect-only", true, true],
  "storage.object.get": [1011, "storage.bucket.reader", "verified-offset", "stop-stream-or-terminal", false, false],
  "storage.object.range": [1012, "storage.bucket.reader", "verified-offset", "stop-stream-or-terminal", false, false],
  "storage.object.delete": [1013, "storage.bucket.writer", "chain-idempotent", "pre-effect-only", true, true],
  "storage.object.status": [1014, "storage.bucket.reader", "none", "stop-stream-or-terminal", false, false],
  "storage.checkpoint.status": [1020, "storage.bucket.reader", "none", "stop-stream-or-terminal", false, false],
  "storage.checkpoint.subscribe": [1021, "storage.bucket.reader", "cursor-256", "pre-effect-only", true, true],
  "storage.replica.status": [1022, "storage.bucket.reader", "none", "stop-stream-or-terminal", false, false],
  "storage.replica.subscribe": [1023, "storage.bucket.reader", "cursor-256", "pre-effect-only", true, true],
  "storage.deletion.status": [1024, "storage.bucket.writer", "none", "stop-stream-or-terminal", false, false],
  "storage.deletion.subscribe": [1025, "storage.bucket.writer", "cursor-256", "pre-effect-only", true, true],
  "storage.drive.read": [1030, "storage.bucket.reader", "none", "stop-stream-or-terminal", false, false],
  "storage.drive.commit": [1031, "storage.bucket.writer", "per-object", "pre-effect-only", true, true],
  "storage.drive.share": [1032, "storage.bucket.admin", "chain-idempotent", "pre-effect-only", true, true],
  "storage.s3.put": [1040, "storage.bucket.writer", "provider-token", "pre-effect-only", true, true],
  "storage.s3.get": [1041, "storage.bucket.reader", "verified-offset", "stop-stream-or-terminal", false, false],
  "storage.s3.list": [1042, "storage.bucket.reader", "cursor-versioned", "stop-stream-or-terminal", false, false],
  "storage.s3.delete": [1043, "storage.bucket.writer", "chain-idempotent", "pre-effect-only", true, true],
  "storage.publish": [1050, "storage.publish", "chain-idempotent", "pre-effect-only", true, true],
  "storage.resolve": [1051, "public", "none", "stop-stream-or-terminal", false, false],
  "storage.keys.export": [1060, "storage.keys.export", "none", "pre-effect-only", true, true],
  "storage.keys.import": [1061, "storage.keys.import", "none", "pre-effect-only", true, true],
} as const satisfies Record<StorageV2Operation, readonly [number, StorageV2GrantScope, StorageV2ResumeMode, StorageV2Cancellation, boolean, boolean]>;

export interface StorageV2PayloadMap {
  "storage.bucket.create": { replicaCount: number; providers: readonly ProviderId[]; encryption: 0 | 1 };
  "storage.bucket.get": { bucketId: BucketId; at?: Hash32 };
  "storage.bucket.grant": { bucketId: BucketId; subject: Subject; role: 0 | 1 | 2; issuedAt: bigint; expiresAt: bigint };
  "storage.bucket.revoke": { bucketId: BucketId; grantId: GrantId; expectedVersion: bigint };
  "storage.object.put": { bucketId: BucketId; cid: ContentId; length: bigint; encrypted: 0 | 1; transferId: OperationId };
  "storage.object.get": { bucketId: BucketId; cid: ContentId };
  "storage.object.range": { bucketId: BucketId; cid: ContentId; offset: bigint; length: bigint };
  "storage.object.delete": { bucketId: BucketId; cid: ContentId; expectedVersion: bigint };
  "storage.object.status": { bucketId: BucketId; cid: ContentId };
  "storage.checkpoint.status": { bucketId: BucketId; root?: Hash32 };
  "storage.checkpoint.subscribe": { bucketId: BucketId; cursor: bigint };
  "storage.replica.status": { bucketId: BucketId };
  "storage.replica.subscribe": { bucketId: BucketId; cursor: bigint };
  "storage.deletion.status": { bucketId: BucketId; cid: ContentId };
  "storage.deletion.subscribe": { bucketId: BucketId; cid: ContentId; cursor: bigint };
  "storage.drive.read": { bucketId: BucketId; path: string; manifest?: ContentId };
  "storage.drive.commit": { bucketId: BucketId; manifest: ContentId; bytes: Uint8Array; expectedVersion: bigint; mode: 0 | 1 | 2 };
  "storage.drive.share": { bucketId: BucketId; subject: Subject; role: 0 | 1 | 2; issuedAt: bigint; expiresAt: bigint };
  "storage.s3.put": { bucket: string; key: Uint8Array; cid: ContentId; metadata: Uint8Array; mediaType: string; ifMatch?: string; transferId: OperationId };
  "storage.s3.get": { bucket: string; key: Uint8Array; version?: bigint };
  "storage.s3.list": { bucket: string; prefix?: Uint8Array; cursor?: Uint8Array; limit: number };
  "storage.s3.delete": { bucket: string; key: Uint8Array; ifMatch?: string; transferId: OperationId };
  "storage.publish": { nameHash: Hash32; cid: ContentId; expectedVersion: bigint };
  "storage.resolve": { name: string; version?: bigint; at?: Hash32 };
  "storage.keys.export": { bucketId: BucketId; keyVersion: number; recipientKey: Uint8Array };
  "storage.keys.import": { bucketId: BucketId; wrappedKey: Uint8Array; replace: 0 | 1; keyVersion: number };
}

export interface StorageV2Finality { readonly number: bigint; readonly hash: Hash32 }
export interface StorageV2Checkpoint { readonly root: Hash32; readonly from: bigint; readonly to: bigint; readonly replicas: number }
export interface StorageV2ProviderReceipt { readonly provider: ProviderId; readonly cid: ContentId; readonly length: bigint; readonly signature: Uint8Array }
export interface StorageV2ResultMap {
  "storage.bucket.create": { bucketId: BucketId; version: bigint; finalized: StorageV2Finality };
  "storage.bucket.get": { owner: Bytes32; version: bigint; replicaCount: number; primary: ProviderId; providers: readonly ProviderId[]; finalized: StorageV2Finality };
  "storage.bucket.grant": { grantId: GrantId; version: bigint; finalized: StorageV2Finality };
  "storage.bucket.revoke": { grantId: GrantId; version: bigint; finalized: StorageV2Finality };
  "storage.object.put": { receipt: StorageV2ProviderReceipt; publishable: boolean; finalized: StorageV2Finality };
  "storage.object.get": { cid: ContentId; length: bigint; checkpoint: StorageV2Checkpoint };
  "storage.object.range": { cid: ContentId; offset: bigint; length: bigint; total: bigint; checkpoint: StorageV2Checkpoint };
  "storage.object.delete": { version: bigint; pending: number; confirmed: number; finalized: StorageV2Finality };
  "storage.object.status": { state: 0 | 1 | 2 | 3 | 4; receipt?: StorageV2ProviderReceipt; checkpoint?: StorageV2Checkpoint; replicas: number; publishable: boolean; finalized: StorageV2Finality };
  "storage.checkpoint.status": { checkpoint: StorageV2Checkpoint; sequence: number; block: bigint; quorum: number; finalized: StorageV2Finality };
  "storage.checkpoint.subscribe": { operationId: OperationId; cursor: bigint };
  "storage.replica.status": { primary: ProviderId; providers: readonly ProviderId[]; confirmed: number; lag: bigint; eligibility: number; finalized: StorageV2Finality };
  "storage.replica.subscribe": { operationId: OperationId; cursor: bigint };
  "storage.deletion.status": { version: bigint; confirmations: number; root: Hash32; finalized: StorageV2Finality };
  "storage.deletion.subscribe": { operationId: OperationId; cursor: bigint };
  "storage.drive.read": { manifest: ContentId; entry: ContentId; version: bigint; finalized: StorageV2Finality };
  "storage.drive.commit": { manifest: ContentId; version: bigint; checkpoint: StorageV2Checkpoint; finalized: StorageV2Finality };
  "storage.drive.share": { grantId: GrantId; version: bigint; finalized: StorageV2Finality };
  "storage.s3.put": { etag: string; version: bigint; finalized: StorageV2Finality };
  "storage.s3.get": { cid: ContentId; etag: string; version: bigint; finalized: StorageV2Finality };
  "storage.s3.list": { cids: readonly ContentId[]; cursor?: Uint8Array; version: bigint; finalized: StorageV2Finality };
  "storage.s3.delete": { version: bigint; remainingHistory: number; finalized: StorageV2Finality };
  "storage.publish": { nameHash: Hash32; cid: ContentId; finalized: StorageV2Finality };
  "storage.resolve": { cid: ContentId; version: bigint; checkpoint: StorageV2Checkpoint; finalized: StorageV2Finality };
  "storage.keys.export": { wrappedKey: Uint8Array; algorithm: number; keyVersion: number };
  "storage.keys.import": { keyId: KeyId; keyVersion: number };
}

export type StorageV2StateChangingOperation =
  | "storage.bucket.create" | "storage.bucket.grant" | "storage.bucket.revoke"
  | "storage.object.put" | "storage.object.delete" | "storage.checkpoint.subscribe"
  | "storage.replica.subscribe" | "storage.deletion.subscribe" | "storage.drive.commit"
  | "storage.drive.share" | "storage.s3.put" | "storage.s3.delete" | "storage.publish"
  | "storage.keys.export" | "storage.keys.import";

type GrantInput<Operation extends StorageV2Operation> = Operation extends "storage.resolve"
  ? { readonly grantId?: never }
  : { readonly grantId: GrantId };
type OperationInput<Operation extends StorageV2Operation> = Operation extends StorageV2StateChangingOperation
  ? { readonly operationId: OperationId; readonly idempotencyKey?: Uint8Array }
  : { readonly operationId?: never; readonly idempotencyKey?: never };

export type StorageV2IntentInput<Operation extends StorageV2Operation> = {
  readonly requestId: RequestId;
  readonly productId: string;
  readonly deadlineBlock: bigint;
  readonly payload: StorageV2PayloadMap[Operation];
} & GrantInput<Operation> & OperationInput<Operation>;

export interface StorageV2Intent<Operation extends StorageV2Operation = StorageV2Operation> {
  readonly protocol: typeof STORAGE_V2_PROTOCOL;
  readonly major: typeof STORAGE_V2_MAJOR;
  readonly minor: typeof STORAGE_V2_MINOR;
  readonly registrySha256: typeof STORAGE_V2_REGISTRY_SHA256;
  readonly requestId: RequestId;
  readonly productId: string;
  readonly operation: Operation;
  readonly code: (typeof STORAGE_V2_OPERATIONS)[Operation][0];
  readonly grantId?: GrantId;
  readonly operationId?: OperationId;
  readonly idempotencyKey?: Uint8Array;
  readonly deadlineBlock: bigint;
  readonly payload: StorageV2PayloadMap[Operation];
}

export type StorageV2ByteProgressOperation = "storage.object.get" | "storage.object.range" | "storage.s3.get";
export type StorageV2Progress<Operation extends StorageV2Operation> = Operation extends StorageV2ByteProgressOperation
  ? { readonly kind: "progress"; readonly requestId: RequestId; readonly seq: number; readonly offset: bigint; readonly bytes: Uint8Array }
  : { readonly kind: "progress"; readonly requestId: RequestId; readonly seq: number; readonly completed: bigint; readonly total?: bigint; readonly chunksAcked?: bigint; readonly replicasConfirmed?: bigint; readonly bytes?: never };
export interface StorageV2ErrorEvent {
  readonly kind: "error";
  readonly requestId: RequestId;
  readonly seq: number;
  readonly code: number;
  readonly name: string;
  readonly retryable: boolean;
  readonly details?: { readonly message?: string; readonly lower?: bigint; readonly upper?: bigint; readonly hash?: Uint8Array };
}
export type StorageV2Event<Operation extends StorageV2Operation = StorageV2Operation> =
  | { readonly kind: "accepted"; readonly requestId: RequestId; readonly seq: 0; readonly state: 0 | 1 | 2 | 3 | 4 }
  | StorageV2Progress<Operation>
  | { readonly kind: "result"; readonly requestId: RequestId; readonly seq: number; readonly value: StorageV2ResultMap[Operation] }
  | StorageV2ErrorEvent
  | { readonly kind: "cancelled"; readonly requestId: RequestId; readonly seq: number };

export type StorageV2Resume =
  | { readonly kind: "chain-idempotent"; readonly operationId: OperationId }
  | { readonly kind: "provider-token"; readonly token: Uint8Array }
  | { readonly kind: "verified-offset"; readonly offset: bigint; readonly proof: Hash32 }
  | { readonly kind: "cursor-256"; readonly cursor: bigint }
  | { readonly kind: "per-object"; readonly cid: ContentId; readonly version: bigint }
  | { readonly kind: "cursor-versioned"; readonly cursor: Uint8Array; readonly version: bigint };

export interface StorageV2Execution<Operation extends StorageV2Operation> {
  readonly events: AsyncIterable<StorageV2Event<Operation>>;
  cancel(): Promise<void>;
  resume(resume: StorageV2Resume): AsyncIterable<StorageV2Event<Operation>>;
}

export interface StorageV2Transport {
  start<Operation extends StorageV2Operation>(intent: StorageV2Intent<Operation>): StorageV2Execution<Operation>;
}

const U64_MAX = 18_446_744_073_709_551_615n;

function bytes(value: Uint8Array, length: number, field: string): Uint8Array {
  if (!(value instanceof Uint8Array) || value.byteLength !== length) {
    throw new TypeError(`${field} must contain exactly ${length} bytes`);
  }
  return value.slice();
}

function text(value: string, max: number, field: string): string {
  const length = new TextEncoder().encode(value).byteLength;
  if (length < 1 || length > max || value.normalize("NFC") !== value) {
    throw new TypeError(`${field} must be NFC UTF-8 containing 1-${max} bytes`);
  }
  return value;
}

function u64(value: bigint, field: string): bigint {
  if (value < 0n || value > U64_MAX) throw new TypeError(`${field} must be an unsigned u64`);
  return value;
}

export function storageV2Bytes16(value: Uint8Array, field = "bytes16"): Bytes16 {
  return bytes(value, 16, field) as Bytes16;
}

export function storageV2Bytes32(value: Uint8Array, field = "bytes32"): Bytes32 {
  return bytes(value, 32, field) as Bytes32;
}

export function storageV2OperationContract(operation: StorageV2Operation): StorageV2OperationContract {
  const [code, grantScope, resume, cancellation, operationIdRequired, stateChanging] = STORAGE_V2_OPERATIONS[operation];
  return { code, grantScope, resume, cancellation, operationIdRequired, stateChanging };
}

export function createStorageV2Intent<Operation extends StorageV2Operation>(
  operation: Operation,
  input: StorageV2IntentInput<Operation>,
): StorageV2Intent<Operation> {
  const contract = storageV2OperationContract(operation);
  const requestId = storageV2Bytes16(input.requestId, "requestId");
  const productId = text(input.productId, 128, "productId");
  const deadlineBlock = u64(input.deadlineBlock, "deadlineBlock");
  const grantId = "grantId" in input && input.grantId !== undefined
    ? storageV2Bytes32(input.grantId, "grantId")
    : undefined;
  if (contract.grantScope === "public" ? grantId !== undefined : grantId === undefined) {
    throw new TypeError(contract.grantScope === "public" ? "public operation cannot carry a grant" : `operation requires ${contract.grantScope} grant`);
  }
  const operationId = "operationId" in input && input.operationId !== undefined
    ? storageV2Bytes16(input.operationId, "operationId")
    : undefined;
  if (contract.operationIdRequired !== (operationId !== undefined)) {
    throw new TypeError(contract.operationIdRequired ? "operationId is required" : "read-only operation cannot carry operationId");
  }
  const idempotencyKey = "idempotencyKey" in input && input.idempotencyKey !== undefined
    ? input.idempotencyKey.slice()
    : undefined;
  if (idempotencyKey !== undefined && (idempotencyKey.byteLength < 1 || idempotencyKey.byteLength > 64)) {
    throw new TypeError("idempotencyKey must contain 1-64 bytes");
  }
  validateStorageV2Payload(operation, input.payload);
  return {
    protocol: STORAGE_V2_PROTOCOL,
    major: STORAGE_V2_MAJOR,
    minor: STORAGE_V2_MINOR,
    registrySha256: STORAGE_V2_REGISTRY_SHA256,
    requestId,
    productId,
    operation,
    code: contract.code as (typeof STORAGE_V2_OPERATIONS)[Operation][0],
    ...(grantId === undefined ? {} : { grantId }),
    ...(operationId === undefined ? {} : { operationId }),
    ...(idempotencyKey === undefined ? {} : { idempotencyKey }),
    deadlineBlock,
    payload: input.payload,
  };
}

export class StorageV2EventSequence<Operation extends StorageV2Operation> {
  readonly #operation: Operation;
  readonly #requestId: RequestId;
  #next = 0;
  #terminal = false;
  #cancelRequested = false;
  #resumeAuthority: boolean;

  constructor(operation: Operation, requestId: RequestId) {
    this.#operation = operation;
    this.#requestId = requestId.slice() as RequestId;
    this.#resumeAuthority = storageV2OperationContract(operation).resume !== "none";
  }

  requestCancel(): boolean {
    if (this.#terminal || this.#cancelRequested) return false;
    this.#cancelRequested = true;
    this.#resumeAuthority = false;
    return true;
  }

  authorizeResume(resume: StorageV2Resume): void {
    if (!this.#resumeAuthority || this.#cancelRequested || this.#terminal) throw new TypeError("resume authority is not live");
    validateStorageV2Resume(this.#operation, resume);
  }

  accept(event: StorageV2Event<Operation>): void {
    if (this.#terminal) throw new TypeError("event received after terminal storage v2 event");
    validateStorageV2EventEnvelope(this.#operation, event as StorageV2Event<StorageV2Operation>);
    if (event.requestId.length !== this.#requestId.length
      || event.requestId.some((value, index) => value !== this.#requestId[index])) {
      throw new TypeError("event requestId does not match intent");
    }
    if (event.seq !== this.#next) throw new TypeError(`event sequence must be ${this.#next}`);
    if (this.#next === 0 && event.kind !== "accepted") throw new TypeError("first event must be accepted");
    if (this.#next > 0 && event.kind === "accepted") throw new TypeError("accepted event must appear exactly once");
    if (this.#cancelRequested && event.kind !== "cancelled") throw new TypeError("cancelled operation cannot emit later progress or effects");
    this.#next += 1;
    this.#terminal = event.kind === "result" || event.kind === "error" || event.kind === "cancelled";
    if (this.#terminal) this.#resumeAuthority = false;
  }

  get terminal(): boolean { return this.#terminal; }
  get resumeAuthority(): boolean { return this.#resumeAuthority; }
}

export function validateStorageV2Resume(operation: StorageV2Operation, resume: StorageV2Resume): void {
  const expected = storageV2OperationContract(operation).resume;
  if (expected === "none" || resume.kind !== expected) {
    throw new TypeError(expected === "none" ? `${operation} is not resumable` : `${operation} requires ${expected} resume state`);
  }
  switch (resume.kind) {
    case "chain-idempotent":
      storageV2Bytes16(resume.operationId, "resume.operationId");
      return;
    case "provider-token":
      if (!(resume.token instanceof Uint8Array) || resume.token.byteLength < 1 || resume.token.byteLength > 4096) {
        throw new TypeError("resume.token must contain 1-4096 bytes");
      }
      return;
    case "verified-offset":
      u64(resume.offset, "resume.offset");
      storageV2Bytes32(resume.proof, "resume.proof");
      return;
    case "cursor-256":
      u64(resume.cursor, "resume.cursor");
      return;
    case "per-object":
      text(resume.cid, 128, "resume.cid");
      u64(resume.version, "resume.version");
      return;
    case "cursor-versioned":
      if (!(resume.cursor instanceof Uint8Array) || resume.cursor.byteLength < 1 || resume.cursor.byteLength > 2048) {
        throw new TypeError("resume.cursor must contain 1-2048 bytes");
      }
      u64(resume.version, "resume.version");
  }
}
