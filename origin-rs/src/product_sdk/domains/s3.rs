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

use serde::{Deserialize, Serialize};

use super::common::{
	ensure_bytes, invalid, AccountId, BucketId, ContentCommitment, DomainResult, FinalizedQuery,
	Hash32, ObjectId, PageRequest, SubmitAndFinalize, Validate,
};

pub const MAX_BUCKET_NAME_BYTES: usize = 63;
pub const MAX_OBJECT_KEY_BYTES: usize = 1024;

pub type S3Read = FinalizedQuery<S3Query>;
pub type S3Write = SubmitAndFinalize<S3Command>;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BucketStatus {
	Active,
	Archived,
	Deleted,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BucketName(String);

impl BucketName {
	pub fn new(value: impl Into<String>) -> DomainResult<Self> {
		let value = value.into();
		let bytes = value.as_bytes();
		let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
		if bytes.is_empty()
			|| bytes.len() > MAX_BUCKET_NAME_BYTES
			|| !alphanumeric(bytes[0])
			|| !alphanumeric(bytes[bytes.len() - 1])
		{
			return Err(invalid("invalid S3 bucket name"));
		}
		let mut previous = 0;
		for byte in bytes.iter().copied() {
			if !(alphanumeric(byte) || byte == b'.' || byte == b'-')
				|| (byte == b'.' && previous == b'.')
				|| (byte == b'.' && previous == b'-')
				|| (byte == b'-' && previous == b'.')
			{
				return Err(invalid("invalid S3 bucket name"));
			}
			previous = byte;
		}
		Ok(Self(value))
	}

	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl Validate for BucketName {
	fn validate(&self) -> DomainResult<()> {
		Self::new(self.0.clone()).map(|_| ())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ObjectKey(Vec<u8>);

impl ObjectKey {
	pub fn new(value: Vec<u8>) -> DomainResult<Self> {
		ensure_bytes(&value, 1, MAX_OBJECT_KEY_BYTES, "S3 object key")?;
		Ok(Self(value))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl Validate for ObjectKey {
	fn validate(&self) -> DomainResult<()> {
		ensure_bytes(&self.0, 1, MAX_OBJECT_KEY_BYTES, "S3 object key")
	}
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ObjectKeyPrefix(Vec<u8>);

impl ObjectKeyPrefix {
	pub fn new(value: Vec<u8>) -> DomainResult<Self> {
		ensure_bytes(&value, 0, MAX_OBJECT_KEY_BYTES, "S3 object key prefix")?;
		Ok(Self(value))
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl Validate for ObjectKeyPrefix {
	fn validate(&self) -> DomainResult<()> {
		ensure_bytes(&self.0, 0, MAX_OBJECT_KEY_BYTES, "S3 object key prefix")
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectListCursor {
	pub snapshot_version: u64,
	pub last_key: ObjectKey,
}

impl Validate for ObjectListCursor {
	fn validate(&self) -> DomainResult<()> {
		self.last_key.validate()
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectListRequest {
	#[serde(default)]
	pub prefix: Option<ObjectKeyPrefix>,
	#[serde(default)]
	pub cursor: Option<ObjectListCursor>,
	pub limit: u32,
}

impl Validate for ObjectListRequest {
	fn validate(&self) -> DomainResult<()> {
		self.prefix.as_ref().map_or(Ok(()), Validate::validate)?;
		self.cursor.as_ref().map_or(Ok(()), Validate::validate)?;
		if !(1..=100).contains(&self.limit) {
			return Err(invalid("S3 object page limit must be between 1 and 100"));
		}
		Ok(())
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedObjectPage {
	pub version: u16,
	pub finalized_block_hash: Hash32,
	pub items: Vec<ObjectKey>,
	pub next_cursor: Option<ObjectListCursor>,
	pub snapshot_version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BucketView {
	pub bucket: BucketId,
	pub name: BucketName,
	pub owner: AccountId,
	pub controllers: Vec<AccountId>,
	pub status: BucketStatus,
	pub versioning_enabled: bool,
	pub version: u64,
	pub live_objects: u32,
	pub created_at: super::common::BlockNumber,
	pub updated_at: super::common::BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectView {
	pub object: ObjectId,
	pub bucket: BucketId,
	pub key: ObjectKey,
	pub content: Option<ContentCommitment>,
	pub provider_commitment: Option<ContentCommitment>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: super::common::BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectVersionView {
	pub content: Option<ContentCommitment>,
	pub provider_commitment: Option<ContentCommitment>,
	pub version: u64,
	pub deleted: bool,
	pub updated_by: AccountId,
	pub updated_at: super::common::BlockNumber,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", content = "result", rename_all = "snake_case")]
pub enum S3Response {
	Bucket(super::common::FinalizedValue<BucketView>),
	BucketId(super::common::FinalizedValue<BucketId>),
	Buckets(super::common::FinalizedPage<BucketId>),
	Object(super::common::FinalizedValue<ObjectView>),
	Objects(FinalizedObjectPage),
	ObjectHistory(super::common::FinalizedPage<ObjectVersionView>),
	ObjectId(super::common::FinalizedValue<ObjectId>),
	IsBucketOwner(super::common::FinalizedValue<bool>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "query", content = "arguments", rename_all = "snake_case")]
pub enum S3Query {
	BucketById { bucket: BucketId },
	BucketByName { name: BucketName },
	OwnerBuckets { owner: AccountId, page: PageRequest },
	BucketObjects { bucket: BucketId, page: ObjectListRequest },
	ObjectByKey { bucket: BucketId, key: ObjectKey },
	ObjectHistory { bucket: BucketId, key: ObjectKey, page: PageRequest },
	ObjectId { bucket: BucketId, key: ObjectKey },
	IsBucketOwner { owner: AccountId, bucket: BucketId },
}

impl Validate for S3Query {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::BucketById { bucket } => bucket.validate(),
			Self::BucketByName { name } => name.validate(),
			Self::OwnerBuckets { owner, page } => {
				owner.validate()?;
				page.validate()
			},
			Self::BucketObjects { bucket, page } => {
				bucket.validate()?;
				page.validate()
			},
			Self::ObjectByKey { bucket, key } | Self::ObjectId { bucket, key } => {
				bucket.validate()?;
				key.validate()
			},
			Self::ObjectHistory { bucket, key, page } => {
				bucket.validate()?;
				key.validate()?;
				page.validate()
			},
			Self::IsBucketOwner { owner, bucket } => {
				owner.validate()?;
				bucket.validate()
			},
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", content = "arguments", rename_all = "snake_case")]
pub enum S3Command {
	CreateBucket {
		name: BucketName,
	},
	SetController {
		bucket: BucketId,
		expected_bucket_version: u64,
		controller: AccountId,
		enabled: bool,
	},
	TransferBucket {
		bucket: BucketId,
		expected_bucket_version: u64,
		new_owner: AccountId,
	},
	SetArchived {
		bucket: BucketId,
		expected_bucket_version: u64,
		archived: bool,
	},
	SetVersioning {
		bucket: BucketId,
		expected_bucket_version: u64,
		enabled: bool,
	},
	PutObject {
		bucket: BucketId,
		key: ObjectKey,
		content: ContentCommitment,
		expected_object_version: Option<u64>,
	},
	DeleteObject {
		bucket: BucketId,
		key: ObjectKey,
		expected_object_version: u64,
	},
	DeleteBucket {
		bucket: BucketId,
		expected_bucket_version: u64,
	},
}

impl Validate for S3Command {
	fn validate(&self) -> DomainResult<()> {
		match self {
			Self::CreateBucket { name } => name.validate(),
			Self::SetController { bucket, expected_bucket_version, controller, .. } => {
				bucket.validate()?;
				ensure_version(*expected_bucket_version, "bucket")?;
				controller.validate()
			},
			Self::TransferBucket { bucket, expected_bucket_version, new_owner } => {
				bucket.validate()?;
				ensure_version(*expected_bucket_version, "bucket")?;
				new_owner.validate()
			},
			Self::SetArchived { bucket, expected_bucket_version, .. }
			| Self::SetVersioning { bucket, expected_bucket_version, .. }
			| Self::DeleteBucket { bucket, expected_bucket_version } => {
				bucket.validate()?;
				ensure_version(*expected_bucket_version, "bucket")
			},
			Self::PutObject { bucket, key, content, expected_object_version } => {
				bucket.validate()?;
				key.validate()?;
				content.validate()?;
				if expected_object_version == &Some(0) {
					return Err(invalid("expected object version must be non-zero"));
				}
				Ok(())
			},
			Self::DeleteObject { bucket, key, expected_object_version } => {
				bucket.validate()?;
				key.validate()?;
				ensure_version(*expected_object_version, "object")
			},
		}
	}
}

fn ensure_version(version: u64, resource: &str) -> DomainResult<()> {
	if version == 0 {
		Err(invalid(format!("expected {resource} version must be non-zero")))
	} else {
		Ok(())
	}
}

/// Marker returned by transports after deterministic object-ID resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedObject {
	pub object: ObjectId,
	pub bucket: BucketId,
	pub key: ObjectKey,
}
