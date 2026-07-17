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
 * Private replacement-first application journey. It intentionally stays outside the package
 * entrypoint until the Host-v2 authority cutover is complete.
 */

import {
  digestContent,
  parseContentCid,
  rawContentAddress,
} from "@cord-network/origin-sdk-cloud-storage";
import {
  contentCommitment,
  type AccountId,
  type ContentCommitment,
  type NameId,
  type NamesEvent,
} from "@cord-network/origin-sdk-names";

export type StorageOperationV2 =
  | "storage.object.put"
  | "storage.drive.commit"
  | "storage.object.status"
  | "storage.publish"
  | "storage.resolve"
  | "storage.object.get";

const STORAGE_CODES: Readonly<Record<StorageOperationV2, number>> = {
  "storage.object.put": 1010,
  "storage.object.get": 1011,
  "storage.object.status": 1014,
  "storage.drive.commit": 1031,
  "storage.publish": 1050,
  "storage.resolve": 1051,
};
const STORAGE_REGISTRY_SHA256 = "d17c24596fbae30c300d57ae8e51bc0c7b149ab2e91c2b9c751bedd3fbc1eeba";
const utf8 = new TextEncoder();

export interface PrivateStorageIntentV2<Operation extends StorageOperationV2 = StorageOperationV2> {
  readonly protocol: "cord.origin.host/2";
  readonly major: 2;
  readonly minor: 0;
  readonly registrySha256: typeof STORAGE_REGISTRY_SHA256;
  readonly operation: Operation;
  readonly code: number;
  readonly requestId: Uint8Array;
  readonly productId: string;
  readonly grantId?: Uint8Array;
  readonly operationId?: Uint8Array;
  readonly deadlineBlock: bigint;
  readonly payload: Readonly<Record<string, unknown>>;
}

export interface PrivateStorageIntentFactoryV2 {
  create<Operation extends StorageOperationV2>(
    operation: Operation,
    input: Readonly<Record<string, unknown>>,
  ): PrivateStorageIntentV2<Operation>;
}

/** Exact upload body bound to one storage.object.put intent. The executor must consume it once. */
export interface PrivateStorageUploadV2 {
  readonly cid: string;
  readonly length: bigint;
  readonly bytes: AsyncIterable<Uint8Array>;
}

export interface PrivateStorageExecutorV2 {
  execute<Operation extends StorageOperationV2>(
    intent: PrivateStorageIntentV2<Operation>,
    upload: Operation extends "storage.object.put" ? PrivateStorageUploadV2 : undefined,
    signal?: AbortSignal,
  ): Promise<unknown>;
}

export interface FinalityProofV2 {
  readonly blockHash: `0x${string}`;
  readonly blockNumber: bigint;
}

export interface VerifiedFinalizedCheckpointV2 {
  readonly finality: "finalized";
  readonly canonical: true;
  readonly verified: true;
  readonly finalized: FinalityProofV2;
}

export interface VerifiedFinalizedBlockV2 extends VerifiedFinalizedCheckpointV2 {
  readonly parentHash: `0x${string}`;
}

export interface NamesAuthorityStateV2 {
  readonly name: NameId;
  readonly owner: AccountId;
  readonly controllers: readonly AccountId[];
  readonly active: boolean;
  /** The only application locator kept by Names. Storage remains the CID authority. */
  readonly content: ContentCommitment | null;
}

export interface FinalizedNamesObservationV2 {
  readonly finality: "finalized";
  readonly canonical: true;
  readonly finalized: FinalityProofV2;
  readonly parentHash: `0x${string}`;
  /** Actual frame_system event index. Gaps for unrelated events are valid. */
  readonly eventIndex: number;
  readonly event: NamesEvent;
  readonly postState: NamesAuthorityStateV2;
}

export interface FinalizedNamesMutationV2 {
  /** Verified consecutive blocks after the index head, including the observation block if new. */
  readonly finalizedBlocks: readonly VerifiedFinalizedBlockV2[];
  readonly observation: FinalizedNamesObservationV2;
}

