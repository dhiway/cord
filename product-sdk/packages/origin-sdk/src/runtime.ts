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


import type {
  AssetPaymentOptions,
  AssetsRuntimeAdapter,
} from "@cord-network/origin-sdk-assets";
import type { AttestationRuntimeAdapter } from "@cord-network/origin-sdk-attestation";
import type { CloudStorageRuntimeAdapter } from "@cord-network/origin-sdk-cloud-storage";
import type { IdentityRuntimeAdapter } from "@cord-network/origin-sdk-identity";
import type { NamesRuntimeAdapter } from "@cord-network/origin-sdk-names";
import type { PersonhoodRuntimeAdapter } from "@cord-network/origin-sdk-personhood";
import type { ResourcesRuntimeAdapter } from "@cord-network/origin-sdk-resources";
import type { PreparedTransaction } from "@cord-network/origin-sdk-tx";
import type { OriginAppRuntime } from "./index.ts";

export interface RuntimePrepareContext {
  readonly payment?: AssetPaymentOptions;
}

/**
 * Descriptor-backed execution boundary owned by the platform integration.
 * Targets are generated metadata/runtime-API names and payloads are decoded domain values.
 */
export interface CommonsRuntimeExecutor {
  read<T>(
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    signal?: AbortSignal,
  ): Promise<T>;
  prepare(
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    context?: RuntimePrepareContext,
    signal?: AbortSignal,
  ): Promise<PreparedTransaction>;
}

