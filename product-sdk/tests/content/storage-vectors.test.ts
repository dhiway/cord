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
import { blake2b256 } from "../../packages/origin-sdk-crypto/src/index.ts";

type Vector = {
  readonly id: string;
  readonly mode?: "none" | "xchacha20poly1305-v1";
  readonly plaintext_len?: number;
  readonly stored_len?: number;
  readonly chunk_count?: number;
  readonly digest_hex?: string;
  readonly cid?: string;
  readonly envelope_hex?: string;
  readonly expected_error?: string;
  readonly pre_encryption_rejection?: boolean;
};

const vectors = JSON.parse(
  readFileSync(new URL("../../../docs/specs/storage-v1.vectors.json", import.meta.url), "utf8"),
) as { readonly version: number; readonly vectors: readonly Vector[] };
const bounds = Object.fromEntries(
  readFileSync(new URL("../../../docs/specs/storage-bounds-v1.toml", import.meta.url), "utf8")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const [key, raw] = line.split("=", 2).map((part) => part.trim());
      return [key, Number(raw)];
    }),
) as Record<string, number>;

const byId = (id: string): Vector => {
  const found = vectors.vectors.find((entry) => entry.id === id);
  assert.ok(found, `missing frozen storage vector ${id}`);
  return found;
};
const bytes = (hex: string): Uint8Array => Uint8Array.from(hex.match(/../g)?.map((pair) => Number.parseInt(pair, 16)) ?? []);
const hex = (value: Uint8Array): string => [...value].map((byte) => byte.toString(16).padStart(2, "0")).join("");
const cidDigest = (cid: string): Uint8Array => {
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  let accumulator = 0;
  let bits = 0;
  const decoded: number[] = [];
  for (const character of cid.slice(1)) {
    accumulator = accumulator * 32 + alphabet.indexOf(character);
    bits += 5;
    while (bits >= 8) {
      const divisor = 2 ** (bits - 8);
      decoded.push(Math.floor(accumulator / divisor) & 0xff);
      accumulator %= divisor;
      bits -= 8;
    }
  }
  assert.equal(accumulator, 0, "CID has non-canonical base32 padding");
  assert.deepEqual(decoded.slice(0, 6), [0x01, 0x55, 0xa0, 0xe4, 0x02, 0x20]);
  assert.equal(decoded.length, 38);
  return Uint8Array.from(decoded.slice(6));
};

class HostStorageAdmissionError extends Error {
  readonly code = "STORAGE_OBJECT_TOO_LARGE";
}

/** Test oracle for the host-owned pre-encryption boundary; this is not provider plaintext logic. */
function hostStoredLength(mode: "none" | "xchacha20poly1305-v1", plaintextLength: number): number {
  const plaintextMaximum = mode === "none"
    ? bounds.max_none_plaintext_bytes
    : bounds.max_encrypted_plaintext_bytes;
  const storedLength = plaintextLength + (mode === "none" ? 0 : bounds.xchacha_envelope_overhead_bytes);
  if (plaintextLength > plaintextMaximum || storedLength > bounds.max_stored_object_bytes) {
    throw new HostStorageAdmissionError();
  }
  return storedLength;
}

test("AC2 storage vectors bind SDK CID behavior and the host/provider byte boundary", () => {
  assert.equal(vectors.version, 1);
  assert.equal(bounds.chunk_bytes, 262_144);
  assert.equal(bounds.max_chunks, 256);
  assert.equal(bounds.max_stored_object_bytes, 67_108_864);
  assert.equal(bounds.max_none_plaintext_bytes, 67_108_864);
  assert.equal(bounds.max_encrypted_plaintext_bytes, 67_108_823);
  assert.equal(bounds.xchacha_envelope_overhead_bytes, 41);

  for (const entry of vectors.vectors.filter((candidate) => candidate.cid)) {
    assert.match(entry.cid!, /^b[a-z2-7]+$/, `${entry.id} is not CIDv1 base32lower`);
    assert.equal(hex(cidDigest(entry.cid!)), entry.digest_hex, entry.id);

    let stored: Uint8Array | undefined;
    if (entry.envelope_hex !== undefined) stored = bytes(entry.envelope_hex);
    else if (entry.mode !== "xchacha20poly1305-v1" && entry.plaintext_len !== undefined) {
      stored = new Uint8Array(entry.plaintext_len).fill(0xa5);
    }
    if (stored !== undefined) {
      assert.equal(stored.byteLength, entry.stored_len, entry.id);
      assert.equal(hex(blake2b256(stored)), entry.digest_hex, entry.id);
    }
    if (entry.stored_len !== undefined) {
      if (entry.chunk_count !== undefined) {
        assert.equal(entry.chunk_count, Math.ceil(entry.stored_len / bounds.chunk_bytes), entry.id);
      }
      assert.ok(entry.stored_len <= bounds.max_stored_object_bytes, entry.id);
    }
  }

  const encryptedMaximum = byId("encrypted-max-plaintext-67108823");
  assert.equal(
    hostStoredLength("xchacha20poly1305-v1", encryptedMaximum.plaintext_len!),
    encryptedMaximum.stored_len,
  );
  const encryptedPlusOne = byId("encrypted-plaintext-plus-1");
  assert.equal(encryptedPlusOne.stored_len, bounds.max_stored_object_bytes + 1);
  assert.equal(encryptedPlusOne.pre_encryption_rejection, true);
  assert.throws(
    () => hostStoredLength("xchacha20poly1305-v1", encryptedPlusOne.plaintext_len!),
    { code: encryptedPlusOne.expected_error },
  );

  // The provider has no plaintext or encryption-mode authority. Its independent Rust contract test
  // consumes only this stored length and rejects the same plus-one boundary.
  assert.equal(encryptedPlusOne.stored_len, encryptedPlusOne.plaintext_len! + 41);
});
