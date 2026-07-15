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
import test from "node:test";
import { createHostClient } from "@cord-network/origin-sdk-host";
import { createFakeHost } from "@cord-network/origin-sdk-host/testing";
import {
  bytesCodec,
  createLocalStorage,
  jsonCodec,
  utf8Codec,
} from "../src/index.ts";
import { createFakeLocalStorage } from "../src/testing.ts";

const product = { id: "storage.app", name: "Storage" } as const;

test("typed local storage stays product and namespace scoped", async () => {
  const fake = createFakeHost();
  fake.grant(product.id, "local-storage");
  const host = createHostClient(fake.bridge, product);
  const settings = createLocalStorage(host, "settings");
  const cache = createLocalStorage(host, "cache");

  assert.equal((await settings.set("theme", "dark", utf8Codec)).success, true);
  assert.equal((await settings.get("theme", utf8Codec)).success && (await settings.get("theme", utf8Codec)).value, "dark");
  assert.equal((await cache.get("theme", utf8Codec)).success && (await cache.get("theme", utf8Codec)).value, undefined);
  assert.equal((await settings.delete("theme")).success, true);
  assert.equal((await settings.get("theme", utf8Codec)).success && (await settings.get("theme", utf8Codec)).value, undefined);
});

test("codecs clone bytes and reject invalid persisted JSON", async () => {
  const storage = createFakeLocalStorage(product);
  const source = new Uint8Array([1, 2]);
  await storage.set("bytes", source, bytesCodec);
  source[0] = 9;
  const stored = await storage.get("bytes", bytesCodec);
  assert.deepEqual(stored.success && stored.value, new Uint8Array([1, 2]));

  await storage.set("record", "not-an-object", utf8Codec);
  const objectCodec = jsonCodec<{ enabled: boolean }>(
    (value): value is { enabled: boolean } =>
      typeof value === "object" && value !== null && typeof (value as { enabled?: unknown }).enabled === "boolean",
  );
  const decoded = await storage.get("record", objectCodec);
  assert.equal(!decoded.success && decoded.error.code, "decode_failed");
});

test("invalid keys and missing permissions return typed failures", async () => {
  const fake = createFakeHost();
  const storage = createLocalStorage(createHostClient(fake.bridge, product), "app");
  const invalid = await storage.set("bad key", "x", utf8Codec);
  assert.equal(!invalid.success && invalid.error.code, "invalid_key");
  const denied = await storage.get("key", utf8Codec);
  assert.equal(!denied.success && denied.error.code, "permission_denied");
});
