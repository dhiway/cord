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

//! Private host-v2 execution binding. The public SDK cannot reach this module before P4/P5.

use std::io::{Read, Write};

use ciborium::value::Value;
use sha2::{Digest, Sha256};

use crate::product_sdk::host_outbox::PrepareHostOutboxV1;

use super::{
	codec::{CodecError, Dto},
	desktop::{
		DesktopTransportError, DurableDesktopEvent, DurableDesktopHostV2, ResumeTokenVerifierV2,
	},
	generated::{
		IdentityAccountFrame, IdentityEntitlementsReadFrame, IdentityHumanityProveFrame,
		IdentityHumanityStatusFrame, IdentityProfileDiscloseFrame, IdentityProfileReadFrame,
		IdentitySubjectDeriveFrame, OperationCode, Production, RequestV2, StorageBucketCreateFrame,
		StorageBucketGetFrame, StorageBucketGrantFrame, StorageBucketRevokeFrame,
		StorageCheckpointStatusFrame, StorageCheckpointSubscribeFrame, StorageDeletionStatusFrame,
		StorageDeletionSubscribeFrame, StorageDriveCommitFrame, StorageDriveReadFrame,
		StorageDriveShareFrame, StorageKeysExportFrame, StorageKeysImportFrame,
		StorageObjectDeleteFrame, StorageObjectGetFrame, StorageObjectPutFrame,
		StorageObjectRangeFrame, StorageObjectStatusFrame, StoragePublishFrame,
		StorageReplicaStatusFrame, StorageReplicaSubscribeFrame, StorageResolveFrame,
		StorageS3DeleteFrame, StorageS3GetFrame, StorageS3ListFrame, StorageS3PutFrame,
		TransactionSignFrame, OPERATIONS,
	},
};

