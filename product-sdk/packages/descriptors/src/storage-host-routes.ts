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
  type AccountId, type AgreementId, type BlockNumber, type BucketId, type BucketName,
  type ChallengeId, type ContainerId, type ContentCommitment, type ContentHash, type DecimalU64,
  type DriveId, type DriveName, type ObjectId, type ObjectKey, type ObjectListInput, type PageInput,
  type ProviderEndpoint, type ProviderId, type ProviderServiceKey, type ProviderStatus,
} from "@cord-network/origin-sdk-cloud-storage";
import { finalizedRead, submitAndFinalize, type RequestContext } from "../../../src/host.ts";

export const providerHostRoutes = {
  providerById(context: RequestContext, provider: ProviderId) {
    return finalizedRead("provider", context, "storage", "provider_by_id", { provider });
  },

  providers(context: RequestContext, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "providers", { ...page(input) });
  },

  agreementById(context: RequestContext, agreement_id: AgreementId) {
    return finalizedRead("provider", context, "storage", "agreement_by_id", { agreement_id });
  },

  providerAgreements(context: RequestContext, provider: ProviderId, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "provider_agreements", {
      provider,
      ...page(input),
    });
  },

  agreementNonce(context: RequestContext, owner: AccountId) {
    return finalizedRead("provider", context, "storage", "agreement_nonce", { owner });
  },

  challengeById(context: RequestContext, challenge_id: ChallengeId) {
    return finalizedRead("provider", context, "storage", "challenge_by_id", { challenge_id });
  },

  challengesAt(context: RequestContext, block: BlockNumber, input?: PageInput) {
    return finalizedRead("provider", context, "storage", "challenges_at", {
      block,
      ...page(input),
    });
  },

  canAcceptCapacity(context: RequestContext, provider: ProviderId, additional_bytes: DecimalU64) {
    return finalizedRead("provider", context, "storage", "can_accept_capacity", {
      provider,
      additional_bytes,
    });
  },

  bucketCheckpoint(context: RequestContext, bucket: BucketId) {
    return finalizedRead("provider", context, "storage", "bucket_checkpoint", { bucket });
  },

  registerProvider(
    context: RequestContext,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    service_key: ProviderServiceKey,
    capacity_bytes: DecimalU64,
  ) {
    return submitAndFinalize("provider", context, "storage", "register_provider", {
      provider,
      endpoint,
      service_key,
      capacity_bytes,
    });
  },

  updateProvider(
    context: RequestContext,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    service_key: ProviderServiceKey,
    capacity_bytes: DecimalU64,
  ) {
    return submitAndFinalize("provider", context, "storage", "update_provider", {
      provider,
      endpoint,
      service_key,
      capacity_bytes,
    });
  },

  setProviderStatus(context: RequestContext, provider: ProviderId, status: ProviderStatus) {
    return submitAndFinalize("provider", context, "storage", "set_provider_status", { provider, status });
  },

  removeProvider(context: RequestContext, provider: ProviderId) {
    return submitAndFinalize("provider", context, "storage", "remove_provider", { provider });
  },

  heartbeat(context: RequestContext) {
    return submitAndFinalize("provider", context, "storage", "heartbeat", {});
  },

  proposeAgreement(
    context: RequestContext,
    input: {
      readonly provider: ProviderId;
      readonly container_ref: ContainerId;
      readonly content_commitment: ContentCommitment;
      readonly reservation_ref: ReservationId | null;
      readonly bytes: DecimalU64;
      readonly expires_at: BlockNumber;
    },
  ) {
    return submitAndFinalize("provider", context, "storage", "propose_agreement", { ...input });
  },

  acceptAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "accept_agreement", { agreement_id });
  },

  cancelAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "cancel_agreement", { agreement_id });
  },

  issueChallenge(
    context: RequestContext,
    agreement_id: AgreementId,
    expected_commitment: ContentCommitment,
    due_at: BlockNumber,
  ) {
    return submitAndFinalize("provider", context, "storage", "issue_challenge", {
      agreement_id,
      expected_commitment,
      due_at,
    });
  },

  timeoutChallenge(context: RequestContext, challenge_id: ChallengeId) {
    return submitAndFinalize("provider", context, "storage", "timeout_challenge", { challenge_id });
  },

  requestRenewal(context: RequestContext, agreement_id: AgreementId, expires_at: BlockNumber) {
    return submitAndFinalize("provider", context, "storage", "request_renewal", {
      agreement_id,
      expires_at,
    });
  },

  acknowledgeManifestDeletion(
	context: RequestContext,
	manifest: ContentCommitment,
	evidence_hash: ContentCommitment,
	service_key: ProviderServiceKey,
	signature: string,
  ) {
	return submitAndFinalize("provider", context, "storage", "acknowledge_manifest_deletion", {
	  manifest,
	  evidence_hash,
	  service_key,
	  signature,
	});
  },

  acceptRenewal(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "accept_renewal", { agreement_id });
  },

  expireAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "expire_agreement", { agreement_id });
  },

  pruneAgreement(context: RequestContext, agreement_id: AgreementId) {
    return submitAndFinalize("provider", context, "storage", "prune_agreement", { agreement_id });
  },

} as const;

