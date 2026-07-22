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
import { OriginSdkError, asSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err, ok } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

declare const contractType: unique symbol;
export type ReviveAddress = string & { readonly [contractType]: "ReviveAddress" };
export type Selector = string & { readonly [contractType]: "Selector" };

export interface ContractCodec<T> {
  encode(value: T): Uint8Array;
  decode(bytes: Uint8Array): T;
}

export interface ContractMessage<Input, Output> {
  readonly name: string;
  readonly selector: Selector;
  readonly mutates: boolean;
  readonly input: ContractCodec<Input>;
  readonly output: ContractCodec<Output>;
}

export interface AppContractDefinition<Messages extends Record<string, ContractMessage<unknown, unknown>>> {
  readonly name: string;
  readonly domain: string;
  readonly messages: Messages;
}

export interface ReviveDryRunResult {
  readonly reverted: boolean;
  readonly data: Uint8Array;
  readonly gasRequired: bigint;
  readonly storageDeposit: bigint;
  readonly debugMessage?: string;
}

export interface ReviveRuntimeAdapter {
  dryRun(at:`0x${string}`, request:{readonly address:ReviveAddress;readonly input:Uint8Array;readonly value:bigint}, signal?:AbortSignal):Promise<ReviveDryRunResult>;
  prepareCall(at:`0x${string}`, request:{readonly address:ReviveAddress;readonly input:Uint8Array;readonly value:bigint;readonly gasLimit:bigint;readonly storageDepositLimit:bigint|null}, signal?:AbortSignal):Promise<PreparedTransaction>;
  prepareInstantiate(at:`0x${string}`, request:{readonly code:Uint8Array;readonly constructorData:Uint8Array;readonly value:bigint;readonly gasLimit:bigint;readonly storageDepositLimit:bigint|null;readonly salt:Uint8Array}, signal?:AbortSignal):Promise<PreparedTransaction>;
}

export const FORBIDDEN_NATIVE_CONTRACT_DOMAINS = [
  "names", "identity", "personhood", "attestation", "storage", "provider",
  "drive", "s3", "assets", "payments", "sponsorship", "resources", "statement-store",
] as const;

export function reviveAddress(value:string):ReviveAddress {
  if(!/^0x[0-9a-fA-F]{40}$/.test(value)) throw new TypeError("Revive address must be 20 bytes");
  return value.toLowerCase() as ReviveAddress;
}
export function selector(value:string):Selector {
  if(!/^0x[0-9a-fA-F]{8}$/.test(value)) throw new TypeError("Contract selector must be four bytes");
  return value.toLowerCase() as Selector;
}

function selectorBytes(value:Selector):Uint8Array {
  return Uint8Array.from(value.slice(2).match(/../g)!.map((pair)=>Number.parseInt(pair,16)));
}
function callData<Input>(message:ContractMessage<Input,unknown>,input:Input):Uint8Array {
  const encoded=message.input.encode(input);if(!(encoded instanceof Uint8Array))throw new TypeError("Contract codec must return bytes");
  const output=new Uint8Array(4+encoded.length);output.set(selectorBytes(message.selector));output.set(encoded,4);return output;
}
function amount(value:bigint|number|undefined,label:string):bigint {const result=BigInt(value??0);if(result<0n)throw new TypeError(`${label} must not be negative`);return result}

export function defineAppContract<Messages extends Record<string,ContractMessage<unknown,unknown>>>(
  definition:AppContractDefinition<Messages>,
):AppContractDefinition<Messages>{
  const domain=definition.domain.trim().toLowerCase();
  if(!/^[a-z][a-z0-9-]{1,63}$/.test(domain))throw new TypeError("Contract domain is invalid");
  if(FORBIDDEN_NATIVE_CONTRACT_DOMAINS.includes(domain as never))throw new OriginSdkError({source:"contracts",domain:"policy",code:"native_authority_forbidden",message:`${domain} is canonical native Commons authority`});
  if(!definition.name.trim()||Object.keys(definition.messages).length<1)throw new TypeError("Contract definition requires a name and messages");
  for(const [name,message] of Object.entries(definition.messages)){if(name!==message.name)throw new TypeError(`Contract message key mismatch: ${name}`);selector(message.selector)}
  return {...definition,domain,messages:{...definition.messages}};
}

export function createReviveContractClient(
  chain:CommonsChainClient,
  runtime:ReviveRuntimeAdapter,
){
  return {
    async query<Input,Output>(address:ReviveAddress,message:ContractMessage<Input,Output>,input:Input,options:{readonly value?:bigint}={},signal?:AbortSignal):Promise<SdkResult<{readonly value:Output;readonly gasRequired:bigint;readonly storageDeposit:bigint}>>{
      try{reviveAddress(address);if(message.mutates)throw new TypeError("Mutating contract messages must be prepared, not queried");const snapshot=await chain.finalizedSnapshot(signal);if(!snapshot.success)return snapshot;const result=await runtime.dryRun(snapshot.value.block.hash,{address,input:callData(message,input),value:amount(options.value,"value")},signal);if(result.reverted)return err(new OriginSdkError({source:"contracts",domain:"query",code:"contract_reverted",message:result.debugMessage??"Contract query reverted"}));return ok({value:message.output.decode(result.data),gasRequired:result.gasRequired,storageDeposit:result.storageDeposit})}catch(error){return err(asSdkError(error,{source:"contracts",domain:"query",code:"query_failed",retryable:false}))}
    },
    async prepareCall<Input>(address:ReviveAddress,message:ContractMessage<Input,unknown>,input:Input,options:{readonly value?:bigint;readonly gasLimit:bigint;readonly storageDepositLimit?:bigint|null},signal?:AbortSignal):Promise<SdkResult<PreparedTransaction>>{
      try{reviveAddress(address);if(!message.mutates)throw new TypeError("Read-only contract messages must use query");const request={address,input:callData(message,input),value:amount(options.value,"value"),gasLimit:amount(options.gasLimit,"gasLimit"),storageDepositLimit:options.storageDepositLimit===undefined?null:options.storageDepositLimit};return prepareAtFinalized(chain,({block})=>runtime.prepareCall(block.hash,request,signal),signal)}catch(error){return err(asSdkError(error,{source:"contracts",domain:"call",code:"call_construction_failed",retryable:false}))}
    },
    async prepareInstantiate(request:{readonly code:Uint8Array;readonly constructorData:Uint8Array;readonly value?:bigint;readonly gasLimit:bigint;readonly storageDepositLimit?:bigint|null;readonly salt?:Uint8Array},signal?:AbortSignal):Promise<SdkResult<PreparedTransaction>>{
      try{if(!request.code.length)throw new TypeError("Contract code must not be empty");return prepareAtFinalized(chain,({block})=>runtime.prepareInstantiate(block.hash,{code:request.code.slice(),constructorData:request.constructorData.slice(),value:amount(request.value,"value"),gasLimit:amount(request.gasLimit,"gasLimit"),storageDepositLimit:request.storageDepositLimit===undefined?null:request.storageDepositLimit,salt:request.salt?.slice()??new Uint8Array()},signal),signal)}catch(error){return err(asSdkError(error,{source:"contracts",domain:"instantiate",code:"instantiate_construction_failed",retryable:false}))}
    },
  };
}

export const REVIVE_CONTRACTS_CONTRACT={optionalLeaf:true,umbrellaDependency:false,runtimeApi:"ReviveApi.call",pallet:"Revive",rawPalletIndicesAccepted:false,nativeAuthorityAllowed:false} as const;
