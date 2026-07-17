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
const fromHex = (value: string): Uint8Array => Uint8Array.from(Buffer.from(value, "hex"));
const hex = (byte: number): `0x${string}` => `0x${byte.toString(16).padStart(2, "0").repeat(32)}`;
const finalizedHash = hex(0x42);

function request(operation: "storage.replica.status") {
  const vector = vectors.find((candidate: any) => candidate.operation === operation && candidate.id.endsWith("-positive"));
  const frame = decodeHostV2("RequestV2", fromHex(vector.wire_hex)).value as any;
  frame[7] = 101;
  return { frame, encoded: encodeHostV2("RequestV2", frame) };
}

async function dispatchReplica(read: (at: `0x${string}`, target: string, payload: Readonly<Record<string, unknown>>) => Promise<unknown>) {
  const exact = request("storage.replica.status");
  const bridge = new PrivateCordCommonsRuntimeBridgeV2({
    signer: {} as never,
    runtime: { read, async prepare() { throw new Error("unexpected prepare"); } },
    finality: {
      async finalized() { return { number: 99n, hash: finalizedHash, proof: Uint8Array.of(1) }; },
      async verify() { throw new Error("unexpected verify"); },
    },
    events: { async events() { throw new Error("unexpected events"); } },
  });
  const authority = await bridge.finalizedAuthority({
    operation: "storage.replica.status", code: 1022,
    productId: exact.frame[2], requestId: exact.frame[1],
  });
  const events = [];
  const terminalBlocks = [];
  for await (const response of bridge.dispatch({
    operation: "storage.replica.status", request: exact.encoded, authority,
  })) {
    events.push(decodeHostV2("EventV2", response.event).value as any);
    terminalBlocks.push(response.terminalBlock);
  }
  return { events, terminalBlocks };
}

test("replica status is derived from one finalized control, checkpoint, and replica authority", async () => {
  const bucket = hex(0x22);
  const primary = hex(0x31);
  const replicas = [hex(0x32), hex(0x33), hex(0x34)];
  const expectedTargets = [
    "StorageProviderApi.control_bucket", "StorageProviderApi.checkpoint",
    "StorageProviderApi.replica_checkpoint", "StorageProviderApi.replica_checkpoint",
    "StorageProviderApi.replica_checkpoint",
    "StorageProviderApi.provider_is_eligible", "StorageProviderApi.provider_is_eligible",
    "StorageProviderApi.provider_is_eligible", "StorageProviderApi.provider_is_eligible",
  ];
  const calls: { readonly at: string; readonly target: string; readonly payload: Readonly<Record<string, unknown>> }[] = [];
  const checkpoints = [88n, 87n, 86n];
  const { events, terminalBlocks } = await dispatchReplica(async (at, target, payload) => {
    calls.push({ at, target, payload });
    if (target === "StorageProviderApi.control_bucket") {
      return { version: 9, value: { bucket_id: bucket, owner: hex(0x30), version: 7, primary, replicas } };
    }
    if (target === "StorageProviderApi.checkpoint") {
      return {
        version: 9,
        value: {
          bucket_id: bucket, commitment: { mmr_root: hex(0x40), start_seq: 1, leaf_count: 1 },
          checkpoint_block: 88n, primary_signers: 1, commitment_nonce: 4,
          replica_confirmations: [],
        },
      };
    }
    if (target === "StorageProviderApi.replica_checkpoint") {
      return checkpoints[replicas.indexOf(payload.provider as `0x${string}`)];
    }
    return payload.provider !== replicas[2];
  });
  assert.deepEqual(calls.map((call) => call.target), expectedTargets);
  assert.ok(calls.every((call) => call.at === finalizedHash));
  assert.deepEqual(calls.slice(2, 5).map((call) => call.payload), replicas.map((provider) => ({ bucket_id: bucket, provider })));
  assert.deepEqual(calls.slice(5).map((call) => call.payload), [primary, ...replicas].map((provider) => ({ provider })));
  assert.deepEqual(terminalBlocks, [99n, 99n]);
  assert.deepEqual(events.map((event) => [Number(event[2]), Number(event[3])]), [[0, 0], [1, 2]]);
  const result = events[1][4];
  assert.equal(Buffer.from(result[0]).toString("hex"), primary.slice(2));
  assert.deepEqual(result[1].map((provider: Uint8Array) => Buffer.from(provider).toString("hex")), replicas.map((provider) => provider.slice(2)));
  assert.deepEqual([Number(result[2]), Number(result[3]), Number(result[4])], [1, 2, 3]);
  assert.deepEqual([Number(result[5][0]), Buffer.from(result[5][1]).toString("hex")], [99, finalizedHash.slice(2)]);
});

