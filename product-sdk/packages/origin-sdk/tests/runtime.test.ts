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
import { paymentOptions } from "@cord-network/origin-sdk-assets";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  ORIGIN_RUNTIME_EXECUTOR_CONTRACT,
  createOriginAppRuntime,
  type CommonsRuntimeExecutor,
} from "../src/index.ts";

const at = `0x${"11".repeat(32)}` as const;
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };

test("one descriptor executor generates every native application adapter", async () => {
  const seen: Array<{ kind: string; target: string; payload: unknown; context?: unknown }> = [];
  const executor = {
    async read(_at, target, payload) {
      seen.push({ kind: "read", target, payload });
      return { target };
    },
    async prepare(_at, target, payload, context) {
      seen.push({ kind: "prepare", target, payload, context });
      return transaction;
    },
  } satisfies CommonsRuntimeExecutor;
  const runtime = createOriginAppRuntime(executor);

  await runtime.identity.identityStatus(at, "5Account");
  await runtime.attestation.schemaCount(at);
  await runtime.names.resolveContentPublication(at, `0x${"22".repeat(32)}`);
  await runtime.storage.read(at, "storage", "account_authorization", { account: "5Account" });
  const payment = paymentOptions();
  await runtime.assets.prepare(at, "Assets.transfer", { id: 1 }, payment);

  assert.deepEqual(seen.map(({ kind, target }) => `${kind}:${target}`), [
    "read:IdentityPersonhoodApi.identity_status",
    "read:AttestationApi.schema_count",
    "read:NamesApi.resolve_content_publication",
    "read:storage.account_authorization",
    "prepare:Assets.transfer",
  ]);
  assert.deepEqual(seen[4]?.context, { payment });
  assert.equal(ORIGIN_RUNTIME_EXECUTOR_CONTRACT.adapterCount, 5);
  assert.equal(ORIGIN_RUNTIME_EXECUTOR_CONTRACT.rawScaleAccepted, false);
  assert.equal(ORIGIN_RUNTIME_EXECUTOR_CONTRACT.palletIndicesAccepted, false);
  assert.equal(ORIGIN_RUNTIME_EXECUTOR_CONTRACT.applicationEndpointsAccepted, false);
});
