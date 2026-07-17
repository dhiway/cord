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
import { parseContentCid } from "../../origin-sdk-cloud-storage/src/content.ts";
import { decodeHostV2, encodeHostV2 } from "../src/internal/v2/codec.ts";

const repo = resolve(import.meta.dirname, "../../../..");
const vectors = JSON.parse(readFileSync(resolve(repo, "docs/specs/origin-host-registry-v2.vectors.json"), "utf8")).vectors;
const fromHex = (value: string): Uint8Array => Uint8Array.from(Buffer.from(value, "hex"));
const hex = (byte: number): `0x${string}` => `0x${byte.toString(16).padStart(2, "0").repeat(32)}`;
const finalizedHash = hex(0x42);

function request(operation: "storage.replica.status" | "storage.s3.list") {
  const vector = vectors.find((candidate: any) => candidate.operation === operation && candidate.id.endsWith("-positive"));
  const frame = decodeHostV2("RequestV2", fromHex(vector.wire_hex)).value as any;
  frame[7] = 101;
  return { frame, encoded: encodeHostV2("RequestV2", frame) };
}

async function dispatchS3(
  mutate: (frame: any) => void,
  read: (at: `0x${string}`, target: string, payload: Readonly<Record<string, unknown>>) => Promise<unknown>,
) {
  const exact = request("storage.s3.list");
  mutate(exact.frame);
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
    operation: "storage.s3.list", code: 1042,
    productId: exact.frame[2], requestId: exact.frame[1],
  });
  const events = [];
  const terminalBlocks = [];
  for await (const response of bridge.dispatch({
    operation: "storage.s3.list", request: encodeHostV2("RequestV2", exact.frame), authority,
  })) {
    events.push(decodeHostV2("EventV2", response.event).value as any);
    terminalBlocks.push(response.terminalBlock);
  }
  return { events, terminalBlocks };
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

const textEncoder = new TextEncoder();
function snapshotCursor(version: bigint, key: string): Uint8Array {
  const encodedKey = textEncoder.encode(key);
  const cursor = new Uint8Array(8 + encodedKey.length);
  new DataView(cursor.buffer).setBigUint64(0, version);
  cursor.set(encodedKey, 8);
  return cursor;
}

test("S3 list resolves one bounded stable snapshot into authoritative object CIDs", async () => {
  const bucketName = "festival-bucket";
  const bucket = hex(0x51);
  const prefix = textEncoder.encode("images/");
  const inputCursor = snapshotCursor(7n, "images/a.png");
  const keys = [textEncoder.encode("images/b.png"), textEncoder.encode("images/c.png")];
  const commitments = [new Uint8Array(32).fill(0xaa), new Uint8Array(32).fill(0xbb)];
  const calls: { readonly at: string; readonly target: string; readonly payload: Readonly<Record<string, unknown>> }[] = [];
  const { events, terminalBlocks } = await dispatchS3((frame) => {
    frame[8] = { 0: bucketName, 1: prefix, 2: inputCursor, 3: 2 };
  }, async (at, target, payload) => {
    calls.push({ at, target, payload });
    if (target === "S3RegistryApi.bucket_by_name") {
      return { version: 9, value: { bucket_id: bucket, name: textEncoder.encode(bucketName), status: "Active" } };
    }
    if (target === "S3RegistryApi.object_keys") {
      return {
        type: "Ok",
        value: {
          version: 9, items: keys,
          next_cursor: { snapshot_version: 7n, last_key: keys[1] }, snapshot_version: 7n,
        },
      };
    }
    const index = keys.findIndex((key) => Buffer.from(key).equals(Buffer.from(payload.key as Uint8Array)));
    assert.equal(target, "S3RegistryApi.object");
    assert.notEqual(index, -1);
    return {
      version: 9,
      value: { bucket_id: bucket, key: keys[index], content_hash: commitments[index], deleted: false },
    };
  });
  assert.deepEqual(calls.map((call) => call.target), [
    "S3RegistryApi.bucket_by_name", "S3RegistryApi.object_keys",
    "S3RegistryApi.object", "S3RegistryApi.object",
  ]);
  assert.ok(calls.every((call) => call.at === finalizedHash));
  assert.deepEqual(calls[0].payload, { name: textEncoder.encode(bucketName) });
  assert.deepEqual(calls[1].payload, {
    bucket_id: bucket, prefix,
    cursor: { snapshot_version: 7n, last_key: textEncoder.encode("images/a.png") }, limit: 2,
  });
  assert.deepEqual(calls.slice(2).map((call) => call.payload), keys.map((key) => ({ bucket_id: bucket, key })));
  assert.deepEqual(terminalBlocks, [99n, 99n]);
  assert.deepEqual(events.map((event) => [Number(event[2]), Number(event[3])]), [[0, 0], [1, 2]]);
  const result = events[1][4];
  assert.equal(result[0].length, 2);
  for (let index = 0; index < result[0].length; index += 1) {
    const cid = parseContentCid(result[0][index]);
    assert.deepEqual([cid.version, cid.codec, cid.multihash], [1, "raw", "blake2b-256"]);
    assert.deepEqual(cid.digest, commitments[index]);
  }
  assert.deepEqual(result[1], snapshotCursor(7n, "images/c.png"));
  assert.deepEqual([Number(result[2]), Number(result[3][0]), Buffer.from(result[3][1]).toString("hex")], [7, 99, finalizedHash.slice(2)]);
});

test("S3 list maps the native stale snapshot error without reading objects", async () => {
  let reads = 0;
  const { events } = await dispatchS3((frame) => {
    frame[8] = { 0: "festival-bucket", 2: snapshotCursor(7n, "a"), 3: 2 };
  }, async (_at, target) => {
    reads += 1;
    if (target === "S3RegistryApi.bucket_by_name") {
      return {
        version: 9,
        value: { bucket_id: hex(0x51), name: textEncoder.encode("festival-bucket"), status: "Active" },
      };
    }
    assert.equal(target, "S3RegistryApi.object_keys");
    return { type: "Err", value: "CursorStale" };
  });
  assert.equal(reads, 2);
  assert.deepEqual(events.map((event) => [Number(event[2]), Number(event[3])]), [[0, 0], [1, 3]]);
  assert.deepEqual([Number(events[1][4][0]), events[1][4][1], events[1][4][2]], [261, "STORAGE_CURSOR_STALE", true]);
});

test("S3 list rejects non-increasing finalized object pages", async () => {
  const bucket = hex(0x51);
  await assert.rejects(async () => {
    await dispatchS3((frame) => {
      frame[8] = { 0: "festival-bucket", 3: 2 };
    }, async (_at, target) => {
      if (target === "S3RegistryApi.bucket_by_name") {
        return {
          version: 9,
          value: { bucket_id: bucket, name: textEncoder.encode("festival-bucket"), status: "Active" },
        };
      }
      assert.equal(target, "S3RegistryApi.object_keys");
      return {
        type: "Ok",
        value: {
          version: 9, items: [textEncoder.encode("b"), textEncoder.encode("a")],
          next_cursor: null, snapshot_version: 1n,
        },
      };
    });
  }, /strictly increasing key order/);
});
