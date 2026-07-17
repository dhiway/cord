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
import { encodeStorageV2Intent, storageV2Hex } from "../src/internal/storage-v2-codec.ts";
import {
  STORAGE_V2_ERRORS,
  validateStorageV2Error,
  validateStorageV2Payload,
  validateStorageV2Progress,
  validateStorageV2Result,
} from "../src/internal/storage-v2-validation.ts";

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

test("frozen error tuples and per-operation scopes are exhaustive", async () => {
  const root = resolve(import.meta.dirname, "../../../..", "docs/specs");
  const errorsRegistry = JSON.parse(await readFile(resolve(root, "origin-host-registry-v2.errors.json"), "utf8")) as {
    errors: Array<{ code: number; name: string; retryable: boolean }>;
  };
  assert.deepEqual(
    Object.entries(STORAGE_V2_ERRORS).map(([code, [name, retryable]]) => ({ code: Number(code), name, retryable })),
    errorsRegistry.errors.map(({ code, name, retryable }) => ({ code, name, retryable })),
  );

  const { operations } = await frozenStorageOperations();
  const requestId = bytes16(0x11);
  for (const frozenOperation of operations) {
    const operation = frozenOperation.name as StorageV2Operation;
    const allowed = new Set((frozenOperation.allowed_errors as Array<{ code: number }>).map(({ code }) => code));
    for (const [codeText, [name, retryable]] of Object.entries(STORAGE_V2_ERRORS)) {
      const code = Number(codeText);
      const validate = () => validateStorageV2Error(operation, {
        kind: "error", requestId, seq: 1, code, name, retryable,
      });
      if (allowed.has(code)) assert.doesNotThrow(validate, `${operation} must allow ${code}`);
      else assert.throws(validate, /scope drift/, `${operation} must reject ${code}`);
    }
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

test("hostile payload and result bounds fail closed", () => {
  const requestId = bytes16(0x11);
  const grantId = bytes32(0x22);
  const operationId = bytes16(0x33);
  const bucketId = bytes32(0x44);
  const reject = (operation: StorageV2Operation, payload: unknown, pattern: RegExp) => assert.throws(
    () => validateStorageV2Payload(operation, payload as never), pattern,
  );
  reject("storage.bucket.create", { replicaCount: 1, providers: [bytes32(1)], encryption: 0 }, /2-4 providers/);
  reject("storage.bucket.grant", { bucketId, subject: bytes32(2), role: 1, issuedAt: 2n, expiresAt: 2n }, /greater than issuedAt/);
  reject("storage.object.range", { bucketId, cid: "bafk", offset: 0n, length: 0n }, /positive u64/);
  reject("storage.drive.commit", { bucketId, manifest: "bafk", bytes: new Uint8Array(), expectedVersion: 0n, mode: 0 }, /1-4194304 bytes/);
  reject("storage.s3.list", { bucket: "bucket", limit: 101 }, /1..100/);
  reject("storage.keys.export", { bucketId, keyVersion: 1, recipientKey: new Uint8Array(31) }, /32-256 bytes/);
  reject("storage.object.put", { bucketId, cid: "bafk", length: 1n, encrypted: 0, transferId: operationId, extra: true }, /record shape/);

  assert.throws(() => validateStorageV2Result("storage.object.put", {
    receipt: { provider: bytes32(1), cid: "bafk", length: 1n, signature: new Uint8Array(63) },
    publishable: true,
    finalized: { number: 1n, hash: bytes32(2) },
  }), /64 bytes/);
  assert.throws(() => validateStorageV2Result("storage.object.range", {
    cid: "bafk", offset: 9n, length: 2n, total: 10n,
    checkpoint: { root: bytes32(1), from: 1n, to: 2n, replicas: 2 },
  }), /range exceeds total/);
  assert.throws(() => validateStorageV2Result("storage.keys.export", {
    wrappedKey: new Uint8Array(31), algorithm: 1, keyVersion: 1,
  }), /32-1024 bytes/);
});

test("progress and frozen error tuples are operation-exact", () => {
  const requestId = bytes16(0x11);
  assert.throws(() => validateStorageV2Progress("storage.object.get", {
    kind: "progress", requestId, seq: 1, completed: 1n,
  } as never), /offset,bytes/);
  assert.throws(() => validateStorageV2Progress("storage.object.put", {
    kind: "progress", requestId, seq: 1, offset: 0n, bytes: new Uint8Array([1]),
  } as never), /completed/);
  assert.throws(() => validateStorageV2Progress("storage.s3.get", {
    kind: "progress", requestId, seq: 1, offset: 0n, bytes: new Uint8Array(4_194_305),
  }), /0-4194304 bytes/);
  validateStorageV2Error("storage.object.put", {
    kind: "error", requestId, seq: 1, code: 114, name: "HOST_OUTBOX_FULL", retryable: true,
    details: { message: "capacity", lower: 1n, upper: 2n, hash: bytes32(3) },
  });
  assert.throws(() => validateStorageV2Error("storage.object.put", {
    kind: "error", requestId, seq: 1, code: 114, name: "HOST_OUTBOX_FULL", retryable: false,
  }), /code\/name\/retryability\/scope drift/);
  assert.throws(() => validateStorageV2Error("storage.object.put", {
    kind: "error", requestId, seq: 1, code: 999, name: "UNKNOWN", retryable: false,
  }), /code\/name\/retryability\/scope drift/);
  assert.throws(() => validateStorageV2Error("storage.keys.export", {
    kind: "error", requestId, seq: 1, code: 200, name: "STORAGE_CHUNK_OUT_OF_ORDER", retryable: false,
  }), /code\/name\/retryability\/scope drift/);
});

test("cancel is idempotent, terminal, and revokes resume authority", () => {
  const requestId = bytes16(0x11);
  const sequence = new StorageV2EventSequence("storage.object.put", requestId);
  sequence.accept({ kind: "accepted", requestId, seq: 0, state: 0 });
  sequence.authorizeResume({ kind: "provider-token", token: new Uint8Array([1]) });
  assert.equal(sequence.resumeAuthority, true);
  assert.equal(sequence.requestCancel(), true);
  assert.equal(sequence.requestCancel(), false);
  assert.equal(sequence.resumeAuthority, false);
  assert.throws(() => sequence.authorizeResume({ kind: "provider-token", token: new Uint8Array([1]) }), /not live/);
  assert.throws(() => sequence.accept({ kind: "progress", requestId, seq: 1, completed: 1n }), /cannot emit later progress/);
  sequence.accept({ kind: "cancelled", requestId, seq: 1 });
  assert.equal(sequence.terminal, true);
  assert.throws(() => sequence.accept({ kind: "cancelled", requestId, seq: 2 }), /after terminal/);
});

test("typed TS codec emits the frozen canonical CBOR frame", async () => {
  const requestId=bytes16(0x11);const grantId=bytes32(0x22);const operationId=bytes16(0x33);
  const intent=createStorageV2Intent("storage.bucket.create",{
    requestId,productId:"festival",grantId,operationId,deadlineBlock:100n,
    payload:{replicaCount:1,providers:[bytes32(0x22),bytes32(0x22)],encryption:0},
  });
  const vectors=JSON.parse(await readFile(resolve(import.meta.dirname,"../../../..","docs/specs/origin-host-registry-v2.vectors.json"),"utf8")) as {vectors:Array<{id:string;wire_hex:string}>};
  const golden=vectors.vectors.find(({id})=>id==="1000-positive");
  assert.ok(golden);
  assert.equal(storageV2Hex(encodeStorageV2Intent(intent)),golden.wire_hex);
});

test("resume state is operation-specific and v2 remains outside the public entrypoint", async () => {
  assert.equal(storageV2OperationContract("storage.object.put").resume, "provider-token");
  validateStorageV2Resume("storage.object.put", { kind: "provider-token", token: new Uint8Array([1]) });
  assert.throws(() => validateStorageV2Resume("storage.object.put", {
    kind: "provider-token", token: new Uint8Array(),
  }), /1-4096 bytes/);
  assert.throws(() => validateStorageV2Resume("storage.s3.list", {
    kind: "cursor-versioned", cursor: new Uint8Array(2049), version: 0n,
  }), /1-2048 bytes/);
  assert.throws(() => validateStorageV2Resume("storage.drive.commit", {
    kind: "per-object", cid: "e\u0301", version: 0n,
  }), /NFC/);
  assert.throws(() => validateStorageV2Resume("storage.object.put", {
    kind: "verified-offset", offset: 0n, proof: bytes32(7),
  }), /requires provider-token/);
  assert.throws(() => validateStorageV2Resume("storage.resolve", {
    kind: "chain-idempotent", operationId: bytes16(8),
  }), /not resumable/);

  const publicIndex = await readFile(resolve(import.meta.dirname, "../src/index.ts"), "utf8");
  assert.doesNotMatch(publicIndex, /storage-v2-intents|createStorageV2Intent|STORAGE_V2_PROTOCOL/);
});