const MAX_EVENTS_PER_CALL: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostRequestMetaV2 {
	pub(crate) request_id: [u8; 16],
	pub(crate) operation_id: Option<[u8; 16]>,
	pub(crate) deadline_block: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderOutboxContextV2 {
	pub(crate) outbox_id: [u8; 16],
	pub(crate) generation: u64,
	pub(crate) intended_cursor: u32,
	pub(crate) negotiated_tuple: [u8; 32],
	pub(crate) provider_id: [u8; 32],
	pub(crate) provider_endpoint_hash: [u8; 32],
	pub(crate) created_at: u64,
	pub(crate) authority_expires_at: u64,
	pub(crate) terminal_block: u64,
	pub(crate) prepare_nonce: [u8; 24],
	pub(crate) mark_sent_nonce: [u8; 24],
	pub(crate) install_nonce: [u8; 24],
	pub(crate) mark_ack_nonce: [u8; 24],
	pub(crate) confirm_nonce: [u8; 24],
	pub(crate) compact_nonce: [u8; 24],
}

pub(crate) struct HostCallV2<'a, P: Production> {
	pub(crate) frame: &'a Dto<P>,
	pub(crate) authority: &'a [u8],
	pub(crate) meta: HostRequestMetaV2,
	pub(crate) outbox: &'a ProviderOutboxContextV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostExecutionV2 {
	pub(crate) events: Vec<Vec<u8>>,
	pub(crate) terminal_response_hash: Option<[u8; 32]>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum HostExecutionErrorV2 {
	#[error(transparent)]
	Codec(#[from] CodecError),
	#[error(transparent)]
	Desktop(#[from] DesktopTransportError),
	#[error("HOST_OPERATION_REGISTRY_MISMATCH")]
	Registry,
	#[error("HOST_BACKEND_REJECTED: {0}")]
	Backend(&'static str),
	#[error("HOST_EVENT_LIMIT_EXCEEDED")]
	EventLimit,
	#[error("HOST_APP_BINDING_INVALID")]
	AppBinding,
}

macro_rules! define_backend_trait {
	($name:ident { $($method:ident: $frame:ty),+ $(,)? }) => {
		pub(crate) trait $name {
			$(fn $method(
				&mut self,
				call: HostCallV2<'_, $frame>,
			) -> Result<HostExecutionV2, HostExecutionErrorV2>;)+
		}
	};
}

// The authority split is intentional: no one trait can claim the whole storage surface.
define_backend_trait!(ProviderByteStorageV2 {
	object_put: StorageObjectPutFrame,
	object_get: StorageObjectGetFrame,
	object_range: StorageObjectRangeFrame,
	object_status: StorageObjectStatusFrame,
});

define_backend_trait!(CommonsStorageControlV2 {
	bucket_create: StorageBucketCreateFrame,
	bucket_get: StorageBucketGetFrame,
	bucket_grant: StorageBucketGrantFrame,
	bucket_revoke: StorageBucketRevokeFrame,
	object_delete: StorageObjectDeleteFrame,
	checkpoint_status: StorageCheckpointStatusFrame,
	checkpoint_subscribe: StorageCheckpointSubscribeFrame,
	replica_status: StorageReplicaStatusFrame,
	replica_subscribe: StorageReplicaSubscribeFrame,
	deletion_status: StorageDeletionStatusFrame,
	deletion_subscribe: StorageDeletionSubscribeFrame,
	drive_read: StorageDriveReadFrame,
	drive_commit: StorageDriveCommitFrame,
	drive_share: StorageDriveShareFrame,
	s3_put: StorageS3PutFrame,
	s3_get: StorageS3GetFrame,
	s3_list: StorageS3ListFrame,
	s3_delete: StorageS3DeleteFrame,
	storage_publish: StoragePublishFrame,
	storage_resolve: StorageResolveFrame,
});

define_backend_trait!(HostStorageKeystoreV2 {
	keys_export: StorageKeysExportFrame,
	keys_import: StorageKeysImportFrame,
});

/// Encryption policy and transformations are host-owned, never provider or Commons state.
pub(crate) trait HostStorageEncryptionV2 {
	fn prepare(
		&mut self,
		operation: OperationCode,
		exact_frame: &[u8],
	) -> Result<(), HostExecutionErrorV2>;
	fn complete(
		&mut self,
		operation: OperationCode,
		execution: &mut HostExecutionV2,
	) -> Result<(), HostExecutionErrorV2>;
}

define_backend_trait!(FinalizedIdentityRuntimeV2 {
	identity_account: IdentityAccountFrame,
	identity_humanity_status: IdentityHumanityStatusFrame,
	identity_entitlements_read: IdentityEntitlementsReadFrame,
});

define_backend_trait!(HostIdentityAuthorityV2 {
	identity_profile_read: IdentityProfileReadFrame,
	identity_profile_disclose: IdentityProfileDiscloseFrame,
	identity_humanity_prove: IdentityHumanityProveFrame,
	identity_subject_derive: IdentitySubjectDeriveFrame,
});

define_backend_trait!(HostSigningAuthorityV2 { transaction_sign: TransactionSignFrame });

macro_rules! define_closed_requests {
	($($variant:ident: $frame:ty => $code:ident),+ $(,)?) => {
		pub(crate) enum ClosedHostRequestV2 {
			$($variant(Dto<$frame>),)+
		}

		impl ClosedHostRequestV2 {
			pub(crate) fn decode(
				bytes: &[u8],
			) -> Result<(Self, HostRequestMetaV2), HostExecutionErrorV2> {
				let envelope = Dto::<RequestV2>::decode(bytes)?;
				let operation = uint_field(envelope.value(), 3)
					.and_then(|value| u16::try_from(value).ok())
					.and_then(OperationCode::from_u16)
					.ok_or(HostExecutionErrorV2::Registry)?;
				let request_id = fixed_field(envelope.value(), 1, 16)?
					.try_into()
					.map_err(|_| HostExecutionErrorV2::Registry)?;
				let operation_id = optional_fixed_field(envelope.value(), 5, 16)?
					.map(|value| value.try_into().expect("operation id length was checked"));
				let deadline_block =
					uint_field(envelope.value(), 7).ok_or(HostExecutionErrorV2::Registry)?;
				let meta = HostRequestMetaV2 { request_id, operation_id, deadline_block };
				let request = match operation {
					$(OperationCode::$code => Self::$variant(Dto::<$frame>::decode(bytes)?),)+
				};
				Ok((request, meta))
			}

			pub(crate) const fn operation(&self) -> OperationCode {
				match self {
					$(Self::$variant(_) => OperationCode::$code,)+
				}
			}
		}
	};
}

define_closed_requests!(
	StorageBucketCreate: StorageBucketCreateFrame => StorageBucketCreate,
	StorageBucketGet: StorageBucketGetFrame => StorageBucketGet,
	StorageBucketGrant: StorageBucketGrantFrame => StorageBucketGrant,
	StorageBucketRevoke: StorageBucketRevokeFrame => StorageBucketRevoke,
	StorageObjectPut: StorageObjectPutFrame => StorageObjectPut,
	StorageObjectGet: StorageObjectGetFrame => StorageObjectGet,
	StorageObjectRange: StorageObjectRangeFrame => StorageObjectRange,
	StorageObjectDelete: StorageObjectDeleteFrame => StorageObjectDelete,
	StorageObjectStatus: StorageObjectStatusFrame => StorageObjectStatus,
	StorageCheckpointStatus: StorageCheckpointStatusFrame => StorageCheckpointStatus,
	StorageCheckpointSubscribe: StorageCheckpointSubscribeFrame => StorageCheckpointSubscribe,
	StorageReplicaStatus: StorageReplicaStatusFrame => StorageReplicaStatus,
	StorageReplicaSubscribe: StorageReplicaSubscribeFrame => StorageReplicaSubscribe,
	StorageDeletionStatus: StorageDeletionStatusFrame => StorageDeletionStatus,
	StorageDeletionSubscribe: StorageDeletionSubscribeFrame => StorageDeletionSubscribe,
	StorageDriveRead: StorageDriveReadFrame => StorageDriveRead,
	StorageDriveCommit: StorageDriveCommitFrame => StorageDriveCommit,
	StorageDriveShare: StorageDriveShareFrame => StorageDriveShare,
	StorageS3Put: StorageS3PutFrame => StorageS3Put,
	StorageS3Get: StorageS3GetFrame => StorageS3Get,
	StorageS3List: StorageS3ListFrame => StorageS3List,
	StorageS3Delete: StorageS3DeleteFrame => StorageS3Delete,
	StoragePublish: StoragePublishFrame => StoragePublish,
	StorageResolve: StorageResolveFrame => StorageResolve,
	StorageKeysExport: StorageKeysExportFrame => StorageKeysExport,
	StorageKeysImport: StorageKeysImportFrame => StorageKeysImport,
	IdentityAccount: IdentityAccountFrame => IdentityAccount,
	IdentityProfileRead: IdentityProfileReadFrame => IdentityProfileRead,
	IdentityProfileDisclose: IdentityProfileDiscloseFrame => IdentityProfileDisclose,
	IdentityHumanityStatus: IdentityHumanityStatusFrame => IdentityHumanityStatus,
	IdentityHumanityProve: IdentityHumanityProveFrame => IdentityHumanityProve,
	IdentitySubjectDerive: IdentitySubjectDeriveFrame => IdentitySubjectDerive,
	IdentityEntitlementsRead: IdentityEntitlementsReadFrame => IdentityEntitlementsRead,
	TransactionSign: TransactionSignFrame => TransactionSign,
);

pub(crate) struct CordHostDispatcherV2<P, C, K, IR, IH, G> {
	provider_bytes: P,
	commons: C,
	keystore: K,
	identity_runtime: IR,
	identity_host: IH,
	signing: G,
}

impl<P, C, K, IR, IH, G> CordHostDispatcherV2<P, C, K, IR, IH, G>
where
	P: ProviderByteStorageV2,
	C: CommonsStorageControlV2,
	K: HostStorageKeystoreV2,
	IR: FinalizedIdentityRuntimeV2,
	IH: HostIdentityAuthorityV2,
	G: HostSigningAuthorityV2,
{
	pub(crate) fn new(
		provider_bytes: P,
		commons: C,
		keystore: K,
		identity_runtime: IR,
		identity_host: IH,
		signing: G,
	) -> Self {
		Self { provider_bytes, commons, keystore, identity_runtime, identity_host, signing }
	}

	pub(crate) fn dispatch(
		&mut self,
		exact_request: &[u8],
		exact_authority: &[u8],
		outbox: &ProviderOutboxContextV2,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let (request, meta) = ClosedHostRequestV2::decode(exact_request)?;
		let operation = request.operation();
		if !OPERATIONS.iter().any(|binding| binding.code == operation as u16) {
			return Err(HostExecutionErrorV2::Registry);
		}
		macro_rules! call {
			($backend:ident, $method:ident, $frame:ident) => {
				self.$backend.$method(HostCallV2 {
					frame: &$frame,
					authority: exact_authority,
					meta,
					outbox,
				})
			};
		}
		match request {
			ClosedHostRequestV2::StorageBucketCreate(frame) => call!(commons, bucket_create, frame),
			ClosedHostRequestV2::StorageBucketGet(frame) => call!(commons, bucket_get, frame),
			ClosedHostRequestV2::StorageBucketGrant(frame) => call!(commons, bucket_grant, frame),
			ClosedHostRequestV2::StorageBucketRevoke(frame) => call!(commons, bucket_revoke, frame),
			ClosedHostRequestV2::StorageObjectPut(frame) => {
				call!(provider_bytes, object_put, frame)
			},
			ClosedHostRequestV2::StorageObjectGet(frame) => {
				call!(provider_bytes, object_get, frame)
			},
			ClosedHostRequestV2::StorageObjectRange(frame) => {
				call!(provider_bytes, object_range, frame)
			},
			ClosedHostRequestV2::StorageObjectDelete(frame) => call!(commons, object_delete, frame),
			ClosedHostRequestV2::StorageObjectStatus(frame) => {
				call!(provider_bytes, object_status, frame)
			},
			ClosedHostRequestV2::StorageCheckpointStatus(frame) => {
				call!(commons, checkpoint_status, frame)
			},
			ClosedHostRequestV2::StorageCheckpointSubscribe(frame) => {
				call!(commons, checkpoint_subscribe, frame)
			},
			ClosedHostRequestV2::StorageReplicaStatus(frame) => {
				call!(commons, replica_status, frame)
			},
			ClosedHostRequestV2::StorageReplicaSubscribe(frame) => {
				call!(commons, replica_subscribe, frame)
			},
			ClosedHostRequestV2::StorageDeletionStatus(frame) => {
				call!(commons, deletion_status, frame)
			},
			ClosedHostRequestV2::StorageDeletionSubscribe(frame) => {
				call!(commons, deletion_subscribe, frame)
			},
			ClosedHostRequestV2::StorageDriveRead(frame) => call!(commons, drive_read, frame),
			ClosedHostRequestV2::StorageDriveCommit(frame) => call!(commons, drive_commit, frame),
			ClosedHostRequestV2::StorageDriveShare(frame) => call!(commons, drive_share, frame),
			ClosedHostRequestV2::StorageS3Put(frame) => call!(commons, s3_put, frame),
			ClosedHostRequestV2::StorageS3Get(frame) => call!(commons, s3_get, frame),
			ClosedHostRequestV2::StorageS3List(frame) => call!(commons, s3_list, frame),
			ClosedHostRequestV2::StorageS3Delete(frame) => call!(commons, s3_delete, frame),
			ClosedHostRequestV2::StoragePublish(frame) => call!(commons, storage_publish, frame),
			ClosedHostRequestV2::StorageResolve(frame) => call!(commons, storage_resolve, frame),
			ClosedHostRequestV2::StorageKeysExport(frame) => call!(keystore, keys_export, frame),
			ClosedHostRequestV2::StorageKeysImport(frame) => call!(keystore, keys_import, frame),
			ClosedHostRequestV2::IdentityAccount(frame) => {
				call!(identity_runtime, identity_account, frame)
			},
			ClosedHostRequestV2::IdentityProfileRead(frame) => {
				call!(identity_host, identity_profile_read, frame)
			},
			ClosedHostRequestV2::IdentityProfileDisclose(frame) => {
				call!(identity_host, identity_profile_disclose, frame)
			},
			ClosedHostRequestV2::IdentityHumanityStatus(frame) => {
				call!(identity_runtime, identity_humanity_status, frame)
			},
			ClosedHostRequestV2::IdentityHumanityProve(frame) => {
				call!(identity_host, identity_humanity_prove, frame)
			},
			ClosedHostRequestV2::IdentitySubjectDerive(frame) => {
				call!(identity_host, identity_subject_derive, frame)
			},
			ClosedHostRequestV2::IdentityEntitlementsRead(frame) => {
				call!(identity_runtime, identity_entitlements_read, frame)
			},
			ClosedHostRequestV2::TransactionSign(frame) => call!(signing, transaction_sign, frame),
		}
	}
}

pub(crate) struct ProviderSuccessorV2 {
	pub(crate) exact_request: Vec<u8>,
	pub(crate) outbox: ProviderOutboxContextV2,
}

pub(crate) trait ProviderContinuationSourceV2 {
	fn upload_chunks(
		&mut self,
		operation: OperationCode,
		exact_request: &[u8],
	) -> Result<Option<Vec<Vec<u8>>>, HostExecutionErrorV2>;

	fn next_generation(
		&mut self,
		operation: OperationCode,
		predecessor: &ProviderOutboxContextV2,
		exact_resume_token: &[u8],
		cursor: u32,
		response_hash: [u8; 32],
	) -> Result<Option<ProviderSuccessorV2>, HostExecutionErrorV2>;
}

/// Concrete provider byte/recovery adapter over the existing durable local IPC kernel.
pub(crate) struct DurableCordProviderV2<'a, S, V, C> {
	host: DurableDesktopHostV2<'a, S>,
	resume_verifier: V,
	continuations: C,
}

impl<'a, S: Read + Write, V, C> DurableCordProviderV2<'a, S, V, C>
where
	V: ResumeTokenVerifierV2,
	C: ProviderContinuationSourceV2,
{
	pub(crate) fn new(
		host: DurableDesktopHostV2<'a, S>,
		resume_verifier: V,
		continuations: C,
	) -> Self {
		Self { host, resume_verifier, continuations }
	}

	fn execute<P: Production>(
		&mut self,
		call: HostCallV2<'_, P>,
	) -> Result<HostExecutionV2, HostExecutionErrorV2> {
		let operation = OperationCode::from_u16(
			uint_field(call.frame.value(), 3)
				.and_then(|value| value.try_into().ok())
				.ok_or(HostExecutionErrorV2::Registry)?,
		)
		.ok_or(HostExecutionErrorV2::Registry)?;
		let request_id = call.meta.request_id;
		let operation_id = operation_id_for(operation, request_id, call.meta.operation_id)?;
		let mut current = call.outbox.clone();
		let exact_request = call.frame.canonical().to_vec();
		if operation == OperationCode::StorageObjectPut {
			let chunks = self
				.continuations
				.upload_chunks(operation, &exact_request)?
				.ok_or(HostExecutionErrorV2::Backend("HOST_UPLOAD_SOURCE_UNAVAILABLE"))?;
			self.host.stage_upload(
				&exact_request,
				operation_id,
				chunks,
				current.created_at,
				current.authority_expires_at,
				current.prepare_nonce,
			)?;
		}
		self.host.prepare_and_send(
			PrepareHostOutboxV1 {
				outbox_id: current.outbox_id,
				exact_request_bytes: exact_request,
				exact_authority_bytes: call.authority.to_vec(),
				exact_payload_bytes: None,
				request_id,
				operation_id,
				generation: current.generation,
				intended_cursor: current.intended_cursor,
				negotiated_tuple: current.negotiated_tuple,
				provider_id: current.provider_id,
				provider_endpoint_hash: current.provider_endpoint_hash,
				expected_response_kind: 2,
				created_at: current.created_at,
				authority_expires_at: current.authority_expires_at,
			},
			current.prepare_nonce,
			current.mark_sent_nonce,
		)?;
		let mut events = Vec::new();
		for _ in 0..MAX_EVENTS_PER_CALL {
			match self.host.receive_continuation(
				&mut self.resume_verifier,
				current.terminal_block,
				current.install_nonce,
				current.mark_ack_nonce,
				current.confirm_nonce,
				current.compact_nonce,
			)? {
				DurableDesktopEvent::Continuation {
					events: generation_events,
					resume_token,
					cursor,
					response_hash,
				} => {
					events.extend(generation_events);
					let next = self
						.continuations
						.next_generation(operation, &current, &resume_token, cursor, response_hash)?
						.ok_or(HostExecutionErrorV2::Backend(
							"HOST_CONTINUATION_SOURCE_EXHAUSTED",
						))?;
					let ProviderSuccessorV2 { exact_request, outbox: next_outbox } = next;
					let (next_request, meta) = ClosedHostRequestV2::decode(&exact_request)?;
					if meta.request_id != request_id ||
						operation_id_for(operation, request_id, meta.operation_id)? !=
							operation_id || next_request.operation() != operation
					{
						return Err(HostExecutionErrorV2::Registry);
					}
					let exact_payload = if operation == OperationCode::StorageObjectPut {
						self.host.staged_upload_payload(
							&exact_request,
							operation_id,
							next_outbox.intended_cursor,
						)?
					} else {
						None
					};
					self.host.prepare_successor_and_send(
						current.outbox_id,
						PrepareHostOutboxV1 {
							outbox_id: next_outbox.outbox_id,
							exact_request_bytes: exact_request,
							exact_authority_bytes: resume_token,
							exact_payload_bytes: exact_payload,
							request_id,
							operation_id,
							generation: next_outbox.generation,
							intended_cursor: next_outbox.intended_cursor,
							negotiated_tuple: next_outbox.negotiated_tuple,
							provider_id: next_outbox.provider_id,
							provider_endpoint_hash: next_outbox.provider_endpoint_hash,
							expected_response_kind: 2,
							created_at: next_outbox.created_at,
							authority_expires_at: next_outbox.authority_expires_at,
						},
						next_outbox.prepare_nonce,
						next_outbox.mark_sent_nonce,
						current.compact_nonce,
					)?;
					current = next_outbox;
				},
				DurableDesktopEvent::Terminal { events: generation_events, response_hash } => {
					events.extend(generation_events);
					return Ok(HostExecutionV2 {
						events,
						terminal_response_hash: Some(response_hash),
					});
				},
				DurableDesktopEvent::NonTerminal(_) =>
					return Err(HostExecutionErrorV2::Backend("HOST_GENERATION_UNINSTALLED")),
			}
		}
		Err(HostExecutionErrorV2::EventLimit)
	}
}

fn operation_id_for(
	operation: OperationCode,
	request_id: [u8; 16],
	explicit: Option<[u8; 16]>,
) -> Result<[u8; 16], HostExecutionErrorV2> {
	if let Some(explicit) = explicit {
		return Ok(explicit);
	}
	if !matches!(
		operation,
		OperationCode::StorageObjectGet |
			OperationCode::StorageObjectRange |
			OperationCode::StorageObjectStatus
	) {
		return Ok([0; 16]);
	}
	let mut hash = Sha256::new();
	hash.update(b"cord/provider/private-object-query/v1");
	hash.update((operation as u16).to_be_bytes());
	hash.update(request_id);
	let digest = hash.finalize();
	digest[..16].try_into().map_err(|_| HostExecutionErrorV2::Registry)
}

/// Provider bytes and host encryption are composed without transferring either authority.
pub(crate) struct CordProviderByteBackendV2<'a, S, V, C, E> {
	provider: DurableCordProviderV2<'a, S, V, C>,
	encryption: E,
}

impl<'a, S, V, C, E> CordProviderByteBackendV2<'a, S, V, C, E> {
	pub(crate) fn new(provider: DurableCordProviderV2<'a, S, V, C>, encryption: E) -> Self {
		Self { provider, encryption }
	}
}

macro_rules! provider_byte_method {
	($method:ident, $frame:ty, $operation:ident) => {
		fn $method(
			&mut self,
			call: HostCallV2<'_, $frame>,
		) -> Result<HostExecutionV2, HostExecutionErrorV2> {
			self.encryption.prepare(OperationCode::$operation, call.frame.canonical())?;
			let mut execution = self.provider.execute(call)?;
			self.encryption.complete(OperationCode::$operation, &mut execution)?;
			Ok(execution)
		}
	};
}

impl<'a, S, V, C, E> ProviderByteStorageV2 for CordProviderByteBackendV2<'a, S, V, C, E>
where
	S: Read + Write,
	V: ResumeTokenVerifierV2,
	C: ProviderContinuationSourceV2,
	E: HostStorageEncryptionV2,
{
	provider_byte_method!(object_put, StorageObjectPutFrame, StorageObjectPut);
	provider_byte_method!(object_get, StorageObjectGetFrame, StorageObjectGet);
	provider_byte_method!(object_range, StorageObjectRangeFrame, StorageObjectRange);
	provider_byte_method!(object_status, StorageObjectStatusFrame, StorageObjectStatus);
}

fn uint_field(value: &Value, wanted: u64) -> Option<u64> {
	let Value::Map(fields) = value else { return None };
	fields.iter().find_map(|(key, value)| match (key, value) {
		(Value::Integer(key), Value::Integer(value))
			if u64::try_from(*key).ok() == Some(wanted) =>
			u64::try_from(*value).ok(),
		_ => None,
	})
}

fn fixed_field(value: &Value, key: u64, length: usize) -> Result<Vec<u8>, HostExecutionErrorV2> {
	optional_fixed_field(value, key, length)?.ok_or(HostExecutionErrorV2::Registry)
}

fn optional_fixed_field(
	value: &Value,
	wanted: u64,
	length: usize,
) -> Result<Option<Vec<u8>>, HostExecutionErrorV2> {
	let Value::Map(fields) = value else { return Err(HostExecutionErrorV2::Registry) };
	for (key, value) in fields {
		if matches!(key, Value::Integer(key) if u64::try_from(*key).ok() == Some(wanted)) {
			let Value::Bytes(bytes) = value else { return Err(HostExecutionErrorV2::Registry) };
			if bytes.len() != length {
				return Err(HostExecutionErrorV2::Registry);
			}
			return Ok(Some(bytes.clone()));
		}
	}
	Ok(None)
}

#[cfg(test)]
mod tests {
	use std::{
		cell::RefCell,
		collections::{BTreeMap, BTreeSet},
		rc::Rc,
	};

	use sp_crypto_hashing::blake2_256;

	use super::*;

	#[derive(Clone, Default)]
	struct RouteBackend(Rc<RefCell<Vec<(&'static str, OperationCode)>>>);

	macro_rules! implement_routes {
		($backend:ident, $authority:literal, { $($method:ident: $frame:ty => $operation:ident),+ $(,)? }) => {
			impl $backend for RouteBackend {
				$(fn $method(
					&mut self,
					_call: HostCallV2<'_, $frame>,
				) -> Result<HostExecutionV2, HostExecutionErrorV2> {
					self.0.borrow_mut().push(($authority, OperationCode::$operation));
					Ok(HostExecutionV2 { events: Vec::new(), terminal_response_hash: None })
				})+
			}
		};
	}

	implement_routes!(ProviderByteStorageV2, "provider", {
		object_put: StorageObjectPutFrame => StorageObjectPut,
		object_get: StorageObjectGetFrame => StorageObjectGet,
		object_range: StorageObjectRangeFrame => StorageObjectRange,
		object_status: StorageObjectStatusFrame => StorageObjectStatus,
	});
	implement_routes!(CommonsStorageControlV2, "commons", {
		bucket_create: StorageBucketCreateFrame => StorageBucketCreate,
		bucket_get: StorageBucketGetFrame => StorageBucketGet,
		bucket_grant: StorageBucketGrantFrame => StorageBucketGrant,
		bucket_revoke: StorageBucketRevokeFrame => StorageBucketRevoke,
		object_delete: StorageObjectDeleteFrame => StorageObjectDelete,
		checkpoint_status: StorageCheckpointStatusFrame => StorageCheckpointStatus,
		checkpoint_subscribe: StorageCheckpointSubscribeFrame => StorageCheckpointSubscribe,
		replica_status: StorageReplicaStatusFrame => StorageReplicaStatus,
		replica_subscribe: StorageReplicaSubscribeFrame => StorageReplicaSubscribe,
		deletion_status: StorageDeletionStatusFrame => StorageDeletionStatus,
		deletion_subscribe: StorageDeletionSubscribeFrame => StorageDeletionSubscribe,
		drive_read: StorageDriveReadFrame => StorageDriveRead,
		drive_commit: StorageDriveCommitFrame => StorageDriveCommit,
		drive_share: StorageDriveShareFrame => StorageDriveShare,
		s3_put: StorageS3PutFrame => StorageS3Put,
		s3_get: StorageS3GetFrame => StorageS3Get,
		s3_list: StorageS3ListFrame => StorageS3List,
		s3_delete: StorageS3DeleteFrame => StorageS3Delete,
		storage_publish: StoragePublishFrame => StoragePublish,
		storage_resolve: StorageResolveFrame => StorageResolve,
	});
	implement_routes!(HostStorageKeystoreV2, "keystore", {
		keys_export: StorageKeysExportFrame => StorageKeysExport,
		keys_import: StorageKeysImportFrame => StorageKeysImport,
	});
	implement_routes!(FinalizedIdentityRuntimeV2, "identity-runtime", {
		identity_account: IdentityAccountFrame => IdentityAccount,
		identity_humanity_status: IdentityHumanityStatusFrame => IdentityHumanityStatus,
		identity_entitlements_read: IdentityEntitlementsReadFrame => IdentityEntitlementsRead,
	});
	implement_routes!(HostIdentityAuthorityV2, "identity-host", {
		identity_profile_read: IdentityProfileReadFrame => IdentityProfileRead,
		identity_profile_disclose: IdentityProfileDiscloseFrame => IdentityProfileDisclose,
		identity_humanity_prove: IdentityHumanityProveFrame => IdentityHumanityProve,
		identity_subject_derive: IdentitySubjectDeriveFrame => IdentitySubjectDerive,
	});
	implement_routes!(HostSigningAuthorityV2, "signing", {
		transaction_sign: TransactionSignFrame => TransactionSign,
	});

	fn expected_authority(operation: OperationCode) -> &'static str {
		match operation {
			OperationCode::StorageObjectPut |
			OperationCode::StorageObjectGet |
			OperationCode::StorageObjectRange |
			OperationCode::StorageObjectStatus => "provider",
			OperationCode::StorageKeysExport | OperationCode::StorageKeysImport => "keystore",
			OperationCode::IdentityAccount |
			OperationCode::IdentityHumanityStatus |
			OperationCode::IdentityEntitlementsRead => "identity-runtime",
			OperationCode::IdentityProfileRead |
			OperationCode::IdentityProfileDisclose |
			OperationCode::IdentityHumanityProve |
			OperationCode::IdentitySubjectDerive => "identity-host",
			OperationCode::TransactionSign => "signing",
			_ => "commons",
		}
	}

	fn route_outbox() -> ProviderOutboxContextV2 {
		ProviderOutboxContextV2 {
			outbox_id: [1; 16],
			generation: 1,
			intended_cursor: 0,
			negotiated_tuple: [2; 32],
			provider_id: [3; 32],
			provider_endpoint_hash: [4; 32],
			created_at: 1,
			authority_expires_at: 100,
			terminal_block: 2,
			prepare_nonce: [5; 24],
			mark_sent_nonce: [6; 24],
			install_nonce: [7; 24],
			mark_ack_nonce: [8; 24],
			confirm_nonce: [9; 24],
			compact_nonce: [10; 24],
		}
	}

	#[derive(Clone, Debug, Eq, PartialEq)]
	struct FinalizedFixture {
		number: u64,
		hash: [u8; 32],
	}

	#[derive(Clone, Debug, Eq, PartialEq)]
	struct CheckpointFixture {
		root: [u8; 32],
		finalized: FinalizedFixture,
		publishable: bool,
	}

	#[derive(Clone, Debug, Eq, PartialEq)]
	struct PublishedFixture {
		cid: String,
		finalized: FinalizedFixture,
	}

	#[derive(Default)]
	struct ProviderFixtureState {
		objects: BTreeMap<String, Vec<u8>>,
		checkpoints: BTreeMap<String, CheckpointFixture>,
		trace: Vec<&'static str>,
	}

	impl ProviderFixtureState {
		fn put_exact(
			&mut self,
			cid: &str,
			digest: [u8; 32],
			bytes: &[u8],
		) -> Result<(), HostExecutionErrorV2> {
			if blake2_256(bytes) != digest ||
				self.objects.insert(cid.into(), bytes.to_vec()).is_some()
			{
				return Err(HostExecutionErrorV2::AppBinding);
			}
			self.trace.push("provider.object.put");
			Ok(())
		}

		fn publishable_checkpoint(
			&mut self,
			cid: &str,
			finalized: FinalizedFixture,
		) -> Result<CheckpointFixture, HostExecutionErrorV2> {
			let bytes = self.objects.get(cid).ok_or(HostExecutionErrorV2::AppBinding)?;
			let checkpoint =
				CheckpointFixture { root: blake2_256(bytes), finalized, publishable: true };
			self.checkpoints.insert(cid.into(), checkpoint.clone());
			self.trace.push("provider.checkpoint.publishable");
			Ok(checkpoint)
		}
	}

	#[derive(Default)]
	struct RuntimeFixtureState {
		height: u64,
		canonical: BTreeMap<u64, [u8; 32]>,
		published: BTreeMap<[u8; 32], PublishedFixture>,
		names: BTreeMap<[u8; 32], ([u8; 32], FinalizedFixture)>,
		trace: Vec<&'static str>,
	}

	impl RuntimeFixtureState {
		fn finalize(&mut self, tag: &[u8]) -> FinalizedFixture {
			self.height += 1;
			let mut material = self.height.to_be_bytes().to_vec();
			material.extend_from_slice(tag);
			let finalized = FinalizedFixture { number: self.height, hash: blake2_256(&material) };
			self.canonical.insert(finalized.number, finalized.hash);
			finalized
		}

		fn publish(
			&mut self,
			name_hash: [u8; 32],
			cid: &str,
			checkpoint: &CheckpointFixture,
		) -> Result<FinalizedFixture, HostExecutionErrorV2> {
			if !checkpoint.publishable ||
				self.canonical.get(&checkpoint.finalized.number) !=
					Some(&checkpoint.finalized.hash)
			{
				return Err(HostExecutionErrorV2::AppBinding);
			}
			let finalized = self.finalize(b"storage.publish");
			self.published.insert(
				name_hash,
				PublishedFixture { cid: cid.into(), finalized: finalized.clone() },
			);
			self.trace.push("commons.storage.publish");
			Ok(finalized)
		}

		fn bind_name(
			&mut self,
			name: [u8; 32],
			content_commitment: [u8; 32],
			after: &FinalizedFixture,
		) -> Result<FinalizedFixture, HostExecutionErrorV2> {
			if self.canonical.get(&after.number) != Some(&after.hash) {
				return Err(HostExecutionErrorV2::AppBinding);
			}
			let finalized = self.finalize(b"names.set_content");
			self.names.insert(name, (content_commitment, finalized.clone()));
			self.trace.push("commons.names.set_content_commitment");
			Ok(finalized)
		}
	}

	struct AppJourneyInput<'a> {
		content_cid: &'a str,
		content_digest: [u8; 32],
		content: &'a [u8],
		manifest_cid: &'a str,
		manifest_digest: [u8; 32],
		manifest: &'a [u8],
		storage_name_hash: [u8; 32],
		name: [u8; 32],
	}

	fn execute_app_journey(
		provider: &mut ProviderFixtureState,
		runtime: &mut RuntimeFixtureState,
		input: AppJourneyInput<'_>,
	) -> Result<(String, FinalizedFixture), HostExecutionErrorV2> {
		provider.put_exact(input.content_cid, input.content_digest, input.content)?;
		provider.put_exact(input.manifest_cid, input.manifest_digest, input.manifest)?;
		let checkpoint_head = runtime.finalize(b"provider.checkpoint");
		let checkpoint = provider.publishable_checkpoint(input.manifest_cid, checkpoint_head)?;
		let published =
			runtime.publish(input.storage_name_hash, input.manifest_cid, &checkpoint)?;
		let names_finalized = runtime.bind_name(input.name, input.manifest_digest, &published)?;

		let (commitment, binding_finality) =
			runtime.names.get(&input.name).ok_or(HostExecutionErrorV2::AppBinding)?;
		let resolved = runtime
			.published
			.get(&input.storage_name_hash)
			.ok_or(HostExecutionErrorV2::AppBinding)?;
		if commitment != &input.manifest_digest ||
			binding_finality != &names_finalized ||
			resolved.cid != input.manifest_cid ||
			resolved.finalized != published ||
			provider.objects.get(&resolved.cid).map(Vec::as_slice) != Some(input.manifest) ||
			provider.checkpoints.get(&resolved.cid) != Some(&checkpoint)
		{
			return Err(HostExecutionErrorV2::AppBinding);
		}
		runtime.trace.push("commons.storage.resolve");
		Ok((resolved.cid.clone(), names_finalized))
	}

	#[test]
	fn generated_registry_is_closed_and_every_operation_has_a_named_backend_method() {
		assert_eq!(OPERATIONS.len(), 34);
		let codes = OPERATIONS.iter().map(|binding| binding.code).collect::<BTreeSet<_>>();
		assert_eq!(codes.len(), 34);
		for binding in OPERATIONS {
			assert_eq!(
				OperationCode::from_u16(binding.code).map(|code| code as u16),
				Some(binding.code)
			);
			assert!(!binding.name.is_empty());
		}
		let storage =
			OPERATIONS.iter().filter(|binding| (1000..1100).contains(&binding.code)).count();
		let identity =
			OPERATIONS.iter().filter(|binding| (1100..1200).contains(&binding.code)).count();
		let signing = OPERATIONS.iter().filter(|binding| binding.code == 1200).count();
		assert_eq!((storage, identity, signing), (26, 7, 1));
	}

	#[test]
	fn every_frozen_positive_request_decodes_and_routes_to_exactly_one_authority() {
		let operations: serde_json::Value = serde_json::from_str(include_str!(
			"../../../../docs/specs/origin-host-registry-v2.operations.json"
		))
		.expect("frozen operations are JSON");
		let vectors: serde_json::Value = serde_json::from_str(include_str!(
			"../../../../docs/specs/origin-host-registry-v2.vectors.json"
		))
		.expect("frozen vectors are JSON");
		let routes = RouteBackend::default();
		let mut dispatcher = CordHostDispatcherV2::new(
			routes.clone(),
			routes.clone(),
			routes.clone(),
			routes.clone(),
			routes.clone(),
			routes.clone(),
		);
		let mut decoded = BTreeSet::new();
		for operation in operations["operations"].as_array().expect("operations are an array") {
			let positive =
				operation["positive_vectors"].as_array().expect("positive vectors are an array");
			assert_eq!(positive.len(), 1, "one authoritative positive vector per operation");
			let vector_id = positive[0].as_str().expect("vector ID is text");
			let vector = vectors["vectors"]
				.as_array()
				.expect("vectors are an array")
				.iter()
				.find(|vector| vector["id"] == vector_id)
				.unwrap_or_else(|| panic!("missing frozen vector {vector_id}"));
			let wire = hex::decode(vector["wire_hex"].as_str().expect("wire is hexadecimal"))
				.expect("frozen wire is hexadecimal");
			let (request, _) = ClosedHostRequestV2::decode(&wire)
				.unwrap_or_else(|error| panic!("{vector_id} failed closed decode: {error}"));
			let code = request.operation();
			assert_eq!(code as u64, operation["code"].as_u64().expect("operation code"));
			assert!(decoded.insert(code as u16), "duplicate operation route");
			let before = routes.0.borrow().len();
			dispatcher
				.dispatch(&wire, b"authority-bound-by-selected-backend", &route_outbox())
				.unwrap_or_else(|error| panic!("{vector_id} dispatch failed: {error}"));
			let recorded = routes.0.borrow();
			assert_eq!(recorded.len(), before + 1, "{vector_id} did not route exactly once");
			assert_eq!(recorded[before], (expected_authority(code), code), "{vector_id}");
		}
		assert_eq!(decoded.len(), 34);
	}

	#[test]
	fn app_journey_uses_stateful_provider_checkpoint_and_canonical_runtime_state() {
		let content = b"CORD private app payload";
		let manifest = b"CORD manifest naming the exact app payload";
		let content_digest = blake2_256(content);
		let manifest_digest = blake2_256(manifest);
		let mut provider = ProviderFixtureState::default();
		let mut runtime = RuntimeFixtureState::default();

		let (cid, finalized) = execute_app_journey(
			&mut provider,
			&mut runtime,
			AppJourneyInput {
				content_cid: "bafk-cord-content",
				content_digest,
				content,
				manifest_cid: "bafk-cord-manifest",
				manifest_digest,
				manifest,
				storage_name_hash: blake2_256(b"festival.origin"),
				name: blake2_256(b"festival"),
			},
		)
		.expect("state-backed journey is valid");

		assert_eq!(cid, "bafk-cord-manifest");
		assert_eq!(runtime.canonical.get(&finalized.number), Some(&finalized.hash));
		assert_eq!(
			provider.trace,
			["provider.object.put", "provider.object.put", "provider.checkpoint.publishable"]
		);
		assert_eq!(
			runtime.trace,
			[
				"commons.storage.publish",
				"commons.names.set_content_commitment",
				"commons.storage.resolve"
			]
		);
	}

	#[test]
	fn app_journey_rejects_unbound_bytes_before_mutating_runtime() {
		let mut provider = ProviderFixtureState::default();
		let mut runtime = RuntimeFixtureState::default();
		let result = execute_app_journey(
			&mut provider,
			&mut runtime,
			AppJourneyInput {
				content_cid: "bafk-cord-content",
				content_digest: [0; 32],
				content: b"different",
				manifest_cid: "bafk-cord-manifest",
				manifest_digest: blake2_256(b"manifest"),
				manifest: b"manifest",
				storage_name_hash: [1; 32],
				name: [2; 32],
			},
		);
		assert!(matches!(result, Err(HostExecutionErrorV2::AppBinding)));
		assert!(runtime.published.is_empty());
		assert!(runtime.names.is_empty());
	}
}
