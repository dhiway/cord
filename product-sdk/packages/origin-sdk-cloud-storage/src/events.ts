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
  invalidDomainInput,
  type AccountId,
  type AgreementId,
  type BlockHash,
  type BlockNumber,
  type BucketId,
  type ChallengeId,
  type ContentHash,
  type ContentCommitment,
  type DecimalU64,
  type DriveId,
  type ObjectId,
  type ProviderId,
} from "./types.ts";
import type { AgreementStatus, ProviderStatus } from "./provider.ts";

export type BucketRole = "reader" | "writer" | "admin";
export type DriveRole = "reader" | "writer" | "admin";
export type CommitmentState = "publishable" | "pending" | "tombstoned" | "missing";
export type DriveNodeKind = "directory" | "file";

export interface CheckpointCommitment {
  readonly mmr_root: ContentCommitment;
  readonly start_seq: DecimalU64;
  readonly leaf_count: DecimalU64;
}

export type StorageNativeEventKind =
  | "provider_registered" | "provider_updated" | "provider_status_changed" | "provider_removed" | "heartbeat"
  | "storage_bucket_created" | "storage_bucket_grant_changed" | "agreement_transitioned" | "agreement_capacity_released" | "agreement_provider_rebound"
  | "challenge_issued" | "challenge_proved" | "challenge_timed_out" | "checkpoint_accepted" | "checkpoint_equivocation"
  | "replica_selected" | "primary_promoted" | "bucket_replica_replaced" | "manifest_commitment_changed" | "manifest_deletion_acknowledged"
  | "drive_created" | "drive_root_updated" | "drive_grant_changed" | "drive_transferred" | "drive_archived" | "drive_node_written" | "drive_node_removed"
  | "s3_bucket_created" | "s3_controller_changed" | "s3_bucket_transferred" | "s3_bucket_archived" | "s3_bucket_versioning_changed"
  | "s3_object_put" | "s3_object_deleted" | "s3_object_purged" | "s3_bucket_deleted" | "s3_object_history_pruned";

