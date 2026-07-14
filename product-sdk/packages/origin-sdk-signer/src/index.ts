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
import { err } from "@cord-network/origin-sdk-result";
import type { HostAccount, OriginHostClient } from "@cord-network/origin-sdk-host";
export interface SignRequest { readonly account:string; readonly payload:Uint8Array; readonly purpose:string; }
export interface OriginSigner { accounts(signal?:AbortSignal):Promise<SdkResult<readonly HostAccount[]>>; sign(request:SignRequest,signal?:AbortSignal):Promise<SdkResult<Uint8Array>>; }
export function createHostSigner(host:OriginHostClient):OriginSigner{return{accounts:(signal)=>host.accounts(signal),sign:(request,signal)=>host.sign(request,signal)}}
export interface InjectedSignerProvider { accounts(signal?:AbortSignal):Promise<SdkResult<readonly HostAccount[]>>; approveAndSign(request:SignRequest,signal?:AbortSignal):Promise<SdkResult<Uint8Array>>; }
export function createInjectedSigner(provider:InjectedSignerProvider):OriginSigner{return{async accounts(signal){try{return await provider.accounts(signal)}catch(error){return err(asSdkError(error,{source:"signer",domain:"accounts",code:"provider_unavailable",retryable:true}))}},async sign(request,signal){try{return await provider.approveAndSign(request,signal)}catch(error){return err(asSdkError(error,{source:"signer",domain:"approval",code:signal?.aborted?"cancelled":"signing_rejected",retryable:false}))}}}}
