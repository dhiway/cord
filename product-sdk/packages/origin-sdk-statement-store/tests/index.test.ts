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
import { accountId } from "@cord-network/origin-sdk-identity";
import type { ResourcesClient } from "@cord-network/origin-sdk-resources";
import { ok } from "@cord-network/origin-sdk-result";
import {
  STATEMENT_STORE_EXCLUSIONS,
  createHostStatementStoreTransport,
  createStatementStoreClient,
  statementHash,
  statementTopic,
  type StatementStoreTransport,
} from "../src/index.ts";

const product = { id: "festival.app", name: "Festival" };
const account = accountId("5Festival");
const topic = statementTopic(`0x${"11".repeat(32)}`);
const hash = statementHash(`0x${"22".repeat(32)}`);
const resources = { statementAllowances: async () => ok([]) } as unknown as ResourcesClient;

test("statement operations require host permission and clone application bytes", async () => {
  const fake = createFakeHost();
  fake.grant(product.id, "statements");
  const seen: Uint8Array[] = [];
  const transport = {
    async submit(_product, draft) { seen.push(draft.data); draft.data[0] = 99; return hash; },
    async query() { return [{ hash, account, topics: [topic], data: Uint8Array.of(7) }]; },
    async *subscribe() {},
  } satisfies StatementStoreTransport;
  const client = createStatementStoreClient(createHostClient(fake.bridge, product), transport, resources);
  const data = Uint8Array.of(1, 2, 3);
  const submitted = await client.submit({ account, topics: [topic], data });
  const queried = await client.query({ kind: "broadcasts", topics: [topic] });
  assert.equal(submitted.success, true);
  assert.equal(data[0], 1);
  assert.notEqual(seen[0], data);
  assert.equal(queried.success && queried.value[0]?.data[0], 7);
});

test("invalid data fails before permission and unsafe RPCs stay excluded", async () => {
  const fake = createFakeHost();
  const client = createStatementStoreClient(createHostClient(fake.bridge, product), {} as StatementStoreTransport, resources);
  const result = await client.submit({ account, topics: [topic], data: new Uint8Array() });
  assert.equal(result.success, false);
  assert.equal(result.success ? "" : result.error.code, "invalid_input");
  assert.deepEqual(STATEMENT_STORE_EXCLUSIONS.map(({ target }) => target), ["statement_dump", "statement_remove"]);
});

test("subscriptions are permission-gated and disposable", async () => {
  const fake = createFakeHost(); fake.grant(product.id, "statements");
  const controller = new AbortController();
  const transport = {
    async submit() { return hash; }, async query() { return []; },
    async *subscribe() { yield [{ hash, account, topics: [topic], data: Uint8Array.of(1) }]; controller.abort(); },
  } satisfies StatementStoreTransport;
  const client = createStatementStoreClient(createHostClient(fake.bridge, product), transport, resources);
  const updates = [];
  for await (const update of client.subscribe({ kind: "broadcasts", topics: [topic] }, controller.signal)) updates.push(update);
  assert.equal(updates.length, 1);
  assert.equal(updates[0]?.success, true);
  assert.equal(controller.signal.aborted, true);
});


test("host statement adapter uses only the active product-scoped host transport", async () => {
  const fake = createFakeHost();
  fake.grant(product.id, "statements");
  const host = createHostClient(fake.bridge, product);
  const transport = createHostStatementStoreTransport(host);
  const submitted = await transport.submit(product, {
    account,
    topics: [topic],
    data: Uint8Array.of(4),
  });
  const records = await transport.query(product, { kind: "broadcasts", topics: [topic] });
  assert.match(submitted, /^0x[0-9a-f]{64}$/);
  assert.equal(records.length, 1);
  assert.equal(records[0]?.topics[0], topic);
  await assert.rejects(
    () => transport.query({ id: "other.app", name: "Other" }, { kind: "broadcasts", topics: [] }),
    /product does not match/,
  );
});