export interface NamesAuthorityProofV2 extends FinalityProofV2 {
  readonly name: NameId;
  readonly controller: AccountId;
  readonly owner: AccountId;
  readonly revision: number;
}

interface IndexedNamesStateV2 extends NamesAuthorityStateV2 {
  readonly finalized: FinalityProofV2;
  readonly revision: number;
}

const hash = (value: string, label: string): `0x${string}` => {
  if (!/^0x[0-9a-f]{64}$/.test(value)) throw new TypeError(`${label} must be a lowercase 32-byte hash`);
  return value as `0x${string}`;
};

function proof(value: FinalityProofV2, label: string): FinalityProofV2 {
  hash(value.blockHash, `${label} hash`);
  if (typeof value.blockNumber !== "bigint" || value.blockNumber < 0n) {
    throw new TypeError(`${label} number must be an unsigned integer`);
  }
  return { ...value };
}

function sameFinality(left: FinalityProofV2, right: FinalityProofV2): boolean {
  return left.blockHash === right.blockHash && left.blockNumber === right.blockNumber;
}

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function bytesHex(value: Uint8Array): `0x${string}` {
  return `0x${Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

/** Canonical storage name key: Blake2b-256 over the exact normalized UTF-8 name. */
export function deriveStorageNameHashV2(storageName: string): Uint8Array {
  if (storageName.length < 1 || storageName.length > 253 || storageName.normalize("NFC") !== storageName
    || storageName.trim() !== storageName || utf8.encode(storageName).includes(0)) {
    throw new TypeError("storage name must be non-empty normalized UTF-8 without surrounding whitespace or NUL");
  }
  return digestContent("blake2b-256", utf8.encode(storageName));
}

function cidCommitment(cid: string): ContentCommitment {
  return contentCommitment(bytesHex(parseContentCid(cid).digest));
}

function verifyApplicationBindings(input: PublishOriginAppV2Input): void {
  if (!(input.contentBytes instanceof Uint8Array) || !(input.manifestBytes instanceof Uint8Array)) {
    throw new TypeError("application content and manifest must be byte arrays");
  }
  const contentAddress = parseContentCid(input.contentCid);
  if (contentAddress.codec !== "raw"
    || rawContentAddress(input.contentBytes, contentAddress.multihash).cid !== input.contentCid) {
    throw new TypeError("content CID does not match the exact content bytes");
  }
  const manifestAddress = parseContentCid(input.manifestCid);
  if (manifestAddress.codec !== "raw"
    || rawContentAddress(input.manifestBytes, manifestAddress.multihash).cid !== input.manifestCid) {
    throw new TypeError("manifest CID does not match the exact manifest bytes");
  }
  if (cidCommitment(input.manifestCid) !== input.contentCommitment) {
    throw new TypeError("Names content commitment does not match the manifest CID digest");
  }
  if (!(input.nameHash instanceof Uint8Array)
    || !equalBytes(input.nameHash, deriveStorageNameHashV2(input.storageName))) {
    throw new TypeError("storage name hash does not match the canonical storage name");
  }
}

function equalWireValue(left: unknown, right: unknown): boolean {
  if (left instanceof Uint8Array || right instanceof Uint8Array) {
    return left instanceof Uint8Array && right instanceof Uint8Array && equalBytes(left, right);
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) && left.length === right.length
      && left.every((item, index) => equalWireValue(item, right[index]));
  }
  if (typeof left === "object" && left !== null && typeof right === "object" && right !== null) {
    const leftRecord = left as Record<string, unknown>;
    const rightRecord = right as Record<string, unknown>;
    const leftKeys = Object.keys(leftRecord).sort();
    const rightKeys = Object.keys(rightRecord).sort();
    return leftKeys.length === rightKeys.length
      && leftKeys.every((key, index) => key === rightKeys[index]
        && equalWireValue(leftRecord[key], rightRecord[key]));
  }
  return left === right;
}

function eventName(event: NamesEvent): NameId | null {
  return "name" in event.data ? event.data.name : null;
}

function exactAuthorityState(value: NamesAuthorityStateV2): NamesAuthorityStateV2 {
  if (!Array.isArray(value.controllers) || new Set(value.controllers).size !== value.controllers.length) {
    throw new TypeError("Names controllers must be unique");
  }
  if (!value.active && value.content !== null) {
    throw new TypeError("inactive Names state cannot retain application content");
  }
  return { ...value, controllers: [...value.controllers] };
}

function sameAccounts(left: readonly AccountId[], right: readonly AccountId[]): boolean {
  return left.length === right.length && left.every((account, index) => account === right[index]);
}

function sameAuthorityState(left: NamesAuthorityStateV2, right: NamesAuthorityStateV2): boolean {
  return left.name === right.name && left.owner === right.owner
    && sameAccounts(left.controllers, right.controllers) && left.active === right.active
    && left.content === right.content;
}

/** Rebuildable projection over an explicitly advanced, verified finalized chain. */
export class FinalizedNamesEventIndexV2 {
  readonly #states = new Map<NameId, IndexedNamesStateV2>();
  readonly #canonicalHashes = new Map<bigint, `0x${string}`>();
  #head: FinalityProofV2;
  #headParentHash: `0x${string}` | null = null;
  #eventIndex = -1;

  constructor(checkpoint: VerifiedFinalizedCheckpointV2) {
    if (checkpoint.finality !== "finalized" || checkpoint.canonical !== true || checkpoint.verified !== true) {
      throw new TypeError("Names index requires a verified canonical finalized bootstrap checkpoint");
    }
    this.#head = proof(checkpoint.finalized, "Names bootstrap checkpoint");
    this.#canonicalHashes.set(this.#head.blockNumber, this.#head.blockHash);
  }

  get head(): FinalityProofV2 { return { ...this.#head }; }

  /** Advance finality even when a block contains no Names event. */
  advance(block: VerifiedFinalizedBlockV2): void {
    if (block.finality !== "finalized" || block.canonical !== true || block.verified !== true) {
      throw new TypeError("Names index accepts verified canonical finalized blocks only");
    }
    const finalized = proof(block.finalized, "Names finalized block");
    hash(block.parentHash, "Names finalized parent hash");
    if (finalized.blockNumber !== this.#head.blockNumber + 1n || block.parentHash !== this.#head.blockHash) {
      throw new TypeError("Names finalized block would introduce a gap or reorg");
    }
    this.#headParentHash = block.parentHash;
    this.#head = finalized;
    this.#eventIndex = -1;
    this.#canonicalHashes.set(finalized.blockNumber, finalized.blockHash);
  }

  append(observation: FinalizedNamesObservationV2): void {
    if (observation.finality !== "finalized" || observation.canonical !== true) {
      throw new TypeError("Names index accepts canonical finalized events only");
    }
    proof(observation.finalized, "Names finalized event block");
    hash(observation.parentHash, "Names finalized event parent hash");
    if (!sameFinality(observation.finalized, this.#head)
      || observation.parentHash !== this.#headParentHash) {
      throw new TypeError("Names event is not in the explicitly advanced finalized head");
    }
    if (!Number.isSafeInteger(observation.eventIndex) || observation.eventIndex < 0
      || observation.eventIndex <= this.#eventIndex) {
      throw new TypeError("Names event index is not a monotonic system event index");
    }
    const state = exactAuthorityState(observation.postState);
    const affected = eventName(observation.event);
    if (affected === null) throw new TypeError("Names event does not identify an authority state");
    if (affected !== state.name) throw new TypeError("Names event and post-state refer to different names");
    const prior = this.#states.get(state.name);
    switch (observation.event.event) {
      case "name_registered":
        if (prior !== undefined || observation.event.data.owner !== state.owner || !state.active) {
          throw new TypeError("Names registration post-state is inconsistent");
        }
        break;
      case "name_transferred":
        if (prior === undefined || prior.owner !== observation.event.data.from
          || state.owner !== observation.event.data.to || !state.active
          || state.controllers.length !== 0 || prior.content !== state.content) {
          throw new TypeError("Names transfer post-state is inconsistent");
        }
        break;
      case "controller_added":
        if (prior === undefined || prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length + 1
          || !prior.controllers.every((controller) => state.controllers.includes(controller))
          || !state.controllers.includes(observation.event.data.controller)
          || state.owner !== prior.owner || state.active !== prior.active || state.content !== prior.content) {
          throw new TypeError("Names controller addition post-state is inconsistent");
        }
        break;
      case "controller_removed":
        if (prior === undefined || !prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length - 1
          || state.controllers.includes(observation.event.data.controller)
          || !state.controllers.every((controller) => prior.controllers.includes(controller))
          || state.owner !== prior.owner || state.active !== prior.active || state.content !== prior.content) {
          throw new TypeError("Names controller removal post-state is inconsistent");
        }
        break;
      case "content_set":
        if (prior === undefined || observation.event.data.present !== (state.content !== null)
          || state.owner !== prior.owner || !sameAccounts(state.controllers, prior.controllers)
          || state.active !== prior.active) {
          throw new TypeError("Names content event post-state is inconsistent");
        }
        break;
      case "name_released":
      case "expired_name_removed":
      case "emergency_name_revoked":
        if (prior === undefined || state.active || state.owner !== prior.owner) {
          throw new TypeError("Names revocation post-state is inconsistent");
        }
        break;
      default:
        if (prior === undefined || !sameAuthorityState(prior, state)) {
          throw new TypeError("Names event has no reconstructible predecessor");
        }
    }
    const revision = (prior?.revision ?? 0) + 1;
    this.#states.set(state.name, {
      ...state,
      controllers: [...state.controllers],
      finalized: { ...observation.finalized },
      revision,
    });
    this.#eventIndex = observation.eventIndex;
  }

  apply(mutation: FinalizedNamesMutationV2, requiredAncestor?: FinalityProofV2): void {
    const states = new Map(this.#states);
    const canonicalHashes = new Map(this.#canonicalHashes);
    const head = this.#head;
    const headParentHash = this.#headParentHash;
    const eventIndex = this.#eventIndex;
    try {
      for (const block of mutation.finalizedBlocks) this.advance(block);
      this.append(mutation.observation);
      if (requiredAncestor !== undefined) {
        this.assertCanonicalDescendant(mutation.observation.finalized, requiredAncestor);
      }
    } catch (error) {
      this.#states.clear();
      for (const [name, state] of states) this.#states.set(name, state);
      this.#canonicalHashes.clear();
      for (const [number, blockHash] of canonicalHashes) this.#canonicalHashes.set(number, blockHash);
      this.#head = head;
      this.#headParentHash = headParentHash;
      this.#eventIndex = eventIndex;
      throw error;
    }
  }

  assertCanonicalDescendant(descendant: FinalityProofV2, ancestor: FinalityProofV2): void {
    proof(descendant, "descendant finality");
    proof(ancestor, "ancestor finality");
    if (descendant.blockNumber < ancestor.blockNumber
      || this.#canonicalHashes.get(descendant.blockNumber) !== descendant.blockHash
      || this.#canonicalHashes.get(ancestor.blockNumber) !== ancestor.blockHash) {
      throw new TypeError("finality proofs are not in the same verified canonical chain");
    }
  }

  authority(name: NameId, controller: AccountId): NamesAuthorityProofV2 {
    const state = this.#states.get(name);
    if (state === undefined || !state.active) throw new TypeError("Names authority is not active");
    if (state.owner !== controller && !state.controllers.includes(controller)) {
      throw new TypeError("publisher is not the finalized owner or controller");
    }
    return { name, controller, owner: state.owner, revision: state.revision, ...state.finalized };
  }

  assertAuthority(authority: NamesAuthorityProofV2): IndexedNamesStateV2 {
    const current = this.#states.get(authority.name);
    if (current === undefined || !current.active
      || current.owner !== authority.owner || current.revision !== authority.revision
      || current.finalized.blockHash !== authority.blockHash
      || current.finalized.blockNumber !== authority.blockNumber
      || (current.owner !== authority.controller && !current.controllers.includes(authority.controller))) {
      throw new TypeError("Names authority changed after the finalized proof");
    }
    return { ...current, controllers: [...current.controllers], finalized: { ...current.finalized } };
  }

  resolve(name: NameId): IndexedNamesStateV2 {
    const state = this.#states.get(name);
    if (state === undefined || !state.active || state.content === null) {
      throw new TypeError("application is not live in the finalized Names index");
    }
    return { ...state, controllers: [...state.controllers], finalized: { ...state.finalized } };
  }
}

export interface PrivateNamesBindingV2 {
  bind(
    input: {
      readonly proof: NamesAuthorityProofV2;
      readonly content: ContentCommitment;
      /** Adapter guarantee: the bind must finalize at this block or a canonical descendant. */
      readonly after: FinalityProofV2;
    },
    signal?: AbortSignal,
  ): Promise<FinalizedNamesMutationV2>;
  retract(
    proof: NamesAuthorityProofV2,
    signal?: AbortSignal,
  ): Promise<FinalizedNamesMutationV2>;
  resolve(
    name: NameId,
    at: FinalityProofV2,
    signal?: AbortSignal,
  ): Promise<{ readonly state: NamesAuthorityStateV2; readonly finalized: FinalityProofV2 }>;
}

export interface PrivateOriginAppCacheV2 {
  has(cid: string, signal?: AbortSignal): Promise<boolean>;
}

export interface PublishOriginAppV2Input {
  readonly productId: string;
  readonly name: NameId;
  readonly storageName: string;
  readonly nameHash: Uint8Array;
  readonly controller: AccountId;
  readonly contentCommitment: ContentCommitment;
  readonly contentCid: string;
  readonly manifestCid: string;
  readonly contentBytes: Uint8Array;
  readonly manifestBytes: Uint8Array;
  readonly bucketId: Uint8Array;
  readonly writerGrantId: Uint8Array;
  readonly readerGrantId: Uint8Array;
  readonly publishGrantId: Uint8Array;
  readonly deadlineBlock: bigint;
  readonly expectedDriveVersion: bigint;
  readonly expectedPublishVersion?: bigint;
  requestId(): Uint8Array;
  operationId(): Uint8Array;
}

export interface ResolveOriginAppV2Input {
  readonly productId: string;
  readonly name: NameId;
  readonly storageName: string;
  readonly bucketId: Uint8Array;
  readonly readerGrantId: Uint8Array;
  readonly deadlineBlock: bigint;
  requestId(): Uint8Array;
}

function exactIntent<Operation extends StorageOperationV2>(
  factory: PrivateStorageIntentFactoryV2,
  operation: Operation,
  input: Readonly<Record<string, unknown>>,
): PrivateStorageIntentV2<Operation> {
  const intent = factory.create(operation, input);
  if (intent.protocol !== "cord.origin.host/2" || intent.major !== 2
    || intent.minor !== 0 || intent.registrySha256 !== STORAGE_REGISTRY_SHA256
    || intent.operation !== operation || intent.code !== STORAGE_CODES[operation]
    || !equalWireValue(intent.requestId, input.requestId)
    || intent.productId !== input.productId || intent.deadlineBlock !== input.deadlineBlock
    || !equalWireValue(intent.grantId, input.grantId)
    || !equalWireValue(intent.operationId, input.operationId)
    || !equalWireValue(intent.payload, input.payload)) {
    throw new TypeError("storage factory did not return the canonical private Host-v2 intent");
  }
  return intent;
}

function result(
  value: unknown,
  required: readonly string[],
  label: string,
  optional: readonly string[] = [],
): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be a record`);
  }
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record);
  if (required.some((key) => !(key in record))
    || keys.some((key) => !required.includes(key) && !optional.includes(key))) {
    throw new TypeError(`${label} is incomplete or contains unknown fields`);
  }
  return record;
}

