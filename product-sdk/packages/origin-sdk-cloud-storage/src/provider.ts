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

import { invalidDomainInput, type AccountId, type AgreementId, type BlockNumber, type BucketId, type ChallengeId, type ContentCommitment, type DecimalU64, type Hash32, type ProviderId } from "./types.ts";

declare const providerType: unique symbol;
export type ProviderEndpoint = string & { readonly [providerType]: "ProviderEndpoint" };
export type ProviderServiceKey = string & { readonly [providerType]: "ProviderServiceKey" };

export type ProviderStatus = "active" | "suspended";
export type AgreementStatus = "proposed" | "active" | "suspended" | "cancelled" | "expired";
export type ChallengeStatus = "open" | "proved" | "timed_out";

const utf8 = new TextEncoder();
function boundedProviderText<Kind extends "ProviderEndpoint" | "ProviderServiceKey">(
  value: string,
  kind: Kind,
  maxBytes: number,
): string & { readonly [providerType]: Kind } {
  if (!value || utf8.encode(value).length > maxBytes) {
    invalidDomainInput("provider", kind, `${kind} must contain 1-${maxBytes} UTF-8 bytes`);
  }
  return value as string & { readonly [providerType]: Kind };
}

export const providerEndpoint = (value: string): ProviderEndpoint =>
  boundedProviderText(value, "ProviderEndpoint", 512);
export const providerServiceKey = (value: string): ProviderServiceKey =>
  boundedProviderText(value, "ProviderServiceKey", 128);

export interface ProviderOrganizationView {
  readonly entity_id: string;
  readonly attestation_id: Hash32;
  readonly schema_id: Hash32;
  readonly sla_commitment: Hash32;
  readonly sla_version: number;
  readonly valid_from: BlockNumber;
  readonly valid_until: BlockNumber;
  readonly rotation_predecessor: Hash32 | null;
}

export interface ProviderServiceKeyView {
  readonly active: ProviderServiceKey;
  readonly active_version: DecimalU64;
  readonly previous: ProviderServiceKey | null;
  readonly pending: ProviderServiceKey | null;
  readonly pending_version: DecimalU64 | null;
  readonly pending_effective_at: BlockNumber | null;
}

export interface ProviderView {
  readonly provider: ProviderId;
  readonly endpoint: ProviderEndpoint;
  readonly organization: ProviderOrganizationView;
  readonly service_key: ProviderServiceKeyView;
  readonly capacity_bytes: DecimalU64;
  readonly allocated_bytes: DecimalU64;
  readonly pending_bytes: DecimalU64;
  readonly status: ProviderStatus;
  readonly last_heartbeat: BlockNumber;
  readonly overdue_challenges: number;
  readonly authority_validated_at: BlockNumber | null;
}

export interface AgreementView {
  readonly agreement_id: AgreementId;
  readonly owner: AccountId;
  readonly bucket_id: BucketId;
  readonly primary: ProviderId;
  readonly replicas: readonly ProviderId[];
  readonly bytes: DecimalU64;
  readonly created_at: BlockNumber;
  readonly expires_at: BlockNumber;
  readonly release_at: BlockNumber | null;
  readonly state_version: DecimalU64;
  readonly status: AgreementStatus;
}

export interface ChallengeView {
  readonly challenge_id: ChallengeId;
  readonly provider: ProviderId;
  readonly bucket_id: BucketId;
  readonly expected_commitment: CommitmentView;
  readonly location: ChunkLocationView;
  readonly due_at: BlockNumber;
  readonly status: ChallengeStatus;
}

export interface CommitmentView {
  readonly mmr_root: ContentCommitment;
  readonly start_seq: DecimalU64;
  readonly leaf_count: DecimalU64;
}

export interface ChunkLocationView {
  readonly leaf_index: DecimalU64;
  readonly chunk_index: number;
}

export interface CheckpointView {
  readonly bucket_id: BucketId;
  readonly commitment: CommitmentView;
  readonly checkpoint_block: BlockNumber;
  readonly primary_signers: number;
  readonly commitment_nonce: BlockNumber;
  readonly replica_confirmations: readonly ProviderId[];
}
