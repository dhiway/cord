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
 * Private Host-v2 projection over the descriptor-backed Commons executor. This module owns no
 * runtime state and is intentionally absent from every package export map.
 */

import { parseContentCid } from "../packages/origin-sdk-cloud-storage/src/content.ts";
import type { OriginSigner } from "../packages/origin-sdk-signer/src/index.ts";
import type { CommonsRuntimeExecutor } from "../packages/origin-sdk/src/runtime.ts";
import { decodeHostV2, encodeHostV2Value, type HostV2Map } from "../packages/origin-sdk-host/src/internal/v2/codec.ts";
import {
  HOST_V2_ERROR_BINDINGS, HOST_V2_OPERATION_BINDINGS,
  type HostV2TypeName,
} from "../packages/origin-sdk-host/src/internal/v2/generated.ts";
import type {
  PrivateCommonsOperationV2, PrivateCommonsRuntimeBridgeV2, PrivateFinalizedHostAuthorityV2,
} from "./browser-host-v2.ts";

type WireMap = Record<number, unknown>;
type HashHex = `0x${string}`;

export interface PrivateCommonsFinalityTransportV2 {
  finalized(signal?: AbortSignal): Promise<{
    readonly number: bigint; readonly hash: HashHex; readonly proof: Uint8Array;
  }>;
  verify(hash: HashHex, signal?: AbortSignal): Promise<{
    readonly number: bigint; readonly hash: HashHex; readonly proof: Uint8Array;
  }>;
}

export interface PrivateCommonsFinalizedEventV2 {
  readonly pallet: string;
  readonly event: string;
  readonly fields: Readonly<Record<string, unknown>>;
  readonly eventIndex: number;
}

/** Decoded finalized events supplied by the same descriptor/PAPI transaction transport. */
export interface PrivateCommonsFinalizedEventTransportV2 {
  events(input: {
    readonly blockHash: HashHex; readonly transactionHash: HashHex;
  }, signal?: AbortSignal): Promise<readonly PrivateCommonsFinalizedEventV2[]>;
}

interface Versioned<T> { readonly version: number; readonly value: T | null }
interface ControlBucket {
  readonly bucket_id: unknown; readonly owner: unknown; readonly version: number | bigint;
  readonly primary: unknown; readonly replicas: readonly unknown[];
}
interface CheckpointInfo {
  readonly bucket_id: unknown;
  readonly commitment: { readonly mmr_root: unknown; readonly start_seq: number | bigint; readonly leaf_count: number | bigint };
  readonly checkpoint_block: number | bigint;
  readonly primary_signers: number | bigint;
  readonly commitment_nonce: number | bigint;
  readonly replica_confirmations: readonly unknown[];
}
interface ManifestInfo {
  readonly manifest: unknown; readonly bucket_id: unknown; readonly state: unknown;
  readonly checkpoint: number | bigint | null;
}
interface ContentPublicationInfo { readonly content: unknown; readonly revision: number | bigint }

class CommonsHostFailure extends Error {
  readonly code: number;
  constructor(code: number, message: string) { super(message); this.name = "CommonsHostFailure"; this.code = code; }
}

const utf8 = new TextEncoder();
const BASE32 = "abcdefghijklmnopqrstuvwxyz234567";

