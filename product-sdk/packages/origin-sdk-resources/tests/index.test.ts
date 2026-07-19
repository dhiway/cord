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
import { accountId } from "@cord-network/origin-sdk-identity";
import type { PersonhoodReadAdapter } from "@cord-network/origin-sdk-personhood";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  communicationIdentifier,
  createResourcesClient,
  membershipProof,
  RESOURCES_ADMIN_EXCLUSIONS,
  type ResourcesRuntimeAdapter,
} from "../src/index.ts";

const blockHash = `0x${"77".repeat(32)}` as const;
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

test("resource profile pins personhood, consumer, and allowance reads to one block", async () => {
  let finalizedCalls = 0;
  const seen: string[] = [];
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: blockHash, number: 11n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const personhood: PersonhoodReadAdapter = {
    async personhoodStatus(at) { seen.push(at); return { version: 1, value: { full_personal_id: null, full_recognized: false, lite_recognized: true } }; },
    async attestationAllowance(at) { seen.push(at); return { version: 1, value: { remaining: 1 } }; },
  };
  const runtime: ResourcesRuntimeAdapter = {
    async consumer(at) { seen.push(at); return { identifier_key: communicationIdentifier(`0x${"01".repeat(65)}`), credibility: { kind: "lite" } }; },
    async statementAllowances(at) { seen.push(at); return []; },
    async storageClaim(at) { seen.push(at); return null; },
    async registerLitePerson(at) { seen.push(at); return transaction; },
    async registerPerson(at) { seen.push(at); return transaction; },
    async touchPersonAuthorization(at) { seen.push(at); return transaction; },
    async updateIdentifierKey(at) { seen.push(at); return transaction; },
    async setStatementAllowance(at) { seen.push(at); return transaction; },
    async claimLongTermStorage(at) { seen.push(at); return transaction; },
    async cancelLongTermStorage(at) { seen.push(at); return transaction; },
  };
  const client = createResourcesClient(chain, runtime, personhood);
  const profile = await client.profile(accountId("5Festival"));
  assert.equal(profile.success, true);
  assert.equal(finalizedCalls, 1);
  assert.deepEqual(seen, [blockHash, blockHash, blockHash, blockHash]);

  const prepared = await client.prepareSetStatementAllowance({
    period: 20_000,
    sequence: 1,
    target_account: accountId("5Statements"),
    authorization: { kind: "as-resources", proof: membershipProof("0x0102"), ring_index: 3, collection: "lite-people" },
  });
  assert.equal(prepared.success, true);
  assert.equal(seen.at(-1), blockHash);
});

test("resource proof validation fails before finality and maintenance calls stay excluded", async () => {
  let finalizedCalls = 0;
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: blockHash, number: 11n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const runtime = {} as ResourcesRuntimeAdapter;
  const personhood = {} as PersonhoodReadAdapter;
  const client = createResourcesClient(chain, runtime, personhood);
  const result = await client.prepareSetStatementAllowance({
    period: -1,
    sequence: 0,
    target_account: accountId("5Statements"),
    authorization: { kind: "as-resources", proof: "not-hex" as any, ring_index: 0, collection: "people" },
  });
  assert.equal(result.success, false);
  assert.equal(finalizedCalls, 0);
  assert.ok(RESOURCES_ADMIN_EXCLUSIONS.some(({ target }) => target === "Resources.expire_long_term_storage_reservations"));
});
