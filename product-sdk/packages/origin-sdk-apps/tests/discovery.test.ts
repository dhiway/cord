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
import { contentCommitment, nameId } from "@cord-network/origin-sdk-names";
import { ok } from "@cord-network/origin-sdk-result";
import {
  createMemoryOriginAppDiscoveryTransport,
  createOriginAppDiscoveryClient,
  type OriginAppsClient,
  type ResolvedOriginApp,
} from "../src/index.ts";

test("discovery is reconstructible metadata and revalidates native authority", async () => {
  const name = nameId(`0x${"22".repeat(32)}`);
  const commitment = contentCommitment(`0x${"33".repeat(32)}`);
  let current = commitment;
  const resolved = {
    manifestCommitment: commitment,
    manifest: { product: { id: "festival.app" }, channel: "stable" },
    finalized: { hash: `0x${"44".repeat(32)}` },
  } as ResolvedOriginApp;
  const apps = {
    async resolveApp() { return ok({ ...resolved, manifestCommitment: current }); },
  } as OriginAppsClient;
  const discovery = createOriginAppDiscoveryClient(apps, createMemoryOriginAppDiscoveryTransport());
  const published = await discovery.publish(name, { displayName: "Festival", categories: ["events"] });
  assert.equal(published.success, true);
  assert.equal((await discovery.search({ text: "fest" })).success, true);

  current = contentCommitment(`0x${"55".repeat(32)}`);
  const stale = await discovery.search({ categories: ["events"] });
  assert.equal(stale.success && stale.value.length, 0);
  assert.equal((await discovery.retract(name)).success, true);
});