export function createOriginAppRuntime(
  executor: CommonsRuntimeExecutor,
): OriginAppRuntime {
  const read = <T>(
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    signal?: AbortSignal,
  ): Promise<T> => executor.read(at, target, payload, signal);
  const prepare = (
    at: `0x${string}`,
    target: string,
    payload: Readonly<Record<string, unknown>>,
    signal?: AbortSignal,
  ): Promise<PreparedTransaction> => executor.prepare(at, target, payload, undefined, signal);

  const identity: IdentityRuntimeAdapter = {
    identityStatus: (at, account, signal) =>
      read(at, "IdentityPersonhoodApi.identity_status", { account }, signal),
    setIdentity: (at, info, signal) => prepare(at, "People.set_identity", { info }, signal),
    clearIdentity: (at, signal) => prepare(at, "People.clear_identity", {}, signal),
    requestJudgement: (at, registrar, signal) =>
      prepare(at, "People.request_judgement", { registrar }, signal),
    cancelJudgementRequest: (at, registrar, signal) =>
      prepare(at, "People.cancel_request", { registrar }, signal),
    provideJudgement: (at, target, judgement, identityHash, signal) =>
      prepare(at, "People.provide_judgement", {
        target, judgement, identity_hash: identityHash,
      }, signal),
  };

  const personhood: PersonhoodRuntimeAdapter = {
    personhoodStatus: (at, account, signal) =>
      read(at, "IdentityPersonhoodApi.personhood_status", { account }, signal),
    attestationAllowance: (at, account, signal) =>
      read(at, "IdentityPersonhoodApi.attestation_allowance", { account }, signal),
    attestLitePerson: (at, input, signal) =>
      prepare(at, "PeopleLite.attest", { ...input }, signal),
  };

  const resources: ResourcesRuntimeAdapter = {
    consumer: (at, account, signal) => read(at, "Resources.Consumers", { account }, signal),
    statementAllowances: (at, account, signal) => read(
      at,
      "Resources.StmtStoreAllowanceByAccount+StatementStoreAllowances",
      { account },
      signal,
    ),
    storageClaim: (at, reservationId, signal) =>
      read(at, "Resources.StorageClaims", { reservation_id: reservationId }, signal),
    registerLitePerson: (at, input, signal) =>
      prepare(at, "Resources.register_lite_person", { ...input }, signal),
    registerPerson: (at, input, signal) =>
      prepare(at, "Resources.register_person", { ...input }, signal),
    touchPersonAuthorization: (at, authorization, signal) =>
      prepare(at, "Resources.touch_person_authorization", { authorization }, signal),
    updateIdentifierKey: (at, identifierKey, signal) =>
      prepare(at, "Resources.update_identifier_key", { identifier_key: identifierKey }, signal),
    setStatementAllowance: (at, input, signal) =>
      prepare(at, "Resources.set_statement_store_account", { ...input }, signal),
    claimLongTermStorage: (at, input, signal) =>
      prepare(at, "Resources.claim_long_term_storage", { ...input }, signal),
    cancelLongTermStorage: (at, reservationId, signal) => prepare(
      at,
      "Resources.cancel_long_term_storage_reservation",
      { reservation_id: reservationId },
      signal,
    ),
  };

  const attestation: AttestationRuntimeAdapter = {
    schemaById: (at, schema, signal) =>
      read(at, "AttestationApi.schema_by_id", { schema }, signal),
    attestationById: (at, attestation, signal) =>
      read(at, "AttestationApi.attestation_by_id", { attestation }, signal),
    liveStatus: (at, attestation, signal) =>
      read(at, "AttestationApi.attestation_live_status", { attestation }, signal),
    creatorSchemas: (at, creator, request, signal) =>
      read(at, "AttestationApi.creator_schemas", { creator, ...request }, signal),
    issuerAttestations: (at, issuer, request, signal) =>
      read(at, "AttestationApi.issuer_attestations", { issuer, ...request }, signal),
    subjectSchemaAttestations: (at, subject, schema, request, signal) => read(
      at,
      "AttestationApi.subject_schema_attestations",
      { subject, schema, ...request },
      signal,
    ),
    nextDelegatedNonce: (at, issuer, signal) =>
      read(at, "AttestationApi.next_delegated_nonce", { issuer }, signal),
    schemaCount: (at, signal) => read(at, "AttestationApi.schema_count", {}, signal),
    attestationCount: (at, signal) =>
      read(at, "AttestationApi.attestation_count", {}, signal),
    nextIssuanceNonce: (at, issuer, signal) =>
      read(at, "AttestationApi.next_issuance_nonce", { issuer }, signal),
    externalStatus: (at, issuer, status, signal) =>
      read(at, "AttestationApi.external_status", { issuer, status }, signal),
    createSchema: (at, input, signal) =>
      prepare(at, "Attestation.create_schema", { ...input }, signal),
    setSchemaStatus: (at, schema, status, signal) =>
      prepare(at, "Attestation.set_schema_status", { schema, status }, signal),
    issue: (at, input, signal) => prepare(at, "Attestation.issue", { ...input }, signal),
    issueDelegated: (at, input, signal) =>
      prepare(at, "Attestation.issue_delegated", { ...input }, signal),
    issueBatch: (at, items, signal) =>
      prepare(at, "Attestation.issue_batch", { items: [...items] }, signal),
    revoke: (at, attestation, signal) =>
      prepare(at, "Attestation.revoke", { attestation }, signal),
    revokeDelegated: (at, input, signal) =>
      prepare(at, "Attestation.revoke_delegated", { ...input }, signal),
    issueDelegatedBatch: (at, items, signal) =>
      prepare(at, "Attestation.issue_delegated_batch", { items: [...items] }, signal),
    revokeBatch: (at, attestations, signal) => prepare(
      at,
      "Attestation.revoke_batch",
      { attestations: [...attestations] },
      signal,
    ),
    revokeDelegatedBatch: (at, items, signal) =>
      prepare(at, "Attestation.revoke_delegated_batch", { items: [...items] }, signal),
    revokeExternalStatus: (at, status, signal) =>
      prepare(at, "Attestation.revoke_external_status", { status }, signal),
    revokeExternalStatusBatch: (at, statuses, signal) => prepare(
      at,
      "Attestation.revoke_external_status_batch",
      { statuses: [...statuses] },
      signal,
    ),
  };

  const names: NamesRuntimeAdapter = {
    labelPolicyVersion: (at, signal) =>
      read(at, "NamesApi.label_policy_version", {}, signal),
    nameById: (at, name, signal) => read(at, "NamesApi.name_by_id", { name }, signal),
    rootNameByNormalizedLabel: (at, label, signal) =>
      read(at, "NamesApi.root_name_by_normalized_label", { label }, signal),
    ownerNames: (at, owner, request, signal) =>
      read(at, "NamesApi.owner_names", { owner, ...request }, signal),
    controllers: (at, name, signal) =>
      read(at, "NamesApi.controllers", { name }, signal),
    resolveAddress: (at, name, signal) =>
      read(at, "NamesApi.resolve_address", { name }, signal),
    resolveSubject: (at, name, signal) =>
      read(at, "NamesApi.resolve_subject", { name }, signal),
    resolveAttestation: (at, name, signal) =>
      read(at, "NamesApi.resolve_attestation", { name }, signal),
    resolveContentPublication: (at, name, signal) =>
      read(at, "NamesApi.resolve_content_publication", { name }, signal),
    resolveText: (at, name, key, signal) =>
      read(at, "NamesApi.resolve_text", { name, key }, signal),
    primaryName: (at, owner, signal) =>
      read(at, "NamesApi.primary_name", { owner }, signal),
    nameStatus: (at, name, signal) =>
      read(at, "NamesApi.name_status", { name }, signal),
    commit: (at, commitment, signal) =>
      prepare(at, "Names.commit", { commitment }, signal),
    cancelCommitment: (at, commitment, signal) =>
      prepare(at, "Names.cancel_commitment", { commitment }, signal),
    pruneExpiredCommitment: (at, owner, commitment, signal) => prepare(
      at,
      "Names.prune_expired_commitment",
      { owner, commitment },
      signal,
    ),
    register: (at, parent, label, salt, signal) =>
      prepare(at, "Names.register", { parent, label, salt }, signal),
    renew: (at, name, additionalPeriod, signal) => prepare(
      at,
      "Names.renew",
      { name, additional_period: additionalPeriod },
      signal,
    ),
    transfer: (at, name, newOwner, signal) =>
      prepare(at, "Names.transfer", { name, new_owner: newOwner }, signal),
    addController: (at, name, controller, signal) =>
      prepare(at, "Names.add_controller", { name, controller }, signal),
    removeController: (at, name, controller, signal) =>
      prepare(at, "Names.remove_controller", { name, controller }, signal),
    setAddress: (at, name, address, signal) =>
      prepare(at, "Names.set_address", { name, address }, signal),
    setSubject: (at, name, subject, signal) =>
      prepare(at, "Names.set_subject", { name, subject }, signal),
    setAttestation: (at, name, attestation, signal) =>
      prepare(at, "Names.set_attestation", { name, attestation }, signal),
    publishContent: (at, name, content, expectedRevision, operationId, signal) =>
      prepare(at, "Names.publish_content", { name, content, expected_revision: expectedRevision, operation_id: operationId }, signal),
    setText: (at, name, key, value, signal) =>
      prepare(at, "Names.set_text", { name, key, value }, signal),
    setPrimaryName: (at, name, signal) =>
      prepare(at, "Names.set_primary_name", { name }, signal),
    release: (at, name, signal) => prepare(at, "Names.release", { name }, signal),
    removeExpiredName: (at, name, signal) =>
      prepare(at, "Names.remove_expired_name", { name }, signal),
  };

  const storage: CloudStorageRuntimeAdapter = {
    read: (at, service, method, payload, signal) =>
      read(at, `${service}.${method}`, payload, signal),
    prepare: (at, service, method, payload, signal) =>
      prepare(at, `${service}.${method}`, payload, signal),
  };

  const assets: AssetsRuntimeAdapter = {
    read: (at, target, payload, signal) => read(at, target, payload, signal),
    prepare: (at, target, payload, payment, signal) => executor.prepare(
      at,
      target,
      payload,
      { payment },
      signal,
    ),
  };

  return { identity, personhood, resources, attestation, names, storage, assets };
}

export const ORIGIN_RUNTIME_EXECUTOR_CONTRACT = {
  targetSource: "generated Commons metadata and runtime APIs",
  rawScaleAccepted: false,
  palletIndicesAccepted: false,
  applicationEndpointsAccepted: false,
  adapterCount: 7,
} as const;
