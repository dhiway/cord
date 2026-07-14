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
import type { AccountId } from "@cord-network/origin-sdk-identity";
import { accountId } from "@cord-network/origin-sdk-identity";
import type {
  AttestationAllowanceView,
  PersonhoodReadAdapter,
  PersonhoodStatusView,
} from "@cord-network/origin-sdk-personhood";
import type { Versioned } from "@cord-network/origin-sdk-identity";
import { err, ok } from "@cord-network/origin-sdk-result";
import { prepareAtFinalized, type PreparedTransaction } from "@cord-network/origin-sdk-tx";

declare const resourcesType: unique symbol;
export type CommunicationIdentifier = string & { readonly [resourcesType]: "CommunicationIdentifier" };
export type Alias = string & { readonly [resourcesType]: "Alias" };
export type MembershipProof = string & { readonly [resourcesType]: "MembershipProof" };
export type OffchainSignature = string & { readonly [resourcesType]: "OffchainSignature" };
export type ReservationId = bigint & { readonly [resourcesType]: "ReservationId" };

export type MembershipCollection = "people" | "lite-people";
export type ConsumerCredibility =
  | { readonly kind: "lite" }
  | { readonly kind: "person"; readonly alias: Alias; readonly last_update: bigint; readonly demoted: boolean };
export interface ConsumerInfo {
  readonly identifier_key: CommunicationIdentifier;
  readonly credibility: ConsumerCredibility;
}
export interface StatementAllowance {
  readonly alias: Alias;
  readonly period: number;
  readonly sequence: number;
  readonly account: AccountId;
  readonly since: bigint;
}
export type ReservationPurpose =
  | { readonly kind: "membership"; readonly period: number; readonly alias: Alias; readonly counter: number; readonly collection: MembershipCollection }
  | { readonly kind: "proof-of-ink"; readonly candidate_hash: string; readonly allocation_index: number };
export interface StorageClaim {
  readonly reservation_id: ReservationId;
  readonly purpose: ReservationPurpose;
  readonly owner: AccountId;
  readonly created_at: number;
}

export interface PersonAuthorization {
  readonly kind: "as-person";
  readonly proof: MembershipProof;
  readonly ring_index: number;
  readonly revision: number;
}
export interface LitePersonAuthorization {
  readonly kind: "people-lite-auth";
  readonly proof: MembershipProof;
  readonly ring_index: number;
}
export interface ResourceAuthorization {
  readonly kind: "as-resources";
  readonly proof: MembershipProof;
  readonly ring_index: number;
  readonly revision?: number;
  readonly collection: MembershipCollection;
}

export interface RegisterLitePersonInput {
  readonly identifier_key: CommunicationIdentifier;
  readonly authorization: LitePersonAuthorization;
}
export interface RegisterPersonInput {
  readonly linked_lite_identity: AccountId;
  readonly lite_identity_proof: OffchainSignature;
  readonly authorization: PersonAuthorization;
}
export interface StatementAllowanceInput {
  readonly period: number;
  readonly sequence: number;
  readonly target_account: AccountId;
  readonly authorization: ResourceAuthorization;
}
export interface LongTermStorageClaimInput {
  readonly period: number;
  readonly counter: number;
  readonly account: AccountId;
  readonly authorization: ResourceAuthorization & { readonly revision: number };
}

export interface ResourceProfile {
  readonly finalized_hash: `0x${string}`;
  readonly finalized_number: bigint;
  readonly personhood: Versioned<PersonhoodStatusView>;
  readonly attestation_allowance: Versioned<AttestationAllowanceView>;
  readonly consumer: ConsumerInfo | null;
  readonly statement_allowances: readonly StatementAllowance[];
}

export type ResourceEvent =
  | { readonly kind: "person-registered"; readonly alias: Alias; readonly account: AccountId }
  | { readonly kind: "lite-person-registered"; readonly account: AccountId }
  | { readonly kind: "person-authorization-touched"; readonly account: AccountId }
  | { readonly kind: "identifier-key-updated"; readonly account: AccountId }
  | { readonly kind: "statement-allowance-set"; readonly alias: Alias; readonly period: number; readonly sequence: number; readonly account: AccountId }
  | { readonly kind: "long-term-storage-reserved"; readonly reservation_id: ReservationId; readonly account: AccountId }
  | { readonly kind: "long-term-storage-reservation-cancelled"; readonly reservation_id: ReservationId; readonly account: AccountId };

