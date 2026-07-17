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
import { CLOUD_STORAGE_ADMIN_EXCLUSIONS, CLOUD_STORAGE_NATIVE_BINDINGS, createCloudStorageClient, providerRequests, driveRequests, s3Requests, digestContent, decimalU64, objectKey, objectKeyPrefix, type BucketId, type CloudStorageRuntimeAdapter, type DriveIdPage, type ProviderId } from "../src/index.ts";
const finalizedHash=`0x${"77".repeat(32)}` as const;
const identity={genesis_hash:COMMONS_NETWORK_BINDING.genesis_hash,spec_version:COMMONS_NETWORK_BINDING.spec_version,transaction_version:COMMONS_NETWORK_BINDING.transaction_version,metadata_hash:COMMONS_NETWORK_BINDING.metadata_hash,descriptor_contract_sha256:COMMONS_NETWORK_BINDING.descriptor_contract_sha256,chain_spec_source_sha256:COMMONS_NETWORK_BINDING.chain_spec_source_sha256};
const tx:PreparedTransaction={async *signSubmitAndWatch(){}};
test("cloud storage composite reads and writes pin one verified finalized block",async()=>{const seen:string[]=[];const chain=createCommonsChainClient({finalizedBlock:async()=>({hash:finalizedHash,number:5n}),runtimeIdentity:async()=>identity,disconnect:async()=>{}});const runtime={async read(at,_service,method){seen.push(`${at}:${method}`);return method},async prepare(at,_service,method){seen.push(`${at}:${method}`);return tx}} satisfies CloudStorageRuntimeAdapter;const client=createCloudStorageClient(chain,runtime);const together=await client.readTogether([providerRequests.providerById("5Provider" as ProviderId),driveRequests.nextDriveNonce("5Owner" as never)]);const bucket=`0x${"11".repeat(32)}` as BucketId;const prepared=await client.prepare(s3Requests.putObject(bucket,objectKey("apps/root"),`0x${"22".repeat(32)}` as never,null));assert.equal(together.success,true);assert.equal(prepared.success,true);assert.deepEqual(seen,[`${finalizedHash}:provider_by_id`,`${finalizedHash}:next_drive_nonce`,`${finalizedHash}:put_object`])});
test("storage coverage and portable digest stay frozen",()=>{const pageVersion:DriveIdPage["version"]=8;assert.equal(pageVersion,8);assert.deepEqual(CLOUD_STORAGE_NATIVE_BINDINGS.runtimeApis,["StorageProviderApi.v11","DriveRegistryApi.v2","S3RegistryApi.v3"]);assert.equal(CLOUD_STORAGE_NATIVE_BINDINGS.routeCount,48);assert.equal(CLOUD_STORAGE_NATIVE_BINDINGS.appRouteCount,43);assert.deepEqual(CLOUD_STORAGE_NATIVE_BINDINGS.services,["StorageProvider","Drive","S3"]);assert.equal(Object.keys(providerRequests).length+Object.keys(driveRequests).length+Object.keys(s3Requests).length,CLOUD_STORAGE_NATIVE_BINDINGS.appRouteCount);assert.equal(CLOUD_STORAGE_ADMIN_EXCLUSIONS.length,5);assert.equal(digestContent("blake2b-256",new Uint8Array()).length,32)});
test("storage reads expose only bucket checkpoints and snapshot-bound object listing",()=>{const bucket=`0x${"11".repeat(32)}` as BucketId;assert.deepEqual(providerRequests.bucketCheckpoint(bucket),{kind:"read",service:"provider",method:"bucket_checkpoint",payload:{bucket}});for(const retired of["ownerAgreements","containerAgreements","openChallengeCount","providerCheckpoint","providerRoot","deletionAcknowledgement"])assert.equal(retired in providerRequests,false);assert.deepEqual(s3Requests.bucketObjectKeys(bucket,{prefix:objectKeyPrefix("images/"),cursor:{snapshot_version:decimalU64(7),last_key:objectKey("images/a")},limit:25}).payload,{bucket,prefix:"images/",cursor:{snapshot_version:"7",last_key:"images/a"},limit:25});assert.throws(()=>s3Requests.bucketObjectKeys(bucket,{limit:0}),/between 1 and 100/)});
