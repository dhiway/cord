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

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { parseContentCid, rawContentAddress } from "@cord-network/origin-sdk-cloud-storage";
import { contentCommitment, nameId, type AccountId, type BlockNumber, type ContentCommitment, type NormalizedLabel } from "@cord-network/origin-sdk-names";
import { createStorageV2Intent } from "../../origin-sdk-cloud-storage/src/internal/storage-v2-intents.ts";
import {
  FinalizedNamesEventIndexV2, createPrivateOriginAppsV2, decodePrivateOriginAppManifestV2,
  encodePrivateOriginAppManifestV2,
  type FinalityProofV2, type FinalizedNamesMutationV2, type FinalizedNamesObservationV2,
  type NamesAuthorityProofV2, type NamesAuthorityStateV2, type PrivateNamesBindingV2,
  type PrivateStorageExecutorV2, type PrivateStorageIntentFactoryV2, type PrivateStorageIntentV2,
  type PrivateOriginAppManifestV2, type PrivateStorageUploadV2, type PublishOriginAppV2Input,
  type StorageOperationV2, type VerifiedFinalizedBlockV2,
} from "../src/internal/v2-publishing.ts";

const bytes = (length: number, fill: number) => new Uint8Array(length).fill(fill);
const hex = (fill: string) => `0x${fill.repeat(64)}` as `0x${string}`;
const fp = (number: bigint, fill: string): FinalityProofV2 => ({ blockHash: hex(fill), blockNumber: number });
const wf = (number: bigint, fill: number) => ({ number, hash: bytes(32, Number.parseInt(`${fill}${fill}`, 16)) });
const block = (number: bigint, fill: string, parent: string): VerifiedFinalizedBlockV2 => ({ finality: "finalized", canonical: true, verified: true, finalized: fp(number, fill), parentHash: hex(parent) });
const bootstrap = () => ({ finality: "finalized" as const, canonical: true as const, verified: true as const, finalized: fp(0n, "0") });
const owner = "5Owner" as AccountId, controller = "5Controller" as AccountId, nextOwner = "5Next" as AccountId;
const name = nameId(`0x${"22".repeat(32)}`), storageName = "festival.origin";
const contentBytes = bytes(12, 7), contentCid = rawContentAddress(contentBytes).cid;
const nameHash = Uint8Array.from(Buffer.from(name.slice(2), "hex"));
const bytesHex = (value: Uint8Array) => `0x${Buffer.from(value).toString("hex")}` as `0x${string}`;
const appManifest: PrivateOriginAppManifestV2 = {
  schema: "cord.origin.private-app-manifest", schemaVersion: 2, productId: "festival.app", nameId: name,
  storageName, storageNameHash: bytesHex(nameHash), content: { cid: contentCid, length: contentBytes.length },
  metadata: { version: "1.0.0", channel: "stable", entrypoint: "index.html", contentFormat: "pwa", requestedCapabilities: ["identity.profile", "storage.content"] },
};
const manifestBytes = encodePrivateOriginAppManifestV2(appManifest);
const manifestCid = rawContentAddress(manifestBytes).cid;
const commitment = contentCommitment(bytesHex(parseContentCid(manifestCid).digest));
const bucketId = bytes(32, 4);
const checkpoint = { root: bytes(32, 5), from: 1n, to: 2n, replicas: 2 }, storageFinality = wf(2n, 2);

const factory: PrivateStorageIntentFactoryV2 = { create(operation, input) { return createStorageV2Intent(operation, input as never) as unknown as PrivateStorageIntentV2<typeof operation>; } };