test("replica status fails closed when finalized replica lag is unavailable", async () => {
  const bucket = hex(0x22);
  const replicas = [hex(0x32), hex(0x33)];
  const { events } = await dispatchReplica(async (_at, target, payload) => {
    if (target === "StorageProviderApi.control_bucket") {
      return { version: 9, value: { bucket_id: bucket, owner: hex(0x30), version: 7, primary: hex(0x31), replicas } };
    }
    if (target === "StorageProviderApi.checkpoint") {
      return {
        version: 9,
        value: {
          bucket_id: bucket, commitment: { mmr_root: hex(0x40), start_seq: 1, leaf_count: 1 },
          checkpoint_block: 88n, primary_signers: 1, commitment_nonce: 4,
          replica_confirmations: [],
        },
      };
    }
    assert.equal(target, "StorageProviderApi.replica_checkpoint");
    return payload.provider === replicas[0] ? 87n : null;
  });
  assert.deepEqual(events.map((event) => [Number(event[2]), Number(event[3])]), [[0, 0], [1, 3]]);
  assert.deepEqual([Number(events[1][4][0]), events[1][4][1], events[1][4][2]], [255, "PROVIDER_INELIGIBLE", true]);
});

test("replica status rejects checkpoints ahead of finalized canonical authority", async () => {
  const bucket = hex(0x22);
  const replicas = [hex(0x32), hex(0x33)];
  await assert.rejects(async () => {
    await dispatchReplica(async (_at, target, payload) => {
      if (target === "StorageProviderApi.control_bucket") {
        return { version: 9, value: { bucket_id: bucket, owner: hex(0x30), version: 7, primary: hex(0x31), replicas } };
      }
      if (target === "StorageProviderApi.checkpoint") {
        return {
          version: 9,
          value: {
            bucket_id: bucket, commitment: { mmr_root: hex(0x40), start_seq: 1, leaf_count: 1 },
            checkpoint_block: 88n, primary_signers: 1, commitment_nonce: 4,
            replica_confirmations: [],
          },
        };
      }
      assert.equal(target, "StorageProviderApi.replica_checkpoint");
      return payload.provider === replicas[0] ? 89n : 88n;
    });
  }, /replica checkpoint is ahead of the canonical checkpoint/);
});

test("replica status preserves accepted then bounded not-found error sequencing", async () => {
  let reads = 0;
  const { events, terminalBlocks } = await dispatchReplica(async (at, target, payload) => {
    reads += 1;
    assert.equal(at, finalizedHash);
    assert.equal(target, "StorageProviderApi.control_bucket");
    assert.equal(payload.bucket_id, hex(0x22));
    return { version: 9, value: null };
  });
  assert.equal(reads, 1);
  assert.deepEqual(terminalBlocks, [99n, 99n]);
  assert.deepEqual(events.map((event) => [Number(event[2]), Number(event[3])]), [[0, 0], [1, 3]]);
  assert.deepEqual([Number(events[1][4][0]), events[1][4][1], events[1][4][2]], [250, "BUCKET_NOT_FOUND", false]);
});
