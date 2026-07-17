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
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import {
	NAMES_ADMIN_EXCLUSIONS,
	blockNumber,
	contentCommitment,
	createNamesClient,
	nameId,
	normalizedLabel,
	operationId,
  registrationCommitment,
  registrationSalt,
  type NamesRuntimeAdapter,
} from "../src/index.ts";

const finalizedHash = `0x${"88".repeat(32)}` as const;
const transaction: PreparedTransaction = { async *signSubmitAndWatch() {} };
const runtimeIdentity = {
  genesis_hash: COMMONS_NETWORK_BINDING.genesis_hash,
  spec_version: COMMONS_NETWORK_BINDING.spec_version,
  transaction_version: COMMONS_NETWORK_BINDING.transaction_version,
  metadata_hash: COMMONS_NETWORK_BINDING.metadata_hash,
  descriptor_contract_sha256: COMMONS_NETWORK_BINDING.descriptor_contract_sha256,
  chain_spec_source_sha256: COMMONS_NETWORK_BINDING.chain_spec_source_sha256,
};

test("names reads and writes use the verified finalized hash", async () => {
  const seen: string[] = [];
  const chain = createCommonsChainClient({
    async finalizedBlock() { return { hash: finalizedHash, number: 13n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const runtime = {
    async nameById(at: string) { seen.push(at); return { version: 1, value: null }; },
    async register(at: string) { seen.push(at); return transaction; },
  } as unknown as NamesRuntimeAdapter;
  const client = createNamesClient(chain, runtime);
  const name = nameId(`0x${"99".repeat(32)}`);
  const read = await client.nameById(name);
  const prepared = await client.prepareRegister(null, normalizedLabel("festival"), registrationSalt("secret"));
  assert.equal(read.success, true);
  assert.equal(prepared.success, true);
  assert.deepEqual(seen, [finalizedHash, finalizedHash]);
});

test("invalid names input fails before finality and administration stays excluded", async () => {
  let finalizedCalls = 0;
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: finalizedHash, number: 13n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const client = createNamesClient(chain, {} as NamesRuntimeAdapter);
  const result = await client.preparePruneExpiredCommitment(accountId("5Owner"), "bad" as ReturnType<typeof registrationCommitment>);
  assert.equal(result.success, false);
  assert.equal(finalizedCalls, 0);
  assert.deepEqual(NAMES_ADMIN_EXCLUSIONS.map(({ target }) => target), [
    "Names.reserve_name",
    "Names.clear_reservation",
    "Names.set_label_protection",
    "Names.set_paused",
    "Names.force_transfer",
    "Names.force_revoke",
    "Names.set_registrar",
  ]);
});

test("content publication forwards the caller deadline before the random operation id", async () => {
	const seen: unknown[][] = [];
	const chain = createCommonsChainClient({
		async finalizedBlock() { return { hash: finalizedHash, number: 13n }; },
		async runtimeIdentity() { return runtimeIdentity; },
		async disconnect() {},
	});
	const runtime = {
		async publishContent(...args: unknown[]) { seen.push(args); return transaction; },
	} as unknown as NamesRuntimeAdapter;
	const name = nameId(`0x${"99".repeat(32)}`);
	const content = contentCommitment(`0x${"77".repeat(32)}`);
	const deadline = blockNumber(20);
	const id = operationId(`0x${"10".repeat(16)}`);
	const prepared = await createNamesClient(chain, runtime)
		.preparePublishContent(name, content, "0", deadline, id);
	assert.equal(prepared.success, true);
	assert.deepEqual(seen, [[finalizedHash, name, content, "0", deadline, id, undefined]]);
});
