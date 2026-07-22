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

import type { CommonsChainClient } from "@cord-network/origin-sdk-chain-client";
import { COMMONS_NETWORK_BINDING } from "@cord-network/origin-sdk-descriptors";
import type { SdkResult } from "@cord-network/origin-sdk-errors";
import { ok } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";
import type { CloudStorageReadRequest, CloudStorageWriteRequest, StorageService } from "./requests.ts";

export interface CloudStorageRuntimeAdapter {
  read<T>(at:`0x${string}`,service:StorageService,method:string,payload:Readonly<Record<string,unknown>>,signal?:AbortSignal):Promise<T>;
  prepare(at:`0x${string}`,service:StorageService,method:string,payload:Readonly<Record<string,unknown>>,signal?:AbortSignal):Promise<PreparedTransaction>;
}
export interface PinnedCloudStorageResult { readonly finalized_hash:`0x${string}`; readonly finalized_number:bigint; readonly values:readonly unknown[] }
export interface CloudStorageClient {
  read<T=unknown>(request:CloudStorageReadRequest,signal?:AbortSignal):Promise<SdkResult<T>>;
  readTogether(requests:readonly CloudStorageReadRequest[],signal?:AbortSignal):Promise<SdkResult<PinnedCloudStorageResult>>;
  prepare(request:CloudStorageWriteRequest,signal?:AbortSignal):Promise<SdkResult<PreparedTransaction>>;
}
export const CLOUD_STORAGE_NATIVE_BINDINGS={metadataHash:COMMONS_NETWORK_BINDING.metadata_hash,runtimeApis:["StorageProviderApi.v10","DriveRegistryApi.v2","S3RegistryApi.v3"],routeCount:66,appRouteCount:61,services:["TransactionStorage","StorageProvider","Drive","S3"]} as const;
export const CLOUD_STORAGE_ADMIN_EXCLUSIONS=[
 {target:"StorageProvider.register_provider",reason:"provider registry administration"},
 {target:"StorageProvider.update_provider",reason:"provider registry administration"},
 {target:"StorageProvider.set_provider_status",reason:"provider registry administration"},
 {target:"StorageProvider.remove_provider",reason:"provider registry administration"},
 {target:"StorageProvider.issue_challenge",reason:"provider audit administration"},
] as const;
export function createCloudStorageClient(chain:CommonsChainClient,runtime:CloudStorageRuntimeAdapter):CloudStorageClient{return{
 read:(request,signal)=>chain.readFinalized(at=>runtime.read(at,request.service,request.method,request.payload,signal),signal),
 async readTogether(requests,signal){const snapshot=await chain.finalizedSnapshot(signal);if(!snapshot.success)return snapshot;const values=[];for(const request of requests){const value=await snapshot.value.read(at=>runtime.read(at,request.service,request.method,request.payload,signal),signal);if(!value.success)return value;values.push(value.value)}return ok({finalized_hash:snapshot.value.block.hash,finalized_number:snapshot.value.block.number,values})},
 prepare:(request,signal)=>prepareAtFinalized(chain,({block})=>runtime.prepare(block.hash,request.service,request.method,request.payload,signal),signal),
}}
