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
import {
  contentCommitment,
  nameId,
  type AccountId,
  type BlockNumber,
  type ContentCommitment,
  type NamesEvent,
  type NormalizedLabel,
} from "@cord-network/origin-sdk-names";
import { createStorageV2Intent } from "../../origin-sdk-cloud-storage/src/internal/storage-v2-intents.ts";
import {
  FinalizedNamesEventIndexV2,
  createPrivateOriginAppsV2,
  type FinalityProofV2,
  type FinalizedNamesObservationV2,
  type NamesAuthorityProofV2,
  type NamesAuthorityStateV2,
  type PrivateNamesBindingV2,
  type PrivateStorageExecutorV2,
  type PrivateStorageIntentFactoryV2,
  type PrivateStorageIntentV2,
  type PublishOriginAppV2Input,
  type StorageOperationV2,
} from "../src/internal/v2-publishing.ts";

const bytes = (length: number, fill: number) => new Uint8Array(length).fill(fill);
const hex = (fill: string) => `0x${fill.repeat(64)}` as `0x${string}`;
const owner = "5Owner" as AccountId;
const controller = "5Controller" as AccountId;
const nextOwner = "5Next" as AccountId;
const name = nameId(`0x${"22".repeat(32)}`);
const commitment = contentCommitment(`0x${"33".repeat(32)}`);
const bucketId = bytes(32, 4);
const checkpoint = { root: bytes(32, 5), from: 1n, to: 2n, replicas: 2 };
const storageFinality = { number: 10n, hash: bytes(32, 10) };

const factory: PrivateStorageIntentFactoryV2 = {
  create(operation, input) {
    return createStorageV2Intent(operation, input as never) as unknown as PrivateStorageIntentV2<typeof operation>;
  },
};

function registered(
  index: FinalizedNamesEventIndexV2,
  state: NamesAuthorityStateV2 = {
    name,
    owner,
    controllers: [controller],
    active: true,
    content: null,
    cid: null,
  },
): void {
  index.append({
    finality: "finalized",
    canonical: true,
    finalized: { blockHash: hex("1"), blockNumber: 1n },
    parentHash: null,
    eventIndex: 0,
    event: {
      event: "name_registered",
      data: {
        name,
        parent: null,
        label: "festival" as NormalizedLabel,
        owner,
        expires_at: "100" as BlockNumber,
      },
    },
    postState: state,
  });
}

function contentObservation(
  proof: NamesAuthorityProofV2,
  content = commitment,
  cid = "bafk-manifest",
): FinalizedNamesObservationV2 {
  return {
    finality: "finalized",
    canonical: true,
    finalized: { blockHash: hex("2"), blockNumber: 2n },
    parentHash: proof.blockHash,
    eventIndex: 0,
    event: { event: "content_set", data: { name, present: content !== null } },
    postState: {
      name,
      owner: proof.owner,
      controllers: [controller],
      active: true,
      content,
      cid,
    },
  };
}

function publishInput(): PublishOriginAppV2Input {
  let request = 20;
  let operation = 40;
  return {
    productId: "festival.app",
    name,
    storageName: "festival.origin",
    nameHash: bytes(32, 6),
    controller,
    contentCommitment: commitment,
    contentCid: "bafk-content",
    manifestCid: "bafk-manifest",
    contentBytes: bytes(12, 7),
    manifestBytes: bytes(16, 8),
    bucketId,
    writerGrantId: bytes(32, 9),
    readerGrantId: bytes(32, 10),
    publishGrantId: bytes(32, 11),
    deadlineBlock: 100n,
    expectedDriveVersion: 0n,
    requestId: () => bytes(16, request++),
    operationId: () => bytes(16, operation++),
  };
}

function resultFor(operation: StorageOperationV2, publishable = true): unknown {
  switch (operation) {
    case "storage.object.put":
      return {
        receipt: { provider: bytes(32, 1), cid: "bafk-content", length: 12n, signature: bytes(64, 2) },
        publishable: false,
        finalized: storageFinality,
      };
    case "storage.drive.commit":
      return { manifest: "bafk-manifest", version: 1n, checkpoint, finalized: storageFinality };
    case "storage.object.status":
      return {
        state: 2,
        replicas: 2,
        publishable,
        ...(publishable ? { checkpoint } : {}),
        finalized: storageFinality,
      };
    case "storage.publish":
      return { nameHash: bytes(32, 6), cid: "bafk-manifest", finalized: storageFinality };
    case "storage.resolve":
      return { cid: "bafk-manifest", version: 1n, checkpoint, finalized: storageFinality };
    case "storage.object.get":
      return { cid: "bafk-manifest", length: 16n, checkpoint };
  }
}

