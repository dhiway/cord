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
import { accountId, hash32 } from "@cord-network/origin-sdk-identity";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
  createPersonhoodClient,
  PERSONHOOD_ADMIN_EXCLUSIONS,
  type PersonhoodRuntimeAdapter,
} from "../src/index.ts";

const blockHash = `0x${"33".repeat(32)}` as const;
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

test("personhood profile uses one finalized snapshot for both reads", async () => {
  let finalizedCalls = 0;
  const seen: string[] = [];
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: blockHash, number: 9n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const adapter: PersonhoodRuntimeAdapter = {
    async personhoodStatus(at) { seen.push(at); return { version: 1, value: { full_personal_id: "7", full_recognized: true, lite_recognized: true } }; },
    async attestationAllowance(at) { seen.push(at); return { version: 1, value: { remaining: 2 } }; },
    async attestLitePerson(at) { seen.push(at); return transaction; },
  };
  const client = createPersonhoodClient(chain, adapter);
  const profile = await client.profile(accountId("5Person"));
  assert.equal(profile.success, true);
  assert.equal(finalizedCalls, 1);
  assert.deepEqual(seen, [blockHash, blockHash]);

  const prepared = await client.prepareAttestLitePerson({
    candidate: accountId("5Candidate"),
    candidate_signature: { scheme: "sr25519", bytes: `0x${"44".repeat(64)}` },
    ring_vrf_key: hash32(`0x${"55".repeat(32)}`),
    proof_of_ownership: `0x${"66".repeat(64)}` as any,
  });
  assert.equal(prepared.success, true);
  assert.equal(seen.at(-1), blockHash);
});

test("personhood package excludes governance and unrestricted dispatch", () => {
  assert.ok(PERSONHOOD_ADMIN_EXCLUSIONS.some(({ target }) => target === "PeopleLite.dispatch_as_signer"));
  assert.ok(PERSONHOOD_ADMIN_EXCLUSIONS.every(({ reason }) => reason.length > 0));
});
