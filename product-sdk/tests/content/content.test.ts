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
import test from "node:test";
import {
  bitswapContentProvider,
  createContentClient,
  declareContentAddress,
  digestContent,
  gatewayContentProvider,
  parseContentCid,
  verifyContentBlock,
  type ContentCodec,
  type ContentMultihash,
} from "@cord-network/origin-sdk-cloud-storage";
import { ContentError } from "@cord-network/origin-sdk-cloud-storage";

const text = new TextEncoder();
const cidFixture = JSON.parse(
  readFileSync(new URL("../../../docs/sdk/vectors/content-cid-v1.json", import.meta.url), "utf8"),
) as {
  readonly schema: string;
  readonly input_utf8: string;
  readonly codec: ContentCodec;
  readonly multihash: ContentMultihash;
  readonly cid: string;
};
const hex = (bytes: Uint8Array) => [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
const varint = (value: number): number[] => {
  const result: number[] = [];
  do {
    let byte = value % 128;
    value = Math.floor(value / 128);
    if (value) byte |= 0x80;
    result.push(byte);
  } while (value);
  return result;
};
const base32 = (bytes: Uint8Array): string => {
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  let accumulator = 0;
  let bits = 0;
  let result = "";
  for (const byte of bytes) {
    accumulator = accumulator * 256 + byte;
    bits += 8;
    while (bits >= 5) {
      const divisor = 2 ** (bits - 5);
      result += alphabet[Math.floor(accumulator / divisor) & 31];
      accumulator %= divisor;
      bits -= 5;
    }
  }
  if (bits) result += alphabet[(accumulator * 2 ** (5 - bits)) & 31];
  return result;
};
const base58 = (bytes: Uint8Array): string => {
  const alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let value = 0n;
  for (const byte of bytes) value = value * 256n + BigInt(byte);
  let result = "";
  while (value) {
    result = alphabet[Number(value % 58n)] + result;
    value /= 58n;
  }
  let zeroes = 0;
  while (bytes[zeroes] === 0) zeroes += 1;
  return "1".repeat(zeroes) + result;
};
const cid = (bytes: Uint8Array, codec: ContentCodec, hash: ContentMultihash): string => {
  const codecCode = codec === "raw" ? 0x55 : 0x70;
  const hashCode = hash === "sha2-256" ? 0x12 : 0xb220;
  const digest = digestContent(hash, bytes);
  return `b${base32(Uint8Array.from([...varint(1), ...varint(codecCode), ...varint(hashCode), 32, ...digest]))}`;
};
async function errorCode(promise: Promise<unknown>): Promise<string> {
  try {
    await promise;
    return "success";
  } catch (error) {
    assert.ok(error instanceof ContentError);
    return error.code;
  }
}

test("portable SHA2-256 and Blake2b-256 match canonical vectors", () => {
  assert.equal(hex(digestContent("sha2-256", text.encode("abc"))), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  assert.equal(hex(digestContent("blake2b-256", text.encode("abc"))), "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319");
  assert.equal(hex(digestContent("blake2b-256", new Uint8Array())), "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8");
  const multiBlock = Uint8Array.from({ length: 129 }, (_, index) => index % 251);
  assert.equal(hex(digestContent("blake2b-256", multiBlock)), "f7f3c46ba2564ff4c4c162da1f5b605f9f1c4aa6a20652a9f9a337c1a2f5b9c9");
});

test("raw Blake2b-256 CID matches the Orbis TransactionStorage fixture", () => {
  assert.equal(cidFixture.schema, "cord.content-cid-vector.v1");
  const bytes = text.encode(cidFixture.input_utf8);
  assert.equal(cid(bytes, cidFixture.codec, cidFixture.multihash), cidFixture.cid);
  assert.equal(verifyContentBlock(cidFixture.cid, bytes).multihash, "blake2b-256");
});

test("CIDv0 is accepted only as canonical DAG-PB plus SHA2-256", () => {
  const bytes = text.encode("CIDv0 block");
  const value = base58(Uint8Array.from([0x12, 0x20, ...digestContent("sha2-256", bytes)]));
  assert.ok(value.startsWith("Qm"));
  assert.deepEqual(
    { ...parseContentCid(value), digest: undefined },
    { version: 0, codec: "dag-pb", multihash: "sha2-256", digest: undefined },
  );
  assert.equal(verifyContentBlock(value, bytes).version, 0);
});

test("CID declarations bind codec and multihash before transport", async () => {
  const bytes = text.encode("provider block");
  const value = cid(bytes, "raw", "blake2b-256");
  assert.deepEqual(parseContentCid(value), {
    version: 1,
    codec: "raw",
    multihash: "blake2b-256",
    digest: digestContent("blake2b-256", bytes),
  });
  assert.equal(verifyContentBlock(value, bytes).codec, "raw");
  assert.throws(() => declareContentAddress(value, "dag-pb", "blake2b-256"), { code: "content_integrity" });
  assert.throws(() => parseContentCid(`${value}a`), { code: "content_integrity" });
  let called = false;
  const client = createContentClient({ providers: [{ id: "unused", kind: "gateway", async fetchBlock() { called = true; return bytes; } }] });
  assert.equal(await errorCode(client.fetch({ cid: value, codec: "raw", multihash: "sha2-256" })), "content_integrity");
  assert.equal(called, false);
});

test("gateway and Bitswap sources fail over in exact caller order and verify bytes", async () => {
  const bytes = text.encode("festival content");
  const value = cid(bytes, "raw", "blake2b-256");
  const calls: string[] = [];
  const gateway = gatewayContentProvider({
    id: "gateway-a",
    baseUrl: "https://gateway.example/ipfs",
    async transport(locator) {
      calls.push(locator);
      throw new Error("offline");
    },
  });
  const bitswap = bitswapContentProvider({
    id: "bitswap-b",
    async transport(request) {
      calls.push(`bitswap:${request.cid}`);
      return bytes;
    },
  });
  const unused = bitswapContentProvider({ id: "unused", async transport() { throw new Error("must not run"); } });
  const client = createContentClient({ providers: [gateway, bitswap, unused] });
  assert.deepEqual(await client.fetch(declareContentAddress(value, "raw", "blake2b-256")), bytes);
  assert.deepEqual(calls, [`https://gateway.example/ipfs/${value}`, `bitswap:${value}`]);
});

test("integrity and unavailability are distinct typed terminal errors", async () => {
  const expected = text.encode("expected");
  const value = cid(expected, "raw", "sha2-256");
  const corrupt = createContentClient({
    providers: [
      { id: "one", kind: "gateway", async fetchBlock() { return text.encode("corrupt-1"); } },
      { id: "two", kind: "bitswap", async fetchBlock() { return text.encode("corrupt-2"); } },
    ],
  });
  assert.equal(await errorCode(corrupt.fetch(declareContentAddress(value, "raw", "sha2-256"))), "content_integrity");
  const offline = createContentClient({
    providers: [{ id: "offline", kind: "gateway", async fetchBlock() { throw new Error("offline"); } }],
  });
  assert.equal(await errorCode(offline.fetch(declareContentAddress(value, "raw", "sha2-256"))), "content_unavailable");
});

test("abort signals cancel an in-flight caller transport with one typed result", async () => {
  const bytes = text.encode("never delivered");
  const value = cid(bytes, "raw", "sha2-256");
  const controller = new AbortController();
  const client = createContentClient({
    providers: [{ id: "hanging", kind: "bitswap", fetchBlock: () => new Promise(() => {}) }],
  });
  const pending = client.fetch(declareContentAddress(value, "raw", "sha2-256"), { signal: controller.signal });
  controller.abort();
  try {
    await pending;
    assert.fail("fetch should be cancelled");
  } catch (error) {
    assert.ok(error instanceof ContentError);
    assert.equal(error.code, "content_unavailable");
    assert.equal(error.details.cancelled, true);
  }
});

test("abort signals also interrupt a source blocked between streamed chunks", async () => {
  const bytes = text.encode("stream never completes");
  const value = cid(bytes, "raw", "sha2-256");
  const controller = new AbortController();
  const client = createContentClient({
    providers: [{
      id: "hanging-stream",
      kind: "gateway",
      async fetchBlock() {
        return (async function* () {
          yield bytes.slice(0, 1);
          await new Promise(() => {});
        })();
      },
    }],
  });
  const pending = client.fetch(declareContentAddress(value, "raw", "sha2-256"), { signal: controller.signal });
  await new Promise((resolve) => setTimeout(resolve, 0));
  controller.abort();
  assert.equal(await errorCode(pending), "content_unavailable");
});

test("streamed blocks and reconstructed content are bounded", async () => {
  const bytes = text.encode("12345");
  const value = cid(bytes, "raw", "sha2-256");
  const client = createContentClient({
    maxBlockBytes: 4,
    providers: [{
      id: "stream",
      kind: "gateway",
      async fetchBlock() {
        return (async function* () { yield bytes.slice(0, 3); yield bytes.slice(3); })();
      },
    }],
  });
  assert.equal(await errorCode(client.fetch(declareContentAddress(value, "raw", "sha2-256"))), "content_unavailable");
});

test("DAG-PB is honest: decoder is required and can load only CID-verified linked blocks", async () => {
  const root = text.encode("encoded dag-pb root");
  const child = text.encode("verified UnixFS leaf");
  const rootCid = cid(root, "dag-pb", "sha2-256");
  const childCid = cid(child, "raw", "blake2b-256");
  const source = bitswapContentProvider({
    id: "blocks",
    async transport(request) {
      if (request.cid === rootCid) return root;
      if (request.cid === childCid) return child;
      throw new Error("missing");
    },
  });
  const withoutDecoder = createContentClient({ providers: [source] });
  assert.equal(
    await errorCode(withoutDecoder.fetch(declareContentAddress(rootCid, "dag-pb", "sha2-256"))),
    "content_unavailable",
  );
  let verifiedRoot = false;
  const withDecoder = createContentClient({
    providers: [source],
    dagPbDecoder: {
      async decode(context) {
        verifiedRoot = hex(context.root) === hex(root);
        return context.loadBlock(childCid);
      },
    },
  });
  assert.deepEqual(
    await withDecoder.fetch(declareContentAddress(rootCid, "dag-pb", "sha2-256")),
    child,
  );
  assert.equal(verifiedRoot, true);
});
