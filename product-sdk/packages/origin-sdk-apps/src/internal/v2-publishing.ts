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

import type {
  AccountId,
  ContentCommitment,
  NameId,
  NamesEvent,
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

export interface PrivateStorageExecutorV2 {
  execute<Operation extends StorageOperationV2>(
    intent: PrivateStorageIntentV2<Operation>,
    signal?: AbortSignal,
  ): Promise<unknown>;
}

export interface FinalityProofV2 {
  readonly blockHash: `0x${string}`;
  readonly blockNumber: bigint;
}

export interface NamesAuthorityStateV2 {
  readonly name: NameId;
  readonly owner: AccountId;
  readonly controllers: readonly AccountId[];
  readonly active: boolean;
  readonly content: ContentCommitment | null;
  readonly cid: string | null;
}

export interface FinalizedNamesObservationV2 {
  readonly finality: "finalized";
  readonly canonical: true;
  readonly finalized: FinalityProofV2;
  readonly parentHash: `0x${string}` | null;
  readonly eventIndex: number;
  readonly event: NamesEvent;
  readonly postState: NamesAuthorityStateV2;
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

function equalBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
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
  if (!value.active && (value.content !== null || value.cid !== null)) {
    throw new TypeError("inactive Names state cannot retain application content");
  }
  if ((value.content === null) !== (value.cid === null)) {
    throw new TypeError("Names content commitment and storage CID must be present together");
  }
  if (value.cid !== null && (value.cid.length < 1 || value.cid.length > 128 || value.cid.normalize("NFC") !== value.cid)) {
    throw new TypeError("Names storage CID is invalid");
  }
  return { ...value, controllers: [...value.controllers] };
}

function sameAccounts(left: readonly AccountId[], right: readonly AccountId[]): boolean {
  return left.length === right.length && left.every((account, index) => account === right[index]);
}

function sameAuthorityState(left: NamesAuthorityStateV2, right: NamesAuthorityStateV2): boolean {
  return left.name === right.name && left.owner === right.owner
    && sameAccounts(left.controllers, right.controllers) && left.active === right.active
    && left.content === right.content && left.cid === right.cid;
}

/** Rebuildable projection: every mutation is justified by one canonical finalized Names event. */
export class FinalizedNamesEventIndexV2 {
  readonly #states = new Map<NameId, IndexedNamesStateV2>();
  #head: FinalityProofV2 | null = null;
  #headParentHash: `0x${string}` | null = null;
  #eventIndex = -1;

  append(observation: FinalizedNamesObservationV2): void {
    if (observation.finality !== "finalized" || observation.canonical !== true) {
      throw new TypeError("Names index accepts canonical finalized events only");
    }
    hash(observation.finalized.blockHash, "finalized block hash");
    if (typeof observation.finalized.blockNumber !== "bigint" || observation.finalized.blockNumber < 0n) {
      throw new TypeError("Names finalized block number must be an unsigned integer");
    }
    if (observation.parentHash !== null) hash(observation.parentHash, "finalized parent hash");
    if (!Number.isSafeInteger(observation.eventIndex) || observation.eventIndex < 0) {
      throw new TypeError("Names event index must be a non-negative safe integer");
    }
    const sameBlock = this.#head?.blockHash === observation.finalized.blockHash;
    if (this.#head !== null) {
      if (sameBlock) {
        if (observation.parentHash !== this.#headParentHash
          || observation.finalized.blockNumber !== this.#head.blockNumber
          || observation.eventIndex !== this.#eventIndex + 1) {
          throw new TypeError("Names finalized event ordering is not reconstructible");
        }
      } else if (observation.parentHash !== this.#head.blockHash
        || observation.finalized.blockNumber !== this.#head.blockNumber + 1n
        || observation.eventIndex !== 0) {
        throw new TypeError("Names finalized event would introduce a gap or reorg");
      }
    } else if (observation.parentHash !== null || observation.eventIndex !== 0) {
      throw new TypeError("Names index genesis observation is not self-contained");
    }
    const state = exactAuthorityState(observation.postState);
    const affected = eventName(observation.event);
    if (affected === null) {
      throw new TypeError("Names event does not identify an authority state");
    }
    if (affected !== state.name) {
      throw new TypeError("Names event and post-state refer to different names");
    }
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
          || state.controllers.length !== 0
          || prior.content !== state.content || prior.cid !== state.cid) {
          throw new TypeError("Names transfer post-state is inconsistent");
        }
        break;
      case "controller_added":
        if (prior === undefined || prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length + 1
          || !prior.controllers.every((controller) => state.controllers.includes(controller))
          || !state.controllers.includes(observation.event.data.controller)
          || state.owner !== prior.owner || state.active !== prior.active
          || state.content !== prior.content || state.cid !== prior.cid) {
          throw new TypeError("Names controller addition post-state is inconsistent");
        }
        break;
      case "controller_removed":
        if (prior === undefined || !prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length - 1
          || state.controllers.includes(observation.event.data.controller)
          || !state.controllers.every((controller) => prior.controllers.includes(controller))
          || state.owner !== prior.owner || state.active !== prior.active
          || state.content !== prior.content || state.cid !== prior.cid) {
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
    this.#head = { ...observation.finalized };
    this.#headParentHash = observation.parentHash;
    this.#eventIndex = observation.eventIndex;
  }

  authority(name: NameId, controller: AccountId): NamesAuthorityProofV2 {
    const state = this.#states.get(name);
    if (state === undefined || !state.active) throw new TypeError("Names authority is not active");
    if (state.owner !== controller && !state.controllers.includes(controller)) {
      throw new TypeError("publisher is not the finalized owner or controller");
    }
    return {
      name,
      controller,
      owner: state.owner,
      revision: state.revision,
      ...state.finalized,
    };
  }

  assertAuthority(proof: NamesAuthorityProofV2): IndexedNamesStateV2 {
    const current = this.#states.get(proof.name);
    if (current === undefined || !current.active
      || current.owner !== proof.owner || current.revision !== proof.revision
      || current.finalized.blockHash !== proof.blockHash
      || current.finalized.blockNumber !== proof.blockNumber
      || (current.owner !== proof.controller && !current.controllers.includes(proof.controller))) {
      throw new TypeError("Names authority changed after the finalized proof");
    }
    return { ...current, controllers: [...current.controllers], finalized: { ...current.finalized } };
  }

  resolve(name: NameId): IndexedNamesStateV2 {
    const state = this.#states.get(name);
    if (state === undefined || !state.active || state.content === null || state.cid === null) {
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
      readonly cid: string;
    },
    signal?: AbortSignal,
  ): Promise<FinalizedNamesObservationV2>;
  retract(
    proof: NamesAuthorityProofV2,
    signal?: AbortSignal,
  ): Promise<FinalizedNamesObservationV2>;
  resolve(
    name: NameId,
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
  if (typeof record.number !== "bigint" || record.number < 0n || typeof record.hash !== "object"
    || !(record.hash instanceof Uint8Array) || record.hash.length !== 32) {
    throw new TypeError(`${label} is invalid`);
  }
  return {
    blockNumber: record.number,
    blockHash: hash(`0x${Array.from(record.hash, (byte) => byte.toString(16).padStart(2, "0")).join("")}`, label),
  };
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
  return {
    root: record.root.slice(),
    from: record.from,
    to: record.to,
    replicas: record.replicas as number,
  };
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
    signal?: AbortSignal,
  ) => storage.execute(exactIntent(factory, operation, input), signal);

  return {
    async publish(
      input: PublishOriginAppV2Input,
      signal?: AbortSignal,
    ): Promise<{ readonly storageFinalized: FinalityProofV2; readonly namesFinalized: FinalityProofV2 }> {
      const authority = index.authority(input.name, input.controller);
      // A cache hit is only a local optimization hint. It never proves a live finalized checkpoint.
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
      }, signal), ["receipt", "publishable", "finalized"], "storage.object.put result");
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
      }, signal), ["manifest", "version", "checkpoint", "finalized"], "storage.drive.commit result");
      if (commit.manifest !== input.manifestCid || typeof commit.version !== "bigint"
        || commit.version !== input.expectedDriveVersion + 1n) {
        throw new TypeError("drive commit result is not bound to the expected manifest version");
      }
      const committedCheckpoint = checkpoint(commit.checkpoint, "drive commit checkpoint");
      const commitFinality = finalized(commit.finalized, "drive commit finality");
      if (commitFinality.blockNumber < putFinality.blockNumber) {
        throw new TypeError("drive commit finality predates the uploaded content");
      }

      let publishable: { readonly checkpoint: CheckpointV2; readonly finalized: FinalityProofV2 } | undefined;
      for (let attempt = 0; attempt < 8; attempt += 1) {
        const status = result(await execute("storage.object.status", {
          ...common,
          requestId: input.requestId(),
          grantId: input.readerGrantId,
          payload: { bucketId: input.bucketId, cid: input.manifestCid },
        }, signal), ["state", "replicas", "publishable", "finalized",], "storage.object.status result", ["receipt", "checkpoint"]);
        const statusFinality = finalized(status.finalized, "object status finality");
        if (!Number.isSafeInteger(status.state) || (status.state as number) < 0 || (status.state as number) > 4
          || !Number.isSafeInteger(status.replicas) || (status.replicas as number) < 0
          || typeof status.publishable !== "boolean" || statusFinality.blockNumber < commitFinality.blockNumber) {
          throw new TypeError("storage object status is invalid or predates the drive commit");
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
          ...(input.expectedPublishVersion === undefined
            ? {}
            : { expectedVersion: input.expectedPublishVersion }),
        },
      }, signal), ["nameHash", "cid", "finalized"], "storage.publish result");
      if (published.cid !== input.manifestCid
        || !(published.nameHash instanceof Uint8Array)
        || !equalBytes(published.nameHash, input.nameHash)) {
        throw new TypeError("storage publish result is not bound to the application name and manifest");
      }
      const storageFinalized = finalized(published.finalized, "storage publish finality");
      if (storageFinalized.blockNumber < publishable.finalized.blockNumber) {
        throw new TypeError("storage publish finality predates publishability");
      }

      index.assertAuthority(authority);
      const bound = await names.bind({
        proof: authority,
        content: input.contentCommitment,
        cid: input.manifestCid,
      }, signal);
      index.append(bound);
      const live = index.resolve(input.name);
      if (live.content !== input.contentCommitment || live.cid !== input.manifestCid) {
        throw new TypeError("finalized Names bind does not match the published manifest");
      }
      return { storageFinalized, namesFinalized: live.finalized };
    },

    async retract(
      name: NameId,
      controller: AccountId,
      signal?: AbortSignal,
    ): Promise<FinalityProofV2> {
      const proof = index.authority(name, controller);
      index.assertAuthority(proof);
      const observation = await names.retract(proof, signal);
      index.append(observation);
      try {
        index.resolve(name);
      } catch {
        return observation.finalized;
      }
      throw new TypeError("finalized Names retract left application content live");
    },

    async resolve(
      input: ResolveOriginAppV2Input,
      signal?: AbortSignal,
    ): Promise<{ readonly cid: string; readonly length: bigint; readonly checkpoint: unknown }> {
      const live = index.resolve(input.name);
      const liveCid = live.cid;
      if (liveCid === null) throw new TypeError("application has no finalized storage CID");
      const native = await names.resolve(input.name, signal);
      const nativeState = exactAuthorityState(native.state);
      if (!nativeState.active || nativeState.name !== input.name
        || nativeState.content !== live.content || nativeState.cid !== liveCid
        || native.finalized.blockHash !== live.finalized.blockHash
        || native.finalized.blockNumber !== live.finalized.blockNumber) {
        throw new TypeError("live finalized Names resolution disagrees with its event index");
      }
      const at = Uint8Array.from(live.finalized.blockHash.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
      const common = { productId: input.productId, deadlineBlock: input.deadlineBlock };
      const resolved = result(await execute("storage.resolve", {
        ...common,
        requestId: input.requestId(),
        payload: { name: input.storageName, at },
      }, signal), ["cid", "version", "checkpoint", "finalized"], "storage.resolve result");
      finalized(resolved.finalized, "storage resolve finality");
      if (resolved.cid !== liveCid || typeof resolved.version !== "bigint" || resolved.version < 0n) {
        throw new TypeError("Names and storage resolution disagree");
      }
      const resolvedCheckpoint = checkpoint(resolved.checkpoint, "storage resolve checkpoint");
      const object = result(await execute("storage.object.get", {
        ...common,
        requestId: input.requestId(),
        grantId: input.readerGrantId,
        payload: { bucketId: input.bucketId, cid: liveCid },
      }, signal), ["cid", "length", "checkpoint"], "storage.object.get result");
      const objectCheckpoint = checkpoint(object.checkpoint, "storage object checkpoint");
      if (object.cid !== liveCid || typeof object.length !== "bigint" || object.length < 0n
        || !sameCheckpoint(objectCheckpoint, resolvedCheckpoint)) {
        throw new TypeError("storage object result is not bound to the resolved manifest");
      }
      return { cid: liveCid, length: object.length, checkpoint: objectCheckpoint };
    },
  };
}
