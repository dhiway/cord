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
  ATTESTATION_ADMIN_EXCLUSIONS,
  createAttestationClient,
  schemaDefinition,
  schemaId,
  type AttestationRuntimeAdapter,
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

test("attestation reads and writes use the verified finalized hash", async () => {
  const seen: string[] = [];
  const chain = createCommonsChainClient({
    async finalizedBlock() { return { hash: finalizedHash, number: 13n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const runtime = {
    async schemaById(at: string) { seen.push(at); return { version: 1, value: null }; },
    async createSchema(at: string) { seen.push(at); return transaction; },
  } as unknown as AttestationRuntimeAdapter;
  const client = createAttestationClient(chain, runtime);
  const schema = schemaId(`0x${"99".repeat(32)}`);
  const read = await client.schemaById(schema);
  const prepared = await client.prepareCreateSchema({
    definition: schemaDefinition('{"type":"object"}'),
    authorized_issuers: [accountId("5Issuer")],
    revocable: true,
    unique: false,
    index_policy: "issuer",
  });
  assert.equal(read.success, true);
  assert.equal(prepared.success, true);
  assert.deepEqual(seen, [finalizedHash, finalizedHash]);
});

test("attestation batches fail before finality and administration stays excluded", async () => {
  let finalizedCalls = 0;
  const chain = createCommonsChainClient({
    async finalizedBlock() { finalizedCalls++; return { hash: finalizedHash, number: 13n }; },
    async runtimeIdentity() { return runtimeIdentity; },
    async disconnect() {},
  });
  const client = createAttestationClient(chain, {} as AttestationRuntimeAdapter);
  const result = await client.prepareIssueBatch([]);
  assert.equal(result.success, false);
  assert.equal(finalizedCalls, 0);
  assert.deepEqual(ATTESTATION_ADMIN_EXCLUSIONS.map(({ target }) => target), [
    "Attestation.set_emergency_pause",
    "Attestation.force_schema_status",
    "Attestation.force_revoke",
  ]);
});
