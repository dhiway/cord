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

import { page, type AccountId, type AgreementId, type BlockNumber, type BucketId, type ChallengeId, type ContainerId, type ContentCommitment, type ContentHash, type DecimalU64, type DriveId, type ObjectId, type PageInput, type ProviderAllocationId, type ProviderId, type ReservationId } from "./types.ts";
import type { Base64Content, CidConfig, StorageRef, TransactionRef } from "./storage.ts";
import type { ProviderEndpoint, ProviderServiceKey, ProviderStatus } from "./provider.ts";
import type { DriveName } from "./drive.ts";
import type { BucketName, ObjectKey, ObjectListInput } from "./s3.ts";

export type StorageService = "storage" | "provider" | "drive" | "s3";
export interface CloudStorageReadRequest { readonly kind:"read"; readonly service:StorageService; readonly method:string; readonly payload:Readonly<Record<string,unknown>> }
export interface CloudStorageWriteRequest { readonly kind:"write"; readonly service:StorageService; readonly method:string; readonly payload:Readonly<Record<string,unknown>> }
const readRequest=(service:StorageService,method:string,payload:Readonly<Record<string,unknown>>):CloudStorageReadRequest=>({kind:"read",service,method,payload});
const writeRequest=(service:StorageService,method:string,payload:Readonly<Record<string,unknown>>):CloudStorageWriteRequest=>({kind:"write",service,method,payload});

export const storageRequests = {
  accountAuthorization(account: AccountId) {
    return readRequest("storage", "account_authorization", { account });
  },

  canStore(account: AccountId, data_len: number) {
    return readRequest("storage", "can_store", { account, data_len });
  },

  canRenew(account: AccountId, entry: TransactionRef) {
    return readRequest("storage", "can_renew", { account, entry: { ...entry } });
  },

  storedContentProvenance(reference: StorageRef) {
    return readRequest("storage", "stored_content_provenance", {
      reference: { ...reference },
    });
  },

  resourceReservation(reservation_id: ReservationId) {
    return readRequest("storage", "resource_reservation", { reservation_id });
  },

  resourceReservationLink(
    reservation_id: ReservationId,
    content_hash: ContentHash,
  ) {
    return readRequest("storage", "resource_reservation_link", {
      reservation_id,
      content_hash,
    });
  },

  resourceProviderRef(reservation_id: ReservationId) {
    return readRequest("storage", "resource_provider_ref", { reservation_id });
  },

  store(content_base64: Base64Content) {
    return writeRequest("storage", "store", { content_base64 });
  },

  storeWithCidConfig(cid_config: CidConfig, content_base64: Base64Content) {
    return writeRequest("storage", "store_with_cid_config", {
      cid_config: { ...cid_config },
      content_base64,
    });
  },

  storeReserved(
    reservation_id: ReservationId,
    cid_config: CidConfig,
    content_base64: Base64Content,
  ) {
    return writeRequest("storage", "store_reserved", {
      reservation_id,
      cid_config: { ...cid_config },
      content_base64,
    });
  },

  renewReserved(
    reservation_id: ReservationId,
    content_hash: ContentHash,
  ) {
    return writeRequest("storage", "renew_reserved", {
      reservation_id,
      content_hash,
    });
  },

  attachProvider(
    reservation_id: ReservationId,
    provider_ref: ProviderAllocationId,
  ) {
    return writeRequest("storage", "attach_provider", {
      reservation_id,
      provider_ref,
    });
  },

  renew(entry: TransactionRef) {
    return writeRequest("storage", "renew", { entry: { ...entry } });
  },

  forceRenew(entry: TransactionRef) {
    return writeRequest("storage", "force_renew", { entry: { ...entry } });
  },

  enableAutoRenew(content_hash: ContentHash) {
    return writeRequest("storage", "enable_auto_renew", { content_hash });
  },

  disableAutoRenew(content_hash: ContentHash) {
    return writeRequest("storage", "disable_auto_renew", { content_hash });
  },
} as const;

