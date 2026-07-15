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

import { invalidDomainInput, type AccountId, type BlockNumber, type BucketId, type ContentHash, type DecimalU64, type ObjectId, type PageInput } from "./types.ts";

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
