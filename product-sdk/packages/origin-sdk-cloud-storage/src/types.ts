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

import { OriginSdkError } from "@cord-network/origin-sdk-errors";
import type { AccountId, Hash32, Versioned } from "@cord-network/origin-sdk-identity";
export type { AccountId, Hash32, Versioned };

declare const storageType: unique symbol;
type StorageHash<K extends string> = Hash32 & { readonly [storageType]: K };
export type ContentHash = StorageHash<"ContentHash">;
export type ContentCommitment = StorageHash<"ContentCommitment">;
export type ProviderId = AccountId & { readonly [storageType]: "ProviderId" };
export type ProviderAllocationId = StorageHash<"ProviderAllocationId">;
export type AgreementId = StorageHash<"AgreementId">;
export type ChallengeId = StorageHash<"ChallengeId">;
export type ContainerId = StorageHash<"ContainerId">;
export type DriveId = StorageHash<"DriveId">;
export type BucketId = StorageHash<"BucketId">;
export type ObjectId = StorageHash<"ObjectId">;
export type BlockHash = StorageHash<"BlockHash">;
export type DecimalU64 = string & { readonly [storageType]: "DecimalU64" };
export type BlockNumber = string & { readonly [storageType]: "BlockNumber" };
export type ReservationId = string & { readonly [storageType]: "ReservationId" };
export interface PageInput { readonly cursor?: number | null; readonly limit?: number; }
export interface PageRequest { readonly cursor: number | null; readonly limit: number; }
export interface IdPage<Id> { readonly version: 1; readonly items: readonly Id[]; readonly next_cursor: number | null; readonly finalized_hash: BlockHash; }
export function page(input: PageInput = {}): PageRequest {
  const cursor=input.cursor??null, limit=input.limit??50;
  if(cursor!==null&&(!Number.isSafeInteger(cursor)||cursor<0||cursor>0xffff_ffff)) throw new TypeError("cursor must be a u32");
  if(!Number.isSafeInteger(limit)||limit<0) throw new TypeError("limit must be non-negative");
  return {cursor,limit:Math.min(limit,100)};
}
export function invalidDomainInput(domain:string,operation:string,message:string):never{throw new OriginSdkError({source:"cloud-storage",domain:`${domain}.${operation}`,code:"invalid_input",message,retryable:false})}
export function decimalU64(value:string|number,label="value"):DecimalU64{const text=String(value);if(!/^(0|[1-9][0-9]*)$/.test(text)||BigInt(text)>0xffff_ffff_ffff_ffffn)throw new TypeError(`${label} must be a u64`);return text as DecimalU64}
export function reservationId(value:string|number):ReservationId{return decimalU64(value,"reservation id") as unknown as ReservationId}