function registration(): FinalizedNamesObservationV2 {
  return { finality: "finalized", canonical: true, finalized: fp(1n, "1"), parentHash: hex("0"), eventIndex: 3,
    event: { event: "name_registered", data: { name, parent: null, label: "festival" as NormalizedLabel, owner, expires_at: "100" as BlockNumber } },
    postState: { name, owner, controllers: [controller], active: true, content: null } };
}
function newIndex(): FinalizedNamesEventIndexV2 { const index = new FinalizedNamesEventIndexV2(bootstrap()); index.advance(block(1n, "1", "0")); index.append(registration()); return index; }
function contentObs(at: FinalityProofV2, parentHash: `0x${string}`, who: AccountId, content: ContentCommitment | null, eventIndex = 5): FinalizedNamesObservationV2 {
  return { finality: "finalized", canonical: true, finalized: at, parentHash, eventIndex,
    event: { event: "content_set", data: { name, present: content !== null } }, postState: { name, owner: who, controllers: [controller], active: true, content } };
}
function activatePublication(index: FinalizedNamesEventIndexV2, names: NamesBinding): void {
  index.advance(block(2n, "2", "1")); index.advance(block(3n, "3", "2"));
  const observation = contentObs(fp(3n, "3"), hex("2"), owner, commitment);
  index.append(observation); names.state = observation.postState; names.finalized = observation.finalized;
}
function publishInput(overrides: Partial<PublishOriginAppV2Input> = {}): PublishOriginAppV2Input {
  let request = 20, operation = 40;
  return { productId: "festival.app", name, storageName, nameHash: nameHash.slice(), controller, contentCommitment: commitment,
    contentCid, manifestCid, contentBytes: contentBytes.slice(), manifestBytes: manifestBytes.slice(), bucketId,
    writerGrantId: bytes(32, 9), readerGrantId: bytes(32, 10), publishGrantId: bytes(32, 11), deadlineBlock: 100n,
    expectedDriveVersion: 0n, requestId: () => bytes(16, request++), operationId: () => bytes(16, operation++), ...overrides };
}
function resultFor(operation: StorageOperationV2, publishable = true, resolveFinality = wf(3n, 3)): unknown {
  switch (operation) {
    case "storage.object.put": return { receipt: { provider: bytes(32, 1), cid: contentCid, length: BigInt(contentBytes.length), signature: bytes(64, 2) }, publishable: false, finalized: storageFinality };
    case "storage.drive.commit": return { manifest: manifestCid, version: 1n, checkpoint, finalized: storageFinality };
    case "storage.object.status": return { state: 2, replicas: 2, publishable, ...(publishable ? { checkpoint } : {}), finalized: storageFinality };
    case "storage.publish": return { nameHash: nameHash.slice(), cid: manifestCid, finalized: storageFinality };
    case "storage.resolve": return { cid: manifestCid, version: 1n, checkpoint, finalized: resolveFinality };
    case "storage.object.get": return { cid: manifestCid, length: BigInt(manifestBytes.length), checkpoint };
  }
}
async function collect(upload: PrivateStorageUploadV2): Promise<Uint8Array> {
  const chunks: Uint8Array[] = []; let length = 0;
  for await (const chunk of upload.bytes) { chunks.push(chunk.slice()); length += chunk.length; }
  const body = new Uint8Array(length); let offset = 0;
  for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.length; }
  return body;
}
class StorageExecutor implements PrivateStorageExecutorV2 {
  readonly operations: StorageOperationV2[] = []; readonly uploads: Uint8Array[] = []; readonly uploadStreams: PrivateStorageUploadV2[] = [];
  statusCalls = 0; resolveFinality = wf(3n, 3);
  readonly onOperation?: (operation: StorageOperationV2) => void;
  constructor(onOperation?: (operation: StorageOperationV2) => void) { this.onOperation = onOperation; }
  async execute<Operation extends StorageOperationV2>(intent: PrivateStorageIntentV2<Operation>, upload: Operation extends "storage.object.put" ? PrivateStorageUploadV2 : undefined): Promise<unknown> {
    this.operations.push(intent.operation); this.onOperation?.(intent.operation);
    if (intent.operation === "storage.object.put") { assert.ok(upload); this.uploadStreams.push(upload); const body = await collect(upload); assert.equal(upload.length, BigInt(body.length)); assert.equal(upload.cid, rawContentAddress(body).cid); this.uploads.push(body); }
    else assert.equal(upload, undefined);
    if (intent.operation === "storage.resolve") assert.equal((intent.payload as Record<string, unknown>).name, "festival");
    if (intent.operation === "storage.object.status") return resultFor(intent.operation, ++this.statusCalls > 1);
    return resultFor(intent.operation, true, this.resolveFinality);
  }
}
class NamesBinding implements PrivateNamesBindingV2 {
  retracts = 0;
  state: NamesAuthorityStateV2 = { name, owner, controllers: [controller], active: true, content: null }; finalized = fp(1n, "1");
  async retract(proof: NamesAuthorityProofV2): Promise<FinalizedNamesMutationV2> {
    this.retracts += 1; const observation = contentObs(fp(4n, "4"), hex("3"), proof.owner, null, 8); this.state = observation.postState; this.finalized = observation.finalized;
    return { finalizedBlocks: [block(4n, "4", "3")], observation };
  }
  async resolve(_name: typeof name, _at: FinalityProofV2) { return { state: this.state, finalized: this.finalized }; }
}
const resolveInput = () => ({ productId: "festival.app", name, storageName, bucketId, readerGrantId: bytes(32, 10), deadlineBlock: 100n, requestId: () => bytes(16, 90) });

 test("private v2 journey streams exact bytes through canonical native Names publication", async () => {
  const index = newIndex(), storage = new StorageExecutor(), names = new NamesBinding(); let cacheChecks = 0;
  const apps = createPrivateOriginAppsV2(factory, storage, names, index, { async has() { cacheChecks += 1; return true; } });
  assert.deepEqual(await apps.publish(publishInput()), { storageFinalized: fp(2n, "2"), namesFinalized: fp(3n, "3") });
  assert.equal(cacheChecks, 1); assert.deepEqual(storage.uploads, [contentBytes]);
  await assert.rejects(() => collect(storage.uploadStreams[0]!), /single-consumption/);
  assert.deepEqual(storage.operations, ["storage.object.put", "storage.drive.commit", "storage.object.status", "storage.object.status", "storage.publish", "storage.resolve"]);
  activatePublication(index, names);
  assert.equal(index.resolve(name).content, commitment); assert.equal((await apps.resolve(resolveInput())).cid, manifestCid);
  assert.deepEqual(await apps.retract(name, controller), fp(4n, "4")); assert.throws(() => index.resolve(name), /not live/);
});

