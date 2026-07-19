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
import { createCommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  accountId,
  createIdentityClient,
  hash32,
  IDENTITY_ADMIN_EXCLUSIONS,
  type IdentityInfo,
  type IdentityRuntimeAdapter,
} from "../src/index.ts";

const blockHash = `0x${"11".repeat(32)}` as const;
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};
const info: IdentityInfo = {
  display: { kind: "raw", value: "Foundation" },
  legal: { kind: "none" },
  web: { kind: "none" },
  email: { kind: "none" },
  image: { kind: "blake2_256", hash: hash32(`0x${"22".repeat(32)}`) },
  additional: [],
};

function fixture() {
  let finalizedCalls = 0;
  const seen: string[] = [];
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: blockHash, number: 7n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const adapter: IdentityRuntimeAdapter = {
    async identityStatus(at) { seen.push(at); return { version: 1, value: { registered: true, judgement_count: 0, requested: 0, reasonable: 0, known_good: 0, out_of_date: 0, low_quality: 0, erroneous: 0 } }; },
    async setIdentity(at) { seen.push(at); return transaction; },
    async clearIdentity(at) { seen.push(at); return transaction; },
    async requestJudgement(at) { seen.push(at); return transaction; },
    async cancelJudgementRequest(at) { seen.push(at); return transaction; },
    async provideJudgement(at) { seen.push(at); return transaction; },
  };
  return { client: createIdentityClient(chain, adapter), seen, finalizedCalls: () => finalizedCalls };
}

test("identity reads and transactions bind to a verified finalized hash", async () => {
  const { client, seen } = fixture();
  const status = await client.status(accountId("5Foundation"));
  const prepared = await client.prepareSetIdentity(info);
  assert.equal(status.success, true);
  assert.equal(prepared.success, true);
  assert.deepEqual(seen, [blockHash, blockHash]);
});

test("identity validation fails before querying a finalized block", async () => {
  const { client, finalizedCalls } = fixture();
  const invalid = await client.prepareSetIdentity({ ...info, display: { kind: "raw", value: "x".repeat(33) } });
  assert.equal(invalid.success, false);
  assert.equal(finalizedCalls(), 0);
});

test("identity package explicitly excludes administrative People calls", () => {
  assert.deepEqual(IDENTITY_ADMIN_EXCLUSIONS.map(({ target }) => target), [
    "People.add_registrar",
    "People.kill_identity",
    "People.add_username_authority",
    "People.remove_username_authority",
    "People.remove_registrar",
  ]);
});