export interface ResourcesRuntimeAdapter {
  consumer(at: `0x${string}`, account: AccountId, signal?: AbortSignal): Promise<ConsumerInfo | null>;
  statementAllowances(at: `0x${string}`, account: AccountId, signal?: AbortSignal): Promise<readonly StatementAllowance[]>;
  storageClaim(at: `0x${string}`, reservationId: ReservationId, signal?: AbortSignal): Promise<StorageClaim | null>;
  registerLitePerson(at: `0x${string}`, input: RegisterLitePersonInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  registerPerson(at: `0x${string}`, input: RegisterPersonInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  touchPersonAuthorization(at: `0x${string}`, authorization: PersonAuthorization, signal?: AbortSignal): Promise<PreparedTransaction>;
  updateIdentifierKey(at: `0x${string}`, identifierKey: CommunicationIdentifier, signal?: AbortSignal): Promise<PreparedTransaction>;
  setStatementAllowance(at: `0x${string}`, input: StatementAllowanceInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  claimLongTermStorage(at: `0x${string}`, input: LongTermStorageClaimInput, signal?: AbortSignal): Promise<PreparedTransaction>;
  cancelLongTermStorage(at: `0x${string}`, reservationId: ReservationId, signal?: AbortSignal): Promise<PreparedTransaction>;
}

export interface ResourcesClient {
  consumer(account: AccountId, signal?: AbortSignal): Promise<SdkResult<ConsumerInfo | null>>;
  statementAllowances(account: AccountId, signal?: AbortSignal): Promise<SdkResult<readonly StatementAllowance[]>>;
  storageClaim(reservationId: ReservationId, signal?: AbortSignal): Promise<SdkResult<StorageClaim | null>>;
  profile(account: AccountId, signal?: AbortSignal): Promise<SdkResult<ResourceProfile>>;
  prepareRegisterLitePerson(input: RegisterLitePersonInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareRegisterPerson(input: RegisterPersonInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareTouchPersonAuthorization(authorization: PersonAuthorization, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareUpdateIdentifierKey(identifierKey: CommunicationIdentifier, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareSetStatementAllowance(input: StatementAllowanceInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareClaimLongTermStorage(input: LongTermStorageClaimInput, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
  prepareCancelLongTermStorage(reservationId: ReservationId, signal?: AbortSignal): Promise<SdkResult<PreparedTransaction>>;
}

export const RESOURCES_NATIVE_BINDINGS = {
  metadataHash: COMMONS_NETWORK_BINDING.metadata_hash,
  reads: {
    consumer: "Resources.Consumers",
    statementAllowances: "Resources.StmtStoreAllowanceByAccount+StatementStoreAllowances",
    storageClaim: "Resources.StorageClaims",
  },
  transactions: {
    registerLitePerson: { call: "Resources.register_lite_person", extension: "PeopleLiteAuth" },
    registerPerson: { call: "Resources.register_person", extension: "AsPerson" },
    touchPersonAuthorization: { call: "Resources.touch_person_authorization", extension: "AsPerson" },
    updateIdentifierKey: { call: "Resources.update_identifier_key", extension: null },
    setStatementAllowance: { call: "Resources.set_statement_store_account", extension: "AsResources" },
    claimLongTermStorage: { call: "Resources.claim_long_term_storage", extension: "AsResources" },
    cancelLongTermStorage: { call: "Resources.cancel_long_term_storage_reservation", extension: null },
  },
} as const;

export const RESOURCES_ADMIN_EXCLUSIONS = [
  { target: "Resources.demote_auth_expired", reason: "permissionless maintenance" },
  { target: "Resources.clear_expired_friend_request_sequence", reason: "permissionless maintenance" },
  { target: "Resources.clear_expired_stmt_store_allowances", reason: "permissionless maintenance" },
  { target: "Resources.clear_expired_long_term_storage_aliases", reason: "permissionless maintenance" },
  { target: "Resources.expire_long_term_storage_reservations", reason: "permissionless maintenance" },
] as const;

const invalid = <T>(message: string): SdkResult<T> => err(new OriginSdkError({
  source: "resources",
  domain: "input",
  code: "invalid_input",
  message,
  retryable: false,
}));
const u32 = (value: number, field: string): void => {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) throw new TypeError(`${field} must be a u32`);
};
const u8 = (value: number, field: string): void => {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xff) throw new TypeError(`${field} must be a u8`);
};
const proof = (value: string, field: string): void => {
  if (!/^0x(?:[0-9a-f]{2})+$/.test(value) || value.length > 32770) throw new TypeError(`${field} must be bounded lowercase hex`);
};

export function communicationIdentifier(value: string): CommunicationIdentifier {
  if (!/^0x[0-9a-f]{130}$/.test(value)) throw new TypeError("identifier key must be a lowercase 65-byte value");
  return value as CommunicationIdentifier;
}
export function membershipProof(value: string): MembershipProof { proof(value, "membership proof"); return value as MembershipProof; }
export function offchainSignature(value: string): OffchainSignature { proof(value, "offchain signature"); return value as OffchainSignature; }
export function reservationId(value: bigint | number | string): ReservationId {
  const normalized = BigInt(value);
  if (normalized < 0n || normalized > 0xffff_ffff_ffff_ffffn) throw new TypeError("reservation id must be a u64");
  return normalized as ReservationId;
}
function validateAuthorization(value: PersonAuthorization | LitePersonAuthorization | ResourceAuthorization): void {
  proof(value.proof, "authorization proof");
  u32(value.ring_index, "ring_index");
  if ("revision" in value && value.revision !== undefined) u32(value.revision, "revision");
}

export function createResourcesClient(
  chain: CommonsChainClient,
  runtime: ResourcesRuntimeAdapter,
  personhood: PersonhoodReadAdapter,
): ResourcesClient {
  const validateAccount = <T>(account: AccountId): SdkResult<T> | undefined => {
    try { accountId(account); return undefined; }
    catch (error) { return invalid(error instanceof Error ? error.message : "invalid account"); }
  };
  return {
    async consumer(account, signal) {
      const failure = validateAccount<ConsumerInfo | null>(account); if (failure) return failure;
      return chain.readFinalized((at) => runtime.consumer(at, account, signal), signal);
    },
    async statementAllowances(account, signal) {
      const failure = validateAccount<readonly StatementAllowance[]>(account); if (failure) return failure;
      return chain.readFinalized((at) => runtime.statementAllowances(at, account, signal), signal);
    },
    storageClaim(id, signal) {
      return chain.readFinalized((at) => runtime.storageClaim(at, id, signal), signal);
    },
    async profile(account, signal) {
      const failure = validateAccount<ResourceProfile>(account); if (failure) return failure;
      const snapshot = await chain.finalizedSnapshot(signal);
      if (!snapshot.success) return snapshot;
      const [status, allowance, consumer, statements] = await Promise.all([
        snapshot.value.read((at) => personhood.personhoodStatus(at, account, signal), signal),
        snapshot.value.read((at) => personhood.attestationAllowance(at, account, signal), signal),
        snapshot.value.read((at) => runtime.consumer(at, account, signal), signal),
        snapshot.value.read((at) => runtime.statementAllowances(at, account, signal), signal),
      ]);
      for (const result of [status, allowance, consumer, statements]) if (!result.success) return result;
      if (!status.success || !allowance.success || !consumer.success || !statements.success)
        return invalid("unreachable profile state");
      return ok({
        finalized_hash: snapshot.value.block.hash,
        finalized_number: snapshot.value.block.number,
        personhood: status.value,
        attestation_allowance: allowance.value,
        consumer: consumer.value,
        statement_allowances: statements.value,
      });
    },
    async prepareRegisterLitePerson(input, signal) {
      try { communicationIdentifier(input.identifier_key); validateAuthorization(input.authorization); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid lite registration"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.registerLitePerson(block.hash, input, signal), signal);
    },
    async prepareRegisterPerson(input, signal) {
      try { accountId(input.linked_lite_identity); offchainSignature(input.lite_identity_proof); validateAuthorization(input.authorization); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid person registration"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.registerPerson(block.hash, input, signal), signal);
    },
    async prepareTouchPersonAuthorization(authorization, signal) {
      try { validateAuthorization(authorization); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid person authorization"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.touchPersonAuthorization(block.hash, authorization, signal), signal);
    },
    async prepareUpdateIdentifierKey(identifierKey, signal) {
      try { communicationIdentifier(identifierKey); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid identifier key"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.updateIdentifierKey(block.hash, identifierKey, signal), signal);
    },
    async prepareSetStatementAllowance(input, signal) {
      try { u32(input.period, "period"); u32(input.sequence, "sequence"); accountId(input.target_account); validateAuthorization(input.authorization); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid statement allowance"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.setStatementAllowance(block.hash, input, signal), signal);
    },
    async prepareClaimLongTermStorage(input, signal) {
      try { u32(input.period, "period"); u8(input.counter, "counter"); accountId(input.account); validateAuthorization(input.authorization); }
      catch (error) { return invalid(error instanceof Error ? error.message : "invalid storage claim"); }
      return prepareAtFinalized(chain, ({ block }) => runtime.claimLongTermStorage(block.hash, input, signal), signal);
    },
    prepareCancelLongTermStorage(id, signal) {
      return prepareAtFinalized(chain, ({ block }) => runtime.cancelLongTermStorage(block.hash, id, signal), signal);
    },
  };
}
