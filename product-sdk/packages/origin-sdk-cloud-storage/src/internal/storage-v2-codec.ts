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

import { validateStorageV2Payload } from "./storage-v2-validation.ts";
import type { StorageV2Intent, StorageV2Operation, StorageV2PayloadMap } from "./storage-v2-intents.ts";

type Cbor = bigint | number | string | boolean | Uint8Array | readonly Cbor[] | ReadonlyMap<number, Cbor>;
const utf8 = new TextEncoder();

function concat(parts: readonly Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function head(major: number, value: bigint): Uint8Array {
  if (value < 24n) return new Uint8Array([(major << 5) | Number(value)]);
  if (value <= 0xffn) return new Uint8Array([(major << 5) | 24, Number(value)]);
  if (value <= 0xffffn) return new Uint8Array([(major << 5) | 25, Number(value >> 8n), Number(value)]);
  if (value <= 0xffff_ffffn) return new Uint8Array([(major << 5) | 26, Number(value >> 24n), Number(value >> 16n), Number(value >> 8n), Number(value)]);
  if (value <= 0xffff_ffff_ffff_ffffn) return new Uint8Array([
    (major << 5) | 27,
    Number(value >> 56n), Number(value >> 48n), Number(value >> 40n), Number(value >> 32n),
    Number(value >> 24n), Number(value >> 16n), Number(value >> 8n), Number(value),
  ]);
  throw new TypeError("CBOR integer exceeds u64");
}

function encode(value: Cbor): Uint8Array {
  if (typeof value === "bigint") return head(0, value);
  if (typeof value === "number") return head(0, BigInt(value));
  if (typeof value === "boolean") return new Uint8Array([value ? 0xf5 : 0xf4]);
  if (typeof value === "string") { const bytes=utf8.encode(value); return concat([head(3,BigInt(bytes.length)),bytes]); }
  if (value instanceof Uint8Array) return concat([head(2,BigInt(value.length)),value]);
  if (Array.isArray(value)) return concat([head(4,BigInt(value.length)),...value.map(encode)]);
  if (value instanceof Map) {
    const entries=[...value.entries()].sort(([left],[right])=>left-right);
    return concat([head(5,BigInt(entries.length)),...entries.flatMap(([key,item])=>[encode(key),encode(item)])]);
  }
  throw new TypeError("unsupported CBOR value");
}

const map = (entries: readonly (readonly [number, Cbor | undefined])[]): ReadonlyMap<number, Cbor> =>
  new Map(entries.filter((entry): entry is readonly [number, Cbor] => entry[1] !== undefined));

function payloadMap<Operation extends StorageV2Operation>(operation: Operation, value: StorageV2PayloadMap[Operation]): ReadonlyMap<number, Cbor> {
  const p=value as Record<string, Cbor | undefined>;
  switch(operation) {
    case "storage.bucket.create": return map([[0,p.replicaCount],[1,p.providers],[2,p.encryption]]);
    case "storage.bucket.get": return map([[0,p.bucketId],[1,p.at]]);
    case "storage.bucket.grant": return map([[0,p.bucketId],[1,p.subject],[2,p.role],[3,p.issuedAt],[4,p.expiresAt]]);
    case "storage.bucket.revoke": return map([[0,p.bucketId],[1,p.grantId],[2,p.expectedVersion]]);
    case "storage.object.put": return map([[0,p.bucketId],[1,p.cid],[2,p.length],[3,p.encrypted],[4,p.transferId]]);
    case "storage.object.get": return map([[0,p.bucketId],[1,p.cid]]);
    case "storage.object.range": return map([[0,p.bucketId],[1,p.cid],[2,p.offset],[3,p.length]]);
    case "storage.object.delete": return map([[0,p.bucketId],[1,p.cid],[2,p.expectedVersion]]);
    case "storage.object.status": return map([[0,p.bucketId],[1,p.cid]]);
    case "storage.checkpoint.status": return map([[0,p.bucketId],[1,p.root]]);
    case "storage.checkpoint.subscribe": return map([[0,p.bucketId],[1,p.cursor]]);
    case "storage.replica.status": return map([[0,p.bucketId]]);
    case "storage.replica.subscribe": return map([[0,p.bucketId],[1,p.cursor]]);
    case "storage.deletion.status": return map([[0,p.bucketId],[1,p.cid]]);
    case "storage.deletion.subscribe": return map([[0,p.bucketId],[1,p.cid],[2,p.cursor]]);
    case "storage.drive.read": return map([[0,p.bucketId],[1,p.path],[2,p.manifest]]);
    case "storage.drive.commit": return map([[0,p.bucketId],[1,p.manifest],[2,p.bytes],[3,p.expectedVersion],[4,p.mode]]);
    case "storage.drive.share": return map([[0,p.bucketId],[1,p.subject],[2,p.role],[3,p.issuedAt],[4,p.expiresAt]]);
    case "storage.s3.put": return map([[0,p.bucket],[1,p.key],[2,p.cid],[3,p.metadata],[4,p.mediaType],[5,p.ifMatch],[6,p.transferId]]);
    case "storage.s3.get": return map([[0,p.bucket],[1,p.key],[2,p.version]]);
    case "storage.s3.list": return map([[0,p.bucket],[1,p.prefix],[2,p.cursor],[3,p.limit]]);
    case "storage.s3.delete": return map([[0,p.bucket],[1,p.key],[2,p.ifMatch],[3,p.transferId]]);
    case "storage.publish": return map([[0,p.nameHash],[1,p.cid],[2,p.expectedVersion]]);
    case "storage.resolve": return map([[0,p.name],[1,p.version],[2,p.at]]);
    case "storage.keys.export": return map([[0,p.bucketId],[1,p.keyVersion],[2,p.recipientKey]]);
    case "storage.keys.import": return map([[0,p.bucketId],[1,p.wrappedKey],[2,p.replace],[3,p.keyVersion]]);
  }
}

export function encodeStorageV2Intent<Operation extends StorageV2Operation>(intent: StorageV2Intent<Operation>): Uint8Array {
  validateStorageV2Payload(intent.operation,intent.payload);
  return encode(map([
    [0,2], [1,intent.requestId], [2,intent.productId], [3,intent.code], [4,intent.grantId],
    [5,intent.operationId], [6,intent.idempotencyKey], [7,intent.deadlineBlock],
    [8,payloadMap(intent.operation,intent.payload)],
  ]));
}

export function storageV2Hex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2,"0")).join("");
}