export const providerRequests = {
  providerById(provider: ProviderId) {
    return readRequest("provider", "provider_by_id", { provider });
  },

  providers(input?: PageInput) {
    return readRequest("provider", "providers", { ...page(input) });
  },

  agreementById(agreement_id: AgreementId) {
    return readRequest("provider", "agreement_by_id", { agreement_id });
  },

  providerAgreements(provider: ProviderId, input?: PageInput) {
    return readRequest("provider", "provider_agreements", {
      provider,
      ...page(input),
    });
  },

  agreementNonce(owner: AccountId) {
    return readRequest("provider", "agreement_nonce", { owner });
  },

  challengeById(challenge_id: ChallengeId) {
    return readRequest("provider", "challenge_by_id", { challenge_id });
  },

  challengesAt(block: BlockNumber, input?: PageInput) {
    return readRequest("provider", "challenges_at", {
      block,
      ...page(input),
    });
  },

  canAcceptCapacity(provider: ProviderId, additional_bytes: DecimalU64) {
    return readRequest("provider", "can_accept_capacity", {
      provider,
      additional_bytes,
    });
  },

  bucketCheckpoint(bucket: BucketId) {
    return readRequest("provider", "bucket_checkpoint", { bucket });
  },

  heartbeat() {
    return writeRequest("provider", "heartbeat", {});
  },

  proposeAgreement(
    input: {
      readonly provider: ProviderId;
      readonly container_ref: ContainerId;
      readonly content_commitment: ContentCommitment;
      readonly reservation_ref: ReservationId | null;
      readonly bytes: DecimalU64;
      readonly expires_at: BlockNumber;
    },
  ) {
    return writeRequest("provider", "propose_agreement", { ...input });
  },

  acceptAgreement(agreement_id: AgreementId) {
    return writeRequest("provider", "accept_agreement", { agreement_id });
  },

  cancelAgreement(agreement_id: AgreementId) {
    return writeRequest("provider", "cancel_agreement", { agreement_id });
  },

  submitCheckpoint(
    challenge_id: ChallengeId,
    proof_commitment: ContentCommitment,
  ) {
    return writeRequest("provider", "submit_checkpoint", {
      challenge_id,
      proof_commitment,
    });
  },

  timeoutChallenge(challenge_id: ChallengeId) {
    return writeRequest("provider", "timeout_challenge", { challenge_id });
  },

  requestRenewal(agreement_id: AgreementId, expires_at: BlockNumber) {
    return writeRequest("provider", "request_renewal", {
      agreement_id,
      expires_at,
    });
  },

  acceptRenewal(agreement_id: AgreementId) {
    return writeRequest("provider", "accept_renewal", { agreement_id });
  },

  expireAgreement(agreement_id: AgreementId) {
    return writeRequest("provider", "expire_agreement", { agreement_id });
  },

  pruneAgreement(agreement_id: AgreementId) {
    return writeRequest("provider", "prune_agreement", { agreement_id });
  },

  acknowledgeDeletion(
    agreement_id: AgreementId,
    content_commitment: ContentCommitment,
    tombstone_root: ContentCommitment,
    root_sequence: DecimalU64,
    leaf_index: DecimalU64,
    leaf_count: DecimalU64,
    inclusion_proof: readonly ContentCommitment[],
  ) {
    return writeRequest("provider", "acknowledge_deletion", {
      agreement_id,
      content_commitment,
      tombstone_root,
      root_sequence,
      leaf_index,
      leaf_count,
      inclusion_proof: [...inclusion_proof],
    });
  },

  commitProviderRoot(
    sequence: DecimalU64,
    appended_leaves: readonly ContentCommitment[],
  ) {
    return writeRequest("provider", "commit_provider_root", {
      sequence,
      appended_leaves: [...appended_leaves],
    });
  },
} as const;