function finalized(value: unknown, label: string): FinalityProofV2 {
  const record = result(value, ["number", "hash"], label);
  if (typeof record.number !== "bigint" || record.number < 0n
    || !(record.hash instanceof Uint8Array) || record.hash.length !== 32) {
    throw new TypeError(`${label} is invalid`);
  }
  return { blockNumber: record.number, blockHash: hash(bytesHex(record.hash), label) };
}

interface CheckpointV2 {
  readonly root: Uint8Array;
  readonly from: bigint;
  readonly to: bigint;
  readonly replicas: number;
}

function checkpoint(value: unknown, label: string): CheckpointV2 {
  const record = result(value, ["root", "from", "to", "replicas"], label);
  if (!(record.root instanceof Uint8Array) || record.root.length !== 32
    || typeof record.from !== "bigint" || record.from < 0n
    || typeof record.to !== "bigint" || record.to < record.from
    || !Number.isSafeInteger(record.replicas) || (record.replicas as number) < 1) {
    throw new TypeError(`${label} is invalid`);
  }
  return { root: record.root.slice(), from: record.from, to: record.to, replicas: record.replicas as number };
}

function sameCheckpoint(left: CheckpointV2, right: CheckpointV2): boolean {
  return equalBytes(left.root, right.root) && left.from === right.from && left.to === right.to
    && left.replicas === right.replicas;
}

