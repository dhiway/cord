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
import {
  STORAGE_V2_OPERATIONS,
  storageV2OperationContract,
  type StorageV2Intent,
  type StorageV2Operation,
  type StorageV2PayloadMap,
} from "./storage-v2-intents.ts";

export type StorageV2WireValue = bigint | number | string | boolean | Uint8Array
  | readonly StorageV2WireValue[] | ReadonlyMap<number, StorageV2WireValue>;
type Cbor = StorageV2WireValue;
const utf8 = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

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

export function encodeStorageV2WireValue(value: Cbor): Uint8Array {
  if (typeof value === "bigint") return head(0, value);
  if (typeof value === "number") return head(0, BigInt(value));
  if (typeof value === "boolean") return new Uint8Array([value ? 0xf5 : 0xf4]);
  if (typeof value === "string") { const bytes=utf8.encode(value); return concat([head(3,BigInt(bytes.length)),bytes]); }
  if (value instanceof Uint8Array) return concat([head(2,BigInt(value.length)),value]);
  if (Array.isArray(value)) return concat([head(4,BigInt(value.length)),...value.map(encodeStorageV2WireValue)]);
  if (value instanceof Map) {
    const entries=[...value.entries()].sort(([left],[right])=>left-right);
    return concat([head(5,BigInt(entries.length)),...entries.flatMap(([key,item])=>[encodeStorageV2WireValue(key),encodeStorageV2WireValue(item)])]);
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
  return encodeStorageV2WireValue(map([
    [0,2], [1,intent.requestId], [2,intent.productId], [3,intent.code], [4,intent.grantId],
    [5,intent.operationId], [6,intent.idempotencyKey], [7,intent.deadlineBlock],
    [8,payloadMap(intent.operation,intent.payload)],
  ]));
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

class WireDecoder {
  #offset = 0;
  readonly #source: Uint8Array;

  constructor(source: Uint8Array) { this.#source = source; }

  decode(): Cbor {
    const value = this.item();
    if (this.#offset !== this.#source.length) throw new TypeError("trailing CBOR bytes");
    return value;
  }

  byte(): number {
    const value = this.#source[this.#offset];
    if (value === undefined) throw new TypeError("truncated CBOR value");
    this.#offset += 1;
    return value;
  }

  argument(additional: number): bigint {
    if (additional < 24) return BigInt(additional);
    if (additional === 31) throw new TypeError("indefinite CBOR is noncanonical");
    const width = additional === 24 ? 1 : additional === 25 ? 2 : additional === 26 ? 4 : additional === 27 ? 8 : 0;
    if (width === 0) throw new TypeError("reserved CBOR argument");
    let value = 0n;
    for (let index = 0; index < width; index += 1) value = (value << 8n) | BigInt(this.byte());
    return value;
  }

  length(value: bigint): number {
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) throw new TypeError("CBOR length exceeds safe range");
    return Number(value);
  }

  bytes(length: number): Uint8Array {
    if (this.#offset + length > this.#source.length) throw new TypeError("truncated CBOR bytes");
    const output = this.#source.slice(this.#offset, this.#offset + length);
    this.#offset += length;
    return output;
  }

  item(): Cbor {
    const initial = this.byte();
    const major = initial >> 5;
    const additional = initial & 31;
    if (major === 7) {
      if (additional === 20) return false;
      if (additional === 21) return true;
      throw new TypeError("unsupported CBOR simple value");
    }
    if (major === 6) throw new TypeError("CBOR tags are noncanonical");
    if (major === 1) throw new TypeError("negative CBOR integers are forbidden");
    const argument = this.argument(additional);
    if (major === 0) return argument <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(argument) : argument;
    if (major === 2) return this.bytes(this.length(argument));
    if (major === 3) {
      try {
        const value = decoder.decode(this.bytes(this.length(argument)));
        if (value.normalize("NFC") !== value) throw new TypeError("CBOR text is not NFC");
        return value;
      } catch (error) {
        if (error instanceof TypeError) throw error;
        throw new TypeError("invalid CBOR UTF-8");
      }
    }
    if (major === 4) return Array.from({ length: this.length(argument) }, () => this.item());
    if (major === 5) {
      const output = new Map<number, Cbor>();
      for (let index = 0; index < this.length(argument); index += 1) {
        const key = this.item();
        if (typeof key !== "number" || !Number.isSafeInteger(key) || key < 0) throw new TypeError("CBOR map key must be a safe unsigned integer");
        if (output.has(key)) throw new TypeError("duplicate CBOR map key");
        output.set(key, this.item());
      }
      return output;
    }
    throw new TypeError("unsupported CBOR major type");
  }
}

const PAYLOAD_KEYS: Record<StorageV2Operation, readonly [readonly number[], readonly number[]]> = {
  "storage.bucket.create": [[0,1,2],[]], "storage.bucket.get": [[0],[1]],
  "storage.bucket.grant": [[0,1,2,3,4],[]], "storage.bucket.revoke": [[0,1,2],[]],
  "storage.object.put": [[0,1,2,3,4],[]], "storage.object.get": [[0,1],[]],
  "storage.object.range": [[0,1,2,3],[]], "storage.object.delete": [[0,1,2],[]],
  "storage.object.status": [[0,1],[]], "storage.checkpoint.status": [[0],[1]],
  "storage.checkpoint.subscribe": [[0,1],[]], "storage.replica.status": [[0],[]],
  "storage.replica.subscribe": [[0,1],[]], "storage.deletion.status": [[0,1],[]],
  "storage.deletion.subscribe": [[0,1,2],[]], "storage.drive.read": [[0,1],[2]],
  "storage.drive.commit": [[0,1,2,3,4],[]], "storage.drive.share": [[0,1,2,3,4],[]],
  "storage.s3.put": [[0,1,2,3,4,6],[5]], "storage.s3.get": [[0,1],[2]],
  "storage.s3.list": [[0,3],[1,2]], "storage.s3.delete": [[0,1,3],[2]],
  "storage.publish": [[0,1],[2]], "storage.resolve": [[0],[1,2]],
  "storage.keys.export": [[0,1,2],[]], "storage.keys.import": [[0,1,2,3],[]],
};

function exactMap(value: Cbor, required: readonly number[], optional: readonly number[], label: string): ReadonlyMap<number, Cbor> {
  if (!(value instanceof Map)) throw new TypeError(`${label} must be a CBOR map`);
  const allowed = new Set([...required, ...optional]);
  if (required.some((key) => !value.has(key)) || [...value.keys()].some((key) => !allowed.has(key))) {
    throw new TypeError(`${label} contains missing or unknown fields`);
  }
  return value;
}

function unsigned(value: Cbor | undefined, label: string): bigint {
  if (typeof value !== "number" && typeof value !== "bigint") throw new TypeError(`${label} must be unsigned`);
  const output = BigInt(value);
  if (output < 0n || output > 0xffff_ffff_ffff_ffffn) throw new TypeError(`${label} exceeds u64`);
  return output;
}

/** Decode and close the frozen storage frame without applying higher-level grant-window policy. */
export function decodeCanonicalStorageV2Frame(bytes: Uint8Array): ReadonlyMap<number, Cbor> {
  if (!(bytes instanceof Uint8Array)) throw new TypeError("storage frame must be bytes");
  const value = new WireDecoder(bytes).decode();
  if (!equalBytes(encodeStorageV2WireValue(value), bytes)) throw new TypeError("storage frame is noncanonical");
  if (!(value instanceof Map)) throw new TypeError("storage frame must be a CBOR map");
  const code = Number(unsigned(value.get(3), "operation code"));
  const operation = (Object.entries(STORAGE_V2_OPERATIONS) as Array<[StorageV2Operation, readonly [number, ...unknown[]]]>)
    .find(([, contract]) => contract[0] === code)?.[0];
  if (operation === undefined) throw new TypeError("unknown storage operation code");
  const contract = storageV2OperationContract(operation);
  const required = [0,1,2,3,7,8, ...(contract.grantScope === "public" ? [] : [4]), ...(contract.operationIdRequired ? [5] : [])];
  exactMap(value, required, [6], "storage frame");
  if (value.get(0) !== 2) throw new TypeError("storage frame version mismatch");
  if (!(value.get(1) instanceof Uint8Array) || value.get(1).length !== 16) throw new TypeError("requestId must contain 16 bytes");
  if (typeof value.get(2) !== "string" || utf8.encode(value.get(2)).length < 1 || utf8.encode(value.get(2)).length > 128) throw new TypeError("productId is invalid");
  if (value.has(4) && (!(value.get(4) instanceof Uint8Array) || value.get(4).length !== 32)) throw new TypeError("grantId must contain 32 bytes");
  if (value.has(5) && (!(value.get(5) instanceof Uint8Array) || value.get(5).length !== 16)) throw new TypeError("operationId must contain 16 bytes");
  if (value.has(6) && (!(value.get(6) instanceof Uint8Array) || value.get(6).length < 1 || value.get(6).length > 64)) throw new TypeError("idempotency key is invalid");
  unsigned(value.get(7), "deadline block");
  const [payloadRequired,payloadOptional] = PAYLOAD_KEYS[operation];
  exactMap(value.get(8)!, payloadRequired, payloadOptional, `${operation} payload`);
  return value;
}

export function storageV2Hex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2,"0")).join("");
}