test("Names index requires verified bootstrap, empty-block advancement, and actual monotonic event indices", () => {
  assert.throws(() => new FinalizedNamesEventIndexV2({ ...bootstrap(), verified: false } as never), /verified canonical finalized bootstrap/);
  assert.throws(() => new FinalizedNamesEventIndexV2({ ...bootstrap(), finalized: fp(9n, "9") }), /only at genesis/);
  const index = new FinalizedNamesEventIndexV2(bootstrap());
  assert.throws(() => index.advance({ ...block(1n, "1", "0"), verified: false } as never), /verified canonical/);
  index.advance(block(1n, "1", "0")); index.append(registration());
  index.append(contentObs(fp(1n, "1"), hex("0"), owner, commitment, 7));
  assert.throws(() => index.append(contentObs(fp(1n, "1"), hex("0"), owner, commitment, 7)), /monotonic system event index/);
  index.advance(block(2n, "2", "1")); index.advance(block(3n, "3", "2"));
  assert.deepEqual(index.head, fp(3n, "3")); assert.throws(() => index.advance(block(5n, "5", "9")), /gap or reorg/);
});

for (const race of ["transfer", "revoke"] as const) test(`publish rejects finalized Names ${race} race before confirmation`, async () => {
  const index = newIndex(); let raced = false;
  const storage = new StorageExecutor((operation) => { if (operation !== "storage.publish" || raced) return; raced = true; index.advance(block(2n, "2", "1"));
    index.append({ finality: "finalized", canonical: true, finalized: fp(2n, "2"), parentHash: hex("1"), eventIndex: 9,
      event: race === "transfer" ? { event: "name_transferred", data: { name, from: owner, to: nextOwner } } : { event: "emergency_name_revoked", data: { name } },
      postState: race === "transfer" ? { name, owner: nextOwner, controllers: [], active: true, content: null } : { name, owner, controllers: [], active: false, content: null } }); });
  const names = new NamesBinding(); await assert.rejects(() => createPrivateOriginAppsV2(factory, storage, names, index).publish(publishInput()), /authority changed/);
});

test("publication rejects forked or older native resolution finality", async () => {
  for (const hostile of ["fork", "older"] as const) { const names = new NamesBinding(), storage = new StorageExecutor();
    storage.resolveFinality = hostile === "fork" ? wf(2n, 9) : wf(1n, 1);
    const index = newIndex();
    await assert.rejects(() => createPrivateOriginAppsV2(factory, storage, names, index).publish(publishInput()), /not at or after/, hostile);
    assert.deepEqual(index.head, fp(1n, "1"), "a rejected mutation must not poison the finalized index");
    assert.equal(index.authority(name, controller).blockNumber, 1n);
  }
});

test("content, manifest, name, receipt, and stream are cryptographically cross-bound", async () => {
  for (const [changes, message] of [
    [{ contentBytes: bytes(12, 99) }, /content CID does not match/], [{ manifestBytes: bytes(16, 99) }, /manifest CID does not match/],
    [{ contentCommitment: contentCommitment(`0x${"77".repeat(32)}`) }, /content commitment does not match/], [{ nameHash: bytes(32, 99) }, /publication identifier is not the canonical native NameId/],
  ] as const) { const storage = new StorageExecutor(); await assert.rejects(() => createPrivateOriginAppsV2(factory, storage, new NamesBinding(), newIndex()).publish(publishInput(changes)), message); assert.deepEqual(storage.operations, []); }
  for (const receipt of [{ cid: rawContentAddress(bytes(12, 99)).cid, length: 12n }, { cid: contentCid, length: 13n }]) {
    const storage: PrivateStorageExecutorV2 = { async execute(intent, upload) { if (intent.operation === "storage.object.put") { assert.ok(upload); assert.equal(rawContentAddress(await collect(upload)).cid, contentCid); return { ...resultFor(intent.operation) as object, receipt: { provider: bytes(32, 1), ...receipt, signature: bytes(64, 2) } }; } return resultFor(intent.operation); } };
    await assert.rejects(() => createPrivateOriginAppsV2(factory, storage, new NamesBinding(), newIndex()).publish(publishInput()), /receipt is not bound/);
  }
});

