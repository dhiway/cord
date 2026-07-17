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

use super::*;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidationError {
	Bounds,
	Enum,
	CrossField,
	OperationMismatch,
	ProgressMismatch,
	ErrorMismatch,
	Wire,
}

pub(super) fn valid_text(value: &str, min: usize, max: usize) -> bool {
	let length = value.len();
	length >= min && length <= max && value.nfc().eq(value.chars())
}
fn valid_cid(value: &str) -> bool {
	valid_text(value, 1, 128)
}
fn providers(value: &[ProviderId]) -> bool {
	(2..=4).contains(&value.len())
}

impl StorageV2Payload {
	pub(crate) fn validate(&self) -> Result<(), ValidationError> {
		let valid = match self {
			Self::BucketCreate { providers: p, encryption, .. } => providers(p) && *encryption <= 1,
			Self::BucketGet { .. } | Self::ReplicaStatus { .. } | Self::CheckpointStatus { .. } => {
				true
			},
			Self::BucketGrant { role, issued_at, expires_at, .. }
			| Self::DriveShare { role, issued_at, expires_at, .. } => *role <= 2 && expires_at > issued_at,
			Self::BucketRevoke { .. } => true,
			Self::ObjectPut { cid, encrypted, .. } => valid_cid(cid) && *encrypted <= 1,
			Self::ObjectGet { cid, .. }
			| Self::ObjectDelete { cid, .. }
			| Self::ObjectStatus { cid, .. }
			| Self::DeletionStatus { cid, .. } => valid_cid(cid),
			Self::ObjectRange { cid, length, .. } => valid_cid(cid) && *length > 0,
			Self::CheckpointSubscribe { .. } | Self::ReplicaSubscribe { .. } => true,
			Self::DeletionSubscribe { cid, .. } => valid_cid(cid),
			Self::DriveRead { path, manifest, .. } => {
				valid_text(path, 1, 4096) && manifest.as_ref().is_none_or(|cid| valid_cid(cid))
			},
			Self::DriveCommit { manifest, bytes, mode, .. } => {
				valid_cid(manifest) && (1..=4_194_304).contains(&bytes.len()) && *mode <= 2
			},
			Self::S3Put { bucket, key, cid, metadata, media_type, if_match, .. } => {
				valid_text(bucket, 1, 128)
					&& (1..=1024).contains(&key.len())
					&& valid_cid(cid)
					&& metadata.len() <= 32_768
					&& valid_text(media_type, 1, 256)
					&& if_match.as_ref().is_none_or(|value| valid_text(value, 64, 64))
			},
			Self::S3Get { bucket, key, .. } => {
				valid_text(bucket, 1, 128) && (1..=1024).contains(&key.len())
			},
			Self::S3List { bucket, prefix, cursor, limit } => {
				valid_text(bucket, 1, 128)
					&& prefix.as_ref().is_none_or(|value| value.len() <= 1024)
					&& cursor.as_ref().is_none_or(|value| (1..=2048).contains(&value.len()))
					&& (1..=100).contains(limit)
			},
			Self::S3Delete { bucket, key, if_match, .. } => {
				valid_text(bucket, 1, 128)
					&& (1..=1024).contains(&key.len())
					&& if_match.as_ref().is_none_or(|value| valid_text(value, 64, 64))
			},
			Self::Publish { cid, .. } => valid_cid(cid),
			Self::Resolve { name, .. } => valid_text(name, 1, 256),
			Self::KeysExport { recipient_key, .. } => (32..=256).contains(&recipient_key.len()),
			Self::KeysImport { wrapped_key, replace, .. } => {
				(32..=1024).contains(&wrapped_key.len()) && *replace <= 1
			},
		};
		if valid {
			Ok(())
		} else {
			Err(ValidationError::Bounds)
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Finality {
	pub number: u64,
	pub hash: Hash32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Checkpoint {
	pub root: Hash32,
	pub from: u64,
	pub to: u64,
	pub replicas: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderReceipt {
	pub provider: ProviderId,
	pub cid: String,
	pub length: u64,
	pub signature: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StorageV2Result {
	BucketCreate {
		bucket_id: BucketId,
		version: u64,
		finalized: Finality,
	},
	BucketGet {
		owner: Id32,
		version: u64,
		replica_count: u32,
		primary: ProviderId,
		providers: Vec<ProviderId>,
		finalized: Finality,
	},
	BucketGrant {
		grant_id: GrantId,
		version: u64,
		finalized: Finality,
	},
	BucketRevoke {
		grant_id: GrantId,
		version: u64,
		finalized: Finality,
	},
	ObjectPut {
		receipt: ProviderReceipt,
		publishable: bool,
		finalized: Finality,
	},
	ObjectGet {
		cid: String,
		length: u64,
		checkpoint: Checkpoint,
	},
	ObjectRange {
		cid: String,
		offset: u64,
		length: u64,
		total: u64,
		checkpoint: Checkpoint,
	},
	ObjectDelete {
		version: u64,
		pending: u32,
		confirmed: u32,
		finalized: Finality,
	},
	ObjectStatus {
		state: u8,
		receipt: Option<ProviderReceipt>,
		checkpoint: Option<Checkpoint>,
		replicas: u32,
		publishable: bool,
		finalized: Finality,
	},
	CheckpointStatus {
		checkpoint: Checkpoint,
		sequence: u32,
		block: u64,
		quorum: u32,
		finalized: Finality,
	},
	CheckpointSubscribe {
		operation_id: OperationId,
		cursor: u64,
	},
	ReplicaStatus {
		primary: ProviderId,
		providers: Vec<ProviderId>,
		healthy: u32,
		last_checkpoint: u64,
		pending: u32,
		finalized: Finality,
	},
	ReplicaSubscribe {
		operation_id: OperationId,
		cursor: u64,
	},
	DeletionStatus {
		version: u64,
		confirmations: u32,
		root: Hash32,
		finalized: Finality,
	},
	DeletionSubscribe {
		operation_id: OperationId,
		cursor: u64,
	},
	DriveRead {
		manifest: String,
		entry: String,
		version: u64,
		finalized: Finality,
	},
	DriveCommit {
		manifest: String,
		version: u64,
		checkpoint: Checkpoint,
		finalized: Finality,
	},
	DriveShare {
		grant_id: GrantId,
		version: u64,
		finalized: Finality,
	},
	S3Put {
		etag: String,
		version: u64,
		finalized: Finality,
	},
	S3Get {
		cid: String,
		etag: String,
		version: u64,
		finalized: Finality,
	},
	S3List {
		cids: Vec<String>,
		cursor: Option<Vec<u8>>,
		version: u64,
		finalized: Finality,
	},
	S3Delete {
		version: u64,
		remaining_history: u32,
		finalized: Finality,
	},
	Publish {
		name_hash: Hash32,
		cid: String,
		finalized: Finality,
	},
	Resolve {
		name_id: NameId,
		cid: String,
		version: u64,
		checkpoint: Checkpoint,
		finalized: Finality,
	},
	KeysExport {
		wrapped_key: Vec<u8>,
		algorithm: u16,
		key_version: u32,
	},
	KeysImport {
		key_id: Id32,
		key_version: u32,
	},
}

impl StorageV2Result {
	pub(crate) const fn operation(&self) -> StorageV2Operation {
		match self {
			Self::BucketCreate { .. } => StorageV2Operation::BucketCreate,
			Self::BucketGet { .. } => StorageV2Operation::BucketGet,
			Self::BucketGrant { .. } => StorageV2Operation::BucketGrant,
			Self::BucketRevoke { .. } => StorageV2Operation::BucketRevoke,
			Self::ObjectPut { .. } => StorageV2Operation::ObjectPut,
			Self::ObjectGet { .. } => StorageV2Operation::ObjectGet,
			Self::ObjectRange { .. } => StorageV2Operation::ObjectRange,
			Self::ObjectDelete { .. } => StorageV2Operation::ObjectDelete,
			Self::ObjectStatus { .. } => StorageV2Operation::ObjectStatus,
			Self::CheckpointStatus { .. } => StorageV2Operation::CheckpointStatus,
			Self::CheckpointSubscribe { .. } => StorageV2Operation::CheckpointSubscribe,
			Self::ReplicaStatus { .. } => StorageV2Operation::ReplicaStatus,
			Self::ReplicaSubscribe { .. } => StorageV2Operation::ReplicaSubscribe,
			Self::DeletionStatus { .. } => StorageV2Operation::DeletionStatus,
			Self::DeletionSubscribe { .. } => StorageV2Operation::DeletionSubscribe,
			Self::DriveRead { .. } => StorageV2Operation::DriveRead,
			Self::DriveCommit { .. } => StorageV2Operation::DriveCommit,
			Self::DriveShare { .. } => StorageV2Operation::DriveShare,
			Self::S3Put { .. } => StorageV2Operation::S3Put,
			Self::S3Get { .. } => StorageV2Operation::S3Get,
			Self::S3List { .. } => StorageV2Operation::S3List,
			Self::S3Delete { .. } => StorageV2Operation::S3Delete,
			Self::Publish { .. } => StorageV2Operation::Publish,
			Self::Resolve { .. } => StorageV2Operation::Resolve,
			Self::KeysExport { .. } => StorageV2Operation::KeysExport,
			Self::KeysImport { .. } => StorageV2Operation::KeysImport,
		}
	}
	pub(crate) fn validate(&self) -> Result<(), ValidationError> {
		let valid = match self {
			Self::BucketGet { providers: p, .. } | Self::ReplicaStatus { providers: p, .. } => {
				providers(p)
			},
			Self::ObjectPut { receipt, .. } => valid_cid(&receipt.cid),
			Self::ObjectGet { cid, checkpoint, .. } => {
				valid_cid(cid) && checkpoint.to >= checkpoint.from
			},
			Self::ObjectRange { cid, offset, length, total, checkpoint } => {
				valid_cid(cid)
					&& *length > 0 && offset.checked_add(*length).is_some_and(|end| end <= *total)
					&& checkpoint.to >= checkpoint.from
			},
			Self::ObjectStatus { state, receipt, checkpoint, .. } => {
				*state <= 4
					&& receipt.as_ref().is_none_or(|r| valid_cid(&r.cid))
					&& checkpoint.as_ref().is_none_or(|c| c.to >= c.from)
			},
			Self::CheckpointStatus { checkpoint, .. } => checkpoint.to >= checkpoint.from,
			Self::DriveRead { manifest, entry, .. } => valid_cid(manifest) && valid_cid(entry),
			Self::DriveCommit { manifest, checkpoint, .. } => {
				valid_cid(manifest) && checkpoint.to >= checkpoint.from
			},
			Self::S3Put { etag, .. } => valid_text(etag, 64, 64),
			Self::S3Get { cid, etag, .. } => valid_cid(cid) && valid_text(etag, 64, 64),
			Self::S3List { cids, cursor, .. } => {
				cids.len() <= 100
					&& cids.iter().all(|cid| valid_cid(cid))
					&& cursor.as_ref().is_none_or(|value| (1..=2048).contains(&value.len()))
			},
			Self::Publish { cid, .. } => valid_cid(cid),
			Self::Resolve { cid, checkpoint, .. } => {
				valid_cid(cid) && checkpoint.to >= checkpoint.from
			},
			Self::KeysExport { wrapped_key, .. } => (32..=1024).contains(&wrapped_key.len()),
			_ => true,
		};
		if valid {
			Ok(())
		} else {
			Err(ValidationError::Bounds)
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StorageV2Progress {
	State {
		completed: u64,
		total: Option<u64>,
		chunks_acked: Option<u64>,
		replicas_confirmed: Option<u64>,
	},
	Bytes {
		offset: u64,
		bytes: Vec<u8>,
	},
}
impl StorageV2Progress {
	pub(crate) fn validate(&self, operation: StorageV2Operation) -> Result<(), ValidationError> {
		let byte_operation = matches!(
			operation,
			StorageV2Operation::ObjectGet
				| StorageV2Operation::ObjectRange
				| StorageV2Operation::S3Get
		);
		let valid = match self {
			Self::Bytes { bytes, .. } => byte_operation && bytes.len() <= 4_194_304,
			Self::State { .. } => !byte_operation,
		};
		if valid {
			Ok(())
		} else {
			Err(ValidationError::ProgressMismatch)
		}
	}
}

pub(crate) const STORAGE_V2_ERRORS: [(u16, &str, bool); 89] = [
	(100, "WIRE_SCHEMA_INVALID", false),
	(101, "WIRE_NON_CANONICAL", false),
	(102, "WIRE_VERSION_MISMATCH", false),
	(103, "WIRE_GENESIS_MISMATCH", false),
	(104, "WIRE_DESCRIPTOR_MISMATCH", false),
	(105, "WIRE_SEQUENCE_INVALID", false),
	(106, "REQUEST_DEADLINE_EXPIRED", false),
	(107, "REQUEST_CANCELLED", false),
	(108, "REQUEST_NOT_FOUND", false),
	(109, "GRANT_REQUIRED", false),
	(110, "GRANT_SCOPE_DENIED", false),
	(111, "GRANT_EXPIRED", false),
	(112, "GRANT_REVOKED", false),
	(113, "HOST_OUTBOX_UNAVAILABLE", false),
	(114, "HOST_OUTBOX_FULL", true),
	(115, "HOST_OUTBOX_CORRUPT", false),
	(116, "HOST_OUTBOX_EXPIRED", false),
	(200, "STORAGE_CHUNK_OUT_OF_ORDER", false),
	(201, "STORAGE_CHUNK_TOO_LARGE", false),
	(202, "STORAGE_CHUNK_MISSING", false),
	(203, "STORAGE_LENGTH_MISMATCH", false),
	(204, "STORAGE_CID_MISMATCH", false),
	(205, "STORAGE_OBJECT_TOO_LARGE", false),
	(206, "STORAGE_IDEMPOTENCY_CONFLICT", false),
	(207, "STORAGE_RANGE_INVALID", false),
	(208, "STORAGE_INTEGRITY_FAILED", false),
	(209, "STORAGE_NOT_PUBLISHABLE", true),
	(210, "STORAGE_NOT_FOUND", false),
	(211, "ENCRYPTION_NONCE_REUSE", false),
	(220, "STORAGE_CHECKPOINT_WRONG_DOMAIN", false),
	(221, "STORAGE_CHECKPOINT_WRONG_VERSION", false),
	(222, "STORAGE_CHECKPOINT_WRONG_BUCKET", false),
	(223, "STORAGE_CHECKPOINT_WRONG_KEY", false),
	(224, "STORAGE_CHECKPOINT_STALE_NONCE", true),
	(225, "STORAGE_CHECKPOINT_WRONG_WINDOW", false),
	(226, "CAPABILITY_SIGNATURE_INVALID", false),
	(227, "CAPABILITY_AUDIENCE_INVALID", false),
	(228, "CAPABILITY_CONTENT_INVALID", false),
	(229, "CAPABILITY_NONCE_REPLAY", false),
	(230, "CAPABILITY_EXPIRED", false),
	(231, "CAPABILITY_ISSUER_REVOKED", false),
	(232, "RESUME_SIGNATURE_INVALID", false),
	(233, "RESUME_AUDIENCE_INVALID", false),
	(234, "RESUME_REPLAY", false),
	(235, "RESUME_EXPIRED", false),
	(236, "RESUME_REVOKED", false),
	(237, "RESUME_CURSOR_INVALID", false),
	(238, "PROVIDER_RECOVERY_TABLE_FULL", false),
	(239, "STORAGE_CHECKPOINT_INSUFFICIENT_QUORUM", false),
	(240, "STORAGE_CHECKPOINT_SEQUENCE_INVALID", false),
	(241, "STORAGE_CHECKPOINT_EQUIVOCATION", false),
	(250, "BUCKET_NOT_FOUND", false),
	(251, "BUCKET_VERSION_CONFLICT", false),
	(252, "BUCKET_MEMBER_LIMIT", false),
	(253, "AGREEMENT_INVALID_STATE", false),
	(254, "AGREEMENT_CAPACITY_EXCEEDED", false),
	(255, "PROVIDER_INELIGIBLE", true),
	(256, "PROVIDER_ORG_UNKNOWN", false),
	(257, "PROVIDER_ATTESTATION_INVALID", false),
	(258, "PROVIDER_ATTESTATION_EXPIRED", false),
	(259, "PROVIDER_SLA_INVALID", false),
	(260, "PROVIDER_SERVICE_KEY_INVALID", false),
	(261, "STORAGE_CURSOR_STALE", true),
	(300, "DRIVE_NAME_INVALID", false),
	(301, "DRIVE_PATH_TOO_LONG", false),
	(302, "DRIVE_DEPTH_EXCEEDED", false),
	(303, "DRIVE_CHILD_LIMIT", false),
	(304, "DRIVE_METADATA_LIMIT", false),
	(305, "DRIVE_ORDER_INVALID", false),
	(306, "DRIVE_VERSION_CONFLICT", false),
	(307, "DRIVE_REFERENCE_UNPUBLISHABLE", false),
	(320, "S3_BUCKET_NAME_INVALID", false),
	(321, "S3_KEY_INVALID", false),
	(322, "S3_METADATA_LIMIT", false),
	(323, "S3_PRECONDITION_FAILED", false),
	(324, "S3_NOT_FOUND", false),
	(325, "S3_HISTORY_LIMIT", false),
	(400, "IDENTITY_AUDIENCE_INVALID", false),
	(401, "IDENTITY_CHALLENGE_REPLAY", false),
	(402, "IDENTITY_PROOF_EXPIRED", false),
	(403, "IDENTITY_EPOCH_INVALID", false),
	(404, "IDENTITY_DISCLOSURE_DENIED", false),
	(405, "IDENTITY_HUMANITY_UNAVAILABLE", true),
	(406, "IDENTITY_ENTITLEMENT_UNAVAILABLE", true),
	(407, "SIGNING_CONSENT_REQUIRED", false),
	(408, "IDENTITY_RECOVERY_ENTROPY_FAILED", false),
	(409, "IDENTITY_RECOVERY_INSTALL_FAILED", false),
	(410, "IDENTITY_OLD_INCARNATION", false),
	(411, "IDENTITY_RETIRED_SET_FULL", false),
];
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub(crate) struct StorageV2ErrorDetails {
	pub message: Option<String>,
	pub lower: Option<u64>,
	pub upper: Option<u64>,
	pub hash: Option<Hash32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StorageV2Error {
	pub code: u16,
	pub name: String,
	pub retryable: bool,
	pub details: StorageV2ErrorDetails,
}
impl StorageV2Error {
	pub(crate) fn validate_for(
		&self,
		operation: StorageV2Operation,
	) -> Result<(), ValidationError> {
		let tuple = STORAGE_V2_ERRORS.iter().find(|(code, _, _)| *code == self.code);
		let valid = tuple
			.is_some_and(|(_, name, retryable)| *name == self.name && *retryable == self.retryable)
			&& operation_allows_error_family(operation, error_family(self.code))
			&& self.details.message.as_ref().is_none_or(|value| valid_text(value, 1, 256));
		if valid {
			Ok(())
		} else {
			Err(ValidationError::ErrorMismatch)
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorFamily {
	Common,
	Content,
	Proof,
	Control,
	DriveS3,
	Other,
}

const fn error_family(code: u16) -> ErrorFamily {
	match code {
		100..=116 => ErrorFamily::Common,
		200..=211 => ErrorFamily::Content,
		220..=241 => ErrorFamily::Proof,
		250..=261 => ErrorFamily::Control,
		300..=325 => ErrorFamily::DriveS3,
		_ => ErrorFamily::Other,
	}
}

const fn operation_allows_error_family(operation: StorageV2Operation, family: ErrorFamily) -> bool {
	use ErrorFamily::{Common, Content, Control, DriveS3, Proof};
	match operation {
		StorageV2Operation::BucketCreate
		| StorageV2Operation::BucketGet
		| StorageV2Operation::BucketGrant
		| StorageV2Operation::BucketRevoke => matches!(family, Common | Control),
		StorageV2Operation::ObjectPut => matches!(family, Common | Content | Proof),
		StorageV2Operation::ObjectDelete | StorageV2Operation::Publish => {
			matches!(family, Common | Content | Control)
		},
		StorageV2Operation::ObjectGet
		| StorageV2Operation::ObjectRange
		| StorageV2Operation::ObjectStatus
		| StorageV2Operation::DeletionStatus
		| StorageV2Operation::DeletionSubscribe
		| StorageV2Operation::Resolve => matches!(family, Common | Content),
		StorageV2Operation::CheckpointStatus | StorageV2Operation::CheckpointSubscribe => {
			matches!(family, Common | Proof)
		},
		StorageV2Operation::ReplicaStatus
		| StorageV2Operation::ReplicaSubscribe
		| StorageV2Operation::DriveShare => matches!(family, Common | Control),
		StorageV2Operation::DriveRead
		| StorageV2Operation::DriveCommit
		| StorageV2Operation::S3Put
		| StorageV2Operation::S3Get => matches!(family, Common | DriveS3 | Content),
		StorageV2Operation::S3List => matches!(family, Common | DriveS3 | Control),
		StorageV2Operation::S3Delete => matches!(family, Common | DriveS3),
		StorageV2Operation::KeysExport | StorageV2Operation::KeysImport => {
			matches!(family, Common)
		},
	}
}

struct Encoder(Vec<u8>);
impl Encoder {
	fn head(&mut self, major: u8, value: u64) {
		if value < 24 {
			self.0.push((major << 5) | value as u8)
		} else if value <= u8::MAX.into() {
			self.0.extend([(major << 5) | 24, value as u8])
		} else if value <= u16::MAX.into() {
			self.0.push((major << 5) | 25);
			self.0.extend((value as u16).to_be_bytes())
		} else if value <= u32::MAX.into() {
			self.0.push((major << 5) | 26);
			self.0.extend((value as u32).to_be_bytes())
		} else {
			self.0.push((major << 5) | 27);
			self.0.extend(value.to_be_bytes())
		}
	}
	fn uint(&mut self, value: u64) {
		self.head(0, value)
	}
	fn bytes(&mut self, value: &[u8]) {
		self.head(2, value.len() as u64);
		self.0.extend(value)
	}
	fn text(&mut self, value: &str) {
		self.head(3, value.len() as u64);
		self.0.extend(value.as_bytes())
	}
	fn array(&mut self, length: usize) {
		self.head(4, length as u64)
	}
	fn map(&mut self, length: usize) {
		self.head(5, length as u64)
	}
}
fn encode_payload(encoder: &mut Encoder, payload: &StorageV2Payload) {
	macro_rules! key {
		($key:expr) => {
			encoder.uint($key)
		};
	}
	match payload {
		StorageV2Payload::BucketCreate { replica_count, providers, encryption } => {
			encoder.map(3);
			key!(0);
			encoder.uint((*replica_count).into());
			key!(1);
			encoder.array(providers.len());
			for p in providers {
				encoder.bytes(&p.0)
			}
			key!(2);
			encoder.uint((*encryption).into())
		},
		StorageV2Payload::BucketGet { bucket_id, at } => {
			encoder.map(1 + usize::from(at.is_some()));
			key!(0);
			encoder.bytes(&bucket_id.0);
			if let Some(at) = at {
				key!(1);
				encoder.bytes(&at.0)
			}
		},
		StorageV2Payload::BucketGrant { bucket_id, subject, role, issued_at, expires_at }
		| StorageV2Payload::DriveShare { bucket_id, subject, role, issued_at, expires_at } => {
			encoder.map(5);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.bytes(&subject.0);
			key!(2);
			encoder.uint((*role).into());
			key!(3);
			encoder.uint(*issued_at);
			key!(4);
			encoder.uint(*expires_at)
		},
		StorageV2Payload::BucketRevoke { bucket_id, grant_id, expected_version } => {
			encoder.map(3);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.bytes(&grant_id.0);
			key!(2);
			encoder.uint(*expected_version)
		},
		StorageV2Payload::ObjectPut { bucket_id, cid, length, encrypted, transfer_id } => {
			encoder.map(5);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(cid);
			key!(2);
			encoder.uint(*length);
			key!(3);
			encoder.uint((*encrypted).into());
			key!(4);
			encoder.bytes(&transfer_id.0)
		},
		StorageV2Payload::ObjectGet { bucket_id, cid }
		| StorageV2Payload::ObjectStatus { bucket_id, cid }
		| StorageV2Payload::DeletionStatus { bucket_id, cid } => {
			encoder.map(2);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(cid)
		},
		StorageV2Payload::ObjectRange { bucket_id, cid, offset, length } => {
			encoder.map(4);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(cid);
			key!(2);
			encoder.uint(*offset);
			key!(3);
			encoder.uint(*length)
		},
		StorageV2Payload::ObjectDelete { bucket_id, cid, expected_version } => {
			encoder.map(3);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(cid);
			key!(2);
			encoder.uint(*expected_version)
		},
		StorageV2Payload::CheckpointStatus { bucket_id, root } => {
			encoder.map(1 + usize::from(root.is_some()));
			key!(0);
			encoder.bytes(&bucket_id.0);
			if let Some(root) = root {
				key!(1);
				encoder.bytes(&root.0)
			}
		},
		StorageV2Payload::CheckpointSubscribe { bucket_id, cursor }
		| StorageV2Payload::ReplicaSubscribe { bucket_id, cursor } => {
			encoder.map(2);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.uint(*cursor)
		},
		StorageV2Payload::ReplicaStatus { bucket_id } => {
			encoder.map(1);
			key!(0);
			encoder.bytes(&bucket_id.0)
		},
		StorageV2Payload::DeletionSubscribe { bucket_id, cid, cursor } => {
			encoder.map(3);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(cid);
			key!(2);
			encoder.uint(*cursor)
		},
		StorageV2Payload::DriveRead { bucket_id, path, manifest } => {
			encoder.map(2 + usize::from(manifest.is_some()));
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(path);
			if let Some(cid) = manifest {
				key!(2);
				encoder.text(cid)
			}
		},
		StorageV2Payload::DriveCommit { bucket_id, manifest, bytes, expected_version, mode } => {
			encoder.map(5);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.text(manifest);
			key!(2);
			encoder.bytes(bytes);
			key!(3);
			encoder.uint(*expected_version);
			key!(4);
			encoder.uint((*mode).into())
		},
		StorageV2Payload::S3Put {
			bucket,
			key,
			cid,
			metadata,
			media_type,
			if_match,
			transfer_id,
		} => {
			encoder.map(6 + usize::from(if_match.is_some()));
			key!(0);
			encoder.text(bucket);
			key!(1);
			encoder.bytes(key);
			key!(2);
			encoder.text(cid);
			key!(3);
			encoder.bytes(metadata);
			key!(4);
			encoder.text(media_type);
			if let Some(value) = if_match {
				key!(5);
				encoder.text(value)
			}
			key!(6);
			encoder.bytes(&transfer_id.0)
		},
		StorageV2Payload::S3Get { bucket, key, version } => {
			encoder.map(2 + usize::from(version.is_some()));
			key!(0);
			encoder.text(bucket);
			key!(1);
			encoder.bytes(key);
			if let Some(version) = version {
				key!(2);
				encoder.uint(*version)
			}
		},
		StorageV2Payload::S3List { bucket, prefix, cursor, limit } => {
			encoder.map(2 + usize::from(prefix.is_some()) + usize::from(cursor.is_some()));
			key!(0);
			encoder.text(bucket);
			if let Some(prefix) = prefix {
				key!(1);
				encoder.bytes(prefix)
			}
			if let Some(cursor) = cursor {
				key!(2);
				encoder.bytes(cursor)
			}
			key!(3);
			encoder.uint((*limit).into())
		},
		StorageV2Payload::S3Delete { bucket, key, if_match, transfer_id } => {
			encoder.map(3 + usize::from(if_match.is_some()));
			key!(0);
			encoder.text(bucket);
			key!(1);
			encoder.bytes(key);
			if let Some(value) = if_match {
				key!(2);
				encoder.text(value)
			}
			key!(3);
			encoder.bytes(&transfer_id.0)
		},
		StorageV2Payload::Publish { name_hash, cid, expected_version } => {
			encoder.map(3);
			key!(0);
			encoder.bytes(&name_hash.0);
			key!(1);
			encoder.text(cid);
			key!(2);
			encoder.uint(*expected_version)
		},
		StorageV2Payload::Resolve { name, version, at } => {
			encoder.map(1 + usize::from(version.is_some()) + usize::from(at.is_some()));
			key!(0);
			encoder.text(name);
			if let Some(version) = version {
				key!(1);
				encoder.uint(*version)
			}
			if let Some(at) = at {
				key!(2);
				encoder.bytes(&at.0)
			}
		},
		StorageV2Payload::KeysExport { bucket_id, key_version, recipient_key } => {
			encoder.map(3);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.uint((*key_version).into());
			key!(2);
			encoder.bytes(recipient_key)
		},
		StorageV2Payload::KeysImport { bucket_id, wrapped_key, replace, key_version } => {
			encoder.map(4);
			key!(0);
			encoder.bytes(&bucket_id.0);
			key!(1);
			encoder.bytes(wrapped_key);
			key!(2);
			encoder.uint((*replace).into());
			key!(3);
			encoder.uint((*key_version).into())
		},
	}
}
pub(crate) fn encode_intent(intent: &StorageV2Intent) -> Result<Vec<u8>, ValidationError> {
	intent.payload.validate()?;
	let mut e = Encoder(Vec::new());
	let count = 6
		+ usize::from(intent.grant_id.is_some())
		+ usize::from(intent.operation_id.is_some())
		+ usize::from(intent.idempotency_key.is_some());
	e.map(count);
	e.uint(0);
	e.uint(2);
	e.uint(1);
	e.bytes(&intent.request_id.0);
	e.uint(2);
	e.text(&intent.product_id);
	e.uint(3);
	e.uint(intent.operation.code().into());
	if let Some(grant) = intent.grant_id {
		e.uint(4);
		e.bytes(&grant.0)
	}
	if let Some(operation) = intent.operation_id {
		e.uint(5);
		e.bytes(&operation.0)
	}
	if let Some(key) = &intent.idempotency_key {
		e.uint(6);
		e.bytes(key)
	}
	e.uint(7);
	e.uint(intent.deadline_block);
	e.uint(8);
	encode_payload(&mut e, &intent.payload);
	Ok(e.0)
}

fn wire_uint(value: &ciborium::value::Value) -> Option<u64> {
	let ciborium::value::Value::Integer(value) = value else { return None };
	u64::try_from(*value).ok()
}

pub(crate) fn encode_wire_value(
	value: &ciborium::value::Value,
) -> Result<Vec<u8>, ValidationError> {
	use ciborium::value::Value;
	let mut encoder = Encoder(Vec::new());
	match value {
		Value::Integer(_) => encoder.uint(wire_uint(value).ok_or(ValidationError::Wire)?),
		Value::Bytes(value) => encoder.bytes(value),
		Value::Text(value) if valid_text(value, 0, usize::MAX) => encoder.text(value),
		Value::Text(_) => return Err(ValidationError::Wire),
		Value::Bool(value) => encoder.0.push(if *value { 0xf5 } else { 0xf4 }),
		Value::Array(values) => {
			encoder.array(values.len());
			for value in values {
				encoder.0.extend(encode_wire_value(value)?);
			}
		},
		Value::Map(values) => {
			let mut keys = std::collections::BTreeSet::new();
			let mut entries = Vec::with_capacity(values.len());
			for (key, value) in values {
				let number = wire_uint(key).ok_or(ValidationError::Wire)?;
				if !keys.insert(number) {
					return Err(ValidationError::Wire);
				}
				entries.push((encode_wire_value(key)?, encode_wire_value(value)?));
			}
			entries.sort_by(|left, right| {
				left.0.len().cmp(&right.0.len()).then_with(|| left.0.cmp(&right.0))
			});
			encoder.map(entries.len());
			for (key, value) in entries {
				encoder.0.extend(key);
				encoder.0.extend(value);
			}
		},
		_ => return Err(ValidationError::Wire),
	}
	Ok(encoder.0)
}

fn map_fields(
	value: &ciborium::value::Value,
) -> Result<Vec<(u64, &ciborium::value::Value)>, ValidationError> {
	let ciborium::value::Value::Map(values) = value else { return Err(ValidationError::Wire) };
	let mut fields = Vec::with_capacity(values.len());
	let mut seen = std::collections::BTreeSet::new();
	for (key, value) in values {
		let key = wire_uint(key).ok_or(ValidationError::Wire)?;
		if !seen.insert(key) {
			return Err(ValidationError::Wire);
		}
		fields.push((key, value));
	}
	Ok(fields)
}

fn close_fields(
	fields: &[(u64, &ciborium::value::Value)],
	required: &[u64],
	optional: &[u64],
) -> Result<(), ValidationError> {
	if required.iter().any(|key| !fields.iter().any(|(actual, _)| actual == key))
		|| fields.iter().any(|(key, _)| !required.contains(key) && !optional.contains(key))
	{
		return Err(ValidationError::Wire);
	}
	Ok(())
}

fn payload_keys(operation: StorageV2Operation) -> (&'static [u64], &'static [u64]) {
	match operation {
		StorageV2Operation::BucketCreate => (&[0, 1, 2], &[]),
		StorageV2Operation::BucketGet => (&[0], &[1]),
		StorageV2Operation::BucketGrant => (&[0, 1, 2, 3, 4], &[]),
		StorageV2Operation::BucketRevoke => (&[0, 1, 2], &[]),
		StorageV2Operation::ObjectPut => (&[0, 1, 2, 3, 4], &[]),
		StorageV2Operation::ObjectGet => (&[0, 1], &[]),
		StorageV2Operation::ObjectRange => (&[0, 1, 2, 3], &[]),
		StorageV2Operation::ObjectDelete => (&[0, 1, 2], &[]),
		StorageV2Operation::ObjectStatus => (&[0, 1], &[]),
		StorageV2Operation::CheckpointStatus => (&[0], &[1]),
		StorageV2Operation::CheckpointSubscribe => (&[0, 1], &[]),
		StorageV2Operation::ReplicaStatus => (&[0], &[]),
		StorageV2Operation::ReplicaSubscribe => (&[0, 1], &[]),
		StorageV2Operation::DeletionStatus => (&[0, 1], &[]),
		StorageV2Operation::DeletionSubscribe => (&[0, 1, 2], &[]),
		StorageV2Operation::DriveRead => (&[0, 1], &[2]),
		StorageV2Operation::DriveCommit | StorageV2Operation::DriveShare => (&[0, 1, 2, 3, 4], &[]),
		StorageV2Operation::S3Put => (&[0, 1, 2, 3, 4, 6], &[5]),
		StorageV2Operation::S3Get => (&[0, 1], &[2]),
		StorageV2Operation::S3List => (&[0, 3], &[1, 2]),
		StorageV2Operation::S3Delete => (&[0, 1, 3], &[2]),
		StorageV2Operation::Publish => (&[0, 1, 2], &[]),
		StorageV2Operation::Resolve => (&[0], &[1, 2]),
		StorageV2Operation::KeysExport => (&[0, 1, 2], &[]),
		StorageV2Operation::KeysImport => (&[0, 1, 2, 3], &[]),
	}
}

fn optional_bytes(value: Option<&ciborium::value::Value>, minimum: usize, maximum: usize) -> bool {
	match value {
		None => true,
		Some(ciborium::value::Value::Bytes(value)) => (minimum..=maximum).contains(&value.len()),
		Some(_) => false,
	}
}

pub(crate) fn decode_canonical_storage_frame(bytes: &[u8]) -> Result<Vec<u8>, ValidationError> {
	use ciborium::value::Value;
	let value: Value = ciborium::from_reader(bytes).map_err(|_| ValidationError::Wire)?;
	let canonical = encode_wire_value(&value)?;
	if canonical != bytes {
		return Err(ValidationError::Wire);
	}
	let fields = map_fields(&value)?;
	let field = |key| fields.iter().find_map(|(actual, value)| (*actual == key).then_some(*value));
	if field(0).and_then(wire_uint) != Some(2) {
		return Err(ValidationError::Wire);
	}
	let code = field(3).and_then(wire_uint).ok_or(ValidationError::Wire)?;
	let operation = STORAGE_V2_OPERATIONS
		.iter()
		.copied()
		.find(|operation| u64::from(operation.code()) == code)
		.ok_or(ValidationError::Wire)?;
	let contract = operation.contract();
	let mut required = vec![0, 1, 2, 3, 7, 8];
	if contract.grant_scope != GrantScope::Public {
		required.push(4);
	}
	if contract.operation_id_required {
		required.push(5);
	}
	close_fields(&fields, &required, &[6])?;
	if !matches!(field(1), Some(Value::Bytes(value)) if value.len() == 16)
		|| !matches!(field(2), Some(Value::Text(value)) if valid_text(value, 1, 128))
		|| !optional_bytes(field(4), 32, 32)
		|| !optional_bytes(field(5), 16, 16)
		|| !optional_bytes(field(6), 1, 64)
		|| field(7).and_then(wire_uint).is_none()
	{
		return Err(ValidationError::Wire);
	}
	let payload = field(8).ok_or(ValidationError::Wire)?;
	let payload_fields = map_fields(payload)?;
	let (required, optional) = payload_keys(operation);
	close_fields(&payload_fields, required, optional)?;
	Ok(canonical)
}
