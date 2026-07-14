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

import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err, ok } from "@cord-network/origin-sdk-result";
import type { CommonsChainClient, FinalizedSnapshot } from "@cord-network/origin-sdk-chain-client";
import type { OriginSigner } from "@cord-network/origin-sdk-signer";
export type TransactionStatus = {readonly type:"broadcast"}|{readonly type:"best-chain";readonly blockHash:`0x${string}`}|{readonly type:"finalized";readonly blockHash:`0x${string}`;readonly transactionHash:`0x${string}`}|{readonly type:"rejected";readonly code:string;readonly message:string};
export interface PreparedTransaction { signSubmitAndWatch(signer:OriginSigner,signal?:AbortSignal):AsyncIterable<TransactionStatus>; }
export interface FinalizedReceipt { readonly blockHash:`0x${string}`;readonly transactionHash:`0x${string}`; }
export async function prepareAtFinalized(client:CommonsChainClient,builder:(snapshot:FinalizedSnapshot)=>Promise<PreparedTransaction>,signal?:AbortSignal):Promise<SdkResult<PreparedTransaction>>{const snapshot=await client.finalizedSnapshot(signal);if(!snapshot.success)return snapshot;try{return ok(await builder(snapshot.value))}catch(error){return err(asSdkError(error,{source:"tx",domain:"construction",code:"construction_failed",retryable:false}))}}
export async function submitAndFinalize(transaction:PreparedTransaction,signer:OriginSigner,signal?:AbortSignal):Promise<SdkResult<FinalizedReceipt>>{try{for await(const status of transaction.signSubmitAndWatch(signer,signal)){if(signal?.aborted)return err(new OriginSdkError({source:"tx",domain:"lifecycle",code:"cancelled",message:"Transaction cancelled"}));if(status.type==="rejected")return err(new OriginSdkError({source:"tx",domain:"runtime",code:status.code,message:status.message}));if(status.type==="finalized")return ok({blockHash:status.blockHash,transactionHash:status.transactionHash})}return err(new OriginSdkError({source:"tx",domain:"lifecycle",code:"stream_closed",message:"Transaction stream closed before finalization",retryable:true}))}catch(error){return err(asSdkError(error,{source:"tx",domain:"transport",code:signal?.aborted?"cancelled":"submission_failed",retryable:!signal?.aborted}))}}
export const composeBatch=<T>(transactions:readonly T[],compose:(items:readonly T[])=>PreparedTransaction):SdkResult<PreparedTransaction>=>transactions.length===0?err(new OriginSdkError({source:"tx",domain:"construction",code:"empty_batch",message:"A transaction batch cannot be empty"})):ok(compose(transactions));