export const driveRequests = {
  driveById(drive_id: DriveId) {
    return readRequest("drive", "drive_by_id", { drive_id });
  },

  ownerDrives(owner: AccountId, input?: PageInput) {
    return readRequest("drive", "owner_drives", { owner, ...page(input) });
  },

  driveControllers(drive_id: DriveId, input?: PageInput) {
    return readRequest("drive", "drive_controllers", {
      drive_id,
      ...page(input),
    });
  },

  nextDriveNonce(owner: AccountId) {
    return readRequest("drive", "next_drive_nonce", { owner });
  },

  createDrive(name: DriveName, root_storage_ref: ContentHash | null) {
    return writeRequest("drive", "create_drive", { name, root_storage_ref });
  },

  updateRoot(
    drive_id: DriveId,
    expected_version: DecimalU64,
    root_storage_ref: ContentHash | null,
  ) {
    return writeRequest("drive", "update_root", {
      drive_id,
      expected_version,
      root_storage_ref,
    });
  },

  setController(
    drive_id: DriveId,
    controller: AccountId,
    enabled: boolean,
  ) {
    return writeRequest("drive", "drive.set_controller", {
      drive_id,
      controller,
      enabled,
    });
  },

  transferDrive(drive_id: DriveId, new_owner: AccountId) {
    return writeRequest("drive", "transfer_drive", { drive_id, new_owner });
  },

  archiveDrive(drive_id: DriveId) {
    return writeRequest("drive", "archive_drive", { drive_id });
  },
} as const;

export const s3Requests = {
  bucketById(bucket: BucketId) {
    return readRequest("s3", "bucket_by_id", { bucket });
  },

  bucketByName(name: BucketName) {
    return readRequest("s3", "bucket_by_name", { name });
  },

  ownerBuckets(owner: AccountId, input?: PageInput) {
    return readRequest("s3", "owner_buckets", { owner, ...page(input) });
  },

  bucketObjectKeys(bucket: BucketId, input: ObjectListInput = {}) {
    const limit = input.limit ?? 50;
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
      throw new TypeError("limit must be an integer between 1 and 100");
    }
    return readRequest("s3", "bucket_object_keys", {
      bucket,
      prefix: input.prefix ?? null,
      cursor: input.cursor ? { ...input.cursor } : null,
      limit,
    });
  },

  objectByKey(bucket: BucketId, key: ObjectKey) {
    return readRequest("s3", "object_by_key", { bucket, key });
  },

  objectHistory(bucket: BucketId, key: ObjectKey, input?: PageInput) {
    return readRequest("s3", "object_history", {
      bucket,
      key,
      ...page(input),
    });
  },

  objectId(bucket: BucketId, key: ObjectKey) {
    return readRequest("s3", "object_id", { bucket, key });
  },

  createBucket(name: BucketName) {
    return writeRequest("s3", "create_bucket", { name });
  },

  setController(
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    controller: AccountId,
    enabled: boolean,
  ) {
    return writeRequest("s3", "s3.set_controller", {
      bucket,
      expected_bucket_version,
      controller,
      enabled,
    });
  },

  transferBucket(
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    new_owner: AccountId,
  ) {
    return writeRequest("s3", "transfer_bucket", {
      bucket,
      expected_bucket_version,
      new_owner,
    });
  },

  setArchived(
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    archived: boolean,
  ) {
    return writeRequest("s3", "set_archived", {
      bucket,
      expected_bucket_version,
      archived,
    });
  },

  setVersioning(
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
    enabled: boolean,
  ) {
    return writeRequest("s3", "set_versioning", {
      bucket,
      expected_bucket_version,
      enabled,
    });
  },

  putObject(
    bucket: BucketId,
    key: ObjectKey,
    content_hash: ContentHash,
    expected_object_version: DecimalU64 | null,
  ) {
    return writeRequest("s3", "put_object", {
      bucket,
      key,
      content_hash,
      expected_object_version,
    });
  },

  deleteObject(
    bucket: BucketId,
    key: ObjectKey,
    expected_object_version: DecimalU64,
  ) {
    return writeRequest("s3", "delete_object", {
      bucket,
      key,
      expected_object_version,
    });
  },

  deleteBucket(
    bucket: BucketId,
    expected_bucket_version: DecimalU64,
  ) {
    return writeRequest("s3", "delete_bucket", {
      bucket,
      expected_bucket_version,
    });
  },
} as const;