export const driveHostRoutes = {
  driveById(context: RequestContext, drive_id: DriveId) {
    return finalizedRead("drive", context, "storage", "drive_by_id", { drive_id });
  },

  ownerDrives(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("drive", context, "storage", "owner_drives", { owner, ...page(input) });
  },

  driveControllers(context: RequestContext, drive_id: DriveId, input?: PageInput) {
    return finalizedRead("drive", context, "storage", "drive_controllers", {
      drive_id,
      ...page(input),
    });
  },

  nextDriveNonce(context: RequestContext, owner: AccountId) {
    return finalizedRead("drive", context, "storage", "next_drive_nonce", { owner });
  },

  createDrive(context: RequestContext, name: DriveName, root_storage_ref: ContentHash | null) {
    return submitAndFinalize("drive", context, "storage", "create_drive", { name, root_storage_ref });
  },

  updateRoot(
    context: RequestContext,
    drive_id: DriveId,
    expected_version: DecimalU64,
    root_storage_ref: ContentHash | null,
  ) {
    return submitAndFinalize("drive", context, "storage", "update_root", {
      drive_id,
      expected_version,
      root_storage_ref,
    });
  },

  setController(
    context: RequestContext,
    drive_id: DriveId,
    controller: AccountId,
    enabled: boolean,
  ) {
    return submitAndFinalize("drive", context, "storage", "drive.set_controller", {
      drive_id,
      controller,
      enabled,
    });
  },

  transferDrive(context: RequestContext, drive_id: DriveId, new_owner: AccountId) {
    return submitAndFinalize("drive", context, "storage", "transfer_drive", { drive_id, new_owner });
  },

  archiveDrive(context: RequestContext, drive_id: DriveId) {
    return submitAndFinalize("drive", context, "storage", "archive_drive", { drive_id });
  },
} as const;

export const s3HostRoutes = {
  bucketById(context: RequestContext, bucket: BucketId) {
    return finalizedRead("s3", context, "storage", "bucket_by_id", { bucket });
  },

  bucketByName(context: RequestContext, name: BucketName) {
    return finalizedRead("s3", context, "storage", "bucket_by_name", { name });
  },

  ownerBuckets(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("s3", context, "storage", "owner_buckets", { owner, ...page(input) });
  },

  bucketObjectKeys(context: RequestContext, bucket: BucketId, input: ObjectListInput = {}) {
    const limit = input.limit ?? 50;
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
      throw new TypeError("limit must be an integer between 1 and 100");
    }
    return finalizedRead("s3", context, "storage", "bucket_object_keys", {
      bucket,
      prefix: input.prefix ?? null,
      cursor: input.cursor ? { ...input.cursor } : null,
      limit,
    });
  },

  objectByKey(context: RequestContext, bucket: BucketId, key: ObjectKey) {
    return finalizedRead("s3", context, "storage", "object_by_key", { bucket, key });
  },

  objectHistory(context: RequestContext, bucket: BucketId, key: ObjectKey, input?: PageInput) {
    return finalizedRead("s3", context, "storage", "object_history", {
      bucket,
      key,
      ...page(input),
    });
  },

  objectId(context: RequestContext, bucket: BucketId, key: ObjectKey) {
    return finalizedRead("s3", context, "storage", "object_id", { bucket, key });
  },

  createBucket(context: RequestContext, name: BucketName) {
    return submitAndFinalize("s3", context, "storage", "create_bucket", { name });
  },

  setController(
    context: RequestContext,
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    controller: AccountId,
    enabled: boolean,
  ) {
    return submitAndFinalize("s3", context, "storage", "s3.set_controller", {
      bucket,
      expected_bucket_version,
      controller,
      enabled,
    });
  },

  transferBucket(
    context: RequestContext,
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    new_owner: AccountId,
  ) {
    return submitAndFinalize("s3", context, "storage", "transfer_bucket", {
      bucket,
      expected_bucket_version,
      new_owner,
    });
  },

  setArchived(
    context: RequestContext,
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    archived: boolean,
  ) {
    return submitAndFinalize("s3", context, "storage", "set_archived", {
      bucket,
      expected_bucket_version,
      archived,
    });
  },

  setVersioning(
    context: RequestContext,
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    enabled: boolean,
  ) {
    return submitAndFinalize("s3", context, "storage", "set_versioning", {
      bucket,
      expected_bucket_version,
      enabled,
    });
  },

  putObject(
    context: RequestContext,
    bucket: BucketId,
    key: ObjectKey,
    content_hash: ContentHash,
    expected_object_version: DecimalU64 | null,
  ) {
    return submitAndFinalize("s3", context, "storage", "put_object", {
      bucket,
      key,
      content_hash,
      expected_object_version,
    });
  },

  deleteObject(
    context: RequestContext,
    bucket: BucketId,
    key: ObjectKey,
    expected_object_version: DecimalU64,
  ) {
    return submitAndFinalize("s3", context, "storage", "delete_object", {
      bucket,
      key,
      expected_object_version,
    });
  },

  deleteBucket(
    context: RequestContext,
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
  ) {
    return submitAndFinalize("s3", context, "storage", "delete_bucket", {
      bucket,
      expected_bucket_version,
    });
  },
} as const;