class StorageExecutor implements PrivateStorageExecutorV2 {
  readonly operations: StorageOperationV2[] = [];
  statusCalls = 0;
  readonly onOperation?: (operation: StorageOperationV2) => void;

  constructor(onOperation?: (operation: StorageOperationV2) => void) {
    this.onOperation = onOperation;
  }

  async execute<Operation extends StorageOperationV2>(
    intent: PrivateStorageIntentV2<Operation>,
  ): Promise<unknown> {
    this.operations.push(intent.operation);
    this.onOperation?.(intent.operation);
    if (intent.operation === "storage.object.status") {
      this.statusCalls += 1;
      return resultFor(intent.operation, this.statusCalls > 1);
    }
    return resultFor(intent.operation);
  }
}

class NamesBinding implements PrivateNamesBindingV2 {
  binds = 0;
  retracts = 0;
  state: NamesAuthorityStateV2 = {
    name,
    owner,
    controllers: [controller],
    active: true,
    content: null,
    cid: null,
  };
  finalized: FinalityProofV2 = { blockHash: hex("1"), blockNumber: 1n };

  async bind(input: { proof: NamesAuthorityProofV2; content: ContentCommitment; cid: string }) {
    this.binds += 1;
    const observation = contentObservation(input.proof, input.content, input.cid);
    this.state = observation.postState;
    this.finalized = observation.finalized;
    return observation;
  }

  async retract(proof: NamesAuthorityProofV2) {
    this.retracts += 1;
    const observation: FinalizedNamesObservationV2 = {
      finality: "finalized",
      canonical: true,
      finalized: { blockHash: hex("3"), blockNumber: 3n },
      parentHash: proof.blockHash,
      eventIndex: 0,
      event: { event: "content_set", data: { name, present: false } },
      postState: { ...this.state, content: null, cid: null },
    };
    this.state = observation.postState;
    this.finalized = observation.finalized;
    return observation;
  }

  async resolve() { return { state: this.state, finalized: this.finalized }; }
}

test("private v2 app journey uses canonical storage intents through publishability, Names bind, and resolution", async () => {
  const index = new FinalizedNamesEventIndexV2();
  registered(index);
  const storage = new StorageExecutor();
  const names = new NamesBinding();
  let cacheChecks = 0;
  const apps = createPrivateOriginAppsV2(factory, storage, names, index, {
    async has() { cacheChecks += 1; return true; },
  });

  const published = await apps.publish(publishInput());
  assert.equal(published.storageFinalized.blockNumber, 10n);
  assert.equal(published.namesFinalized.blockNumber, 2n);
  assert.equal(cacheChecks, 1);
  assert.deepEqual(storage.operations, [
    "storage.object.put",
    "storage.drive.commit",
    "storage.object.status",
    "storage.object.status",
    "storage.publish",
  ], "a cache hit must not skip durable prepare, checkpoint, or publish");
  assert.equal(names.binds, 1);

  const resolved = await apps.resolve({
    productId: "festival.app",
    name,
    storageName: "festival.origin",
    bucketId,
    readerGrantId: bytes(32, 10),
    deadlineBlock: 100n,
    requestId: () => bytes(16, 90),
  });
  assert.equal(resolved.cid, "bafk-manifest");
  assert.deepEqual(storage.operations.slice(-2), ["storage.resolve", "storage.object.get"]);

  const retracted = await apps.retract(name, controller);
  assert.equal(retracted.blockNumber, 3n);
  assert.throws(() => index.resolve(name), /not live/);
});

test("finalized Names index rejects unfinalized, reordered, and reorg observations", () => {
  const index = new FinalizedNamesEventIndexV2();
  const valid: FinalizedNamesObservationV2 = {
    finality: "finalized",
    canonical: true,
    finalized: { blockHash: hex("1"), blockNumber: 1n },
    parentHash: null,
    eventIndex: 0,
    event: {
      event: "name_registered",
      data: { name, parent: null, label: "festival" as NormalizedLabel, owner, expires_at: "100" as BlockNumber },
    },
    postState: { name, owner, controllers: [controller], active: true, content: null, cid: null },
  };
  assert.throws(() => index.append({ ...valid, finality: "best" } as never), /finalized events only/);
  assert.throws(() => index.append({ ...valid, canonical: false } as never), /finalized events only/);
  assert.throws(() => index.append({
    ...valid,
    finalized: { ...valid.finalized, blockNumber: -1n },
  }), /unsigned integer/);
  index.append(valid);
  const transfer: NamesEvent = { event: "name_transferred", data: { name, from: owner, to: nextOwner } };
  assert.throws(() => index.append({
    ...valid,
    finalized: { blockHash: hex("3"), blockNumber: 3n },
    parentHash: hex("9"),
    event: transfer,
    postState: { ...valid.postState, owner: nextOwner },
  }), /gap or reorg/);
  assert.throws(() => index.append({
    ...valid,
    parentHash: null,
    eventIndex: 2,
    event: transfer,
    postState: { ...valid.postState, owner: nextOwner },
  }), /ordering/);
  assert.throws(() => index.append({
    ...valid,
    finalized: { blockHash: hex("2"), blockNumber: 2n },
    parentHash: hex("1"),
    event: { event: "content_set", data: { name, present: true } },
    postState: {
      ...valid.postState,
      owner: nextOwner,
      content: commitment,
      cid: "bafk-manifest",
    },
  }), /post-state is inconsistent/);
});

