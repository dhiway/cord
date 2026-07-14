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

import { invalidDomainInput } from "./errors.ts";
import { finalizedRead, submitAndFinalize, type RequestContext } from "./host.ts";
import {
  page,
  type AccountId,
  type BlockNumber,
  type BucketId,
  type ContentHash,
  type DecimalU64,
  type ObjectId,
  type PageInput,
} from "./types.ts";

declare const s3Type: unique symbol;
export type BucketName = string & { readonly [s3Type]: "BucketName" };
export type ObjectKey = string & { readonly [s3Type]: "ObjectKey" };
export type BucketStatus = "active" | "archived" | "deleted";

const utf8 = new TextEncoder();

export function bucketName(value: string): BucketName {
  if (
    utf8.encode(value).length > 63 ||
    !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/.test(value) ||
    value.includes("..") ||
    value.includes(".-") ||
    value.includes("-.")
  ) {
    invalidDomainInput("s3", "bucket_name", "bucket name does not satisfy native S3 policy v1");
  }
  return value as BucketName;
}

export function objectKey(value: string): ObjectKey {
  const length = utf8.encode(value).length;
  if (length < 1 || length > 1_024) {
    invalidDomainInput("s3", "object_key", "object key must contain 1-1024 UTF-8 bytes");
  }
  return value as ObjectKey;
}

export interface BucketView {
  readonly bucket: BucketId;
  readonly name: BucketName;
  readonly owner: AccountId;
  readonly controllers: readonly AccountId[];
  readonly status: BucketStatus;
  readonly versioning_enabled: boolean;
  readonly version: DecimalU64;
  readonly live_objects: number;
  readonly created_at: BlockNumber;
  readonly updated_at: BlockNumber;
}

export interface ObjectView {
  readonly object_id: ObjectId;
  readonly bucket: BucketId;
  readonly key: ObjectKey;
  readonly content_hash: ContentHash | null;
  readonly version: DecimalU64;
  readonly deleted: boolean;
  readonly updated_by: AccountId;
  readonly updated_at: BlockNumber;
}

export interface ObjectVersion {
  readonly content_hash: ContentHash | null;
  readonly version: DecimalU64;
  readonly deleted: boolean;
  readonly updated_by: AccountId;
  readonly updated_at: BlockNumber;
}

export const s3 = {
  bucketById(context: RequestContext, bucket: BucketId) {
    return finalizedRead("s3", context, "storage", "bucket_by_id", { bucket });
  },

  bucketByName(context: RequestContext, name: BucketName) {
    return finalizedRead("s3", context, "storage", "bucket_by_name", { name });
  },

  ownerBuckets(context: RequestContext, owner: AccountId, input?: PageInput) {
    return finalizedRead("s3", context, "storage", "owner_buckets", { owner, ...page(input) });
  },

  bucketObjectKeys(context: RequestContext, bucket: BucketId, input?: PageInput) {
    return finalizedRead("s3", context, "storage", "bucket_object_keys", {
      bucket,
      ...page(input),
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