for (const hostile of ["names-older", "names-fork", "names-owner", "names-controllers", "storage-older", "storage-fork", "cid"] as const) {
  test(`resolve rejects hostile ${hostile} authority`, async () => {
    const index = newIndex(), storage = new StorageExecutor(), names = new NamesBinding();
    const apps = createPrivateOriginAppsV2(factory, storage, names, index); await apps.publish(publishInput());
    activatePublication(index, names);
    if (hostile === "names-older") names.finalized = fp(2n, "2"); if (hostile === "names-fork") names.finalized = fp(3n, "9");
    if (hostile === "names-owner") names.state = { ...names.state, owner: nextOwner };
    if (hostile === "names-controllers") names.state = { ...names.state, controllers: [] };
    if (hostile === "storage-older") storage.resolveFinality = wf(2n, 2); if (hostile === "storage-fork") storage.resolveFinality = wf(3n, 9);
    if (hostile === "cid") { const original = storage.execute.bind(storage), other = rawContentAddress(bytes(4, 44)).cid; storage.execute = async (intent, upload, signal) => intent.operation === "storage.resolve" ? { cid: other, version: 1n, checkpoint, finalized: wf(3n, 3) } : original(intent, upload as never, signal); }
    await assert.rejects(() => apps.resolve(resolveInput()), /exact event-index authority|exact finalized Names authority/);
  });
}

test("private manifest has one canonical schema and rejects swapped bindings or noncanonical bytes", async () => {
  assert.deepEqual(decodePrivateOriginAppManifestV2(manifestBytes), appManifest);
  assert.deepEqual(encodePrivateOriginAppManifestV2(decodePrivateOriginAppManifestV2(manifestBytes)), manifestBytes);
  const otherContent = rawContentAddress(bytes(4, 44)).cid;
  const swapped: readonly PrivateOriginAppManifestV2[] = [
    { ...appManifest, productId: "another.app" },
    { ...appManifest, nameId: nameId(`0x${"44".repeat(32)}`), storageNameHash: `0x${"44".repeat(32)}` },
    { ...appManifest, storageName: "other.origin" },
    { ...appManifest, content: { cid: otherContent, length: 4 } },
    { ...appManifest, content: { ...appManifest.content, length: appManifest.content.length + 1 } },
  ];
  for (const manifest of swapped) {
    const encoded = encodePrivateOriginAppManifestV2(manifest), cid = rawContentAddress(encoded).cid;
    await assert.rejects(
      () => createPrivateOriginAppsV2(factory, new StorageExecutor(), new NamesBinding(), newIndex()).publish(publishInput({ manifestBytes: encoded, manifestCid: cid, contentCommitment: contentCommitment(bytesHex(parseContentCid(cid).digest)) })),
      /does not bind the exact product, name, or content/,
    );
  }
  const noncanonical = new Uint8Array(manifestBytes.length + 1); noncanonical.set(manifestBytes); noncanonical[manifestBytes.length] = 0x20;
  const noncanonicalCid = rawContentAddress(noncanonical).cid;
  await assert.rejects(
    () => createPrivateOriginAppsV2(factory, new StorageExecutor(), new NamesBinding(), newIndex()).publish(publishInput({ manifestBytes: noncanonical, manifestCid: noncanonicalCid, contentCommitment: contentCommitment(bytesHex(parseContentCid(noncanonicalCid).digest)) })),
    /not in canonical encoding/,
  );
  assert.throws(() => decodePrivateOriginAppManifestV2(bytes(16, 8)), /not canonical UTF-8 JSON/);
});

test("storage executor must completely consume the single-use upload stream", async () => {
  const storage: PrivateStorageExecutorV2 = { async execute(intent, upload) { if (intent.operation === "storage.object.put") { assert.ok(upload); upload.bytes[Symbol.asyncIterator](); } return resultFor(intent.operation); } };
  await assert.rejects(() => createPrivateOriginAppsV2(factory, storage, new NamesBinding(), newIndex()).publish(publishInput()), /did not completely consume/);
});

test("private source has no duplicate Names CID authority or public v2 export", async () => {
  const source = await readFile(new URL("../src/internal/v2-publishing.ts", import.meta.url), "utf8");
  const state = source.slice(source.indexOf("export interface NamesAuthorityStateV2"), source.indexOf("export interface FinalizedNamesObservationV2"));
  assert.doesNotMatch(state, /\bcid\b/); assert.doesNotMatch(source, /getPreimage|putPreimage|TransactionStorage|retentionTransactions|reservationTransactions|\bnames\.bind\(/);
  assert.doesNotMatch(await readFile(new URL("../src/index.ts", import.meta.url), "utf8"), /v2-publishing|createPrivateOriginAppsV2|FinalizedNamesEventIndexV2/);
});
