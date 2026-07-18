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
import { ok } from "@cord-network/origin-sdk-result";
import {
  TRUAPI_PROTOCOL,
  TRUAPI_VERSION,
  createHostChainProvider,
  createHostClient,
  createTruApiBridge,
  type TruApiRequest,
} from "../src/index.ts";
import { createFakeHost } from "../src/testing.ts";

const product = { id: "festival.app", name: "Festival" } as const;

test("host client keeps permissions and per-call signing approval host-owned", async () => {
  const fake = createFakeHost({ accounts: [{ address: "5Test" }] });
  const host = createHostClient(fake.bridge, product);
  assert.equal((await host.accounts()).success, false);
  fake.grant(product.id, "accounts");
  assert.deepEqual((await host.accounts()).success && (await host.accounts()).value, [{ address: "5Test" }]);
  fake.grant(product.id, "signing");
  fake.rejectNextSignature();
  const rejected = await host.sign({
    account: "5Test", payload: new Uint8Array([1]), purpose: "check-in",
  });
  assert.equal(rejected.success, false);
  assert.equal(!rejected.success && rejected.error.code, "signing_rejected");
  const signed = await host.sign({
    account: "5Test", payload: new Uint8Array([1, 2]), purpose: "check-in",
  });
  assert.equal(signed.success, true);
  fake.revoke(product.id, "signing");
  const revoked = await host.sign({
    account: "5Test", payload: new Uint8Array([1]), purpose: "check-in",
  });
  assert.equal(!revoked.success && revoked.error.code, "permission_revoked");
});

test("permission expiry, cancellation, and host loss are typed failures", async () => {
  const fake = createFakeHost({ now: () => 100 });
  fake.grant(product.id, "accounts", 100);
  const host = createHostClient(fake.bridge, product, { now: () => 100 });
  const expired = await host.accounts();
  assert.equal(!expired.success && expired.error.code, "permission_expired");

  fake.grant(product.id, "accounts", 101);
  const controller = new AbortController();
  controller.abort();
  const cancelled = await host.accounts(controller.signal);
  assert.equal(!cancelled.success && cancelled.error.code, "cancelled");

  fake.lose();
  const unavailable = await host.accounts();
  assert.equal(!unavailable.success && unavailable.error.code, "host_unavailable");
});

test("local storage and chain remain product and permission scoped", async () => {
  const fake = createFakeHost();
  for (const capability of ["local-storage", "chain"] as const) {
    fake.grant(product.id, capability);
  }
  fake.grant("other.app", "local-storage");
  const host = createHostClient(fake.bridge, product);
  const other = createHostClient(fake.bridge, { id: "other.app", name: "Other" });

  assert.equal((await host.setLocal("key", new Uint8Array([7]))).success, true);
  assert.deepEqual((await host.getLocal("key")).success && (await host.getLocal("key")).value, new Uint8Array([7]));
  assert.equal((await other.getLocal("key")).success && (await other.getLocal("key")).value, undefined);
  assert.equal((await host.deleteLocal("key")).success, true);
  assert.equal((await host.getLocal("key")).success && (await host.getLocal("key")).value, undefined);

  assert.equal((await host.finalizedBlock()).success, true);
  const provider = createHostChainProvider(host);
  assert.deepEqual(await provider.finalizedBlock(), { hash: `0x${"1".repeat(64)}`, number: 1n });
  assert.equal((await provider.runtimeIdentity((await provider.finalizedBlock()).hash)).spec_version, 1);
});

test("statement subscriptions are permission-gated and dispose on abort", async () => {
  const fake = createFakeHost();
  fake.grant(product.id, "statements");
  const host = createHostClient(fake.bridge, product);
  const submitted = await host.submitStatement({
    account: "5Test", topics: ["topic"], data: new Uint8Array([9]),
  });
  assert.equal(submitted.success, true);

  const controller = new AbortController();
  const iterator = host.subscribeStatements(
    { kind: "broadcasts", topics: ["topic"] }, controller.signal,
  )[Symbol.asyncIterator]();
  const first = await iterator.next();
  assert.equal(first.done, false);
  assert.equal(first.value?.success && first.value.value.length, 1);
  assert.equal(fake.activeSubscriptions(), 1);
  controller.abort();
  assert.equal((await iterator.next()).done, true);
  assert.equal(fake.activeSubscriptions(), 0);
});

test("TrUAPI bridge owns versioned request and response envelopes", async () => {
  let captured: TruApiRequest | undefined;
  const bridge = createTruApiBridge({
    async request(request) {
      captured = request;
      return ok({
        protocol: TRUAPI_PROTOCOL,
        version: TRUAPI_VERSION,
        requestId: request.requestId,
        value: { productId: product.id, capability: "accounts" },
      });
    },
  }, { requestId: () => "request-1" });
  const host = createHostClient(bridge, product);
  const grant = await host.authorize("accounts");
  assert.equal(grant.success, true);
  assert.deepEqual(captured, {
    protocol: TRUAPI_PROTOCOL,
    version: TRUAPI_VERSION,
    requestId: "request-1",
    product,
    capability: "permissions",
    operation: "authorize",
    payload: { capability: "accounts" },
  });
});

test("TrUAPI bridge rejects mismatched response envelopes", async () => {
  const bridge = createTruApiBridge({
    async request() {
      return ok({
        protocol: TRUAPI_PROTOCOL,
        version: TRUAPI_VERSION,
        requestId: "wrong-request",
        value: { productId: product.id, capability: "accounts" },
      });
    },
  }, { requestId: () => "request-1" });
  const result = await createHostClient(bridge, product).authorize("accounts");
  assert.equal(!result.success && result.error.code, "response_mismatch");
});
