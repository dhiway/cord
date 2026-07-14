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

import { asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err, ok } from "@cord-network/origin-sdk-result";
import { validateCommonsRuntime, type RuntimeIdentity } from "@cord-network/origin-sdk-descriptors";
export interface FinalizedBlock { readonly hash:`0x${string}`; readonly number:bigint; }
export interface CommonsProvider { finalizedBlock(signal?:AbortSignal):Promise<FinalizedBlock>; runtimeIdentity(at:`0x${string}`,signal?:AbortSignal):Promise<RuntimeIdentity>; disconnect():Promise<void>; }
export interface FinalizedSnapshot { readonly block:FinalizedBlock; read<T>(query:(at:`0x${string}`)=>Promise<T>,signal?:AbortSignal):Promise<SdkResult<T>>; }
export interface CommonsChainClient { finalizedSnapshot(signal?:AbortSignal):Promise<SdkResult<FinalizedSnapshot>>; readFinalized<T>(query:(at:`0x${string}`)=>Promise<T>,signal?:AbortSignal):Promise<SdkResult<T>>; disconnect():Promise<void>; }
export function createCommonsChainClient(provider:CommonsProvider):CommonsChainClient{const snapshot=async(signal?:AbortSignal):Promise<SdkResult<FinalizedSnapshot>>=>{try{const block=await provider.finalizedBlock(signal);const identity=validateCommonsRuntime(await provider.runtimeIdentity(block.hash,signal));if(!identity.success)return identity;return ok({block,async read(query,readSignal){try{return ok(await query(block.hash))}catch(error){return err(asSdkError(error,{source:"chain-client",domain:"query",code:"query_failed",retryable:true}))}}})}catch(error){return err(asSdkError(error,{source:"chain-client",domain:"transport",code:signal?.aborted?"cancelled":"unavailable",retryable:!signal?.aborted}))}};return{finalizedSnapshot:snapshot,async readFinalized(query,signal){const value=await snapshot(signal);return value.success?value.value.read(query,signal):value},disconnect:()=>provider.disconnect()}}
