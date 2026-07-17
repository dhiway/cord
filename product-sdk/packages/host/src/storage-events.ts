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
import type { TypedFinalizedEvent, TypedFinalizedEventSource } from "./attestation-events.ts";
import {
  storageNativeEventOutcome,
  type AccountId,
  type AgreementId,
  type AgreementStatus,
  type BlockHash,
  type BlockNumber,
  type BucketId,
  type BucketRole,
  type ChallengeId,
  type CheckpointCommitment,
  type CommitmentState,
  type ContentCommitment,
  type ContentHash,
  type DecimalU64,
  type DriveId,
  type DriveNodeKind,
  type DriveRole,
  type FinalizedStorageNativeEvent,
  type ObjectId,
  type ProviderId,
  type ProviderStatus,
  type StorageNativeEvent,
  type StorageNativeEventKind,
  type StorageNativeEventSubscription,
  type StorageNativeOutcome,
} from "@cord-network/origin-sdk-cloud-storage";

export interface FinalizedStorageNativeOutcome {
  readonly event: FinalizedStorageNativeEvent;
  readonly outcome: StorageNativeOutcome;
}

const HASH = /^0x[0-9a-f]{64}$/i;
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const U64_MAX = 18_446_744_073_709_551_615n;
const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const UNEXPOSED_STORAGE_PROVIDER_EVENTS = new Set([
  "ServiceKeyRotationScheduled", "ServiceKeyRotated", "ProviderOrganizationRotated",
  "ProviderAuthorityRefreshed", "BucketAuthorityRefreshed", "BucketReconciliationDeferred",
  "FinalizedCheckpointAdvanced", "HostDelegationCreated", "HostDelegationKeyRotated",
  "HostDelegationRevoked", "ChallengeEvidenceOverflowed", "EvidenceRecorded",
  "ProviderIneligible", "CheckpointFallbackPromotionPendingQuorum",
]);