function providerReceipt(value: unknown, label: string) {
  const record = result(value, ["provider", "cid", "length", "signature"], label);
  if (!(record.provider instanceof Uint8Array) || record.provider.length !== 32
    || typeof record.cid !== "string" || record.cid.length < 1 || record.cid.length > 128
    || record.cid.normalize("NFC") !== record.cid
    || typeof record.length !== "bigint" || record.length < 0n
    || !(record.signature instanceof Uint8Array) || record.signature.length !== 64) {
    throw new TypeError(`${label} is invalid`);
  }
  return record as { provider: Uint8Array; cid: string; length: bigint; signature: Uint8Array };
}

function exactUpload(cid: string, source: Uint8Array): PrivateStorageUploadV2 {
  const copy = source.slice();
  return {
    cid,
    length: BigInt(copy.length),
    bytes: {
      async *[Symbol.asyncIterator]() { yield copy; },
    },
  };
}

/** Native-only v2 app publisher: storage publishability precedes the finalized Names bind. */
export function createPrivateOriginAppsV2(
  factory: PrivateStorageIntentFactoryV2,
  storage: PrivateStorageExecutorV2,
  names: PrivateNamesBindingV2,
  index: FinalizedNamesEventIndexV2,
  cache: PrivateOriginAppCacheV2 = { async has() { return false; } },
) {
  const execute = <Operation extends StorageOperationV2>(
    operation: Operation,
    input: Readonly<Record<string, unknown>>,
    upload: Operation extends "storage.object.put" ? PrivateStorageUploadV2 : undefined,
    signal?: AbortSignal,
  ) => storage.execute(exactIntent(factory, operation, input), upload, signal);

  return {
    async publish(
      input: PublishOriginAppV2Input,
      signal?: AbortSignal,
    ): Promise<{ readonly storageFinalized: FinalityProofV2; readonly namesFinalized: FinalityProofV2 }> {
      verifyApplicationBindings(input);
      const authority = index.authority(input.name, input.controller);
      await cache.has(input.manifestCid, signal);
      const common = { productId: input.productId, deadlineBlock: input.deadlineBlock };
      const putOperationId = input.operationId();
      const put = result(await execute("storage.object.put", {
        ...common,
        requestId: input.requestId(),
        grantId: input.writerGrantId,
        operationId: putOperationId,
        payload: {
          bucketId: input.bucketId,
          cid: input.contentCid,
          length: BigInt(input.contentBytes.length),
          encrypted: 0,
          transferId: putOperationId,
        },
      }, exactUpload(input.contentCid, input.contentBytes), signal),
      ["receipt", "publishable", "finalized"], "storage.object.put result");
      const putReceipt = providerReceipt(put.receipt, "object put receipt");
      if (putReceipt.cid !== input.contentCid || putReceipt.length !== BigInt(input.contentBytes.length)
        || typeof put.publishable !== "boolean") {
        throw new TypeError("object put receipt is not bound to the uploaded content");
      }
      const putFinality = finalized(put.finalized, "object put finality");

      const commitOperationId = input.operationId();
      const commit = result(await execute("storage.drive.commit", {
        ...common,
        requestId: input.requestId(),
        grantId: input.writerGrantId,
        operationId: commitOperationId,
        payload: {
          bucketId: input.bucketId,
          manifest: input.manifestCid,
          bytes: input.manifestBytes,
          expectedVersion: input.expectedDriveVersion,
          mode: 0,
        },
      }, undefined, signal), ["manifest", "version", "checkpoint", "finalized"], "storage.drive.commit result");
      if (commit.manifest !== input.manifestCid || typeof commit.version !== "bigint"
        || commit.version !== input.expectedDriveVersion + 1n) {
        throw new TypeError("drive commit result is not bound to the expected manifest version");
      }
      const committedCheckpoint = checkpoint(commit.checkpoint, "drive commit checkpoint");
      const commitFinality = finalized(commit.finalized, "drive commit finality");
      if (commitFinality.blockNumber < putFinality.blockNumber
        || (commitFinality.blockNumber === putFinality.blockNumber
          && commitFinality.blockHash !== putFinality.blockHash)) {
        throw new TypeError("drive commit finality is not at or after the uploaded content");
      }

      let publishable: { readonly checkpoint: CheckpointV2; readonly finalized: FinalityProofV2 } | undefined;
      for (let attempt = 0; attempt < 8; attempt += 1) {
        const status = result(await execute("storage.object.status", {
          ...common,
          requestId: input.requestId(),
          grantId: input.readerGrantId,
          payload: { bucketId: input.bucketId, cid: input.manifestCid },
        }, undefined, signal), ["state", "replicas", "publishable", "finalized"],
        "storage.object.status result", ["receipt", "checkpoint"]);
        const statusFinality = finalized(status.finalized, "object status finality");
        if (!Number.isSafeInteger(status.state) || (status.state as number) < 0 || (status.state as number) > 4
          || !Number.isSafeInteger(status.replicas) || (status.replicas as number) < 0
          || typeof status.publishable !== "boolean" || statusFinality.blockNumber < commitFinality.blockNumber
          || (statusFinality.blockNumber === commitFinality.blockNumber
            && statusFinality.blockHash !== commitFinality.blockHash)) {
          throw new TypeError("storage object status is invalid or not at or after the drive commit");
        }
        if (status.publishable === true && status.checkpoint !== undefined) {
          const statusCheckpoint = checkpoint(status.checkpoint, "publishability checkpoint");
          if (!sameCheckpoint(statusCheckpoint, committedCheckpoint)) {
            throw new TypeError("publishability checkpoint disagrees with the drive commit");
          }
          publishable = { checkpoint: statusCheckpoint, finalized: statusFinality };
          break;
        }
      }
      if (publishable === undefined) throw new TypeError("manifest did not reach a finalized publishable checkpoint");

      const publishOperationId = input.operationId();
      const published = result(await execute("storage.publish", {
        ...common,
        requestId: input.requestId(),
        grantId: input.publishGrantId,
        operationId: publishOperationId,
        payload: {
          nameHash: input.nameHash,
          cid: input.manifestCid,
          ...(input.expectedPublishVersion === undefined ? {} : { expectedVersion: input.expectedPublishVersion }),
        },
      }, undefined, signal), ["nameHash", "cid", "finalized"], "storage.publish result");
      if (published.cid !== input.manifestCid
        || !(published.nameHash instanceof Uint8Array) || !equalBytes(published.nameHash, input.nameHash)) {
        throw new TypeError("storage publish result is not bound to the application name and manifest");
      }
      const storageFinalized = finalized(published.finalized, "storage publish finality");
      if (storageFinalized.blockNumber < publishable.finalized.blockNumber
        || (storageFinalized.blockNumber === publishable.finalized.blockNumber
          && storageFinalized.blockHash !== publishable.finalized.blockHash)) {
        throw new TypeError("storage publish finality is not at or after publishability");
      }

      index.assertAuthority(authority);
      const bound = await names.bind({
        proof: authority,
        content: input.contentCommitment,
        after: storageFinalized,
      }, signal);
      index.apply(bound, storageFinalized);
      const live = index.resolve(input.name);
      index.assertCanonicalDescendant(live.finalized, storageFinalized);
      index.assertCanonicalDescendant(storageFinalized, publishable.finalized);
      index.assertCanonicalDescendant(publishable.finalized, commitFinality);
      index.assertCanonicalDescendant(commitFinality, putFinality);
      if (live.content !== input.contentCommitment) {
        throw new TypeError("finalized Names bind does not match the published manifest commitment");
      }
      return { storageFinalized, namesFinalized: live.finalized };
    },

    async retract(
      name: NameId,
      controller: AccountId,
      signal?: AbortSignal,
    ): Promise<FinalityProofV2> {
      const authority = index.authority(name, controller);
      index.assertAuthority(authority);
      const mutation = await names.retract(authority, signal);
      index.apply(mutation);
      try {
        index.resolve(name);
      } catch {
        return mutation.observation.finalized;
      }
      throw new TypeError("finalized Names retract left application content live");
    },

    async resolve(
      input: ResolveOriginAppV2Input,
      signal?: AbortSignal,
    ): Promise<{ readonly cid: string; readonly length: bigint; readonly checkpoint: unknown }> {
      deriveStorageNameHashV2(input.storageName);
      const live = index.resolve(input.name);
      const native = await names.resolve(input.name, live.finalized, signal);
      const nativeState = exactAuthorityState(native.state);
      if (!nativeState.active || nativeState.name !== input.name || nativeState.content !== live.content
        || !sameFinality(native.finalized, live.finalized)) {
        throw new TypeError("live finalized Names resolution disagrees with its exact event-index authority");
      }
      const at = Uint8Array.from(live.finalized.blockHash.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
      const common = { productId: input.productId, deadlineBlock: input.deadlineBlock };
      const resolved = result(await execute("storage.resolve", {
        ...common,
        requestId: input.requestId(),
        payload: { name: input.storageName, at },
      }, undefined, signal), ["cid", "version", "checkpoint", "finalized"], "storage.resolve result");
      const resolveFinalized = finalized(resolved.finalized, "storage resolve finality");
      if (!sameFinality(resolveFinalized, live.finalized)
        || typeof resolved.cid !== "string" || cidCommitment(resolved.cid) !== live.content
        || typeof resolved.version !== "bigint" || resolved.version < 0n) {
        throw new TypeError("storage resolution is not bound to the exact finalized Names authority");
      }
      const resolvedCheckpoint = checkpoint(resolved.checkpoint, "storage resolve checkpoint");
      const object = result(await execute("storage.object.get", {
        ...common,
        requestId: input.requestId(),
        grantId: input.readerGrantId,
        payload: { bucketId: input.bucketId, cid: resolved.cid },
      }, undefined, signal), ["cid", "length", "checkpoint"], "storage.object.get result");
      const objectCheckpoint = checkpoint(object.checkpoint, "storage object checkpoint");
      if (object.cid !== resolved.cid || typeof object.length !== "bigint" || object.length < 0n
        || !sameCheckpoint(objectCheckpoint, resolvedCheckpoint)) {
        throw new TypeError("storage object result is not bound to the resolved manifest");
      }
      return { cid: resolved.cid, length: object.length, checkpoint: objectCheckpoint };
    },
  };
}
