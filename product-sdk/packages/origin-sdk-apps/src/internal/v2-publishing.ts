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
  nameId,
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
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });
const PRIVATE_APP_MANIFEST_SCHEMA = "cord.origin.private-app-manifest" as const;
const PRIVATE_APP_MANIFEST_VERSION = 2 as const;
export const PRIVATE_APP_HOST_V2_CAPABILITIES = Object.freeze([
  "identity.account", "identity.entitlements", "identity.humanity", "identity.profile",
  "identity.subject", "storage.content", "storage.control", "storage.deletion",
  "storage.drive", "storage.encryption", "storage.proof", "storage.publish",
  "storage.replica", "storage.s3", "transaction.sign",
] as const);
export type PrivateAppHostV2Capability = (typeof PRIVATE_APP_HOST_V2_CAPABILITIES)[number];

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

export interface VerifiedFinalizedNamesBlockV2 extends VerifiedFinalizedBlockV2 {
  /** Every Names pallet event in this block, in frame_system event-index order. */
  readonly namesEventsExhaustive: true;
  readonly observations: readonly FinalizedNamesObservationV2[];
}

export interface NamesAuthorityStateV2 {
  readonly name: NameId;
  readonly owner: AccountId;
  readonly controllers: readonly AccountId[];
  readonly active: boolean;
  readonly expiresAt: bigint;
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
  /** Null only for Names events which cannot change the indexed authority state. */
  readonly postState: NamesAuthorityStateV2 | null;
}

