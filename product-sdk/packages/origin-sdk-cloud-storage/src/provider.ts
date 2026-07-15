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

import { invalidDomainInput, type AccountId, type AgreementId, type BlockNumber, type ChallengeId, type ContainerId, type ContentCommitment, type DecimalU64, type IdPage, type PageInput, type ProviderId, type ReservationId } from "./types.ts";

declare const providerType: unique symbol;
export type ProviderEndpoint = string & { readonly [providerType]: "ProviderEndpoint" };
export type ProviderServiceKey = string & { readonly [providerType]: "ProviderServiceKey" };

export type ProviderStatus = "active" | "suspended";
export type AgreementStatus = "proposed" | "active" | "cancelled" | "expired";
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

export interface ProviderView {
  readonly provider: ProviderId;
  readonly endpoint: ProviderEndpoint;
  readonly service_key: ProviderServiceKey;
  readonly capacity_bytes: DecimalU64;
  readonly allocated_bytes: DecimalU64;
  readonly pending_bytes: DecimalU64;
  readonly status: ProviderStatus;
  readonly last_heartbeat: BlockNumber;
  readonly reputation: number;
}

export interface AgreementView {
  readonly agreement_id: AgreementId;
  readonly owner: AccountId;
  readonly provider: ProviderId;
  readonly container_ref: ContainerId;
  readonly content_commitment: ContentCommitment;
  readonly reservation_ref: ReservationId | null;
  readonly bytes: DecimalU64;
  readonly created_at: BlockNumber;
  readonly expires_at: BlockNumber;
  readonly pending_expiry: BlockNumber | null;
  readonly status: AgreementStatus;
}

export interface ChallengeView {
  readonly challenge_id: ChallengeId;
  readonly provider: ProviderId;
  readonly agreement_id: AgreementId;
  readonly expected_commitment: ContentCommitment;
  readonly due_at: BlockNumber;
  readonly proof_commitment: ContentCommitment | null;
  readonly status: ChallengeStatus;
}

export interface ProviderCheckpoint {
  readonly challenge_id: ChallengeId;
  readonly proof_commitment: ContentCommitment;
  readonly recorded_at: BlockNumber;
}

export interface ProviderRootView {
  readonly sequence: DecimalU64;
  readonly root: ContentCommitment;
  readonly leaf_count: DecimalU64;
  readonly committed_at: BlockNumber;
}

export interface DeletionAcknowledgementView {
  readonly provider: ProviderId;
  readonly content_commitment: ContentCommitment;
  readonly tombstone_root: ContentCommitment;
  readonly root_sequence: DecimalU64;
  readonly leaf_index: DecimalU64;
  readonly leaf_count: DecimalU64;
  readonly proof_commitment: ContentCommitment;
  readonly acknowledged_at: BlockNumber;
}
