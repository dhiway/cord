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
import type { ProductIdentity } from "@cord-network/origin-sdk-host";
import { accountId } from "@cord-network/origin-sdk-identity";
import { createFakeStatementTransport } from "../src/testing.ts";
import { statementTopic } from "../src/index.ts";

const product: ProductIdentity = { id: "festival.app", name: "Festival" };
const topic = statementTopic(`0x${"11".repeat(32)}`);
const query = { kind: "broadcasts", topics: [topic] } as const;

test("fake statement transport records, publishes, streams, and disposes", async () => {
  const fake = createFakeStatementTransport();
  const signal = new AbortController();
  const stream = fake.subscribe(product, query, signal.signal)[Symbol.asyncIterator]();
  await stream.next();
  assert.equal(fake.activeSubscriptions(), 1);

  const draft = { account: accountId("5Festival"), topics: [topic], data: new Uint8Array([1]) };
  const nextUpdate = stream.next();
  await fake.submit(product, draft);
  const update = await nextUpdate;
  assert.equal(update.value?.length, 1);
  assert.equal((await fake.query(product, query)).length, 1);
  assert.deepEqual(fake.calls.map(({ operation }) => operation), ["subscribe", "submit", "query"]);

  signal.abort();
  await stream.next();
  assert.equal(fake.activeSubscriptions(), 0);
  fake.reset();
  assert.equal(fake.records.length, 0);
});