function rejected(message: string): never { throw new ProductSdkError("runtime_rejected", message); }
function hash(value: unknown, field: string): string {
  if (typeof value !== "string" || !HASH.test(value)) rejected(`Storage.${field} is not a 32-byte hash`);
  return value.toLowerCase();
}
function accountBytes(value: unknown, field: string): Uint8Array {
  if (typeof value !== "string") rejected(`Storage.${field} is not an account`);
  if (HASH.test(value)) return Uint8Array.from(value.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
  let integer = 0n;
  for (const character of value) {
    const digit = BASE58.indexOf(character);
    if (digit < 0) rejected(`Storage.${field} is not an SS58 account`);
    integer = integer * 58n + BigInt(digit);
  }
  const decoded: number[] = [];
  while (integer > 0n) { decoded.push(Number(integer & 0xffn)); integer >>= 8n; }
  decoded.reverse();
  for (const character of value) { if (character !== "1") break; decoded.unshift(0); }
  const bytes = Uint8Array.from(decoded);
  if (bytes.length < 35 || bytes[0]! >= 128) rejected(`Storage.${field} does not encode AccountId32`);
  const prefixLength = (bytes[0]! & 0x40) === 0 ? 1 : 2;
  if (bytes.length !== prefixLength + 34) rejected(`Storage.${field} does not encode AccountId32`);
  return bytes.slice(prefixLength, prefixLength + 32);
}
function account(value: unknown, field: string): AccountId {
  accountBytes(value, field);
  return value as AccountId;
}
function decimal(value: unknown, field: string): DecimalU64 {
  const text = typeof value === "bigint" || typeof value === "number" || typeof value === "string" ? String(value) : "";
  if (!DECIMAL.test(text) || BigInt(text) > U64_MAX) rejected(`Storage.${field} is not a u64`);
  return text as DecimalU64;
}
function block(value: unknown, field: string): BlockNumber {
  const text = decimal(value, field);
  if (BigInt(text) > 0xffff_ffffn) rejected(`Storage.${field} is not a block number`);
  return text as BlockNumber;
}
function u32(value: unknown, field: string): number {
  const text = decimal(value, field);
  if (BigInt(text) > 0xffff_ffffn) rejected(`Storage.${field} is not a u32`);
  return Number(text);
}
function u16(value: unknown, field: string): number {
  const decoded = u32(value, field);
  if (decoded > 0xffff) rejected(`Storage.${field} is not a u16`);
  return decoded;
}
function bool(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") rejected(`Storage.${field} is not boolean`);
  return value;
}
function utf8(value: unknown, field: string): string {
  if (typeof value === "string" && !value.startsWith("0x")) return value;
  let encoded: Uint8Array | undefined;
  if (typeof value === "string" && /^0x(?:[0-9a-f]{2})*$/i.test(value))
    encoded = Uint8Array.from(value.slice(2).match(/../g)?.map((pair) => Number.parseInt(pair, 16)) ?? []);
  else if (value instanceof Uint8Array) encoded = value;
  else if (Array.isArray(value) && value.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)) encoded = Uint8Array.from(value);
  else if (value && typeof value === "object") {
    const wrapper = value as { asBytes?: () => unknown; toHex?: () => unknown; value?: unknown };
    if (typeof wrapper.asBytes === "function") return utf8(wrapper.asBytes(), field);
    if (typeof wrapper.toHex === "function") return utf8(wrapper.toHex(), field);
    if ("value" in wrapper) return utf8(wrapper.value, field);
  }
  if (!encoded) rejected(`Storage.${field} is not descriptor-shaped bytes`);
  try { return new TextDecoder("utf-8", { fatal: true }).decode(encoded); }
  catch { rejected(`Storage.${field} is not UTF-8`); }
}
function drivePath(value: unknown, field: string): string {
  const path = utf8(value, field);
  const encoded = new TextEncoder().encode(path);
  if (new TextDecoder("utf-8", { fatal: true }).decode(encoded) !== path)
    rejected(`Storage.${field} is not canonical UTF-8`);
  if (encoded.length < 1 || encoded.length > 4_096 || path[0] !== "/")
    rejected(`Storage.${field} violates the native Drive path bounds`);
  if (path === "/") return path;
  if (path.endsWith("/")) rejected(`Storage.${field} violates the native Drive path bounds`);
  const components = path.slice(1).split("/");
  if (components.length > 64) rejected(`Storage.${field} exceeds the native Drive depth`);
  for (const component of components) {
    const bytes = new TextEncoder().encode(component);
    if (bytes.length < 1 || bytes.length > 256 || component.includes("\0") || component === "." || component === ".." || component.normalize("NFC") !== component)
      rejected(`Storage.${field} contains an invalid native Drive component`);
  }
  return path;
}
function s3BucketName(value: unknown, field: string): string {
  const name = utf8(value, field);
  if (!/^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$/.test(name))
    rejected(`Storage.${field} violates the native S3 bucket-name bounds`);
  return name;
}
function nativeObjectKey(value: unknown, field: string): Uint8Array {
  let decoded: Uint8Array;
  if (value instanceof Uint8Array) decoded = value.slice();
  else if (Array.isArray(value) && value.every((item) => Number.isInteger(item) && item >= 0 && item <= 255)) decoded = Uint8Array.from(value);
  else if (typeof value === "string" && /^0x(?:[0-9a-f]{2})*$/i.test(value)) decoded = Uint8Array.from(value.slice(2).match(/../g)?.map((pair) => Number.parseInt(pair, 16)) ?? []);
  else rejected(`Storage.${field} is not descriptor-shaped bytes`);
  if (decoded.length < 1 || decoded.length > 1_024 || decoded.includes(0)) rejected(`Storage.${field} violates the native object-key bounds`);
  return decoded;
}
function variant<T extends string>(value: unknown, field: string, allowed: readonly T[]): T {
  const raw = typeof value === "string" ? value : value && typeof value === "object" && "type" in value ? String((value as { type: unknown }).type) : "";
  const normalized = raw.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  if (!allowed.includes(normalized as T)) rejected(`Storage.${field} is unknown`);
  return normalized as T;
}
function nullableVariant<T extends string>(value: unknown, field: string, allowed: readonly T[]): T | null {
  return value === null || value === undefined ? null : variant(value, field, allowed);
}
function nullableHash(value: unknown, field: string): ContentCommitment | null {
  return value === null || value === undefined ? null : hash(value, field) as ContentCommitment;
}
function data(native: TypedFinalizedEvent): Readonly<Record<string, unknown>> {
  if (!native.data || typeof native.data !== "object" || Array.isArray(native.data)) rejected(`Storage.${native.event} data is not an object`);
  return native.data;
}
function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < left.length; index += 1) {
    const difference = left[index]! - right[index]!;
    if (difference !== 0) return difference;
  }
  return 0;
}
function byteKey(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
function accounts(value: unknown, field: string, minimum: number, maximum: number, sorted: boolean): readonly ProviderId[] {
  if (!Array.isArray(value)) rejected(`Storage.${field} is not an account list`);
  const decoded = value.map((item, index) => account(item, `${field}[${index}]`) as ProviderId);
  if (decoded.length < minimum || decoded.length > maximum) rejected(`Storage.${field} must contain ${minimum}-${maximum} accounts`);
  const raw = value.map((item, index) => accountBytes(item, `${field}[${index}]`));
  const keys = raw.map(byteKey);
  if (new Set(keys).size !== keys.length) rejected(`Storage.${field} contains duplicate accounts`);
  if (sorted && raw.some((item, index) => index > 0 && compareBytes(raw[index - 1]!, item) >= 0)) rejected(`Storage.${field} is not in canonical account order`);
  return decoded;
}
function commitment(value: unknown): CheckpointCommitment {
  if (!value || typeof value !== "object" || Array.isArray(value)) rejected("Storage.commitment is not an object");
  const entry = value as Readonly<Record<string, unknown>>;
  const result = {
    mmr_root: hash(entry.mmr_root, "commitment.mmr_root") as ContentCommitment,
    start_seq: decimal(entry.start_seq, "commitment.start_seq"),
    leaf_count: decimal(entry.leaf_count, "commitment.leaf_count"),
  };
  if (BigInt(result.leaf_count) === 0n || BigInt(result.start_seq) + BigInt(result.leaf_count) > U64_MAX)
    rejected("Storage.commitment sequence range is invalid");
  return result;
}

function storageBucketCreated(d: Readonly<Record<string, unknown>>): StorageNativeEvent {
  const primary = provider(d.primary, "primary");
  const replicas = accounts(d.replicas, "replicas", 2, 4, false);
  const primaryKey = byteKey(accountBytes(primary, "primary"));
  if (replicas.some((replica) => byteKey(accountBytes(replica, "replica")) === primaryKey))
    rejected("Storage.replicas contains the primary provider");
  return { event: "storage_bucket_created", data: { bucket: bucket(d.bucket_id, "bucket_id"), owner: account(d.owner, "owner"), primary, replicas, version: decimal(d.version, "version") } };
}
function checkpointAccepted(d: Readonly<Record<string, unknown>>): StorageNativeEvent {
  return { event: "checkpoint_accepted", data: { bucket: bucket(d.bucket_id, "bucket_id"), commitment: commitment(d.commitment), checkpoint: block(d.checkpoint, "checkpoint"), replica_confirmations: accounts(d.replica_confirmations, "replica_confirmations", 2, 2, true) } };
}

const provider = (value: unknown, field = "provider") => account(value, field) as ProviderId;
const agreement = (value: unknown) => hash(value, "agreement_id") as AgreementId;
const challenge = (value: unknown) => hash(value, "challenge_id") as ChallengeId;
const drive = (value: unknown) => hash(value, "drive_id") as DriveId;
const bucket = (value: unknown, field = "bucket") => hash(value, field) as BucketId;
const object = (value: unknown) => hash(value, "object") as ObjectId;

/** Decode the curated, exact native Commons storage event surface. */
export function decodeStorageNativeEvent(native: TypedFinalizedEvent): StorageNativeEvent | null {
  if (!new Set(["StorageProvider", "Drive", "S3"]).has(native.pallet)) return null;
  if (native.pallet === "StorageProvider" && UNEXPOSED_STORAGE_PROVIDER_EVENTS.has(native.event)) return null;
  const d = data(native);
  switch (`${native.pallet}.${native.event}`) {
    case "StorageProvider.ProviderRegistered": return { event: "provider_registered", data: { provider: provider(d.provider), capacity_bytes: decimal(d.capacity_bytes, "capacity_bytes") } };
    case "StorageProvider.ProviderUpdated": return { event: "provider_updated", data: { provider: provider(d.provider), capacity_bytes: decimal(d.capacity_bytes, "capacity_bytes") } };
    case "StorageProvider.ProviderStatusChanged": return { event: "provider_status_changed", data: { provider: provider(d.provider), status: variant<ProviderStatus>(d.status, "status", ["active", "suspended"]) } };
    case "StorageProvider.ProviderRemoved": return { event: "provider_removed", data: { provider: provider(d.provider) } };
    case "StorageProvider.Heartbeat": return { event: "heartbeat", data: { provider: provider(d.provider), at: block(d.at, "at") } };
    case "StorageProvider.BucketCreated": return storageBucketCreated(d);
    case "StorageProvider.BucketGrantChanged": return { event: "storage_bucket_grant_changed", data: { bucket: bucket(d.bucket_id, "bucket_id"), account: account(d.account, "account"), role: nullableVariant<BucketRole>(d.role, "role", ["reader", "writer", "admin"]), previous_version: decimal(d.previous_version, "previous_version"), new_version: decimal(d.new_version, "new_version") } };
    case "StorageProvider.AgreementTransitioned": return { event: "agreement_transitioned", data: { agreement: agreement(d.agreement_id), previous: nullableVariant<AgreementStatus>(d.previous, "previous", ["proposed", "active", "suspended", "cancelled", "expired"]), current: variant<AgreementStatus>(d.current, "current", ["proposed", "active", "suspended", "cancelled", "expired"]), previous_version: decimal(d.previous_version, "previous_version"), new_version: decimal(d.new_version, "new_version") } };
    case "StorageProvider.AgreementCapacityReleased": return { event: "agreement_capacity_released", data: { agreement: agreement(d.agreement_id) } };
    case "StorageProvider.AgreementProviderRebound": return { event: "agreement_provider_rebound", data: { agreement: agreement(d.agreement_id), old_provider: provider(d.old_provider, "old_provider"), new_provider: provider(d.new_provider, "new_provider"), status: variant<AgreementStatus>(d.status, "status", ["proposed", "active", "suspended", "cancelled", "expired"]), bytes: decimal(d.bytes, "bytes") } };
    case "StorageProvider.ChallengeIssued": return { event: "challenge_issued", data: { challenge: challenge(d.challenge_id), bucket: bucket(d.bucket_id, "bucket_id"), provider: provider(d.provider), due_at: block(d.due_at, "due_at") } };
    case "StorageProvider.ChallengeProved": return { event: "challenge_proved", data: { challenge: challenge(d.challenge_id), provider: provider(d.provider) } };
    case "StorageProvider.ChallengeTimedOut": return { event: "challenge_timed_out", data: { challenge: challenge(d.challenge_id), provider: provider(d.provider), checkpoint: block(d.checkpoint, "checkpoint") } };
    case "StorageProvider.CheckpointAccepted": return checkpointAccepted(d);
    case "StorageProvider.CheckpointEquivocation": return { event: "checkpoint_equivocation", data: { code: u16(d.code, "code"), bucket: bucket(d.bucket_id, "bucket_id"), provider: provider(d.provider), accepted_root: hash(d.accepted_root, "accepted_root") as ContentCommitment, conflicting_root: hash(d.conflicting_root, "conflicting_root") as ContentCommitment, nonce: block(d.nonce, "nonce") } };
    case "StorageProvider.ReplicaSelected": return { event: "replica_selected", data: { bucket: bucket(d.bucket_id, "bucket_id"), provider: provider(d.provider), checkpoint: block(d.checkpoint, "checkpoint") } };
    case "StorageProvider.PrimaryPromoted": return { event: "primary_promoted", data: { bucket: bucket(d.bucket_id, "bucket_id"), old_provider: provider(d.old_provider, "old_provider"), new_provider: provider(d.new_provider, "new_provider"), checkpoint: block(d.checkpoint, "checkpoint") } };
    case "StorageProvider.BucketReplicaReplaced": return { event: "bucket_replica_replaced", data: { bucket: bucket(d.bucket_id, "bucket_id"), old_provider: provider(d.old_provider, "old_provider"), new_provider: provider(d.new_provider, "new_provider"), previous_version: decimal(d.previous_version, "previous_version"), new_version: decimal(d.new_version, "new_version") } };
    case "StorageProvider.ManifestCommitmentChanged": return { event: "manifest_commitment_changed", data: { manifest: hash(d.manifest, "manifest") as ContentCommitment, bucket: bucket(d.bucket_id, "bucket_id"), state: variant<CommitmentState>(d.state, "state", ["publishable", "pending", "tombstoned", "missing"]), checkpoint: d.checkpoint === null || d.checkpoint === undefined ? null : block(d.checkpoint, "checkpoint") } };
    case "StorageProvider.ManifestDeletionAcknowledged": return { event: "manifest_deletion_acknowledged", data: { manifest: hash(d.manifest, "manifest") as ContentCommitment, bucket: bucket(d.bucket_id, "bucket_id"), provider: provider(d.provider), evidence_hash: hash(d.evidence_hash, "evidence_hash") as ContentCommitment, acknowledged_at: block(d.acknowledged_at, "acknowledged_at") } };
    case "Drive.DriveCreated": return { event: "drive_created", data: { drive: drive(d.drive_id), owner: account(d.owner, "owner"), version: decimal(d.version, "version") } };
    case "Drive.DriveRootUpdated": return { event: "drive_root_updated", data: { drive: drive(d.drive_id), previous_root: nullableHash(d.previous_root, "previous_root"), new_root: hash(d.new_root, "new_root") as ContentCommitment, previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "Drive.GrantChanged": return { event: "drive_grant_changed", data: { drive: drive(d.drive_id), subject: account(d.subject, "subject"), role: nullableVariant<DriveRole>(d.role, "role", ["reader", "writer", "admin"]), previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "Drive.DriveTransferred": return { event: "drive_transferred", data: { drive: drive(d.drive_id), old_owner: account(d.old_owner, "old_owner"), new_owner: account(d.new_owner, "new_owner"), previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "Drive.DriveArchived": return { event: "drive_archived", data: { drive: drive(d.drive_id), previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "Drive.NodeWritten": return { event: "drive_node_written", data: { drive: drive(d.drive_id), path: drivePath(d.path, "path"), kind: variant<DriveNodeKind>(d.kind, "kind", ["directory", "file"]), previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "Drive.NodeRemoved": return { event: "drive_node_removed", data: { drive: drive(d.drive_id), path: drivePath(d.path, "path"), previous_version: decimal(d.previous_version, "previous_version"), version: decimal(d.version, "version") } };
    case "S3.BucketCreated": return { event: "s3_bucket_created", data: { bucket: bucket(d.bucket), name: s3BucketName(d.name, "name"), owner: account(d.owner, "owner") } };
    case "S3.ControllerChanged": return { event: "s3_controller_changed", data: { bucket: bucket(d.bucket), controller: account(d.controller, "controller"), enabled: bool(d.enabled, "enabled"), version: decimal(d.version, "version") } };
    case "S3.BucketTransferred": return { event: "s3_bucket_transferred", data: { bucket: bucket(d.bucket), from: account(d.from, "from"), to: account(d.to, "to"), version: decimal(d.version, "version") } };
    case "S3.BucketArchived": return { event: "s3_bucket_archived", data: { bucket: bucket(d.bucket), archived: bool(d.archived, "archived"), version: decimal(d.version, "version") } };
    case "S3.BucketVersioningChanged": return { event: "s3_bucket_versioning_changed", data: { bucket: bucket(d.bucket), enabled: bool(d.enabled, "enabled"), version: decimal(d.version, "version") } };
    case "S3.ObjectPut": return { event: "s3_object_put", data: { bucket: bucket(d.bucket), object: object(d.object), key: nativeObjectKey(d.key, "key"), content_hash: hash(d.content_hash, "content_hash") as ContentHash, version: decimal(d.version, "version") } };
    case "S3.ObjectDeleted": return { event: "s3_object_deleted", data: { bucket: bucket(d.bucket), object: object(d.object), key: nativeObjectKey(d.key, "key"), version: decimal(d.version, "version") } };
    case "S3.ObjectPurged": return { event: "s3_object_purged", data: { bucket: bucket(d.bucket), key: nativeObjectKey(d.key, "key") } };
    case "S3.BucketDeleted": return { event: "s3_bucket_deleted", data: { bucket: bucket(d.bucket), name: s3BucketName(d.name, "name"), owner: account(d.owner, "owner") } };
    case "S3.ObjectHistoryPruned": return { event: "s3_object_history_pruned", data: { bucket: bucket(d.bucket), key: nativeObjectKey(d.key, "key"), through_version: decimal(d.through_version, "through_version"), removed: u32(d.removed, "removed") } };
    default: throw new ProductSdkError("unsupported_runtime", `unknown native storage event ${native.pallet}.${native.event}`);
  }
}

export async function* subscribeStorageNativeEvents(
  source: TypedFinalizedEventSource,
  subscription: StorageNativeEventSubscription,
  signal: AbortSignal = new AbortController().signal,
): AsyncGenerator<FinalizedStorageNativeOutcome> {
  const wanted = new Set<StorageNativeEventKind>(subscription.kinds);
  for await (const blockResult of source.subscribeFinalizedEvents({ from: subscription.from_finalized_block, signal })) {
    if (signal.aborted) throw new ProductSdkError("cancelled", "storage subscription cancelled");
    const finalizedBlockHash = hash(blockResult.hash, "finalized_block_hash") as BlockHash;
    for (const native of blockResult.events) {
      if (!Number.isSafeInteger(native.index) || native.index < 0 || native.index > 0xffff_ffff) rejected("Storage event index is not a u32");
      const event = decodeStorageNativeEvent(native);
      if (!event || !wanted.has(event.event)) continue;
      const finalized: FinalizedStorageNativeEvent = { finalized_block_hash: finalizedBlockHash, event_index: native.index, event };
      yield { event: finalized, outcome: storageNativeEventOutcome(event) };
    }
  }
}
