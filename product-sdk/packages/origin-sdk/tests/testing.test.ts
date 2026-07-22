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
import { assetWrites } from "@cord-network/origin-sdk-assets";
import { utf8Codec } from "@cord-network/origin-sdk-local-storage";
import { createFakeApp } from "../src/testing.ts";

test("createFakeApp round-trips host storage and records prepared native writes", async () => {
  const app = await createFakeApp();
  assert.equal((await app.storage.set("theme", "dark", utf8Codec)).success, true);
  const theme = await app.storage.get("theme", utf8Codec);
  assert.equal(theme.success && theme.value, "dark");

  const prepared = await app.assets.prepare(assetWrites.transfer(1, "5Bob", 10));
  assert.equal(prepared.success, true);
  assert.equal(app.testing.prepared[0]?.target, "Assets.transfer");

  const read = await app.assets.read({ target: "Assets.Asset", payload: { id: 1 } });
  assert.equal(!read.success && read.error.code, "unconfigured_chain_read");
  assert.equal((await app.close()).success, true);
});

test("createFakeApp accepts native domain overrides", async () => {
  const names = { resolveContent: async () => ({ success: true, value: { value: null } }) } as never;
  const app = await createFakeApp({ overrides: { names } });
  assert.equal(app.names, names);
  await app.close();
});