export interface FinalizedNamesMutationV2 {
  /** Verified consecutive blocks after the index head, including the observation block if new. */
  readonly finalizedBlocks: readonly VerifiedFinalizedNamesBlockV2[];
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

export interface PrivateOriginAppManifestV2 {
  readonly schema: typeof PRIVATE_APP_MANIFEST_SCHEMA;
  readonly schemaVersion: typeof PRIVATE_APP_MANIFEST_VERSION;
  readonly productId: string;
  readonly nameId: NameId;
  readonly storageName: string;
  readonly storageNameHash: `0x${string}`;
  readonly content: {
    readonly cid: string;
    readonly length: number;
  };
  readonly metadata: {
    readonly version: string;
    readonly channel: string;
    readonly entrypoint: string;
    readonly contentFormat: "static" | "pwa";
    readonly requestedCapabilities: readonly PrivateAppHostV2Capability[];
  };
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be a record`);
  }
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, keys: readonly string[], label: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new TypeError(`${label} contains missing or unknown fields`);
  }
}

function canonicalJson(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("private app manifest numbers must be finite");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  const object = record(value, "private app manifest value");
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(object[key])}`).join(",")}}`;
}

function normalizePrivateManifest(value: unknown): PrivateOriginAppManifestV2 {
  const manifest = record(value, "private app manifest");
  exactKeys(manifest, ["schema", "schemaVersion", "productId", "nameId", "storageName", "storageNameHash", "content", "metadata"], "private app manifest");
  if (manifest.schema !== PRIVATE_APP_MANIFEST_SCHEMA || manifest.schemaVersion !== PRIVATE_APP_MANIFEST_VERSION) {
    throw new TypeError("private app manifest schema must be cord.origin.private-app-manifest v2");
  }
  if (typeof manifest.productId !== "string" || !/^[a-z0-9][a-z0-9._-]{2,63}$/.test(manifest.productId)) {
    throw new TypeError("private app manifest productId is invalid");
  }
  if (typeof manifest.nameId !== "string") throw new TypeError("private app manifest NameId is invalid");
  const checkedName = nameId(manifest.nameId);
  if (typeof manifest.storageName !== "string" || typeof manifest.storageNameHash !== "string") {
    throw new TypeError("private app manifest storage name is invalid");
  }
  const derivedNameHash = bytesHex(deriveStorageNameHashV2(manifest.storageName));
  if (manifest.storageNameHash !== derivedNameHash) {
    throw new TypeError("private app manifest storage name hash is not derived from its name");
  }
  const content = record(manifest.content, "private app manifest content");
  exactKeys(content, ["cid", "length"], "private app manifest content");
  if (typeof content.cid !== "string") throw new TypeError("private app manifest content CID is invalid");
  const parsed = parseContentCid(content.cid);
  if (parsed.codec !== "raw" || !Number.isSafeInteger(content.length) || (content.length as number) < 1) {
    throw new TypeError("private app manifest content declaration is invalid");
  }
  const metadata = record(manifest.metadata, "private app manifest metadata");
  exactKeys(metadata, ["version", "channel", "entrypoint", "contentFormat", "requestedCapabilities"], "private app manifest metadata");
  if (typeof metadata.version !== "string"
    || !/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?$/.test(metadata.version)) {
    throw new TypeError("private app manifest version must use semantic version syntax");
  }
  if (typeof metadata.channel !== "string" || !/^[a-z0-9][a-z0-9-]{0,31}$/.test(metadata.channel)) {
    throw new TypeError("private app manifest channel is invalid");
  }
  if (typeof metadata.entrypoint !== "string" || metadata.entrypoint.length < 1
    || metadata.entrypoint.normalize("NFC") !== metadata.entrypoint
    || utf8.encode(metadata.entrypoint).length > 256 || metadata.entrypoint.startsWith("/")
    || metadata.entrypoint.includes("\\") || /[?#]/.test(metadata.entrypoint)
    || metadata.entrypoint.split("/").some((part) => part === "" || part === "." || part === "..")) {
    throw new TypeError("private app manifest entrypoint must be a safe relative path");
  }
  if (metadata.contentFormat !== "static" && metadata.contentFormat !== "pwa") {
    throw new TypeError("private app manifest content format is unsupported");
  }
  const supportedCapabilities = new Set<string>(PRIVATE_APP_HOST_V2_CAPABILITIES);
  if (!Array.isArray(metadata.requestedCapabilities)
    || metadata.requestedCapabilities.some((capability) => typeof capability !== "string"
      || !supportedCapabilities.has(capability))
    || metadata.requestedCapabilities.some((capability, index, values) => index > 0
      && (values[index - 1] as string) >= capability)) {
    throw new TypeError("private app manifest capabilities must be unique supported Host-v2 features in canonical order");
  }
  return {
    schema: PRIVATE_APP_MANIFEST_SCHEMA,
    schemaVersion: PRIVATE_APP_MANIFEST_VERSION,
    productId: manifest.productId,
    nameId: checkedName,
    storageName: manifest.storageName,
    storageNameHash: derivedNameHash,
    content: { cid: content.cid, length: content.length as number },
    metadata: {
      version: metadata.version,
      channel: metadata.channel,
      entrypoint: metadata.entrypoint,
      contentFormat: metadata.contentFormat,
      requestedCapabilities: [...metadata.requestedCapabilities as PrivateAppHostV2Capability[]],
    },
  };
}

export function encodePrivateOriginAppManifestV2(manifest: PrivateOriginAppManifestV2): Uint8Array {
  const bytes = utf8.encode(canonicalJson(normalizePrivateManifest(manifest)));
  if (bytes.length > 65_536) throw new TypeError("private app manifest exceeds 65536 bytes");
  return bytes;
}

export function decodePrivateOriginAppManifestV2(bytes: Uint8Array): PrivateOriginAppManifestV2 {
  if (!(bytes instanceof Uint8Array) || bytes.length < 1 || bytes.length > 65_536) {
    throw new TypeError("private app manifest bytes must contain 1-65536 bytes");
  }
  let parsed: unknown;
  try { parsed = JSON.parse(utf8Decoder.decode(bytes)); }
  catch { throw new TypeError("private app manifest is not canonical UTF-8 JSON"); }
  const manifest = normalizePrivateManifest(parsed);
  if (!equalBytes(bytes, encodePrivateOriginAppManifestV2(manifest))) {
    throw new TypeError("private app manifest bytes are not in canonical encoding");
  }
  return manifest;
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

type ApplicationBindingInputV2 = Pick<PublishOriginAppV2Input,
  "productId" | "name" | "storageName" | "nameHash" | "contentCommitment" | "contentCid"
  | "manifestCid" | "contentBytes" | "manifestBytes">;

function verifyApplicationBindings(input: ApplicationBindingInputV2): void {
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
  const manifest = decodePrivateOriginAppManifestV2(input.manifestBytes);
  if (manifest.productId !== input.productId || manifest.nameId !== input.name
    || manifest.storageName !== input.storageName
    || manifest.storageNameHash !== bytesHex(input.nameHash)
    || manifest.content.cid !== input.contentCid
    || manifest.content.length !== input.contentBytes.length) {
    throw new TypeError("private app manifest does not bind the exact product, name, or content");
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
  if (typeof value.expiresAt !== "bigint" || value.expiresAt < 0n) {
    throw new TypeError("Names expiry must be an unsigned block number");
  }
  if (!value.active && value.content !== null) {
    throw new TypeError("inactive Names state cannot retain application content");
  }
  return { ...value, controllers: [...value.controllers] };
}

function eventBlockNumber(value: unknown, label: string): bigint {
  if (typeof value === "bigint" && value >= 0n) return value;
  if (typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value)) return BigInt(value);
  throw new TypeError(`${label} must be a canonical unsigned block number`);
}

function sameAccounts(left: readonly AccountId[], right: readonly AccountId[]): boolean {
  return left.length === right.length && left.every((account, index) => account === right[index]);
}

function sameAuthorityState(left: NamesAuthorityStateV2, right: NamesAuthorityStateV2): boolean {
  return left.name === right.name && left.owner === right.owner
    && sameAccounts(left.controllers, right.controllers) && left.active === right.active
    && left.content === right.content && left.expiresAt === right.expiresAt;
}

function sameObservation(left: FinalizedNamesObservationV2, right: FinalizedNamesObservationV2): boolean {
  return left.finality === right.finality && left.canonical === right.canonical
    && sameFinality(left.finalized, right.finalized) && left.parentHash === right.parentHash
    && left.eventIndex === right.eventIndex && equalWireValue(left.event, right.event)
    && equalWireValue(left.postState, right.postState);
}

function changesAuthorityState(event: NamesEvent): boolean {
  return ["name_registered", "name_renewed", "name_transferred", "name_released",
    "expired_name_removed", "controller_added", "controller_removed", "content_set",
    "emergency_name_revoked"].includes(event.event);
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
    if (this.#head.blockNumber !== 0n) {
      throw new TypeError("empty Names bootstrap is valid only at genesis");
    }
    this.#canonicalHashes.set(this.#head.blockNumber, this.#head.blockHash);
  }

  get head(): FinalityProofV2 { return { ...this.#head }; }

  #restore(snapshot: {
    readonly states: Map<NameId, IndexedNamesStateV2>;
    readonly canonicalHashes: Map<bigint, `0x${string}`>;
    readonly head: FinalityProofV2;
    readonly headParentHash: `0x${string}` | null;
    readonly eventIndex: number;
  }): void {
    this.#states.clear();
    for (const [name, state] of snapshot.states) this.#states.set(name, state);
    this.#canonicalHashes.clear();
    for (const [number, blockHash] of snapshot.canonicalHashes) this.#canonicalHashes.set(number, blockHash);
    this.#head = snapshot.head;
    this.#headParentHash = snapshot.headParentHash;
    this.#eventIndex = snapshot.eventIndex;
  }

  #snapshot() {
    return {
      states: new Map(this.#states), canonicalHashes: new Map(this.#canonicalHashes),
      head: this.#head, headParentHash: this.#headParentHash, eventIndex: this.#eventIndex,
    };
  }

  #advance(block: VerifiedFinalizedNamesBlockV2): void {
    if (block.finality !== "finalized" || block.canonical !== true || block.verified !== true) {
      throw new TypeError("Names index accepts verified canonical finalized blocks only");
    }
    if (block.namesEventsExhaustive !== true || !Array.isArray(block.observations)) {
      throw new TypeError("Names finalized block must carry its exhaustive ordered Names events");
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
    for (const observation of block.observations) this.#append(observation);
  }

  /** Advance finality with every Names event in that block, transactionally. */
  advance(block: VerifiedFinalizedNamesBlockV2): void {
    const snapshot = this.#snapshot();
    try { this.#advance(block); }
    catch (error) { this.#restore(snapshot); throw error; }
  }

  #append(observation: FinalizedNamesObservationV2): void {
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
    if (!changesAuthorityState(observation.event)) {
      if (observation.postState !== null) {
        throw new TypeError("non-authority Names event must not inject authority post-state");
      }
      this.#eventIndex = observation.eventIndex;
      return;
    }
    if (observation.postState === null) throw new TypeError("authority Names event is missing post-state");
    const state = exactAuthorityState(observation.postState);
    const affected = eventName(observation.event);
    if (affected === null) throw new TypeError("Names event does not identify an authority state");
    if (affected !== state.name) throw new TypeError("Names event and post-state refer to different names");
    const prior = this.#states.get(state.name);
    switch (observation.event.event) {
      case "name_registered":
        if ((prior !== undefined && prior.active && this.#head.blockNumber < prior.expiresAt)
          || observation.event.data.owner !== state.owner
          || !state.active || state.controllers.length !== 0 || state.content !== null
          || state.expiresAt !== eventBlockNumber(observation.event.data.expires_at, "Names registration expiry")
          || state.expiresAt <= this.#head.blockNumber) {
          throw new TypeError("Names registration post-state is inconsistent");
        }
        break;
      case "name_renewed": {
        const expiresAt = eventBlockNumber(observation.event.data.expires_at, "Names renewal expiry");
        if (prior === undefined || !prior.active || this.#head.blockNumber >= prior.expiresAt
          || expiresAt <= prior.expiresAt
          || state.expiresAt !== expiresAt || !state.active || state.expiresAt <= this.#head.blockNumber
          || state.owner !== prior.owner || !sameAccounts(state.controllers, prior.controllers)
          || state.content !== prior.content) {
          throw new TypeError("Names renewal post-state is inconsistent");
        }
        break;
      }
      case "name_transferred":
        if (prior === undefined || !prior.active || this.#head.blockNumber >= prior.expiresAt
          || prior.owner !== observation.event.data.from
          || state.owner !== observation.event.data.to || !state.active
          || state.controllers.length !== 0 || prior.content !== state.content
          || state.expiresAt !== prior.expiresAt) {
          throw new TypeError("Names transfer post-state is inconsistent");
        }
        break;
      case "controller_added":
        if (prior === undefined || !prior.active || this.#head.blockNumber >= prior.expiresAt
          || prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length + 1
          || !prior.controllers.every((controller) => state.controllers.includes(controller))
          || !state.controllers.includes(observation.event.data.controller)
          || state.owner !== prior.owner || state.active !== prior.active || state.content !== prior.content
          || state.expiresAt !== prior.expiresAt) {
          throw new TypeError("Names controller addition post-state is inconsistent");
        }
        break;
      case "controller_removed":
        if (prior === undefined || !prior.active || this.#head.blockNumber >= prior.expiresAt
          || !prior.controllers.includes(observation.event.data.controller)
          || state.controllers.length !== prior.controllers.length - 1
          || state.controllers.includes(observation.event.data.controller)
          || !state.controllers.every((controller) => prior.controllers.includes(controller))
          || state.owner !== prior.owner || state.active !== prior.active || state.content !== prior.content
          || state.expiresAt !== prior.expiresAt) {
          throw new TypeError("Names controller removal post-state is inconsistent");
        }
        break;
      case "content_set":
        if (prior === undefined || !prior.active || this.#head.blockNumber >= prior.expiresAt
          || observation.event.data.present !== (state.content !== null)
          || state.owner !== prior.owner || !sameAccounts(state.controllers, prior.controllers)
          || state.active !== prior.active || state.expiresAt !== prior.expiresAt) {
          throw new TypeError("Names content event post-state is inconsistent");
        }
        break;
      case "name_released":
      case "expired_name_removed":
      case "emergency_name_revoked":
        if (prior === undefined || state.active || state.owner !== prior.owner
          || state.controllers.length !== 0 || state.content !== null
          || state.expiresAt !== prior.expiresAt
          || (observation.event.event === "name_released" && observation.event.data.owner !== prior.owner)
          || (observation.event.event === "expired_name_removed" && this.#head.blockNumber < prior.expiresAt)) {
          throw new TypeError("Names revocation post-state is inconsistent");
        }
        break;
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
    const snapshot = this.#snapshot();
    try {
      if (mutation.finalizedBlocks.length === 0
        || !mutation.finalizedBlocks.some((block) => block.observations.some((item) => sameObservation(item, mutation.observation)))) {
        throw new TypeError("Names mutation result is absent from its exhaustive finalized events");
      }
      for (const block of mutation.finalizedBlocks) this.#advance(block);
      if (requiredAncestor !== undefined) {
        this.assertCanonicalDescendant(mutation.observation.finalized, requiredAncestor);
      }
    } catch (error) {
      this.#restore(snapshot);
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
    if (state === undefined || !state.active || this.#head.blockNumber >= state.expiresAt) {
      throw new TypeError("Names authority is not active");
    }
    if (state.owner !== controller && !state.controllers.includes(controller)) {
      throw new TypeError("publisher is not the finalized owner or controller");
    }
    return { name, controller, owner: state.owner, revision: state.revision, ...state.finalized };
  }

  assertAuthority(authority: NamesAuthorityProofV2): IndexedNamesStateV2 {
    const current = this.#states.get(authority.name);
    if (current === undefined || !current.active || this.#head.blockNumber >= current.expiresAt
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
    if (state === undefined || !state.active || this.#head.blockNumber >= state.expiresAt || state.content === null) {
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

interface PublishOriginAppV2Snapshot extends Omit<PublishOriginAppV2Input, "requestId" | "operationId"> {
  readonly requestIds: readonly Uint8Array[];
  readonly operationIds: readonly Uint8Array[];
}

function inputBytes(value: unknown, label: string, length?: number): Uint8Array {
  if (!(value instanceof Uint8Array) || (length !== undefined && value.length !== length)) {
    throw new TypeError(`${label} must be ${length === undefined ? "bytes" : `${length} bytes`}`);
  }
  return value.slice();
}

function snapshotPublishInput(input: PublishOriginAppV2Input): PublishOriginAppV2Snapshot {
  const snapshot: PublishOriginAppV2Snapshot = {
    productId: `${input.productId}`,
    name: `${input.name}` as NameId,
    storageName: `${input.storageName}`,
    nameHash: inputBytes(input.nameHash, "storage name hash", 32),
    controller: `${input.controller}` as AccountId,
    contentCommitment: `${input.contentCommitment}` as ContentCommitment,
    contentCid: `${input.contentCid}`,
    manifestCid: `${input.manifestCid}`,
    contentBytes: inputBytes(input.contentBytes, "application content"),
    manifestBytes: inputBytes(input.manifestBytes, "application manifest"),
    bucketId: inputBytes(input.bucketId, "application bucket ID", 32),
    writerGrantId: inputBytes(input.writerGrantId, "writer grant ID", 32),
    readerGrantId: inputBytes(input.readerGrantId, "reader grant ID", 32),
    publishGrantId: inputBytes(input.publishGrantId, "publish grant ID", 32),
    deadlineBlock: input.deadlineBlock,
    expectedDriveVersion: input.expectedDriveVersion,
    ...(input.expectedPublishVersion === undefined ? {} : { expectedPublishVersion: input.expectedPublishVersion }),
    // Allocate the complete bounded publish journey before any asynchronous boundary.
    requestIds: Object.freeze(Array.from({ length: 11 }, () => inputBytes(input.requestId(), "request ID", 16))),
    operationIds: Object.freeze(Array.from({ length: 3 }, () => inputBytes(input.operationId(), "operation ID", 16))),
  };
  if (typeof snapshot.deadlineBlock !== "bigint" || snapshot.deadlineBlock < 0n
    || typeof snapshot.expectedDriveVersion !== "bigint" || snapshot.expectedDriveVersion < 0n
    || (snapshot.expectedPublishVersion !== undefined
      && (typeof snapshot.expectedPublishVersion !== "bigint" || snapshot.expectedPublishVersion < 0n))) {
    throw new TypeError("application publish versions and deadline must be unsigned integers");
  }
  verifyApplicationBindings(snapshot);
  return Object.freeze(snapshot);
}

interface ResolveOriginAppV2Snapshot extends Omit<ResolveOriginAppV2Input, "requestId"> {
  readonly requestIds: readonly [Uint8Array, Uint8Array];
}

function snapshotResolveInput(input: ResolveOriginAppV2Input): ResolveOriginAppV2Snapshot {
  if (typeof input.deadlineBlock !== "bigint" || input.deadlineBlock < 0n) {
    throw new TypeError("application resolve deadline must be an unsigned integer");
  }
  return Object.freeze({
    productId: `${input.productId}`,
    name: `${input.name}` as NameId,
    storageName: `${input.storageName}`,
    bucketId: inputBytes(input.bucketId, "application bucket ID", 32),
    readerGrantId: inputBytes(input.readerGrantId, "reader grant ID", 32),
    deadlineBlock: input.deadlineBlock,
    requestIds: Object.freeze([
      inputBytes(input.requestId(), "request ID", 16),
      inputBytes(input.requestId(), "request ID", 16),
    ]) as unknown as readonly [Uint8Array, Uint8Array],
  });
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

function exactUpload(cid: string, source: Uint8Array): {
  readonly upload: PrivateStorageUploadV2;
  assertCompleted(): void;
} {
  const privateBytes = source.slice();
  let iteratorCreated = false;
  let completed = false;
  const bytes: AsyncIterable<Uint8Array> = {
    [Symbol.asyncIterator]() {
      if (iteratorCreated) throw new TypeError("storage upload byte stream is single-consumption");
      iteratorCreated = true;
      let delivered = false;
      return {
        async next(): Promise<IteratorResult<Uint8Array>> {
          if (!delivered) {
            delivered = true;
            return { done: false, value: privateBytes.slice() };
          }
          completed = true;
          privateBytes.fill(0);
          return { done: true, value: undefined };
        },
      };
    },
  };
  return {
    upload: { cid, length: BigInt(privateBytes.length), bytes },
    assertCompleted() {
      if (!completed) throw new TypeError("storage executor did not completely consume the upload byte stream");
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
      const snapshot = snapshotPublishInput(input);
      const authority = index.authority(snapshot.name, snapshot.controller);
      await cache.has(snapshot.manifestCid, signal);
      const common = { productId: snapshot.productId, deadlineBlock: snapshot.deadlineBlock };
      const putOperationId = snapshot.operationIds[0]!.slice();
      const putUpload = exactUpload(snapshot.contentCid, snapshot.contentBytes);
      const putValue = await execute("storage.object.put", {
        ...common,
        requestId: snapshot.requestIds[0]!.slice(),
        grantId: snapshot.writerGrantId.slice(),
        operationId: putOperationId,
        payload: {
          bucketId: snapshot.bucketId.slice(),
          cid: snapshot.contentCid,
          length: BigInt(snapshot.contentBytes.length),
          encrypted: 0,
          transferId: putOperationId,
        },
      }, putUpload.upload, signal);
      putUpload.assertCompleted();
      const put = result(putValue, ["receipt", "publishable", "finalized"], "storage.object.put result");
      const putReceipt = providerReceipt(put.receipt, "object put receipt");
      if (putReceipt.cid !== snapshot.contentCid || putReceipt.length !== BigInt(snapshot.contentBytes.length)
        || typeof put.publishable !== "boolean") {
        throw new TypeError("object put receipt is not bound to the uploaded content");
      }
      const putFinality = finalized(put.finalized, "object put finality");

      const commitOperationId = snapshot.operationIds[1]!.slice();
      const commit = result(await execute("storage.drive.commit", {
        ...common,
        requestId: snapshot.requestIds[1]!.slice(),
        grantId: snapshot.writerGrantId.slice(),
        operationId: commitOperationId,
        payload: {
          bucketId: snapshot.bucketId.slice(),
          manifest: snapshot.manifestCid,
          bytes: snapshot.manifestBytes.slice(),
          expectedVersion: snapshot.expectedDriveVersion,
          mode: 0,
        },
      }, undefined, signal), ["manifest", "version", "checkpoint", "finalized"], "storage.drive.commit result");
      if (commit.manifest !== snapshot.manifestCid || typeof commit.version !== "bigint"
        || commit.version !== snapshot.expectedDriveVersion + 1n) {
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
          requestId: snapshot.requestIds[2 + attempt]!.slice(),
          grantId: snapshot.readerGrantId.slice(),
          payload: { bucketId: snapshot.bucketId.slice(), cid: snapshot.manifestCid },
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

      const publishOperationId = snapshot.operationIds[2]!.slice();
      const published = result(await execute("storage.publish", {
        ...common,
        requestId: snapshot.requestIds[10]!.slice(),
        grantId: snapshot.publishGrantId.slice(),
        operationId: publishOperationId,
        payload: {
          nameHash: snapshot.nameHash.slice(),
          cid: snapshot.manifestCid,
          ...(snapshot.expectedPublishVersion === undefined ? {} : { expectedVersion: snapshot.expectedPublishVersion }),
        },
      }, undefined, signal), ["nameHash", "cid", "finalized"], "storage.publish result");
      if (published.cid !== snapshot.manifestCid
        || !(published.nameHash instanceof Uint8Array) || !equalBytes(published.nameHash, snapshot.nameHash)) {
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
        content: snapshot.contentCommitment,
        after: storageFinalized,
      }, signal);
      index.assertAuthority(authority);
      index.apply(bound, storageFinalized);
      const live = index.resolve(snapshot.name);
      index.assertCanonicalDescendant(live.finalized, storageFinalized);
      index.assertCanonicalDescendant(storageFinalized, publishable.finalized);
      index.assertCanonicalDescendant(publishable.finalized, commitFinality);
      index.assertCanonicalDescendant(commitFinality, putFinality);
      if (live.content !== snapshot.contentCommitment) {
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
      index.assertAuthority(authority);
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
      const snapshot = snapshotResolveInput(input);
      deriveStorageNameHashV2(snapshot.storageName);
      const live = index.resolve(snapshot.name);
      const native = await names.resolve(snapshot.name, live.finalized, signal);
      const nativeState = exactAuthorityState(native.state);
      if (!sameAuthorityState(nativeState, live)
        || !sameFinality(native.finalized, live.finalized)) {
        throw new TypeError("live finalized Names resolution disagrees with its exact event-index authority");
      }
      const at = Uint8Array.from(live.finalized.blockHash.slice(2).match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
      const common = { productId: snapshot.productId, deadlineBlock: snapshot.deadlineBlock };
      const resolved = result(await execute("storage.resolve", {
        ...common,
        requestId: snapshot.requestIds[0].slice(),
        payload: { name: snapshot.storageName, at },
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
        requestId: snapshot.requestIds[1].slice(),
        grantId: snapshot.readerGrantId.slice(),
        payload: { bucketId: snapshot.bucketId.slice(), cid: resolved.cid },
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