function bytes(value: unknown, length: number, label: string): Uint8Array {
  if (value instanceof Uint8Array && value.length === length) return value.slice();
  if (typeof value === "string" && new RegExp(`^0x[0-9a-fA-F]{${length * 2}}$`).test(value)) {
    return Uint8Array.from(value.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
  }
  throw new TypeError(`${label} must contain ${length} bytes`);
}

function hex(value: unknown, length = 32, label = "runtime hash"): HashHex {
  const exact = bytes(value, length, label);
  return `0x${Array.from(exact, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function equal(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function uint(value: unknown, label: string): bigint {
  if ((typeof value !== "number" && typeof value !== "bigint") || BigInt(value) < 0n) {
    throw new TypeError(`${label} must be an unsigned integer`);
  }
  return BigInt(value);
}

function versioned<T>(value: unknown, label: string): Versioned<T> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new TypeError(`${label} is not versioned`);
  const record = value as Record<string, unknown>;
  if (!Number.isSafeInteger(record.version) || (record.version as number) < 1 || !("value" in record)) {
    throw new TypeError(`${label} has an invalid version envelope`);
  }
  return { version: record.version as number, value: (record.value ?? null) as T | null };
}

function enumName(value: unknown): string {
  if (typeof value === "string") return value.toLowerCase();
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const record = value as Record<string, unknown>;
    if (typeof record.type === "string") return record.type.toLowerCase();
    const keys = Object.keys(record);
    if (keys.length === 1) return keys[0]!.toLowerCase();
  }
  throw new TypeError("runtime enum has an unknown descriptor shape");
}

function varint(value: number): number[] {
  const result: number[] = [];
  do { let byte = value & 0x7f; value = Math.floor(value / 128); if (value > 0) byte |= 0x80; result.push(byte); } while (value > 0);
  return result;
}

function base32(value: Uint8Array): string {
  let accumulator = 0; let bits = 0; let output = "";
  for (const byte of value) {
    accumulator = accumulator * 256 + byte; bits += 8;
    while (bits >= 5) { const divisor = 2 ** (bits - 5); output += BASE32[Math.floor(accumulator / divisor) & 31]; accumulator %= divisor; bits -= 5; }
  }
  if (bits > 0) output += BASE32[(accumulator * 2 ** (5 - bits)) & 31];
  return output;
}

/** Commons Names stores a 32-byte commitment, so v2 admits one reversible CID profile. */
function cidForCommitment(commitment: Uint8Array): string {
  return `b${base32(Uint8Array.from([...varint(1), ...varint(0x55), ...varint(0xb220), 32, ...commitment]))}`;
}

function finalityMap(value: PrivateFinalizedHostAuthorityV2): HostV2Map {
  return { 0: value.number, 1: value.hash };
}

function resultEvent(requestId: Uint8Array, sequence: number, result: HostV2Map): Uint8Array {
  return encodeHostV2Value({ 0: 2, 1: requestId, 2: sequence, 3: 2, 4: result });
}

function acceptedEvent(requestId: Uint8Array): Uint8Array {
  return encodeHostV2Value({ 0: 2, 1: requestId, 2: 0, 3: 0, 4: { 0: 0 } });
}

function errorEvent(requestId: Uint8Array, sequence: number, code: number, message: string): Uint8Array {
  const binding = HOST_V2_ERROR_BINDINGS[String(code) as keyof typeof HOST_V2_ERROR_BINDINGS];
  if (!binding) throw new TypeError(`Host-v2 error ${code} is not generated`);
  return encodeHostV2Value({
    0: 2, 1: requestId, 2: sequence, 3: 3,
    4: { 0: code, 1: binding.name, 2: binding.retryable, 3: { 0: message.slice(0, 256) } },
  });
}

function runtimeErrorCode(error: unknown): number | undefined {
  if (error instanceof CommonsHostFailure) return error.code;
  if (!error || typeof error !== "object") return undefined;
  const raw = (error as { code?: unknown }).code;
  if (typeof raw === "number") return raw;
  if (typeof raw !== "string") return undefined;
  const normalized = raw.replace(/([a-z0-9])([A-Z])/g, "$1_$2").replace(/[.\- ]/g, "_").toUpperCase();
  const match = Object.entries(HOST_V2_ERROR_BINDINGS).find(([, binding]) => binding.name === normalized);
  return match ? Number(match[0]) : undefined;
}

function exactFinality(input: { readonly number: bigint; readonly hash: HashHex; readonly proof: Uint8Array }): PrivateFinalizedHostAuthorityV2 {
  if (input.number < 0n || !(input.proof instanceof Uint8Array) || input.proof.length === 0) {
    throw new TypeError("Commons finality transport returned an invalid proof");
  }
  return { number: input.number, hash: bytes(input.hash, 32, "finalized hash"), proof: input.proof.slice() };
}

function normalizedRootLabel(value: unknown): string {
  if (typeof value !== "string" || !/^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(value)) {
    throw new CommonsHostFailure(210, "storage resolve requires one active normalized root name");
  }
  if (utf8.encode(value).length > 63) throw new CommonsHostFailure(210, "storage name exceeds the native Names label bound");
  return value;
}

/**
 * Concrete private bridge. The executor is the existing generated-descriptor boundary; finality
 * and events are verified by the existing PAPI transports rather than retained in this class.
 */
export class PrivateCordCommonsRuntimeBridgeV2 implements PrivateCommonsRuntimeBridgeV2 {
  readonly #runtime: CommonsRuntimeExecutor;
  readonly #signer: OriginSigner;
  readonly #finality: PrivateCommonsFinalityTransportV2;
  readonly #events: PrivateCommonsFinalizedEventTransportV2;

  constructor(input: {
    readonly runtime: CommonsRuntimeExecutor; readonly signer: OriginSigner;
    readonly finality: PrivateCommonsFinalityTransportV2; readonly events: PrivateCommonsFinalizedEventTransportV2;
  }) {
    this.#runtime = input.runtime; this.#signer = input.signer; this.#finality = input.finality; this.#events = input.events;
  }

  async finalizedAuthority(_input: {
    readonly operation: PrivateCommonsOperationV2; readonly code: number;
    readonly productId: string; readonly requestId: Uint8Array;
  }, signal?: AbortSignal): Promise<PrivateFinalizedHostAuthorityV2> {
    return exactFinality(await this.#finality.finalized(signal));
  }

  async *dispatch(input: {
    readonly operation: PrivateCommonsOperationV2; readonly request: Uint8Array;
    readonly authority: PrivateFinalizedHostAuthorityV2; readonly signal?: AbortSignal;
  }): AsyncIterable<{ readonly event: Uint8Array; readonly terminalBlock: bigint }> {
    const binding = HOST_V2_OPERATION_BINDINGS[input.operation];
    const request = decodeHostV2(binding.frame as HostV2TypeName, input.request).value as WireMap;
    const requestId = bytes(request[1], 16, "request ID");
    let sequence = 1;
    let accepted = false;
    try {
      if (Number(request[3]) !== binding.code || request[2] === "") throw new CommonsHostFailure(100, "request binding is invalid");
      if (uint(request[7], "request deadline") <= input.authority.number) throw new CommonsHostFailure(106, "request deadline has expired");
      const payload = request[8] as WireMap;
      let result: HostV2Map; let terminal = input.authority;
      switch (input.operation) {
        case "storage.bucket.get":
          yield { event: acceptedEvent(requestId), terminalBlock: input.authority.number };
          accepted = true;
          ({ result, terminal } = await this.#bucketGet(payload, input.authority, input.signal)); break;
        case "storage.bucket.create": {
          const executed = await this.#bucketCreate(request, payload, input.authority, sequence, input.signal);
          sequence = executed.nextSequence; result = executed.result; terminal = executed.terminal;
          yield { event: acceptedEvent(requestId), terminalBlock: terminal.number };
          accepted = true; break;
        }
        case "storage.checkpoint.status":
          yield { event: acceptedEvent(requestId), terminalBlock: input.authority.number };
          accepted = true;
          ({ result, terminal } = await this.#checkpointStatus(payload, input.authority, input.signal)); break;
        case "storage.publish": {
          const executed = await this.#publish(request, payload, input.authority, sequence, input.signal);
          sequence = executed.nextSequence; result = executed.result; terminal = executed.terminal;
          yield { event: acceptedEvent(requestId), terminalBlock: terminal.number };
          accepted = true; break;
        }
        case "storage.resolve":
          yield { event: acceptedEvent(requestId), terminalBlock: input.authority.number };
          accepted = true;
          ({ result, terminal } = await this.#resolve(payload, input.authority, input.signal)); break;
        default: throw new TypeError(`Commons Host-v2 operation ${input.operation} has no descriptor-backed implementation`);
      }
      yield { event: resultEvent(requestId, sequence, result), terminalBlock: terminal.number };
    } catch (error) {
      const code = runtimeErrorCode(error);
      if (code === undefined || !binding.allowedErrors.includes(code as never)) throw error;
      const message = error instanceof Error ? error.message : "Commons runtime rejected the operation";
      if (!accepted) yield { event: acceptedEvent(requestId), terminalBlock: input.authority.number };
      yield { event: errorEvent(requestId, sequence, code, message), terminalBlock: input.authority.number };
    }
  }

  async #at(payloadAt: unknown, authority: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal): Promise<PrivateFinalizedHostAuthorityV2> {
    if (payloadAt === undefined) return authority;
    const requested = hex(payloadAt, 32, "requested finalized hash");
    const verified = exactFinality(await this.#finality.verify(requested, signal));
    if (hex(verified.hash) !== requested || verified.number > authority.number) throw new CommonsHostFailure(104, "requested block is not verified finalized state");
    return verified;
  }

  async #bucketGet(payload: WireMap, authority: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal) {
    const at = await this.#at(payload[1], authority, signal);
    const bucketId = hex(payload[0], 32, "bucket ID");
    const response = versioned<ControlBucket>(await this.#runtime.read(hex(at.hash), "StorageProviderApi.control_bucket", { bucket_id: bucketId }, signal), "control bucket");
    if (response.value === null) throw new CommonsHostFailure(250, "bucket was not found at finalized state");
    const bucket = response.value;
    if (hex(bucket.bucket_id) !== bucketId || !Array.isArray(bucket.replicas) || bucket.replicas.length < 2 || bucket.replicas.length > 4) {
      throw new TypeError("control bucket response is not bound to the requested bucket");
    }
    return {
      terminal: at,
      result: {
        0: { 0: bytes(bucket.owner, 32, "bucket owner"), 1: uint(bucket.version, "bucket version"), 2: bucket.replicas.length,
          3: bytes(bucket.primary, 32, "bucket primary"), 4: bucket.replicas.map((provider) => bytes(provider, 32, "bucket replica")) },
        1: finalityMap(at),
      } as HostV2Map,
    };
  }

  async #bucketCreate(request: WireMap, payload: WireMap, authority: PrivateFinalizedHostAuthorityV2, sequence: number, signal?: AbortSignal) {
    const replicaCount = Number(uint(payload[0], "replica count"));
    const providers = payload[1];
    if (!Array.isArray(providers) || providers.length !== replicaCount + 1 || replicaCount < 2 || replicaCount > 3) {
      throw new CommonsHostFailure(255, "Commons requires one primary followed by two to three replicas");
    }
    if (Number(payload[2]) !== 0) throw new CommonsHostFailure(100, "runtime bucket encryption policy must be host-managed");
    const prepared = await this.#runtime.prepare(hex(authority.hash), "StorageProvider.create_bucket", {
      policy: hex(request[4], 32, "bucket policy grant"), primary: hex(providers[0], 32, "primary provider"),
      replicas: providers.slice(1).map((provider, index) => hex(provider, 32, `replica ${index}`)),
      operation_id: hex(request[5], 16, "bucket operation ID"),
    }, undefined, signal);
    const receipt = await this.#submit(prepared, signal);
    const terminal = await this.#verifiedReceipt(receipt.blockHash, authority, signal);
    const events = await this.#events.events(receipt, signal);
    const created = events.find((event) => event.pallet === "StorageProvider" && event.event === "BucketCreated");
    if (!created || hex(created.fields.operation_id, 16, "bucket event operation ID") !== hex(request[5], 16, "bucket operation ID")) throw new TypeError("finalized StorageProvider.BucketCreated event is absent or misbound");
    return {
      nextSequence: sequence,
      terminal,
      result: { 0: bytes(created.fields.bucket_id, 32, "created bucket ID"), 1: uint(created.fields.version, "created bucket version"), 2: finalityMap(terminal) } as HostV2Map,
    };
  }

  async #checkpoint(payload: WireMap, at: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal): Promise<CheckpointInfo> {
    const bucketId = hex(payload[0], 32, "checkpoint bucket ID");
    const response = versioned<CheckpointInfo>(await this.#runtime.read(hex(at.hash), "StorageProviderApi.checkpoint", { bucket_id: bucketId }, signal), "checkpoint");
    if (response.value === null) throw new CommonsHostFailure(250, "bucket checkpoint was not found");
    if (hex(response.value.bucket_id) !== bucketId) throw new CommonsHostFailure(222, "checkpoint belongs to another bucket");
    return response.value;
  }

  #checkpointMap(checkpoint: CheckpointInfo): HostV2Map {
    const start = uint(checkpoint.commitment.start_seq, "checkpoint start");
    const count = uint(checkpoint.commitment.leaf_count, "checkpoint leaf count");
    if (count === 0n) throw new CommonsHostFailure(240, "checkpoint has no committed leaves");
    const quorum = Number(uint(checkpoint.primary_signers, "primary confirmations")) + checkpoint.replica_confirmations.length;
    return { 0: bytes(checkpoint.commitment.mmr_root, 32, "checkpoint root"), 1: start, 2: start + count - 1n, 3: quorum };
  }

  async #checkpointStatus(payload: WireMap, authority: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal) {
    const checkpoint = await this.#checkpoint(payload, authority, signal);
    if (payload[1] !== undefined && !equal(bytes(payload[1], 32, "expected checkpoint root"), bytes(checkpoint.commitment.mmr_root, 32, "checkpoint root"))) {
      throw new CommonsHostFailure(241, "checkpoint root does not match finalized canonical state");
    }
    const quorum = Number(uint(checkpoint.primary_signers, "primary confirmations")) + checkpoint.replica_confirmations.length;
    return {
      terminal: authority,
      result: { 0: this.#checkpointMap(checkpoint), 1: Number(uint(checkpoint.commitment_nonce, "checkpoint sequence")),
        2: uint(checkpoint.checkpoint_block, "checkpoint block"), 3: quorum, 4: finalityMap(authority) } as HostV2Map,
    };
  }

  async #publish(request: WireMap, payload: WireMap, authority: PrivateFinalizedHostAuthorityV2, sequence: number, signal?: AbortSignal) {
    const name = hex(payload[0], 32, "Names identifier");
    const cid = payload[1];
    if (typeof cid !== "string") throw new CommonsHostFailure(204, "publication CID is invalid");
    const parsed = parseContentCid(cid);
    if (parsed.version !== 1 || parsed.codec !== "raw" || parsed.multihash !== "blake2b-256") {
      throw new CommonsHostFailure(204, "Names publication requires canonical raw BLAKE2b-256 CIDv1");
    }
    if (cidForCommitment(parsed.digest) !== cid) {
      throw new CommonsHostFailure(204, "Names publication CID must use canonical base32lower encoding");
    }
    const commitment = hex(parsed.digest, 32, "content commitment");
    const manifest = versioned<ManifestInfo>(await this.#runtime.read(hex(authority.hash), "StorageProviderApi.canonical_manifest", { manifest: commitment }, signal), "canonical manifest");
    if (manifest.value === null || enumName(manifest.value.state) !== "publishable") {
      throw new CommonsHostFailure(209, "content is not publishable at finalized Commons state");
    }
    const operationId = hex(request[5], 16, "publication operation ID");
    const prepared = await this.#runtime.prepare(hex(authority.hash), "Names.publish_content", {
      name, content: commitment, expected_revision: payload[2] === undefined ? undefined : uint(payload[2], "expected publication revision"),
      operation_id: operationId,
    }, undefined, signal);
    const receipt = await this.#submit(prepared, signal);
    const terminal = await this.#verifiedReceipt(receipt.blockHash, authority, signal);
    const events = await this.#events.events(receipt, signal);
    const observation = events.find((event) => event.pallet === "Names" && event.event === "ContentSet"
      && hex(event.fields.name, 32, "Names event identifier") === name && event.fields.present === true
      && hex(event.fields.operation_id, 16, "Names event operation ID") === operationId);
    if (!observation) throw new TypeError("finalized Names.ContentSet observation is absent or misbound");
    const resolved = versioned<ContentPublicationInfo>(await this.#runtime.read(receipt.blockHash, "NamesApi.resolve_content_publication", { name }, signal), "Names content publication");
    if (resolved.value === null || hex(resolved.value.content, 32, "resolved Names content") !== commitment
      || uint(resolved.value.revision, "resolved publication revision") !== uint(observation.fields.revision, "Names event revision")) {
      throw new TypeError("finalized Names post-state does not contain the published commitment");
    }
    return { nextSequence: sequence, terminal, result: { 0: bytes(name, 32, "name hash"), 1: cid, 2: finalityMap(terminal) } as HostV2Map };
  }

  async #resolve(payload: WireMap, authority: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal) {
    const at = await this.#at(payload[2], authority, signal);
    const label = normalizedRootLabel(payload[0]);
    const name = versioned<unknown>(await this.#runtime.read(hex(at.hash), "NamesApi.root_name_by_normalized_label", { label }, signal), "root name");
    if (name.value === null) throw new CommonsHostFailure(210, "native Names label was not found");
    const nameId = hex(name.value, 32, "resolved name ID");
    const content = versioned<ContentPublicationInfo>(await this.#runtime.read(hex(at.hash), "NamesApi.resolve_content_publication", { name: nameId }, signal), "Names content publication");
    if (content.value === null) throw new CommonsHostFailure(210, "native Names record has no live content");
    if (payload[1] !== undefined && uint(payload[1], "requested publication version") !== uint(content.value.revision, "publication revision")) {
      throw new CommonsHostFailure(210, "requested publication version is unavailable");
    }
    const commitment = bytes(content.value.content, 32, "Names content commitment");
    const manifest = versioned<ManifestInfo>(await this.#runtime.read(hex(at.hash), "StorageProviderApi.canonical_manifest", { manifest: hex(commitment) }, signal), "canonical manifest");
    if (manifest.value === null || enumName(manifest.value.state) !== "publishable" || manifest.value.checkpoint === null) {
      throw new CommonsHostFailure(209, "resolved content is not checkpoint-publishable");
    }
    const checkpointPayload: WireMap = { 0: bytes(manifest.value.bucket_id, 32, "manifest bucket") };
    const checkpoint = await this.#checkpoint(checkpointPayload, at, signal);
    if (uint(checkpoint.checkpoint_block, "checkpoint block") !== uint(manifest.value.checkpoint, "manifest checkpoint")) {
      throw new TypeError("Names manifest and finalized checkpoint state are inconsistent");
    }
    return {
      terminal: at,
      result: { 0: cidForCommitment(commitment), 1: uint(content.value.revision, "publication revision"), 2: this.#checkpointMap(checkpoint), 3: finalityMap(at) } as HostV2Map,
    };
  }

  async #submit(transaction: Awaited<ReturnType<CommonsRuntimeExecutor["prepare"]>>, signal?: AbortSignal): Promise<{ readonly blockHash: HashHex; readonly transactionHash: HashHex }> {
    for await (const status of transaction.signSubmitAndWatch(this.#signer, signal)) {
      if (status.type === "rejected") {
        const failure = new Error(status.message) as Error & { code?: string };
        failure.code = status.code; throw failure;
      }
      if (status.type === "finalized") return { blockHash: status.blockHash, transactionHash: status.transactionHash };
    }
    throw new TypeError("Commons transaction stream closed before finalized state");
  }

  async #verifiedReceipt(receiptHash: HashHex, authority: PrivateFinalizedHostAuthorityV2, signal?: AbortSignal): Promise<PrivateFinalizedHostAuthorityV2> {
    const verified = exactFinality(await this.#finality.verify(receiptHash, signal));
    if (hex(verified.hash) !== receiptHash || verified.number < authority.number) {
      throw new TypeError("Commons transaction receipt is not the verified finalized descendant returned by finality");
    }
    return verified;
  }
}
