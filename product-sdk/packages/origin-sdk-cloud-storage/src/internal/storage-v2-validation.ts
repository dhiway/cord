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

import type {
  StorageV2Checkpoint,
  StorageV2ErrorEvent,
  StorageV2Operation,
  StorageV2PayloadMap,
  StorageV2Progress,
  StorageV2ProviderReceipt,
  StorageV2ResultMap,
} from "./storage-v2-intents.ts";

const encoder = new TextEncoder();
const U16_MAX = 65_535;
const U32_MAX = 4_294_967_295;
const U64_MAX = 18_446_744_073_709_551_615n;

export const STORAGE_V2_ERRORS = {
  100: ["WIRE_SCHEMA_INVALID", false],
  101: ["WIRE_NON_CANONICAL", false],
  102: ["WIRE_VERSION_MISMATCH", false],
  103: ["WIRE_GENESIS_MISMATCH", false],
  104: ["WIRE_DESCRIPTOR_MISMATCH", false],
  105: ["WIRE_SEQUENCE_INVALID", false],
  106: ["REQUEST_DEADLINE_EXPIRED", false],
  107: ["REQUEST_CANCELLED", false],
  108: ["REQUEST_NOT_FOUND", false],
  109: ["GRANT_REQUIRED", false],
  110: ["GRANT_SCOPE_DENIED", false],
  111: ["GRANT_EXPIRED", false],
  112: ["GRANT_REVOKED", false],
  113: ["HOST_OUTBOX_UNAVAILABLE", false],
  114: ["HOST_OUTBOX_FULL", true],
  115: ["HOST_OUTBOX_CORRUPT", false],
  116: ["HOST_OUTBOX_EXPIRED", false],
  200: ["STORAGE_CHUNK_OUT_OF_ORDER", false],
  201: ["STORAGE_CHUNK_TOO_LARGE", false],
  202: ["STORAGE_CHUNK_MISSING", false],
  203: ["STORAGE_LENGTH_MISMATCH", false],
  204: ["STORAGE_CID_MISMATCH", false],
  205: ["STORAGE_OBJECT_TOO_LARGE", false],
  206: ["STORAGE_IDEMPOTENCY_CONFLICT", false],
  207: ["STORAGE_RANGE_INVALID", false],
  208: ["STORAGE_INTEGRITY_FAILED", false],
  209: ["STORAGE_NOT_PUBLISHABLE", true],
  210: ["STORAGE_NOT_FOUND", false],
  211: ["ENCRYPTION_NONCE_REUSE", false],
  220: ["STORAGE_CHECKPOINT_WRONG_DOMAIN", false],
  221: ["STORAGE_CHECKPOINT_WRONG_VERSION", false],
  222: ["STORAGE_CHECKPOINT_WRONG_BUCKET", false],
  223: ["STORAGE_CHECKPOINT_WRONG_KEY", false],
  224: ["STORAGE_CHECKPOINT_STALE_NONCE", true],
  225: ["STORAGE_CHECKPOINT_WRONG_WINDOW", false],
  226: ["CAPABILITY_SIGNATURE_INVALID", false],
  227: ["CAPABILITY_AUDIENCE_INVALID", false],
  228: ["CAPABILITY_CONTENT_INVALID", false],
  229: ["CAPABILITY_NONCE_REPLAY", false],
  230: ["CAPABILITY_EXPIRED", false],
  231: ["CAPABILITY_ISSUER_REVOKED", false],
  232: ["RESUME_SIGNATURE_INVALID", false],
  233: ["RESUME_AUDIENCE_INVALID", false],
  234: ["RESUME_REPLAY", false],
  235: ["RESUME_EXPIRED", false],
  236: ["RESUME_REVOKED", false],
  237: ["RESUME_CURSOR_INVALID", false],
  238: ["PROVIDER_RECOVERY_TABLE_FULL", false],
  239: ["STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM", false],
  240: ["STORAGE_CHECKPOINT_SEQUENCE_INVALID", false],
  241: ["STORAGE_CHECKPOINT_EQUIVOCATION", false],
  250: ["BUCKET_NOT_FOUND", false],
  251: ["BUCKET_VERSION_CONFLICT", false],
  252: ["BUCKET_MEMBER_LIMIT", false],
  253: ["AGREEMENT_INVALID_STATE", false],
  254: ["AGREEMENT_CAPACITY_EXCEEDED", false],
  255: ["PROVIDER_INELIGIBLE", true],
  256: ["PROVIDER_ORG_UNKNOWN", false],
  257: ["PROVIDER_ATTESTATION_INVALID", false],
  258: ["PROVIDER_ATTESTATION_EXPIRED", false],
  259: ["PROVIDER_SLA_INVALID", false],
  260: ["PROVIDER_SERVICE_KEY_INVALID", false],
  261: ["STORAGE_CURSOR_STALE", true],
  300: ["DRIVE_NAME_INVALID", false],
  301: ["DRIVE_PATH_TOO_LONG", false],
  302: ["DRIVE_DEPTH_EXCEEDED", false],
  303: ["DRIVE_CHILD_LIMIT", false],
  304: ["DRIVE_METADATA_LIMIT", false],
  305: ["DRIVE_ORDER_INVALID", false],
  306: ["DRIVE_VERSION_CONFLICT", false],
  307: ["DRIVE_REFERENCE_UNPUBLISHABLE", false],
  320: ["S3_BUCKET_NAME_INVALID", false],
  321: ["S3_KEY_INVALID", false],
  322: ["S3_METADATA_LIMIT", false],
  323: ["S3_PRECONDITION_FAILED", false],
  324: ["S3_NOT_FOUND", false],
  325: ["S3_HISTORY_LIMIT", false],
  400: ["IDENTITY_AUDIENCE_INVALID", false],
  401: ["IDENTITY_CHALLENGE_REPLAY", false],
  402: ["IDENTITY_PROOF_EXPIRED", false],
  403: ["IDENTITY_EPOCH_INVALID", false],
  404: ["IDENTITY_DISCLOSURE_DENIED", false],
  405: ["IDENTITY_HUMANITY_UNAVAILABLE", true],
  406: ["IDENTITY_ENTITLEMENT_UNAVAILABLE", true],
  407: ["SIGNING_CONSENT_REQUIRED", false],
  408: ["IDENTITY_RECOVERY_ENTROPY_FAILED", false],
  409: ["IDENTITY_RECOVERY_INSTALL_FAILED", false],
  410: ["IDENTITY_OLD_INCARNATION", false],
  411: ["IDENTITY_RETIRED_SET_FULL", false],
} as const satisfies Record<number, readonly [name: string, retryable: boolean]>;

