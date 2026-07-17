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
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { PrivateCordCommonsRuntimeBridgeV2 } from "../../../internal/commons-runtime-bridge-v2.ts";
import { decodeHostV2, encodeHostV2 } from "../src/internal/v2/codec.ts";

const repo = resolve(import.meta.dirname, "../../../..");
const vectors = JSON.parse(readFileSync(resolve(repo, "docs/specs/origin-host-registry-v2.vectors.json"), "utf8")).vectors;
const bytes = (hex: string): Uint8Array => Uint8Array.from(Buffer.from(hex, "hex"));
const finalizedHash = `0x${"42".repeat(32)}` as const;

async function runtimeFailure(operation: "storage.bucket.create" | "storage.publish", runtimeCode: string) {
  const vector = vectors.find((candidate: any) => candidate.operation === operation && candidate.id.endsWith("-positive"));
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any;
  frame[7] = 101;
  if (operation === "storage.bucket.create") {
    frame[8][0] = 2;
    frame[8][1] = [frame[8][1][0], frame[8][1][0], frame[8][1][0]];
  }
  const bridge = new PrivateCordCommonsRuntimeBridgeV2({
    signer: {} as never,
    runtime: {
      async read() { throw new Error("unexpected read"); },
      async prepare() { throw { code: runtimeCode }; },
    },
    finality: {
      async finalized() { return { number: 99n, hash: finalizedHash, proof: Uint8Array.of(1) }; },
      async verify() { throw new Error("unexpected verify"); },
    },
    events: { async events() { throw new Error("unexpected events"); } },
  });
  const authority = await bridge.finalizedAuthority({ operation, code: Number(frame[3]), productId: frame[2], requestId: frame[1] });
  const events = [];
  for await (const response of bridge.dispatch({ operation, request: encodeHostV2("RequestV2", frame), authority })) {
    events.push(decodeHostV2("EventV2", response.event).value as any);
  }
  assert.equal(events.length, 1);
  return events[0][4];
}

test("Commons runtime deadline, replay, and bounded receipt errors stay inside Host-v2", async () => {
  for (const [operation, runtimeCode, code, name] of [
    ["storage.bucket.create", "StorageProvider.OperationDeadlineExpired", 106, "REQUEST_DEADLINE_EXPIRED"],
    ["storage.publish", "Names.OperationDeadlineExpired", 106, "REQUEST_DEADLINE_EXPIRED"],
    ["storage.bucket.create", "StorageProvider.OperationIdConflict", 206, "STORAGE_IDEMPOTENCY_CONFLICT"],
    ["storage.publish", "Names.OperationIdConflict", 206, "STORAGE_IDEMPOTENCY_CONFLICT"],
    ["storage.bucket.create", "StorageProvider.BucketOperationReceiptCapacityReached", 212, "STORAGE_OPERATION_RECEIPT_CAPACITY_REACHED"],
    ["storage.publish", "Names.ContentOperationReceiptCapacityReached", 212, "STORAGE_OPERATION_RECEIPT_CAPACITY_REACHED"],
  ] as const) {
    const error = await runtimeFailure(operation, runtimeCode);
    assert.deepEqual([Number(error[0]), error[1], error[2]], [code, name, false]);
  }
});

test("Commons rejects too-distant operation deadlines before runtime submission", async () => {
  const vector = vectors.find((candidate: any) => candidate.id === "1000-positive");
  const frame = decodeHostV2("RequestV2", bytes(vector.wire_hex)).value as any;
  frame[7] = 228;
  let submissions = 0;
  const bridge = new PrivateCordCommonsRuntimeBridgeV2({
    signer: {} as never,
    runtime: {
      async read() { throw new Error("unexpected read"); },
      async prepare() { submissions += 1; throw new Error("unexpected prepare"); },
    },
    finality: {
      async finalized() { return { number: 99n, hash: finalizedHash, proof: Uint8Array.of(1) }; },
      async verify() { throw new Error("unexpected verify"); },
    },
    events: { async events() { throw new Error("unexpected events"); } },
  });
  const authority = await bridge.finalizedAuthority({ operation: "storage.bucket.create", code: 1000, productId: frame[2], requestId: frame[1] });
  const events = [];
  for await (const response of bridge.dispatch({ operation: "storage.bucket.create", request: encodeHostV2("RequestV2", frame), authority })) {
    events.push(decodeHostV2("EventV2", response.event).value as any);
  }
  assert.equal(submissions, 0);
  assert.deepEqual([Number(events[0][4][0]), events[0][4][1]], [117, "REQUEST_DEADLINE_TOO_FAR"]);
});