for (const race of ["transfer", "revoke"] as const) {
  test(`publish rejects finalized Names ${race} races before binding`, async () => {
    const index = new FinalizedNamesEventIndexV2();
    registered(index);
    let raced = false;
    const storage = new StorageExecutor((operation) => {
      if (operation !== "storage.publish" || raced) return;
      raced = true;
      index.append({
        finality: "finalized",
        canonical: true,
        finalized: { blockHash: hex("2"), blockNumber: 2n },
        parentHash: hex("1"),
        eventIndex: 0,
        event: race === "transfer"
          ? { event: "name_transferred", data: { name, from: owner, to: nextOwner } }
          : { event: "emergency_name_revoked", data: { name } },
        postState: race === "transfer"
          ? { name, owner: nextOwner, controllers: [], active: true, content: null, cid: null }
          : { name, owner, controllers: [], active: false, content: null, cid: null },
      });
    });
    const names = new NamesBinding();
    const apps = createPrivateOriginAppsV2(factory, storage, names, index);
    await assert.rejects(() => apps.publish(publishInput()), /authority changed/);
    assert.equal(names.binds, 0);
  });
}

test("non-canonical intents and cross-bound storage receipts fail before Names binding", async () => {
  const driftIndex = new FinalizedNamesEventIndexV2();
  registered(driftIndex);
  const driftStorage = new StorageExecutor();
  const driftNames = new NamesBinding();
  const driftFactory: PrivateStorageIntentFactoryV2 = {
    create(operation, input) {
      return {
        ...factory.create(operation, input),
        registrySha256: "0".repeat(64),
      } as PrivateStorageIntentV2<typeof operation>;
    },
  };
  await assert.rejects(
    () => createPrivateOriginAppsV2(driftFactory, driftStorage, driftNames, driftIndex).publish(publishInput()),
    /canonical private Host-v2 intent/,
  );
  assert.deepEqual(driftStorage.operations, []);
  assert.equal(driftNames.binds, 0);

  const receiptIndex = new FinalizedNamesEventIndexV2();
  registered(receiptIndex);
  const receiptNames = new NamesBinding();
  const receiptStorage: PrivateStorageExecutorV2 = {
    async execute(intent) {
      if (intent.operation === "storage.object.put") {
        return {
          ...resultFor(intent.operation),
          receipt: { provider: bytes(32, 1), cid: "bafk-other", length: 12n, signature: bytes(64, 2) },
        };
      }
      return resultFor(intent.operation);
    },
  };
  await assert.rejects(
    () => createPrivateOriginAppsV2(factory, receiptStorage, receiptNames, receiptIndex).publish(publishInput()),
    /receipt is not bound/,
  );
  assert.equal(receiptNames.binds, 0);
});

test("hostile cache and Names/storage disagreement cannot bypass live publication", async () => {
  const index = new FinalizedNamesEventIndexV2();
  registered(index);
  const storage = new StorageExecutor();
  const names = new NamesBinding();
  const apps = createPrivateOriginAppsV2(factory, storage, names, index, { async has() { return true; } });
  await apps.publish(publishInput());
  names.state = { ...names.state, cid: "bafk-other" };
  await assert.rejects(() => apps.resolve({
    productId: "festival.app",
    name,
    storageName: "festival.origin",
    bucketId,
    readerGrantId: bytes(32, 10),
    deadlineBlock: 100n,
    requestId: () => bytes(16, 90),
  }), /Names resolution disagrees/);

  const source = await readFile(new URL("../src/internal/v2-publishing.ts", import.meta.url), "utf8");
  assert.doesNotMatch(source, /getPreimage|putPreimage|TransactionStorage|retentionTransactions|reservationTransactions/);
  const publicIndex = await readFile(new URL("../src/index.ts", import.meta.url), "utf8");
  assert.doesNotMatch(publicIndex, /v2-publishing|createPrivateOriginAppsV2|FinalizedNamesEventIndexV2/);
});
