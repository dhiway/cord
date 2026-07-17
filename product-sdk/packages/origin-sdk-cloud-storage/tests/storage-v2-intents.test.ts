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
import { resolve } from "node:path";
import test from "node:test";
import {
  STORAGE_V2_OPERATIONS,
  STORAGE_V2_PROTOCOL,
  StorageV2EventSequence,
  createStorageV2Intent,
  storageV2Bytes16,
  storageV2Bytes32,
  storageV2OperationContract,
  validateStorageV2Resume,
  type StorageV2Operation,
} from "../src/internal/storage-v2-intents.ts";

const bytes16 = (fill: number) => storageV2Bytes16(new Uint8Array(16).fill(fill));
const bytes32 = (fill: number) => storageV2Bytes32(new Uint8Array(32).fill(fill));

async function frozenStorageOperations() {
  const path = resolve(import.meta.dirname, "../../../..", "docs/specs/origin-host-registry-v2.operations.json");
  const registry = JSON.parse(await readFile(path, "utf8")) as {
    protocol: string;
    major: number;
    minor: number;
    operations: Array<Record<string, unknown>>;
  };
  return { registry, operations: registry.operations.filter(({ code }) => typeof code === "number" && code >= 1000 && code <= 1061) };
}

test("private storage v2 contracts exactly project frozen operations 1000-1061", async () => {
  const { registry, operations } = await frozenStorageOperations();
  assert.equal(registry.protocol, STORAGE_V2_PROTOCOL);
  assert.equal(registry.major, 2);
  assert.equal(registry.minor, 0);
  assert.equal(operations.length, 26);
  assert.deepEqual(Object.keys(STORAGE_V2_OPERATIONS), operations.map(({ name }) => name));
  for (const operation of operations) {
    const name = operation.name as StorageV2Operation;
    assert.deepEqual(STORAGE_V2_OPERATIONS[name], [
      operation.code,
      operation.grant_scope,
      operation.resume,
      operation.cancellation,
      operation.operation_id_required,
      operation.state_changing,
    ], name);
  }
});

test("intent creation enforces exact operation code and per-operation grants", () => {
  const requestId = bytes16(0x11);
  const operationId = bytes16(0x33);
  const grantId = bytes32(0x22);
  const bucketId = bytes32(0x44);
  const created = createStorageV2Intent("storage.bucket.create", {
    requestId,
    productId: "festival",
    grantId,
    operationId,
    idempotencyKey: new Uint8Array([9]),
    deadlineBlock: 100n,
    payload: { replicaCount: 2, providers: [bytes32(1), bytes32(2)], encryption: 0 },
  });
  assert.equal(created.code, 1000);
  assert.equal(created.protocol, "cord.origin.host/2");
  assert.notEqual(created.requestId, requestId);
  assert.notEqual(created.grantId, grantId);

  const resolved = createStorageV2Intent("storage.resolve", {
    requestId,
    productId: "festival",
    deadlineBlock: 100n,
    payload: { name: "festival.origin" },
  });
  assert.equal(resolved.code, 1051);
  assert.equal("grantId" in resolved, false);

  assert.throws(() => createStorageV2Intent("storage.object.get", {
    requestId,
    productId: "festival",
    deadlineBlock: 100n,
    payload: { bucketId, cid: "bafk-test" },
  } as never), /storage\.bucket\.reader grant/);
  assert.throws(() => createStorageV2Intent("storage.resolve", {
    requestId,
    productId: "festival",
    grantId,
    deadlineBlock: 100n,
    payload: { name: "festival.origin" },
  } as never), /public operation cannot carry a grant/);
  assert.throws(() => createStorageV2Intent("storage.bucket.create", {
    requestId,
    productId: "festival",
    grantId,
    deadlineBlock: 100n,
    payload: { replicaCount: 2, providers: [bytes32(1), bytes32(2)], encryption: 0 },
  } as never), /operationId is required/);
});

test("events are Accepted-first, contiguous, request-bound, and terminal", () => {
  const requestId = bytes16(0x11);
  const sequence = new StorageV2EventSequence<"storage.object.put">(requestId);
  sequence.accept({ kind: "accepted", requestId, seq: 0, state: 0 });
  sequence.accept({ kind: "progress", requestId, seq: 1, completed: 1n, chunksAcked: 1n });
  sequence.accept({ kind: "cancelled", requestId, seq: 2 });
  assert.equal(sequence.terminal, true);
  assert.throws(() => sequence.accept({ kind: "cancelled", requestId, seq: 3 }), /after terminal/);

  const skipped = new StorageV2EventSequence<"storage.object.get">(requestId);
  assert.throws(() => skipped.accept({ kind: "progress", requestId, seq: 0, completed: 0n }), /first event/);
  skipped.accept({ kind: "accepted", requestId, seq: 0, state: 0 });
  assert.throws(() => skipped.accept({ kind: "error", requestId, seq: 2, code: 105, name: "WIRE_SEQUENCE_INVALID", retryable: false }), /sequence must be 1/);
});

test("resume state is operation-specific and v2 remains outside the public entrypoint", async () => {
  assert.equal(storageV2OperationContract("storage.object.put").resume, "provider-token");
  validateStorageV2Resume("storage.object.put", { kind: "provider-token", token: new Uint8Array([1]) });
  assert.throws(() => validateStorageV2Resume("storage.object.put", {
    kind: "verified-offset", offset: 0n, proof: bytes32(7),
  }), /requires provider-token/);
  assert.throws(() => validateStorageV2Resume("storage.resolve", {
    kind: "chain-idempotent", operationId: bytes16(8),
  }), /not resumable/);

  const publicIndex = await readFile(resolve(import.meta.dirname, "../src/index.ts"), "utf8");
  assert.doesNotMatch(publicIndex, /storage-v2-intents|createStorageV2Intent|STORAGE_V2_PROTOCOL/);
});