function shape(value: unknown, required: readonly string[], optional: readonly string[] = []): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new TypeError("value must be a closed record");
  const record = value as Record<string, unknown>;
  const allowed = new Set([...required, ...optional]);
  if (required.some((key) => !(key in record)) || Object.keys(record).some((key) => !allowed.has(key))) {
    throw new TypeError(`record shape must be exactly ${required.join(",")}${optional.length ? ` plus optional ${optional.join(",")}` : ""}`);
  }
  return record;
}

function uint(value: unknown, max: number, field: string, min = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) {
    throw new TypeError(`${field} must be an integer in ${min}..${max}`);
  }
  return value;
}

function u64(value: unknown, field: string, positive = false): bigint {
  if (typeof value !== "bigint" || value < (positive ? 1n : 0n) || value > U64_MAX) {
    throw new TypeError(`${field} must be ${positive ? "a positive" : "an unsigned"} u64`);
  }
  return value;
}

function fixedBytes(value: unknown, length: number, field: string): Uint8Array {
  if (!(value instanceof Uint8Array) || value.byteLength !== length) throw new TypeError(`${field} must contain exactly ${length} bytes`);
  return value;
}

function rangedBytes(value: unknown, min: number, max: number, field: string): Uint8Array {
  if (!(value instanceof Uint8Array) || value.byteLength < min || value.byteLength > max) {
    throw new TypeError(`${field} must contain ${min}-${max} bytes`);
  }
  return value;
}

function nfcText(value: unknown, min: number, max: number, field: string): string {
  if (typeof value !== "string" || value.normalize("NFC") !== value) throw new TypeError(`${field} must be NFC text`);
  const length = encoder.encode(value).byteLength;
  if (length < min || length > max) throw new TypeError(`${field} must contain ${min}-${max} UTF-8 bytes`);
  return value;
}