type E<K extends StorageNativeEventKind, D> = { readonly event: K; readonly data: Readonly<D> };
export type StorageNativeEvent =
  | E<"provider_registered", { provider: ProviderId; capacity_bytes: DecimalU64 }>
  | E<"provider_updated", { provider: ProviderId; capacity_bytes: DecimalU64 }>
  | E<"provider_status_changed", { provider: ProviderId; status: ProviderStatus }>
  | E<"provider_removed", { provider: ProviderId }>
  | E<"heartbeat", { provider: ProviderId; at: BlockNumber }>
  | E<"storage_bucket_created", { bucket: BucketId; owner: AccountId; primary: ProviderId; replicas: readonly ProviderId[]; version: DecimalU64 }>
  | E<"storage_bucket_grant_changed", { bucket: BucketId; account: AccountId; role: BucketRole | null; previous_version: DecimalU64; new_version: DecimalU64 }>
  | E<"agreement_transitioned", { agreement: AgreementId; previous: AgreementStatus | null; current: AgreementStatus; previous_version: DecimalU64; new_version: DecimalU64 }>
  | E<"agreement_capacity_released", { agreement: AgreementId }>
  | E<"agreement_provider_rebound", { agreement: AgreementId; old_provider: ProviderId; new_provider: ProviderId; status: AgreementStatus; bytes: DecimalU64 }>
  | E<"challenge_issued", { challenge: ChallengeId; bucket: BucketId; provider: ProviderId; due_at: BlockNumber }>
  | E<"challenge_proved", { challenge: ChallengeId; provider: ProviderId }>
  | E<"challenge_timed_out", { challenge: ChallengeId; provider: ProviderId; checkpoint: BlockNumber }>
  | E<"checkpoint_accepted", { bucket: BucketId; commitment: CheckpointCommitment; checkpoint: BlockNumber; replica_confirmations: readonly ProviderId[] }>
  | E<"checkpoint_equivocation", { code: number; bucket: BucketId; provider: ProviderId; accepted_root: ContentCommitment; conflicting_root: ContentCommitment; nonce: BlockNumber }>
  | E<"replica_selected", { bucket: BucketId; provider: ProviderId; checkpoint: BlockNumber }>
  | E<"primary_promoted", { bucket: BucketId; old_provider: ProviderId; new_provider: ProviderId; checkpoint: BlockNumber }>
  | E<"bucket_replica_replaced", { bucket: BucketId; old_provider: ProviderId; new_provider: ProviderId; previous_version: DecimalU64; new_version: DecimalU64 }>
  | E<"manifest_commitment_changed", { manifest: ContentCommitment; bucket: BucketId; state: CommitmentState; checkpoint: BlockNumber | null }>
  | E<"manifest_deletion_acknowledged", { manifest: ContentCommitment; bucket: BucketId; provider: ProviderId; evidence_hash: ContentCommitment; acknowledged_at: BlockNumber }>
  | E<"drive_created", { drive: DriveId; owner: AccountId; version: DecimalU64 }>
  | E<"drive_root_updated", { drive: DriveId; previous_root: ContentCommitment | null; new_root: ContentCommitment; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"drive_grant_changed", { drive: DriveId; subject: AccountId; role: DriveRole | null; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"drive_transferred", { drive: DriveId; old_owner: AccountId; new_owner: AccountId; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"drive_archived", { drive: DriveId; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"drive_node_written", { drive: DriveId; path: string; kind: DriveNodeKind; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"drive_node_removed", { drive: DriveId; path: string; previous_version: DecimalU64; version: DecimalU64 }>
  | E<"s3_bucket_created", { bucket: BucketId; name: string; owner: AccountId }>
  | E<"s3_controller_changed", { bucket: BucketId; controller: AccountId; enabled: boolean; version: DecimalU64 }>
  | E<"s3_bucket_transferred", { bucket: BucketId; from: AccountId; to: AccountId; version: DecimalU64 }>
  | E<"s3_bucket_archived", { bucket: BucketId; archived: boolean; version: DecimalU64 }>
  | E<"s3_bucket_versioning_changed", { bucket: BucketId; enabled: boolean; version: DecimalU64 }>
  | E<"s3_object_put", { bucket: BucketId; object: ObjectId; key: string; content_hash: ContentHash; version: DecimalU64 }>
  | E<"s3_object_deleted", { bucket: BucketId; object: ObjectId; key: string; version: DecimalU64 }>
  | E<"s3_object_purged", { bucket: BucketId; key: string }>
  | E<"s3_bucket_deleted", { bucket: BucketId; name: string; owner: AccountId }>
  | E<"s3_object_history_pruned", { bucket: BucketId; key: string; through_version: DecimalU64; removed: number }>;

export type StorageNativeOutcome = {
  readonly outcome: "provider" | "storage_bucket" | "agreement" | "challenge" | "checkpoint" | "manifest" | "drive" | "s3_bucket" | "s3_object";
  readonly data: {
    readonly action: StorageNativeEventKind;
    readonly id: string | null;
    readonly version: DecimalU64 | null;
  };
};

export interface FinalizedStorageNativeEvent {
  readonly finalized_block_hash: BlockHash;
  readonly event_index: number;
  readonly event: StorageNativeEvent;
}
export interface StorageNativeEventSubscription {
  readonly finality: "finalized";
  readonly from_finalized_block: BlockHash;
  readonly kinds: readonly StorageNativeEventKind[];
}
export function storageNativeEventSubscription(from_finalized_block: BlockHash, kinds: readonly StorageNativeEventKind[]): StorageNativeEventSubscription {
  if (kinds.length < 1 || kinds.length > 37 || new Set(kinds).size !== kinds.length)
    invalidDomainInput("storage", "event_subscription", "subscription requires 1-37 unique event kinds");
  return { finality: "finalized", from_finalized_block, kinds: [...kinds] };
}

export function storageNativeEventOutcome(event: StorageNativeEvent): StorageNativeOutcome {
  const data = event.data as Readonly<Record<string, unknown>>;
  let outcome: StorageNativeOutcome["outcome"];
  if (event.event.startsWith("provider_") || event.event === "heartbeat") outcome = "provider";
  else if (event.event.startsWith("storage_bucket_")) outcome = "storage_bucket";
  else if (event.event.startsWith("agreement_")) outcome = "agreement";
  else if (event.event.startsWith("challenge_")) outcome = "challenge";
  else if (event.event.startsWith("checkpoint_") || event.event === "replica_selected" || event.event === "primary_promoted" || event.event === "bucket_replica_replaced") outcome = "checkpoint";
  else if (event.event.startsWith("manifest_")) outcome = "manifest";
  else if (event.event.startsWith("drive_")) outcome = "drive";
  else if (event.event.startsWith("s3_object_")) outcome = "s3_object";
  else outcome = "s3_bucket";
  const id = outcome === "provider" ? data.provider
    : outcome === "storage_bucket" || outcome === "checkpoint" || outcome === "s3_bucket" ? data.bucket
    : outcome === "agreement" ? data.agreement
    : outcome === "challenge" ? data.challenge
    : outcome === "manifest" ? data.manifest
    : outcome === "drive" ? data.drive
    : data.object ?? data.bucket ?? null;
  const version = data.version ?? data.new_version ?? null;
  return { outcome, data: { action: event.event, id: id as string | null, version: version as DecimalU64 | null } };
}
