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
import { CLOUD_STORAGE_ADMIN_EXCLUSIONS, CLOUD_STORAGE_NATIVE_BINDINGS, createCloudStorageClient, storageRequests, providerRequests, driveRequests, s3Requests, digestContent, type CloudStorageRuntimeAdapter } from "../src/index.ts";
const finalizedHash=`0x${"77".repeat(32)}` as const;
const identity={genesis_hash:COMMONS_NETWORK_BINDING.genesis_hash,spec_version:COMMONS_NETWORK_BINDING.spec_version,transaction_version:COMMONS_NETWORK_BINDING.transaction_version,metadata_hash:COMMONS_NETWORK_BINDING.metadata_hash,descriptor_contract_sha256:COMMONS_NETWORK_BINDING.descriptor_contract_sha256,chain_spec_source_sha256:COMMONS_NETWORK_BINDING.chain_spec_source_sha256};
const tx:PreparedTransaction={async *signSubmitAndWatch(){}};
test("cloud storage composite reads and writes pin one verified finalized block",async()=>{const seen:string[]=[];const chain=createCommonsChainClient({finalizedBlock:async()=>({hash:finalizedHash,number:5n}),runtimeIdentity:async()=>identity,disconnect:async()=>{}});const runtime={async read(at,_service,method){seen.push(`${at}:${method}`);return method},async prepare(at,_service,method){seen.push(`${at}:${method}`);return tx}} satisfies CloudStorageRuntimeAdapter;const client=createCloudStorageClient(chain,runtime);const together=await client.readTogether([storageRequests.accountAuthorization("5Owner"),driveRequests.nextDriveNonce("5Owner")]);const prepared=await client.prepare(storageRequests.store("YQ=="));assert.equal(together.success,true);assert.equal(prepared.success,true);assert.deepEqual(seen,[`${finalizedHash}:account_authorization`,`${finalizedHash}:next_drive_nonce`,`${finalizedHash}:store`])});
test("storage coverage and portable digest stay frozen",()=>{assert.equal(CLOUD_STORAGE_NATIVE_BINDINGS.routeCount,71);assert.equal(CLOUD_STORAGE_NATIVE_BINDINGS.appRouteCount,66);assert.equal(Object.keys(storageRequests).length+Object.keys(providerRequests).length+Object.keys(driveRequests).length+Object.keys(s3Requests).length,66);assert.equal(CLOUD_STORAGE_ADMIN_EXCLUSIONS.length,5);assert.equal(digestContent("blake2b-256",new Uint8Array()).length,32)});
