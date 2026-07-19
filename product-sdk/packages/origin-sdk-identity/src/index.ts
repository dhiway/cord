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
import { OriginSdkError, type SdkResult } from "@cord-network/origin-sdk-errors";
import { err } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

declare const identityType: unique symbol;
export type AccountId = string & { readonly [identityType]: "AccountId" };
export type Hash32 = string & { readonly [identityType]: "Hash32" };

const HASH_32 = /^0x[0-9a-f]{64}$/;
const utf8 = new TextEncoder();

export function accountId(value: string): AccountId {
  if (value.length < 1 || value.length > 128) throw new TypeError("account must contain 1-128 characters");
  return value as AccountId;
}

export function hash32(value: string): Hash32 {
  if (!HASH_32.test(value)) throw new TypeError("hash must be a lowercase 32-byte 0x-prefixed value");
  return value as Hash32;
}

export type IdentityData =
  | { readonly kind: "none" }
  | { readonly kind: "raw"; readonly value: string }
  | { readonly kind: "blake2_256" | "sha2_256" | "keccak_256" | "sha3_256"; readonly hash: Hash32 };

export interface IdentityAdditionalField {
  readonly key: IdentityData;
  readonly value: IdentityData;
}

export interface IdentityInfo {
  readonly display: IdentityData;
  readonly legal: IdentityData;
  readonly web: IdentityData;
  readonly email: IdentityData;
  readonly image: IdentityData;
  readonly additional: readonly IdentityAdditionalField[];
}

export type IdentityJudgement = "reasonable" | "known_good" | "out_of_date" | "low_quality" | "erroneous";

export interface IdentityStatusView {
  readonly registered: boolean;
  readonly judgement_count: number;
  readonly requested: number;
  readonly reasonable: number;
  readonly known_good: number;
  readonly out_of_date: number;
  readonly low_quality: number;
  readonly erroneous: number;
}

export interface Versioned<T> {
  readonly version: 1;
  readonly value: T | null;
}

export type IdentityEvent =
  | { readonly kind: "identity-set"; readonly account: AccountId }
  | { readonly kind: "identity-cleared"; readonly account: AccountId }
  | { readonly kind: "judgement-requested"; readonly account: AccountId; readonly registrar_index: number }
  | { readonly kind: "judgement-cancelled"; readonly account: AccountId; readonly registrar_index: number }
  | { readonly kind: "judgement-given"; readonly target: AccountId; readonly registrar_index: number };

export interface IdentityRuntimeAdapter {
  identityStatus(at: `0x${string}`, account: AccountId, signal?: AbortSignal): Promise<Versioned<IdentityStatusView>>;
  setIdentity(at: `0x${string}`, info: IdentityInfo, signal?: AbortSignal): Promise<PreparedTransaction>;
  clearIdentity(at: `0x${string}`, signal?: AbortSignal): Promise<PreparedTransaction>;
  requestJudgement(at: `0x${string}`, registrar: AccountId, signal?: AbortSignal): Promise<PreparedTransaction>;
  cancelJudgementRequest(at: `0x${string}`, registrar: AccountId, signal?: AbortSignal): Promise<PreparedTransaction>;
  provideJudgement(
    at: `0x${string}`,
    target: AccountId,
    judgement: IdentityJudgement,
    identityHash: Hash32,
    signal?: AbortSignal,
  ): Promise<PreparedTransaction>;
}

export interface IdentityClient {
  status(account: AccountId, signal?: AbortSignal): Promise<SdkResult<Versioned<IdentityStatusView>>>;
  prepareSetIdentity(info: IdentityInfo, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareClearIdentity(signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRequestJudgement(registrar: AccountId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareCancelJudgementRequest(registrar: AccountId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareProvideJudgement(
    target: AccountId,
    judgement: IdentityJudgement,
    identityHash: Hash32,
    signal?: AbortSignal,
  ): Promise<SdkResult<PreparedTransaction>>;
}

export const IDENTITY_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  read: { status: "IdentityPersonhoodApi.identity_status" },
  transactions: {
    setIdentity: "People.set_identity",
    clearIdentity: "People.clear_identity",
    requestJudgement: "People.request_judgement",
    cancelJudgementRequest: "People.cancel_request",
    provideJudgement: "People.provide_judgement",
  },
} as const;

export const IDENTITY_ADMIN_EXCLUSIONS = [
  { target: "People.add_registrar", reason: "runtime administration" },
  { target: "People.kill_identity", reason: "forced mutation" },
  { target: "People.add_username_authority", reason: "username authority administration" },
  { target: "People.remove_username_authority", reason: "username authority administration" },
  { target: "People.remove_registrar", reason: "runtime administration" },
] as const;

const invalid = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "identity",
  domain: "input",
  code: "invalid_input",
  message,
  retryable: false,
}));

function validateData(data: IdentityData, field: string): void {
  if (data.kind === "raw") {
    if (utf8.encode(data.value).length > 32) throw new TypeError(`${field} raw value exceeds 32 UTF-8 bytes`);
  } else if (data.kind !== "none") hash32(data.hash);
}

function normalizeInfo(info: IdentityInfo): IdentityInfo {
  if (info.additional.length > 32) throw new TypeError("identity additional fields exceed 32 entries");
  for (const field of ["display", "legal", "web", "email", "image"] as const) validateData(info[field], field);
  const additional = info.additional.map(({ key, value }, index) => {
    validateData(key, `additional[${index}].key`);
    validateData(value, `additional[${index}].value`);
    return { key: { ...key }, value: { ...value } };
  });
  return { ...info, display: { ...info.display }, legal: { ...info.legal }, web: { ...info.web }, email: { ...info.email }, image: { ...info.image }, additional };
}

export function createIdentityClient(chain: CommonsChainClient, runtime: IdentityRuntimeAdapter): IdentityClient {
  return {
    async status(account, signal) {
      try { accountId(account); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid account"); }
      return chain.readFinalized((at) => runtime.identityStatus(at, account, signal), signal);
    },
    async prepareSetIdentity(info, signal) {
      let normalized: IdentityInfo;
      try { normalized = normalizeInfo(info); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid identity"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.setIdentity(block.hash, normalized, signal), signal);
    },
    prepareClearIdentity(signal) {
      return prepareAtFinalized(chain, ({ block }) => runtime.clearIdentity(block.hash, signal), signal);
    },
    async prepareRequestJudgement(registrar, signal) {
      try { accountId(registrar); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid registrar"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.requestJudgement(block.hash, registrar, signal), signal);
    },
    async prepareCancelJudgementRequest(registrar, signal) {
      try { accountId(registrar); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid registrar"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.cancelJudgementRequest(block.hash, registrar, signal), signal);
    },
    async prepareProvideJudgement(target, judgement, identityHash, signal) {
      try { accountId(target); hash32(identityHash); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid judgement"); }
      return prepareAtFinalized(
        chain,
        ({ block }) => runtime.provideJudgement(block.hash, target, judgement, identityHash, signal),
        signal,
      );
    },
  };
}
