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

import {
  page,
  type AccountId,
  type AttestationId,
  type AttestationInput,
  type DelegatedIssueIntent,
  type DelegatedRevokeIntent,
  type IndexPolicy,
  type IssuerSignature,
  type PageInput,
  type SchemaDefinition,
  type SchemaId,
  type SchemaStatus,
  type SignedDelegatedIssue,
  type SignedDelegatedRevoke,
  type StatusCommitment,
  type SubjectCommitment,
} from "@cord-network/origin-sdk-attestation";
import { invalidDomainInput } from "../../../src/errors.ts";
import { finalizedRead, submitAndFinalize, type RequestContext } from "../../../src/host.ts";

export const attestationHostRoutes = {
  schemaById(context: RequestContext, schema: SchemaId) {
    return finalizedRead("attestation", context, "attestation", "schema_by_id", { schema });
  },

  attestationById(context: RequestContext, attestation: AttestationId) {
    return finalizedRead("attestation", context, "attestation", "attestation_by_id", { attestation });
  },

  liveStatus(context: RequestContext, attestation: AttestationId) {
    return finalizedRead("attestation", context, "attestation", "attestation_live_status", { attestation });
  },

  creatorSchemas(context: RequestContext, creator: AccountId, input?: PageInput) {
    return finalizedRead("attestation", context, "attestation", "creator_schemas", {
      creator,
      ...page(input),
    });
  },

  issuerAttestations(context: RequestContext, issuer: AccountId, input?: PageInput) {
    return finalizedRead("attestation", context, "attestation", "issuer_attestations", {
      issuer,
      ...page(input),
    });
  },

  subjectSchemaAttestations(
    context: RequestContext,
    subject_commitment: SubjectCommitment,
    schema: SchemaId,
    input?: PageInput,
  ) {
    return finalizedRead("attestation", context, "attestation", "subject_schema_attestations", {
      subject_commitment,
      schema,
      ...page(input),
    });
  },

  nextDelegatedNonce(context: RequestContext, issuer: AccountId) {
    return finalizedRead("attestation", context, "attestation", "next_delegated_nonce", { issuer });
  },

  schemaCount(context: RequestContext) {
    return finalizedRead("attestation", context, "attestation", "schema_count", {});
  },

  attestationCount(context: RequestContext) {
    return finalizedRead("attestation", context, "attestation", "attestation_count", {});
  },

  nextIssuanceNonce(context: RequestContext, issuer: AccountId) {
    return finalizedRead("attestation", context, "attestation", "next_issuance_nonce", { issuer });
  },

  externalStatus(
    context: RequestContext,
    issuer: AccountId,
    status_commitment: StatusCommitment,
  ) {
    return finalizedRead("attestation", context, "attestation", "external_status", {
      issuer,
      status_commitment,
    });
  },

  createSchema(
    context: RequestContext,
    definition: SchemaDefinition,
    authorized_issuers: readonly AccountId[],
    revocable: boolean,
    unique: boolean,
    index_policy: IndexPolicy,
  ) {
    if (authorized_issuers.length > 64 || new Set(authorized_issuers).size !== authorized_issuers.length) {
      invalidDomainInput(
        "attestation",
        "create_schema",
        "authorized issuers must be unique and contain at most 64 accounts",
      );
    }
    return submitAndFinalize("attestation", context, "attestation", "create_schema", {
      definition,
      authorized_issuers: [...authorized_issuers],
      revocable,
      unique,
      index_policy,
    });
  },

  setSchemaStatus(context: RequestContext, schema: SchemaId, status: SchemaStatus) {
    return submitAndFinalize("attestation", context, "attestation", "set_schema_status", {
      schema,
      status,
    });
  },

  issue(context: RequestContext, input: AttestationInput) {
    return submitAndFinalize("attestation", context, "attestation", "issue", { ...input });
  },

  issueDelegated(context: RequestContext, intent: DelegatedIssueIntent, signature: IssuerSignature) {
    return submitAndFinalize("attestation", context, "attestation", "issue_delegated", {
      intent: { ...intent },
      signature,
    });
  },

  issueBatch(context: RequestContext, items: readonly AttestationInput[]) {
    if (items.length < 1 || items.length > 64) {
      invalidDomainInput("attestation", "issue_batch", "batch must contain 1-64 attestations");
    }
    return submitAndFinalize("attestation", context, "attestation", "issue_batch", {
      items: items.map((item) => ({ ...item })),
    });
  },

  revoke(context: RequestContext, attestation: AttestationId) {
    return submitAndFinalize("attestation", context, "attestation", "revoke", { attestation });
  },

  revokeDelegated(
    context: RequestContext,
    intent: DelegatedRevokeIntent,
    signature: IssuerSignature,
  ) {
    return submitAndFinalize("attestation", context, "attestation", "revoke_delegated", {
      intent: { ...intent },
      signature,
    });
  },

  issueDelegatedBatch(context: RequestContext, items: readonly SignedDelegatedIssue[]) {
    ensureBatch(items.length, "issue_delegated_batch");
    return submitAndFinalize("attestation", context, "attestation", "issue_delegated_batch", {
      items: items.map(({ intent, signature }) => ({ intent: { ...intent }, signature })),
    });
  },

  revokeBatch(context: RequestContext, attestations: readonly AttestationId[]) {
    ensureBatch(attestations.length, "revoke_batch");
    return submitAndFinalize("attestation", context, "attestation", "revoke_batch", {
      attestations: [...attestations],
    });
  },

  revokeDelegatedBatch(context: RequestContext, items: readonly SignedDelegatedRevoke[]) {
    ensureBatch(items.length, "revoke_delegated_batch");
    return submitAndFinalize("attestation", context, "attestation", "revoke_delegated_batch", {
      items: items.map(({ intent, signature }) => ({ intent: { ...intent }, signature })),
    });
  },

  revokeExternalStatus(context: RequestContext, status_commitment: StatusCommitment) {
    return submitAndFinalize("attestation", context, "attestation", "revoke_external_status", {
      status_commitment,
    });
  },

  revokeExternalStatusBatch(
    context: RequestContext,
    status_commitments: readonly StatusCommitment[],
  ) {
    ensureBatch(status_commitments.length, "revoke_external_status_batch");
    return submitAndFinalize(
      "attestation",
      context,
      "attestation",
      "revoke_external_status_batch",
      { status_commitments: [...status_commitments] },
    );
  },

  setEmergencyPause(context: RequestContext, paused: boolean) {
    return submitAndFinalize("attestation", context, "attestation", "set_emergency_pause", { paused });
  },

  forceSchemaStatus(context: RequestContext, schema: SchemaId, status: SchemaStatus) {
    return submitAndFinalize("attestation", context, "attestation", "force_schema_status", {
      schema,
      status,
    });
  },

  forceRevoke(context: RequestContext, attestation: AttestationId) {
    return submitAndFinalize("attestation", context, "attestation", "force_revoke", { attestation });
  },
} as const;

function ensureBatch(length: number, operation: string): void {
  if (length < 1 || length > 64) {
    invalidDomainInput("attestation", operation, "batch must contain 1-64 items");
  }
}
