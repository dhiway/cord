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

//! Private typed intents for the frozen `cord.origin.host/2` storage operation range.
//! The module stays crate-private until the storage authority cutover.

mod validation;
use validation::{StorageV2Error, StorageV2Progress, StorageV2Result};

pub(crate) const STORAGE_V2_PROTOCOL: &str = "cord.origin.host/2";
pub(crate) const STORAGE_V2_REGISTRY_SHA256: &str =
	super::host_v2::generated::REGISTRY_SHA256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Id16(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Id32(pub [u8; 32]);

pub(crate) type RequestId = Id16;
pub(crate) type OperationId = Id16;
pub(crate) type GrantId = Id32;
pub(crate) type BucketId = Id32;
pub(crate) type ProviderId = Id32;
pub(crate) type Subject = Id32;
pub(crate) type Hash32 = Id32;
pub(crate) type NameId = Id32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GrantScope {
	Public,
	BucketAdmin,
	BucketRead,
	BucketReader,
	BucketWriter,
	Publish,
	KeysExport,
	KeysImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResumeMode {
	None,
	ChainIdempotent,
	ProviderToken,
	VerifiedOffset,
	Cursor256,
	PerObject,
	CursorVersioned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cancellation {
	PreEffectOnly,
	StopStreamOrTerminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub(crate) enum StorageV2Operation {
	BucketCreate = 1000,
	BucketGet = 1001,
	BucketGrant = 1002,
	BucketRevoke = 1003,
	ObjectPut = 1010,
	ObjectGet = 1011,
	ObjectRange = 1012,
	ObjectDelete = 1013,
	ObjectStatus = 1014,
	CheckpointStatus = 1020,
	CheckpointSubscribe = 1021,
	ReplicaStatus = 1022,
	ReplicaSubscribe = 1023,
	DeletionStatus = 1024,
	DeletionSubscribe = 1025,
	DriveRead = 1030,
	DriveCommit = 1031,
	DriveShare = 1032,
	S3Put = 1040,
	S3Get = 1041,
	S3List = 1042,
	S3Delete = 1043,
	Publish = 1050,
	Resolve = 1051,
	KeysExport = 1060,
	KeysImport = 1061,
}

pub(crate) const STORAGE_V2_OPERATIONS: [StorageV2Operation; 26] = [
	StorageV2Operation::BucketCreate,
	StorageV2Operation::BucketGet,
	StorageV2Operation::BucketGrant,
	StorageV2Operation::BucketRevoke,
	StorageV2Operation::ObjectPut,
	StorageV2Operation::ObjectGet,
	StorageV2Operation::ObjectRange,
	StorageV2Operation::ObjectDelete,
	StorageV2Operation::ObjectStatus,
	StorageV2Operation::CheckpointStatus,
	StorageV2Operation::CheckpointSubscribe,
	StorageV2Operation::ReplicaStatus,
	StorageV2Operation::ReplicaSubscribe,
	StorageV2Operation::DeletionStatus,
	StorageV2Operation::DeletionSubscribe,
	StorageV2Operation::DriveRead,
	StorageV2Operation::DriveCommit,
	StorageV2Operation::DriveShare,
	StorageV2Operation::S3Put,
	StorageV2Operation::S3Get,
	StorageV2Operation::S3List,
	StorageV2Operation::S3Delete,
	StorageV2Operation::Publish,
	StorageV2Operation::Resolve,
	StorageV2Operation::KeysExport,
	StorageV2Operation::KeysImport,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationContract {
	pub operation: StorageV2Operation,
	pub name: &'static str,
	pub grant_scope: GrantScope,
	pub resume: ResumeMode,
	pub cancellation: Cancellation,
	pub operation_id_required: bool,
	pub state_changing: bool,
}

impl StorageV2Operation {
	pub const fn code(self) -> u16 {
		self as u16
	}

	pub const fn contract(self) -> OperationContract {
		use Cancellation::{PreEffectOnly as Pre, StopStreamOrTerminal as Stop};
		use GrantScope::{
			BucketAdmin, BucketRead, BucketReader, BucketWriter, KeysExport, KeysImport, Public,
			Publish,
		};
		use ResumeMode::{
			ChainIdempotent, Cursor256, CursorVersioned, None, PerObject, ProviderToken,
			VerifiedOffset,
		};
		let (name, grant_scope, resume, cancellation, operation_id_required, state_changing) =
			match self {
				Self::BucketCreate => {
					("storage.bucket.create", BucketAdmin, ChainIdempotent, Pre, true, true)
				},
				Self::BucketGet => ("storage.bucket.get", BucketRead, None, Stop, false, false),
				Self::BucketGrant => {
					("storage.bucket.grant", BucketAdmin, ChainIdempotent, Pre, true, true)
				},
				Self::BucketRevoke => {
					("storage.bucket.revoke", BucketAdmin, ChainIdempotent, Pre, true, true)
				},
				Self::ObjectPut => {
					("storage.object.put", BucketWriter, ProviderToken, Pre, true, true)
				},
				Self::ObjectGet => {
					("storage.object.get", BucketReader, VerifiedOffset, Stop, false, false)
				},
				Self::ObjectRange => {
					("storage.object.range", BucketReader, VerifiedOffset, Stop, false, false)
				},
				Self::ObjectDelete => {
					("storage.object.delete", BucketWriter, ChainIdempotent, Pre, true, true)
				},
				Self::ObjectStatus => {
					("storage.object.status", BucketReader, None, Stop, false, false)
				},
				Self::CheckpointStatus => {
					("storage.checkpoint.status", BucketReader, None, Stop, false, false)
				},
				Self::CheckpointSubscribe => {
					("storage.checkpoint.subscribe", BucketReader, Cursor256, Pre, true, true)
				},
				Self::ReplicaStatus => {
					("storage.replica.status", BucketReader, None, Stop, false, false)
				},
				Self::ReplicaSubscribe => {
					("storage.replica.subscribe", BucketReader, Cursor256, Pre, true, true)
				},
				Self::DeletionStatus => {
					("storage.deletion.status", BucketWriter, None, Stop, false, false)
				},
				Self::DeletionSubscribe => {
					("storage.deletion.subscribe", BucketWriter, Cursor256, Pre, true, true)
				},
				Self::DriveRead => ("storage.drive.read", BucketReader, None, Stop, false, false),
				Self::DriveCommit => {
					("storage.drive.commit", BucketWriter, PerObject, Pre, true, true)
				},
				Self::DriveShare => {
					("storage.drive.share", BucketAdmin, ChainIdempotent, Pre, true, true)
				},
				Self::S3Put => ("storage.s3.put", BucketWriter, ProviderToken, Pre, true, true),
				Self::S3Get => ("storage.s3.get", BucketReader, VerifiedOffset, Stop, false, false),
				Self::S3List => {
					("storage.s3.list", BucketReader, CursorVersioned, Stop, false, false)
				},
				Self::S3Delete => {
					("storage.s3.delete", BucketWriter, ChainIdempotent, Pre, true, true)
				},
				Self::Publish => ("storage.publish", Publish, ChainIdempotent, Pre, true, true),
				Self::Resolve => ("storage.resolve", Public, None, Stop, false, false),
				Self::KeysExport => ("storage.keys.export", KeysExport, None, Pre, true, true),
				Self::KeysImport => ("storage.keys.import", KeysImport, None, Pre, true, true),
			};
		OperationContract {
			operation: self,
			name,
			grant_scope,
			resume,
			cancellation,
			operation_id_required,
			state_changing,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StorageV2Payload {
	BucketCreate {
		replica_count: u32,
		providers: Vec<ProviderId>,
		encryption: u8,
	},
	BucketGet {
		bucket_id: BucketId,
		at: Option<Hash32>,
	},
	BucketGrant {
		bucket_id: BucketId,
		subject: Subject,
		role: u8,
		issued_at: u64,
		expires_at: u64,
	},
	BucketRevoke {
		bucket_id: BucketId,
		grant_id: GrantId,
		expected_version: u64,
	},
	ObjectPut {
		bucket_id: BucketId,
		cid: String,
		length: u64,
		encrypted: u8,
		transfer_id: OperationId,
	},
	ObjectGet {
		bucket_id: BucketId,
		cid: String,
	},
	ObjectRange {
		bucket_id: BucketId,
		cid: String,
		offset: u64,
		length: u64,
	},
	ObjectDelete {
		bucket_id: BucketId,
		cid: String,
		expected_version: u64,
	},
	ObjectStatus {
		bucket_id: BucketId,
		cid: String,
	},
	CheckpointStatus {
		bucket_id: BucketId,
		root: Option<Hash32>,
	},
	CheckpointSubscribe {
		bucket_id: BucketId,
		cursor: u64,
	},
	ReplicaStatus {
		bucket_id: BucketId,
	},
	ReplicaSubscribe {
		bucket_id: BucketId,
		cursor: u64,
	},
	DeletionStatus {
		bucket_id: BucketId,
		cid: String,
	},
	DeletionSubscribe {
		bucket_id: BucketId,
		cid: String,
		cursor: u64,
	},
	DriveRead {
		bucket_id: BucketId,
		path: String,
		manifest: Option<String>,
	},
	DriveCommit {
		bucket_id: BucketId,
		manifest: String,
		bytes: Vec<u8>,
		expected_version: u64,
		mode: u8,
	},
	DriveShare {
		bucket_id: BucketId,
		subject: Subject,
		role: u8,
		issued_at: u64,
		expires_at: u64,
	},
	S3Put {
		bucket: String,
		key: Vec<u8>,
		cid: String,
		metadata: Vec<u8>,
		media_type: String,
		if_match: Option<String>,
		transfer_id: OperationId,
	},
	S3Get {
		bucket: String,
		key: Vec<u8>,
		version: Option<u64>,
	},
	S3List {
		bucket: String,
		prefix: Option<Vec<u8>>,
		cursor: Option<Vec<u8>>,
		limit: u8,
	},
	S3Delete {
		bucket: String,
		key: Vec<u8>,
		if_match: Option<String>,
		transfer_id: OperationId,
	},
	Publish {
		name_hash: Hash32,
		cid: String,
		expected_version: u64,
	},
	Resolve {
		name: String,
		version: Option<u64>,
		at: Option<Hash32>,
	},
	KeysExport {
		bucket_id: BucketId,
		key_version: u32,
		recipient_key: Vec<u8>,
	},
	KeysImport {
		bucket_id: BucketId,
		wrapped_key: Vec<u8>,
		replace: u8,
		key_version: u32,
	},
}

impl StorageV2Payload {
	pub const fn operation(&self) -> StorageV2Operation {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StorageV2Intent {
	pub protocol: &'static str,
	pub registry_sha256: &'static str,
	pub request_id: RequestId,
	pub product_id: String,
	pub operation: StorageV2Operation,
	pub grant_id: Option<GrantId>,
	pub operation_id: Option<OperationId>,
	pub idempotency_key: Option<Vec<u8>>,
	pub deadline_block: u64,
	pub payload: StorageV2Payload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntentError {
	InvalidProduct,
	GrantRequired,
	GrantForbidden,
	OperationIdRequired,
	OperationIdForbidden,
	InvalidIdempotencyKey,
	InvalidPayload,
}

impl StorageV2Intent {
	pub fn new(
		request_id: RequestId,
		product_id: String,
		grant_id: Option<GrantId>,
		operation_id: Option<OperationId>,
		idempotency_key: Option<Vec<u8>>,
		deadline_block: u64,
		payload: StorageV2Payload,
	) -> Result<Self, IntentError> {
		if !validation::valid_text(&product_id, 1, 128) {
			return Err(IntentError::InvalidProduct);
		}
		payload.validate().map_err(|_| IntentError::InvalidPayload)?;
		let operation = payload.operation();
		let contract = operation.contract();
		match (contract.grant_scope, grant_id) {
			(GrantScope::Public, Some(_)) => return Err(IntentError::GrantForbidden),
			(GrantScope::Public, None) => {},
			(_, None) => return Err(IntentError::GrantRequired),
			(_, Some(_)) => {},
		}
		match (contract.operation_id_required, operation_id) {
			(true, None) => return Err(IntentError::OperationIdRequired),
			(false, Some(_)) => return Err(IntentError::OperationIdForbidden),
			_ => {},
		}
		if idempotency_key.as_ref().is_some_and(|key| key.is_empty() || key.len() > 64) {
			return Err(IntentError::InvalidIdempotencyKey);
		}
		Ok(Self {
			protocol: STORAGE_V2_PROTOCOL,
			registry_sha256: STORAGE_V2_REGISTRY_SHA256,
			request_id,
			product_id,
			operation,
			grant_id,
			operation_id,
			idempotency_key,
			deadline_block,
			payload,
		})
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StorageV2Resume {
	ChainIdempotent { operation_id: OperationId },
	ProviderToken(Vec<u8>),
	VerifiedOffset { offset: u64, proof: Hash32 },
	Cursor256(u64),
	PerObject { cid: String, version: u64 },
	CursorVersioned { cursor: Vec<u8>, version: u64 },
}

impl StorageV2Resume {
	const fn mode(&self) -> ResumeMode {
		match self {
			Self::ChainIdempotent { .. } => ResumeMode::ChainIdempotent,
			Self::ProviderToken(_) => ResumeMode::ProviderToken,
			Self::VerifiedOffset { .. } => ResumeMode::VerifiedOffset,
			Self::Cursor256(_) => ResumeMode::Cursor256,
			Self::PerObject { .. } => ResumeMode::PerObject,
			Self::CursorVersioned { .. } => ResumeMode::CursorVersioned,
		}
	}
}

pub(crate) fn validate_resume(operation: StorageV2Operation, resume: &StorageV2Resume) -> bool {
	operation.contract().resume == resume.mode()
		&& match resume {
			StorageV2Resume::ChainIdempotent { .. }
			| StorageV2Resume::VerifiedOffset { .. }
			| StorageV2Resume::Cursor256(_) => true,
			StorageV2Resume::ProviderToken(token) => (1..=4096).contains(&token.len()),
			StorageV2Resume::PerObject { cid, .. } => validation::valid_text(cid, 1, 128),
			StorageV2Resume::CursorVersioned { cursor, .. } => (1..=2048).contains(&cursor.len()),
		}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StorageV2EventPayload {
	Accepted { state: u8 },
	Progress(StorageV2Progress),
	Result(StorageV2Result),
	Error(StorageV2Error),
	Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StorageV2Event {
	pub request_id: RequestId,
	pub seq: u32,
	pub payload: StorageV2EventPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EventSequenceError {
	RequestMismatch,
	SequenceMismatch,
	AcceptedRequired,
	DuplicateAccepted,
	EventAfterTerminal,
	InvalidAccepted,
	InvalidProgress,
	InvalidResult,
	InvalidError,
	ProgressAfterCancel,
	ResumeRevoked,
}

pub(crate) struct StorageV2EventSequence {
	operation: StorageV2Operation,
	request_id: RequestId,
	next: u32,
	terminal: bool,
	cancel_requested: bool,
	resume_authority: bool,
}

impl StorageV2EventSequence {
	pub fn new(operation: StorageV2Operation, request_id: RequestId) -> Self {
		Self {
			operation,
			request_id,
			next: 0,
			terminal: false,
			cancel_requested: false,
			resume_authority: operation.contract().resume != ResumeMode::None,
		}
	}

	pub fn request_cancel(&mut self) -> bool {
		if self.terminal || self.cancel_requested {
			return false;
		}
		self.cancel_requested = true;
		self.resume_authority = false;
		true
	}

	pub fn authorize_resume(&self, resume: &StorageV2Resume) -> Result<(), EventSequenceError> {
		if !self.resume_authority
			|| self.cancel_requested
			|| self.terminal
			|| !validate_resume(self.operation, resume)
		{
			return Err(EventSequenceError::ResumeRevoked);
		}
		Ok(())
	}

	pub fn accept(&mut self, event: &StorageV2Event) -> Result<(), EventSequenceError> {
		if self.terminal {
			return Err(EventSequenceError::EventAfterTerminal);
		}
		if event.request_id != self.request_id {
			return Err(EventSequenceError::RequestMismatch);
		}
		if event.seq != self.next {
			return Err(EventSequenceError::SequenceMismatch);
		}
		if self.next == 0 && !matches!(event.payload, StorageV2EventPayload::Accepted { .. }) {
			return Err(EventSequenceError::AcceptedRequired);
		}
		if self.next > 0 && matches!(event.payload, StorageV2EventPayload::Accepted { .. }) {
			return Err(EventSequenceError::DuplicateAccepted);
		}
		if self.cancel_requested && !matches!(event.payload, StorageV2EventPayload::Cancelled) {
			return Err(EventSequenceError::ProgressAfterCancel);
		}
		match &event.payload {
			StorageV2EventPayload::Accepted { state } if *state > 4 => {
				return Err(EventSequenceError::InvalidAccepted)
			},
			StorageV2EventPayload::Progress(progress)
				if progress.validate(self.operation).is_err() =>
			{
				return Err(EventSequenceError::InvalidProgress)
			},
			StorageV2EventPayload::Result(result)
				if result.operation() != self.operation || result.validate().is_err() =>
			{
				return Err(EventSequenceError::InvalidResult)
			},
			StorageV2EventPayload::Error(error) if error.validate_for(self.operation).is_err() => {
				return Err(EventSequenceError::InvalidError)
			},
			_ => {},
		}
		self.next += 1;
		self.terminal = matches!(
			event.payload,
			StorageV2EventPayload::Result(_)
				| StorageV2EventPayload::Error(_)
				| StorageV2EventPayload::Cancelled
		);
		if self.terminal {
			self.resume_authority = false;
		}
		Ok(())
	}

	pub const fn terminal(&self) -> bool {
		self.terminal
	}
	pub const fn resume_authority(&self) -> bool {
		self.resume_authority
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn id16(value: u8) -> Id16 {
		Id16([value; 16])
	}
	fn id32(value: u8) -> Id32 {
		Id32([value; 32])
	}

	fn grant_scope(value: &str) -> GrantScope {
		match value {
			"public" => GrantScope::Public,
			"storage.bucket.admin" => GrantScope::BucketAdmin,
			"storage.bucket.read" => GrantScope::BucketRead,
			"storage.bucket.reader" => GrantScope::BucketReader,
			"storage.bucket.writer" => GrantScope::BucketWriter,
			"storage.publish" => GrantScope::Publish,
			"storage.keys.export" => GrantScope::KeysExport,
			"storage.keys.import" => GrantScope::KeysImport,
			_ => panic!("unexpected grant scope"),
		}
	}

	fn cancellation(value: &str) -> Cancellation {
		match value {
			"pre-effect-only" => Cancellation::PreEffectOnly,
			"stop-stream-or-terminal" => Cancellation::StopStreamOrTerminal,
			_ => panic!("unexpected cancellation"),
		}
	}

	fn resume_mode(value: &str) -> ResumeMode {
		match value {
			"none" => ResumeMode::None,
			"chain-idempotent" => ResumeMode::ChainIdempotent,
			"provider-token" => ResumeMode::ProviderToken,
			"verified-offset" => ResumeMode::VerifiedOffset,
			"cursor-256" => ResumeMode::Cursor256,
			"per-object" => ResumeMode::PerObject,
			"cursor-versioned" => ResumeMode::CursorVersioned,
			_ => panic!("unexpected resume mode"),
		}
	}

	#[test]
	fn storage_v2_contracts_match_frozen_registry() {
		let registry: serde_json::Value = serde_json::from_str(include_str!(
			"../../../docs/specs/origin-host-registry-v2.operations.json"
		))
		.unwrap();
		assert_eq!(registry["protocol"], STORAGE_V2_PROTOCOL);
		let operations: Vec<_> = registry["operations"]
			.as_array()
			.unwrap()
			.iter()
			.filter(|value| (1000..=1061).contains(&value["code"].as_u64().unwrap_or_default()))
			.collect();
		assert_eq!(operations.len(), STORAGE_V2_OPERATIONS.len());
		for (operation, frozen) in STORAGE_V2_OPERATIONS.iter().zip(operations) {
			let contract = operation.contract();
			assert_eq!(u64::from(operation.code()), frozen["code"]);
			assert_eq!(contract.name, frozen["name"]);
			assert_eq!(contract.grant_scope, grant_scope(frozen["grant_scope"].as_str().unwrap()));
			assert_eq!(contract.resume, resume_mode(frozen["resume"].as_str().unwrap()));
			assert_eq!(
				contract.cancellation,
				cancellation(frozen["cancellation"].as_str().unwrap())
			);
			assert_eq!(contract.operation_id_required, frozen["operation_id_required"]);
			assert_eq!(contract.state_changing, frozen["state_changing"]);
			let allowed = frozen["allowed_errors"].as_array().unwrap();
			for (code, name, retryable) in validation::STORAGE_V2_ERRORS {
				let expected = allowed.iter().any(|error| error["code"] == u64::from(code));
				let error = StorageV2Error {
					code,
					name: name.into(),
					retryable,
					details: Default::default(),
				};
				assert_eq!(
					error.validate_for(*operation).is_ok(),
					expected,
					"{} scope for error {code}",
					contract.name
				);
			}
		}

		let errors: serde_json::Value = serde_json::from_str(include_str!(
			"../../../docs/specs/origin-host-registry-v2.errors.json"
		))
		.unwrap();
		let errors = errors["errors"].as_array().unwrap();
		assert_eq!(errors.len(), validation::STORAGE_V2_ERRORS.len());
		for ((code, name, retryable), frozen) in validation::STORAGE_V2_ERRORS.iter().zip(errors) {
			assert_eq!(u64::from(*code), frozen["code"]);
			assert_eq!(*name, frozen["name"]);
			assert_eq!(*retryable, frozen["retryable"]);
		}
	}

	#[test]
	fn rust_codec_emits_the_same_canonical_frame_as_typescript() {
		let intent = StorageV2Intent::new(
			id16(0x11),
			"festival".into(),
			Some(id32(0x22)),
			Some(id16(0x33)),
			None,
			100,
			StorageV2Payload::BucketCreate {
				replica_count: 1,
				providers: vec![id32(0x22), id32(0x22)],
				encryption: 0,
			},
		)
		.unwrap();
		let vectors: serde_json::Value = serde_json::from_str(include_str!(
			"../../../docs/specs/origin-host-registry-v2.vectors.json"
		))
		.unwrap();
		let golden = vectors["vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|value| value["id"] == "1000-positive")
			.unwrap();
		assert_eq!(hex::encode(validation::encode_intent(&intent).unwrap()), golden["wire_hex"]);
		for operation in STORAGE_V2_OPERATIONS {
			let id = format!("{}-positive", operation.code());
			let vector = vectors["vectors"]
				.as_array()
				.unwrap()
				.iter()
				.find(|value| value["id"] == id)
				.unwrap();
			let wire = hex::decode(vector["wire_hex"].as_str().unwrap()).unwrap();
			assert_eq!(validation::decode_canonical_storage_frame(&wire).unwrap(), wire, "{id}");
		}
	}

	#[test]
	fn rust_storage_decoder_rejects_noncanonical_unknown_and_open_frames() {
		use ciborium::value::Value;
		let vectors: serde_json::Value = serde_json::from_str(include_str!(
			"../../../docs/specs/origin-host-registry-v2.vectors.json"
		))
		.unwrap();
		let wire = |id: &str| {
			hex::decode(
				vectors["vectors"]
					.as_array()
					.unwrap()
					.iter()
					.find(|value| value["id"] == id)
					.unwrap()["wire_hex"]
					.as_str()
					.unwrap(),
			)
			.unwrap()
		};
		for id in [
			"wire-noncanonical-long-version",
			"wire-noncanonical-indefinite-map",
			"wire-noncanonical-reversed-map",
			"wire-noncanonical-tag",
		] {
			assert!(validation::decode_canonical_storage_frame(&wire(id)).is_err(), "{id}");
		}
		let valid = wire("1000-positive");
		let mut unknown: Value = ciborium::from_reader(valid.as_slice()).unwrap();
		let Value::Map(fields) = &mut unknown else { panic!("frame map") };
		fields.push((Value::Integer(9.into()), Value::Integer(0.into())));
		let unknown = validation::encode_wire_value(&unknown).unwrap();
		assert!(validation::decode_canonical_storage_frame(&unknown).is_err());

		let mut unknown_operation: Value = ciborium::from_reader(valid.as_slice()).unwrap();
		let Value::Map(fields) = &mut unknown_operation else { panic!("frame map") };
		let operation = fields
			.iter_mut()
			.find_map(|(key, value)| {
				matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(3))
					.then_some(value)
			})
			.unwrap();
		*operation = Value::Integer(65_535.into());
		let unknown_operation = validation::encode_wire_value(&unknown_operation).unwrap();
		assert!(validation::decode_canonical_storage_frame(&unknown_operation).is_err());

		let mut open_payload: Value = ciborium::from_reader(valid.as_slice()).unwrap();
		let Value::Map(fields) = &mut open_payload else { panic!("frame map") };
		let payload = fields
			.iter_mut()
			.find_map(|(key, value)| {
				matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(8))
					.then_some(value)
			})
			.unwrap();
		let Value::Map(payload) = payload else { panic!("payload map") };
		payload.push((Value::Integer(9.into()), Value::Integer(0.into())));
		let open_payload = validation::encode_wire_value(&open_payload).unwrap();
		assert!(validation::decode_canonical_storage_frame(&open_payload).is_err());
	}

	#[test]
	fn intent_enforces_grant_and_operation_id_boundaries() {
		let payload = StorageV2Payload::ObjectPut {
			bucket_id: id32(1),
			cid: "bafk-test".into(),
			length: 3,
			encrypted: 0,
			transfer_id: id16(2),
		};
		assert_eq!(
			StorageV2Intent::new(
				id16(3),
				"festival".into(),
				None,
				Some(id16(4)),
				None,
				100,
				payload.clone()
			),
			Err(IntentError::GrantRequired)
		);
		let intent = StorageV2Intent::new(
			id16(3),
			"festival".into(),
			Some(id32(5)),
			Some(id16(4)),
			Some(vec![1]),
			100,
			payload,
		)
		.unwrap();
		assert_eq!(intent.operation.code(), 1010);
		assert_eq!(intent.protocol, "cord.origin.host/2");

		let resolve =
			StorageV2Payload::Resolve { name: "festival.origin".into(), version: None, at: None };
		assert_eq!(
			StorageV2Intent::new(
				id16(3),
				"festival".into(),
				Some(id32(5)),
				None,
				None,
				100,
				resolve.clone()
			),
			Err(IntentError::GrantForbidden)
		);
		assert!(StorageV2Intent::new(id16(3), "festival".into(), None, None, None, 100, resolve)
			.is_ok());
		assert_eq!(
			StorageV2Intent::new(
				id16(3),
				"e\u{301}".into(),
				None,
				None,
				None,
				100,
				StorageV2Payload::Resolve {
					name: "festival.origin".into(),
					version: None,
					at: None,
				},
			),
			Err(IntentError::InvalidProduct)
		);
	}

	#[test]
	fn hostile_payload_result_progress_and_error_values_fail_closed() {
		let invalid_range = StorageV2Payload::ObjectRange {
			bucket_id: id32(1),
			cid: "bafk".into(),
			offset: 0,
			length: 0,
		};
		assert_eq!(
			StorageV2Intent::new(
				id16(1),
				"festival".into(),
				Some(id32(2)),
				None,
				None,
				10,
				invalid_range
			),
			Err(IntentError::InvalidPayload)
		);
		assert_eq!(
			StorageV2Payload::DriveCommit {
				bucket_id: id32(1),
				manifest: "bafk".into(),
				bytes: vec![],
				expected_version: 0,
				mode: 0,
			}
			.validate(),
			Err(validation::ValidationError::Bounds)
		);
		assert_eq!(
			StorageV2Payload::S3List {
				bucket: "bucket".into(),
				prefix: None,
				cursor: None,
				limit: 0,
			}
			.validate(),
			Err(validation::ValidationError::Bounds)
		);
		assert_eq!(
			StorageV2Payload::KeysExport {
				bucket_id: id32(1),
				key_version: 1,
				recipient_key: vec![0; 31],
			}
			.validate(),
			Err(validation::ValidationError::Bounds)
		);

		let resolved = StorageV2Result::Resolve {
			name_id: id32(7),
			cid: "bafk".into(),
			version: 1,
			checkpoint: validation::Checkpoint { root: id32(1), from: 1, to: 2, replicas: 2 },
			finalized: validation::Finality { number: 2, hash: id32(2) },
		};
		assert_eq!(resolved.validate(), Ok(()));

		let invalid_result = StorageV2Result::ObjectRange {
			cid: "bafk".into(),
			offset: 9,
			length: 2,
			total: 10,
			checkpoint: validation::Checkpoint { root: id32(1), from: 1, to: 2, replicas: 2 },
		};
		assert_eq!(invalid_result.validate(), Err(validation::ValidationError::Bounds));
		assert_eq!(
			StorageV2Progress::State {
				completed: 1,
				total: None,
				chunks_acked: None,
				replicas_confirmed: None
			}
			.validate(StorageV2Operation::ObjectGet),
			Err(validation::ValidationError::ProgressMismatch)
		);
		assert_eq!(
			StorageV2Progress::Bytes { offset: 0, bytes: vec![0; 4_194_305] }
				.validate(StorageV2Operation::S3Get),
			Err(validation::ValidationError::ProgressMismatch)
		);
		assert!(StorageV2Error {
			code: 114,
			name: "HOST_OUTBOX_FULL".into(),
			retryable: true,
			details: Default::default()
		}
		.validate_for(StorageV2Operation::ObjectPut)
		.is_ok());
		assert_eq!(
			StorageV2Error {
				code: 114,
				name: "HOST_OUTBOX_FULL".into(),
				retryable: false,
				details: Default::default()
			}
			.validate_for(StorageV2Operation::ObjectPut),
			Err(validation::ValidationError::ErrorMismatch)
		);
		assert_eq!(
			StorageV2Error {
				code: 200,
				name: "STORAGE_CHUNK_OUT_OF_ORDER".into(),
				retryable: false,
				details: Default::default(),
			}
			.validate_for(StorageV2Operation::KeysExport),
			Err(validation::ValidationError::ErrorMismatch)
		);
		assert!(!validate_resume(
			StorageV2Operation::S3List,
			&StorageV2Resume::CursorVersioned { cursor: vec![0; 2049], version: 0 }
		));
		assert!(!validate_resume(
			StorageV2Operation::DriveCommit,
			&StorageV2Resume::PerObject { cid: "e\u{301}".into(), version: 0 }
		));
	}

	#[test]
	fn cancel_is_idempotent_terminal_and_revokes_resume_authority() {
		let request_id = id16(9);
		let mut sequence = StorageV2EventSequence::new(StorageV2Operation::ObjectPut, request_id);
		sequence
			.accept(&StorageV2Event {
				request_id,
				seq: 0,
				payload: StorageV2EventPayload::Accepted { state: 0 },
			})
			.unwrap();
		assert!(sequence.authorize_resume(&StorageV2Resume::ProviderToken(vec![1])).is_ok());
		assert_eq!(
			sequence.authorize_resume(&StorageV2Resume::ProviderToken(vec![])),
			Err(EventSequenceError::ResumeRevoked)
		);
		assert!(sequence.resume_authority());
		assert!(sequence.request_cancel());
		assert!(!sequence.request_cancel());
		assert!(!sequence.resume_authority());
		assert_eq!(
			sequence.authorize_resume(&StorageV2Resume::ProviderToken(vec![1])),
			Err(EventSequenceError::ResumeRevoked)
		);
		assert_eq!(
			sequence.accept(&StorageV2Event {
				request_id,
				seq: 1,
				payload: StorageV2EventPayload::Progress(StorageV2Progress::State {
					completed: 1,
					total: None,
					chunks_acked: Some(1),
					replicas_confirmed: None
				})
			}),
			Err(EventSequenceError::ProgressAfterCancel)
		);
		sequence
			.accept(&StorageV2Event {
				request_id,
				seq: 1,
				payload: StorageV2EventPayload::Cancelled,
			})
			.unwrap();
		assert!(sequence.terminal());
		assert_eq!(
			sequence.accept(&StorageV2Event {
				request_id,
				seq: 2,
				payload: StorageV2EventPayload::Cancelled
			}),
			Err(EventSequenceError::EventAfterTerminal)
		);
	}
}