function cid(value: unknown, field: string): string { return nfcText(value, 1, 128, field); }
function enumValue(value: unknown, max: number, field: string): number { return uint(value, max, field); }
function providers(value: unknown, field: string): void {
  if (!Array.isArray(value) || value.length < 2 || value.length > 4) throw new TypeError(`${field} must contain 2-4 providers`);
  value.forEach((provider, index) => fixedBytes(provider, 32, `${field}[${index}]`));
}
function grantWindow(record: Record<string, unknown>): void {
  const issued = u64(record.issuedAt, "issuedAt");
  const expires = u64(record.expiresAt, "expiresAt");
  if (expires <= issued) throw new TypeError("expiresAt must be greater than issuedAt");
}

export function validateStorageV2Payload<Operation extends StorageV2Operation>(
  operation: Operation,
  value: StorageV2PayloadMap[Operation],
): void {
  let p: Record<string, unknown>;
  switch (operation) {
    case "storage.bucket.create":
      p = shape(value, ["replicaCount", "providers", "encryption"]); uint(p.replicaCount, U32_MAX, "replicaCount"); providers(p.providers, "providers"); enumValue(p.encryption, 1, "encryption"); return;
    case "storage.bucket.get":
      p = shape(value, ["bucketId"], ["at"]); fixedBytes(p.bucketId, 32, "bucketId"); if (p.at !== undefined) fixedBytes(p.at, 32, "at"); return;
    case "storage.bucket.grant":
    case "storage.drive.share":
      p = shape(value, ["bucketId", "subject", "role", "issuedAt", "expiresAt"]); fixedBytes(p.bucketId, 32, "bucketId"); fixedBytes(p.subject, 32, "subject"); enumValue(p.role, 2, "role"); grantWindow(p); return;
    case "storage.bucket.revoke":
      p = shape(value, ["bucketId", "grantId", "expectedVersion"]); fixedBytes(p.bucketId, 32, "bucketId"); fixedBytes(p.grantId, 32, "grantId"); u64(p.expectedVersion, "expectedVersion"); return;
    case "storage.object.put":
      p = shape(value, ["bucketId", "cid", "length", "encrypted", "transferId"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.cid, "cid"); u64(p.length, "length"); enumValue(p.encrypted, 1, "encrypted"); fixedBytes(p.transferId, 16, "transferId"); return;
    case "storage.object.get":
    case "storage.object.status":
    case "storage.deletion.status":
      p = shape(value, ["bucketId", "cid"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.cid, "cid"); return;
    case "storage.object.range":
      p = shape(value, ["bucketId", "cid", "offset", "length"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.cid, "cid"); u64(p.offset, "offset"); u64(p.length, "length", true); return;
    case "storage.object.delete":
      p = shape(value, ["bucketId", "cid", "expectedVersion"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.cid, "cid"); u64(p.expectedVersion, "expectedVersion"); return;
    case "storage.checkpoint.status":
      p = shape(value, ["bucketId"], ["root"]); fixedBytes(p.bucketId, 32, "bucketId"); if (p.root !== undefined) fixedBytes(p.root, 32, "root"); return;
    case "storage.checkpoint.subscribe":
    case "storage.replica.subscribe":
      p = shape(value, ["bucketId", "cursor"]); fixedBytes(p.bucketId, 32, "bucketId"); u64(p.cursor, "cursor"); return;
    case "storage.replica.status":
      p = shape(value, ["bucketId"]); fixedBytes(p.bucketId, 32, "bucketId"); return;
    case "storage.deletion.subscribe":
      p = shape(value, ["bucketId", "cid", "cursor"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.cid, "cid"); u64(p.cursor, "cursor"); return;
    case "storage.drive.read":
      p = shape(value, ["bucketId", "path"], ["manifest"]); fixedBytes(p.bucketId, 32, "bucketId"); nfcText(p.path, 1, 4096, "path"); if (p.manifest !== undefined) cid(p.manifest, "manifest"); return;
    case "storage.drive.commit":
      p = shape(value, ["bucketId", "manifest", "bytes", "expectedVersion", "mode"]); fixedBytes(p.bucketId, 32, "bucketId"); cid(p.manifest, "manifest"); rangedBytes(p.bytes, 1, 4_194_304, "bytes"); u64(p.expectedVersion, "expectedVersion"); enumValue(p.mode, 2, "mode"); return;
    case "storage.s3.put":
      p = shape(value, ["bucket", "key", "cid", "metadata", "mediaType", "transferId"], ["ifMatch"]); nfcText(p.bucket, 1, 128, "bucket"); rangedBytes(p.key, 1, 1024, "key"); cid(p.cid, "cid"); rangedBytes(p.metadata, 0, 32768, "metadata"); nfcText(p.mediaType, 1, 256, "mediaType"); if (p.ifMatch !== undefined) nfcText(p.ifMatch, 64, 64, "ifMatch"); fixedBytes(p.transferId, 16, "transferId"); return;
    case "storage.s3.get":
      p = shape(value, ["bucket", "key"], ["version"]); nfcText(p.bucket, 1, 128, "bucket"); rangedBytes(p.key, 1, 1024, "key"); if (p.version !== undefined) u64(p.version, "version"); return;
    case "storage.s3.list":
      p = shape(value, ["bucket", "limit"], ["prefix", "cursor"]); nfcText(p.bucket, 1, 128, "bucket"); if (p.prefix !== undefined) rangedBytes(p.prefix, 0, 1024, "prefix"); if (p.cursor !== undefined) rangedBytes(p.cursor, 1, 2048, "cursor"); uint(p.limit, 100, "limit", 1); return;
    case "storage.s3.delete":
      p = shape(value, ["bucket", "key", "transferId"], ["ifMatch"]); nfcText(p.bucket, 1, 128, "bucket"); rangedBytes(p.key, 1, 1024, "key"); if (p.ifMatch !== undefined) nfcText(p.ifMatch, 64, 64, "ifMatch"); fixedBytes(p.transferId, 16, "transferId"); return;
    case "storage.publish":
      p = shape(value, ["nameHash", "cid"], ["expectedVersion"]); fixedBytes(p.nameHash, 32, "nameHash"); cid(p.cid, "cid"); if (p.expectedVersion !== undefined) u64(p.expectedVersion, "expectedVersion"); return;
    case "storage.resolve":
      p = shape(value, ["name"], ["version", "at"]); nfcText(p.name, 1, 256, "name"); if (p.version !== undefined) u64(p.version, "version"); if (p.at !== undefined) fixedBytes(p.at, 32, "at"); return;
    case "storage.keys.export":
      p = shape(value, ["bucketId", "keyVersion", "recipientKey"]); fixedBytes(p.bucketId, 32, "bucketId"); uint(p.keyVersion, U32_MAX, "keyVersion"); rangedBytes(p.recipientKey, 32, 256, "recipientKey"); return;
    case "storage.keys.import":
      p = shape(value, ["bucketId", "wrappedKey", "replace", "keyVersion"]); fixedBytes(p.bucketId, 32, "bucketId"); rangedBytes(p.wrappedKey, 32, 1024, "wrappedKey"); enumValue(p.replace, 1, "replace"); uint(p.keyVersion, U32_MAX, "keyVersion"); return;
  }
}

function finality(value: unknown): void {
  const p = shape(value, ["number", "hash"]); u64(p.number, "finalized.number"); fixedBytes(p.hash, 32, "finalized.hash");
}
function checkpoint(value: unknown): void {
  const p = shape(value, ["root", "from", "to", "replicas"]); fixedBytes(p.root, 32, "checkpoint.root"); const from=u64(p.from,"checkpoint.from"); const to=u64(p.to,"checkpoint.to"); if(to<from) throw new TypeError("checkpoint.to must not precede from"); uint(p.replicas,U32_MAX,"checkpoint.replicas");
}
function receipt(value: unknown): void {
  const p=shape(value,["provider","cid","length","signature"]); fixedBytes(p.provider,32,"receipt.provider"); cid(p.cid,"receipt.cid"); u64(p.length,"receipt.length"); fixedBytes(p.signature,64,"receipt.signature");
}
function subscription(value: unknown): void {
  const p=shape(value,["operationId","cursor"]); fixedBytes(p.operationId,16,"operationId"); u64(p.cursor,"cursor");
}

export function validateStorageV2Result<Operation extends StorageV2Operation>(operation: Operation, value: StorageV2ResultMap[Operation]): void {
  let p: Record<string, unknown>;
  switch(operation) {
    case "storage.bucket.create": p=shape(value,["bucketId","version","finalized"]); fixedBytes(p.bucketId,32,"bucketId"); u64(p.version,"version"); finality(p.finalized); return;
    case "storage.bucket.get": p=shape(value,["owner","version","replicaCount","primary","providers","finalized"]); fixedBytes(p.owner,32,"owner");u64(p.version,"version");uint(p.replicaCount,U32_MAX,"replicaCount");fixedBytes(p.primary,32,"primary");providers(p.providers,"providers");finality(p.finalized);return;
    case "storage.bucket.grant": case "storage.bucket.revoke": case "storage.drive.share": p=shape(value,["grantId","version","finalized"]);fixedBytes(p.grantId,32,"grantId");u64(p.version,"version");finality(p.finalized);return;
    case "storage.object.put": p=shape(value,["receipt","publishable","finalized"]);receipt(p.receipt);if(typeof p.publishable!=="boolean")throw new TypeError("publishable must be boolean");finality(p.finalized);return;
    case "storage.object.get": p=shape(value,["cid","length","checkpoint"]);cid(p.cid,"cid");u64(p.length,"length");checkpoint(p.checkpoint);return;
    case "storage.object.range": p=shape(value,["cid","offset","length","total","checkpoint"]);cid(p.cid,"cid");const offset=u64(p.offset,"offset");const length=u64(p.length,"length",true);const total=u64(p.total,"total");if(offset+length>total)throw new TypeError("range exceeds total");checkpoint(p.checkpoint);return;
    case "storage.object.delete": p=shape(value,["version","pending","confirmed","finalized"]);u64(p.version,"version");uint(p.pending,U32_MAX,"pending");uint(p.confirmed,U32_MAX,"confirmed");finality(p.finalized);return;
    case "storage.object.status": p=shape(value,["state","replicas","publishable","finalized"],["receipt","checkpoint"]);enumValue(p.state,4,"state");if(p.receipt!==undefined)receipt(p.receipt);if(p.checkpoint!==undefined)checkpoint(p.checkpoint);uint(p.replicas,U32_MAX,"replicas");if(typeof p.publishable!=="boolean")throw new TypeError("publishable must be boolean");finality(p.finalized);return;
    case "storage.checkpoint.status": p=shape(value,["checkpoint","sequence","block","quorum","finalized"]);checkpoint(p.checkpoint);uint(p.sequence,U32_MAX,"sequence");u64(p.block,"block");uint(p.quorum,U32_MAX,"quorum");finality(p.finalized);return;
    case "storage.checkpoint.subscribe": case "storage.replica.subscribe": case "storage.deletion.subscribe": subscription(value);return;
    case "storage.replica.status": p=shape(value,["primary","providers","healthy","lastCheckpoint","pending","finalized"]);fixedBytes(p.primary,32,"primary");providers(p.providers,"providers");uint(p.healthy,U32_MAX,"healthy");u64(p.lastCheckpoint,"lastCheckpoint");uint(p.pending,U32_MAX,"pending");finality(p.finalized);return;
    case "storage.deletion.status": p=shape(value,["version","confirmations","root","finalized"]);u64(p.version,"version");uint(p.confirmations,U32_MAX,"confirmations");fixedBytes(p.root,32,"root");finality(p.finalized);return;
    case "storage.drive.read": p=shape(value,["manifest","entry","version","finalized"]);cid(p.manifest,"manifest");cid(p.entry,"entry");u64(p.version,"version");finality(p.finalized);return;
    case "storage.drive.commit": p=shape(value,["manifest","version","checkpoint","finalized"]);cid(p.manifest,"manifest");u64(p.version,"version");checkpoint(p.checkpoint);finality(p.finalized);return;
    case "storage.s3.put": p=shape(value,["etag","version","finalized"]);nfcText(p.etag,64,64,"etag");u64(p.version,"version");finality(p.finalized);return;
    case "storage.s3.get": p=shape(value,["cid","etag","version","finalized"]);cid(p.cid,"cid");nfcText(p.etag,64,64,"etag");u64(p.version,"version");finality(p.finalized);return;
    case "storage.s3.list": p=shape(value,["cids","version","finalized"],["cursor"]);if(!Array.isArray(p.cids)||p.cids.length>100)throw new TypeError("cids must contain at most 100 entries");p.cids.forEach((item,index)=>cid(item,`cids[${index}]`));if(p.cursor!==undefined)rangedBytes(p.cursor,1,2048,"cursor");u64(p.version,"version");finality(p.finalized);return;
    case "storage.s3.delete": p=shape(value,["version","remainingHistory","finalized"]);u64(p.version,"version");uint(p.remainingHistory,U32_MAX,"remainingHistory");finality(p.finalized);return;
    case "storage.publish": p=shape(value,["nameHash","cid","finalized"]);fixedBytes(p.nameHash,32,"nameHash");cid(p.cid,"cid");finality(p.finalized);return;
    case "storage.resolve": p=shape(value,["cid","version","checkpoint","finalized"]);cid(p.cid,"cid");u64(p.version,"version");checkpoint(p.checkpoint);finality(p.finalized);return;
    case "storage.keys.export": p=shape(value,["wrappedKey","algorithm","keyVersion"]);rangedBytes(p.wrappedKey,32,1024,"wrappedKey");uint(p.algorithm,U16_MAX,"algorithm");uint(p.keyVersion,U32_MAX,"keyVersion");return;
    case "storage.keys.import": p=shape(value,["keyId","keyVersion"]);fixedBytes(p.keyId,32,"keyId");uint(p.keyVersion,U32_MAX,"keyVersion");return;
  }
}

export function validateStorageV2Progress(operation: StorageV2Operation, progress: StorageV2Progress<StorageV2Operation>): void {
  if (["storage.object.get","storage.object.range","storage.s3.get"].includes(operation)) {
    const p=shape(progress,["kind","requestId","seq","offset","bytes"]);u64(p.offset,"offset");rangedBytes(p.bytes,0,4_194_304,"bytes");return;
  }
  const p=shape(progress,["kind","requestId","seq","completed"],["total","chunksAcked","replicasConfirmed"]);u64(p.completed,"completed");if(p.total!==undefined)u64(p.total,"total");if(p.chunksAcked!==undefined)u64(p.chunksAcked,"chunksAcked");if(p.replicasConfirmed!==undefined)u64(p.replicasConfirmed,"replicasConfirmed");
}

export interface StorageV2ErrorDetails { readonly message?: string; readonly lower?: bigint; readonly upper?: bigint; readonly hash?: Uint8Array }
function errorFamily(code: number): "common" | "content" | "proof" | "control" | "drive-s3" | "other" {
  if (code >= 100 && code <= 116) return "common";
  if (code >= 200 && code <= 211) return "content";
  if (code >= 220 && code <= 241) return "proof";
  if (code >= 250 && code <= 261) return "control";
  if (code >= 300 && code <= 325) return "drive-s3";
  return "other";
}
function allowedErrorFamilies(operation: StorageV2Operation): readonly string[] {
  if (operation.startsWith("storage.bucket.")) return ["common","control"];
  if (["storage.object.put"].includes(operation)) return ["common","content","proof"];
  if (["storage.object.delete","storage.publish"].includes(operation)) return ["common","content","control"];
  if (["storage.object.get","storage.object.range","storage.object.status","storage.deletion.status","storage.deletion.subscribe","storage.resolve"].includes(operation)) return ["common","content"];
  if (["storage.checkpoint.status","storage.checkpoint.subscribe"].includes(operation)) return ["common","proof"];
  if (["storage.replica.status","storage.replica.subscribe","storage.drive.share"].includes(operation)) return ["common","control"];
  if (["storage.drive.read","storage.drive.commit","storage.s3.put","storage.s3.get"].includes(operation)) return ["common","drive-s3","content"];
  if (operation === "storage.s3.list") return ["common","drive-s3","control"];
  if (operation === "storage.s3.delete") return ["common","drive-s3"];
  return ["common"];
}
export function validateStorageV2Error(operation: StorageV2Operation, error: StorageV2ErrorEvent): void {
  const frozen=STORAGE_V2_ERRORS[error.code as keyof typeof STORAGE_V2_ERRORS];
  if(frozen===undefined||error.name!==frozen[0]||error.retryable!==frozen[1]||!allowedErrorFamilies(operation).includes(errorFamily(error.code)))throw new TypeError("error code/name/retryability/scope drift");
  if(error.details!==undefined){const p=shape(error.details,[],["message","lower","upper","hash"]);if(p.message!==undefined)nfcText(p.message,1,256,"details.message");if(p.lower!==undefined)u64(p.lower,"details.lower");if(p.upper!==undefined)u64(p.upper,"details.upper");if(p.hash!==undefined)fixedBytes(p.hash,32,"details.hash");}
}
