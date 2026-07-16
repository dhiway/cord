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

//! Private, unrouted provider recovery transitions for `storage.object.put`.

use std::{array, fs, fs::OpenOptions, io::Write};

use ciborium::value::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sp_core::{ed25519, Pair as _};
use unicode_normalization::UnicodeNormalization;

use super::{
	chunk_hash, operation_key, persist_state, sync_dir, validate_chunk_len, validate_descriptor,
	ChunkRecord, OperationRecord, Phase, StreamingDescriptor, StreamingFault, StreamingStore,
	STAGING,
};
use crate::{
	capability::{
		verify_capability, verify_capability_identity, CapabilityError, CapabilityReplayInspection,
		CapabilityReplayInspector, CapabilityRequest, ProviderCapabilityV1,
	},
	BucketId, CanonicalCid, CapabilityAuthoritySnapshot, ContentError, OperationId,
	MAX_STREAMING_OPERATIONS,
};

const METHOD: u16 = 1010;
const RESUME_DOMAIN: &[u8] = b"cord.provider.resume.v1";
const RECOVERY_TTL: u64 = 256;
const MAX_REQUEST_BYTES: usize = 4096;
const MAX_RESUME_TOKEN_BYTES: usize = 4096;
const MAX_RECOVERY_ENTRY_BYTES: usize = 4 * 1024 * 1024 + 8192;
const MAX_ACK_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum RecoveryError {
	#[error("{0}")]
	Capability(#[from] CapabilityError),
	#[error("{0}")]
	Content(#[from] ContentError),
	#[error("RESUME_SIGNATURE_INVALID")]
	ResumeSignatureInvalid,
	#[error("RESUME_REVOKED")]
	ResumeRevoked,
	#[error("RESUME_AUDIENCE_INVALID")]
	ResumeAudienceInvalid,
	#[error("RESUME_REPLAY")]
	ResumeReplay,
	#[error("RESUME_EXPIRED")]
	ResumeExpired,
	#[error("RESUME_CURSOR_INVALID")]
	ResumeCursorInvalid,
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid,
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical,
}

pub(crate) trait RecoverySigner {
	fn public_key(&self) -> [u8; 32];
	fn sign(&self, message: &[u8]) -> [u8; 64];
}

impl RecoverySigner for ed25519::Pair {
	fn public_key(&self) -> [u8; 32] {
		self.public().0
	}

	fn sign(&self, message: &[u8]) -> [u8; 64] {
		<ed25519::Pair as sp_core::Pair>::sign(self, message).0
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObjectPutRequestV2 {
	pub request_id: [u8; 16],
	pub product_id: String,
	pub grant_id: [u8; 32],
	pub operation_id: [u8; 16],
	pub trace_context: Option<Vec<u8>>,
	pub deadline: u64,
	pub bucket_id: [u8; 32],
	pub cid: CanonicalCid,
	pub object_len: u64,
	pub mode: u8,
}

impl ObjectPutRequestV2 {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, RecoveryError> {
		if bytes.len() > MAX_REQUEST_BYTES {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(RecoveryError::WireSchemaInvalid) };
		let mut fields: [Option<Value>; 9] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key = integer(key)? as usize;
			if key > 8 || fields[key].replace(value).is_some() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		for key in [0usize, 1, 2, 3, 4, 5, 7, 8] {
			if fields[key].is_none() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		if integer(take(&mut fields, 0)?)? != 2 ||
			integer(take(&mut fields, 3)?)? != u64::from(METHOD)
		{
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let request_id = bytes_fixed(take(&mut fields, 1)?)?;
		let product_id = text(take(&mut fields, 2)?, 128)?;
		let grant_id = bytes_fixed(take(&mut fields, 4)?)?;
		let operation_id = bytes_fixed(take(&mut fields, 5)?)?;
		let trace_context =
			fields[6].take().map(|value| bounded_bytes(value, 1, 64)).transpose()?;
		let deadline = integer(take(&mut fields, 7)?)?;
		let Value::Map(body) = take(&mut fields, 8)? else {
			return Err(RecoveryError::WireSchemaInvalid)
		};
		let mut body_fields: [Option<Value>; 5] = array::from_fn(|_| None);
		for (key, value) in body {
			let key = integer(key)? as usize;
			if key > 4 || body_fields[key].replace(value).is_some() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		if body_fields.iter().any(Option::is_none) {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let bucket_id = bytes_fixed(take(&mut body_fields, 0)?)?;
		let cid = CanonicalCid::parse(&text(take(&mut body_fields, 1)?, 128)?)
			.map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let object_len = integer(take(&mut body_fields, 2)?)?;
		let mode: u8 = integer(take(&mut body_fields, 3)?)?
			.try_into()
			.map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let body_operation: [u8; 16] = bytes_fixed(take(&mut body_fields, 4)?)?;
		if mode > 1 || body_operation != operation_id {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let request = Self {
			request_id,
			product_id,
			grant_id,
			operation_id,
			trace_context,
			deadline,
			bucket_id,
			cid,
			object_len,
			mode,
		};
		if request.canonical_bytes() != bytes {
			return Err(RecoveryError::WireNonCanonical)
		}
		Ok(request)
	}

	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		let mut fields = vec![
			(0, uint(2)),
			(1, bstr(&self.request_id)),
			(2, Value::Text(self.product_id.clone())),
			(3, uint(u64::from(METHOD))),
			(4, bstr(&self.grant_id)),
			(5, bstr(&self.operation_id)),
		];
		if let Some(trace_context) = &self.trace_context {
			fields.push((6, bstr(trace_context)));
		}
		fields.extend([
			(7, uint(self.deadline)),
			(
				8,
				Value::Map(vec![
					(uint(0), bstr(&self.bucket_id)),
					(uint(1), Value::Text(self.cid.as_str().into())),
					(uint(2), uint(self.object_len)),
					(uint(3), uint(u64::from(self.mode))),
					(uint(4), bstr(&self.operation_id)),
				]),
			),
		]);
		map(fields)
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResumeTokenV1 {
	pub registry_sha256: [u8; 32],
	pub genesis_hash: [u8; 32],
	pub provider: [u8; 32],
	pub host_key_id: [u8; 32],
	pub operation_id: [u8; 16],
	pub bucket_id: [u8; 32],
	pub cid: CanonicalCid,
	pub object_len: u64,
	pub cursor: u32,
	pub generation: u64,
	pub issued_at: u64,
	pub expires_at: u64,
	pub nonce: [u8; 16],
	pub cancelled: bool,
	pub signature: [u8; 64],
}

impl ResumeTokenV1 {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, RecoveryError> {
		if bytes.len() > MAX_RESUME_TOKEN_BYTES {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(RecoveryError::WireSchemaInvalid) };
		let mut f: [Option<Value>; 16] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key = integer(key)? as usize;
			if key > 15 || f[key].replace(value).is_some() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		if f.iter().any(Option::is_none) || integer(take(&mut f, 0)?)? != 1 {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let token = Self {
			registry_sha256: bytes_fixed(take(&mut f, 1)?)?,
			genesis_hash: bytes_fixed(take(&mut f, 2)?)?,
			provider: bytes_fixed(take(&mut f, 3)?)?,
			host_key_id: bytes_fixed(take(&mut f, 4)?)?,
			operation_id: bytes_fixed(take(&mut f, 5)?)?,
			bucket_id: bytes_fixed(take(&mut f, 6)?)?,
			cid: CanonicalCid::parse(&text(take(&mut f, 7)?, 128)?)
				.map_err(|_| RecoveryError::WireSchemaInvalid)?,
			object_len: integer(take(&mut f, 8)?)?,
			cursor: integer(take(&mut f, 9)?)?
				.try_into()
				.map_err(|_| RecoveryError::WireSchemaInvalid)?,
			generation: integer(take(&mut f, 10)?)?,
			issued_at: integer(take(&mut f, 11)?)?,
			expires_at: integer(take(&mut f, 12)?)?,
			nonce: bytes_fixed(take(&mut f, 13)?)?,
			cancelled: boolean(take(&mut f, 14)?)?,
			signature: bytes_fixed(take(&mut f, 15)?)?,
		};
		if token.canonical_bytes() != bytes {
			return Err(RecoveryError::WireNonCanonical)
		}
		Ok(token)
	}
	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		self.encode(true)
	}
	fn signed_bytes(&self) -> Vec<u8> {
		let mut out = RESUME_DOMAIN.to_vec();
		out.extend(self.encode(false));
		out
	}
	fn encode(&self, signature: bool) -> Vec<u8> {
		let mut entries = vec![
			(0, uint(1)),
			(1, bstr(&self.registry_sha256)),
			(2, bstr(&self.genesis_hash)),
			(3, bstr(&self.provider)),
			(4, bstr(&self.host_key_id)),
			(5, bstr(&self.operation_id)),
			(6, bstr(&self.bucket_id)),
			(7, Value::Text(self.cid.as_str().into())),
			(8, uint(self.object_len)),
			(9, uint(u64::from(self.cursor))),
			(10, uint(self.generation)),
			(11, uint(self.issued_at)),
			(12, uint(self.expires_at)),
			(13, bstr(&self.nonce)),
			(14, Value::Bool(self.cancelled)),
		];
		if signature {
			entries.push((15, bstr(&self.signature)));
		}
		map(entries)
	}
	fn signed(mut self, signer: &dyn RecoverySigner) -> Self {
		self.signature = signer.sign(&self.signed_bytes());
		self
	}
	pub(crate) fn verify(&self, public: [u8; 32]) -> Result<(), RecoveryError> {
		if ed25519::Pair::verify(
			&ed25519::Signature::from_raw(self.signature),
			&self.signed_bytes(),
			&ed25519::Public::from_raw(public),
		) {
			Ok(())
		} else {
			Err(RecoveryError::ResumeSignatureInvalid)
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResponseAckV1 {
	pub request_id: [u8; 16],
	pub operation_id: [u8; 16],
	pub generation: u64,
	pub response_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryEntryV1 {
	pub operation_id: [u8; 16],
	pub request_id: [u8; 16],
	pub generation: u64,
	pub fingerprint: [u8; 32],
	pub nonce: [u8; 16],
	pub prior_cursor: u32,
	pub new_cursor: u32,
	pub response: Vec<u8>,
	pub successor_token: Option<Vec<u8>>,
	pub response_hash: [u8; 32],
	pub effect_state_hash: [u8; 32],
	pub acknowledged: bool,
	pub retain_until: u64,
}

impl RecoveryEntryV1 {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, RecoveryError> {
		if bytes.len() > MAX_RECOVERY_ENTRY_BYTES {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(RecoveryError::WireSchemaInvalid) };
		let mut f: [Option<Value>; 13] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key = integer(key)? as usize;
			if key > 12 || f[key].replace(value).is_some() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		for key in [0usize, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12] {
			if f[key].is_none() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		let entry = Self {
			operation_id: bytes_fixed(take(&mut f, 0)?)?,
			request_id: bytes_fixed(take(&mut f, 1)?)?,
			generation: integer(take(&mut f, 2)?)?,
			fingerprint: bytes_fixed(take(&mut f, 3)?)?,
			nonce: bytes_fixed(take(&mut f, 4)?)?,
			prior_cursor: integer(take(&mut f, 5)?)?
				.try_into()
				.map_err(|_| RecoveryError::WireSchemaInvalid)?,
			new_cursor: integer(take(&mut f, 6)?)?
				.try_into()
				.map_err(|_| RecoveryError::WireSchemaInvalid)?,
			response: bounded_bytes(take(&mut f, 7)?, 0, 4 * 1024 * 1024)?,
			successor_token: f[8].take().map(|value| bounded_bytes(value, 1, 4096)).transpose()?,
			response_hash: bytes_fixed(take(&mut f, 9)?)?,
			effect_state_hash: bytes_fixed(take(&mut f, 10)?)?,
			acknowledged: boolean(take(&mut f, 11)?)?,
			retain_until: integer(take(&mut f, 12)?)?,
		};
		if entry.canonical_bytes() != bytes {
			return Err(RecoveryError::WireNonCanonical)
		}
		Ok(entry)
	}

	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		let mut fields = vec![
			(0, bstr(&self.operation_id)),
			(1, bstr(&self.request_id)),
			(2, uint(self.generation)),
			(3, bstr(&self.fingerprint)),
			(4, bstr(&self.nonce)),
			(5, uint(u64::from(self.prior_cursor))),
			(6, uint(u64::from(self.new_cursor))),
			(7, bstr(&self.response)),
		];
		if let Some(token) = &self.successor_token {
			fields.push((8, bstr(token)));
		}
		fields.extend([
			(9, bstr(&self.response_hash)),
			(10, bstr(&self.effect_state_hash)),
			(11, Value::Bool(self.acknowledged)),
			(12, uint(self.retain_until)),
		]);
		map(fields)
	}
}

impl ResponseAckV1 {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, RecoveryError> {
		if bytes.len() > MAX_ACK_BYTES {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| RecoveryError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(RecoveryError::WireSchemaInvalid) };
		let mut f: [Option<Value>; 4] = array::from_fn(|_| None);
		for (k, v) in entries {
			let k = integer(k)? as usize;
			if k > 3 || f[k].replace(v).is_some() {
				return Err(RecoveryError::WireSchemaInvalid)
			}
		}
		if f.iter().any(Option::is_none) {
			return Err(RecoveryError::WireSchemaInvalid)
		}
		let ack = Self {
			request_id: bytes_fixed(take(&mut f, 0)?)?,
			operation_id: bytes_fixed(take(&mut f, 1)?)?,
			generation: integer(take(&mut f, 2)?)?,
			response_hash: bytes_fixed(take(&mut f, 3)?)?,
		};
		if ack.canonical_bytes() != bytes {
			return Err(RecoveryError::WireNonCanonical)
		}
		Ok(ack)
	}
	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		map(vec![
			(0, bstr(&self.request_id)),
			(1, bstr(&self.operation_id)),
			(2, uint(self.generation)),
			(3, bstr(&self.response_hash)),
		])
	}
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryResponseV1 {
	pub response: Vec<u8>,
	pub successor_token: Option<Vec<u8>>,
	pub response_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecoveryEffect {
	Accepted,
	Progress,
	Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecoveryRecord {
	effect: RecoveryEffect,
	request: String,
	authority: String,
	authority_public_key: String,
	successor_public_key: String,
	provider: String,
	operation_id: String,
	request_id: String,
	generation: u64,
	fingerprint: String,
	nonce: String,
	prior_cursor: u32,
	cursor: u32,
	received_bytes: u64,
	response_sequence: u32,
	response: String,
	successor_token: Option<String>,
	response_hash: String,
	host_key_id: String,
	acknowledged: bool,
	retain_until: u64,
	descriptor: StreamingDescriptor,
	effect_hash: String,
	chunk_hash: String,
	entry_cbor: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CapabilityReplayRecord {
	recovery_key: String,
	fingerprint: String,
	retain_until: u64,
}

struct Fresh;
impl CapabilityReplayInspector for Fresh {
	fn inspect(&self, _: &[u8; 32], _: &[u8; 16], _: &[u8; 32]) -> CapabilityReplayInspection {
		CapabilityReplayInspection::Fresh
	}
}

pub(super) fn validate_recovery_state(state: &super::JournalState) -> Result<(), ContentError> {
	let mut terminal_operations = std::collections::BTreeSet::new();
	let mut chains: std::collections::BTreeMap<String, Vec<&RecoveryRecord>> =
		std::collections::BTreeMap::new();
	for (stored_key, record) in &state.recovery {
		let request_bytes =
			hex::decode(&record.request).map_err(|_| ContentError::IntegrityFailed)?;
		let authority =
			hex::decode(&record.authority).map_err(|_| ContentError::IntegrityFailed)?;
		let request = ObjectPutRequestV2::decode(&request_bytes)
			.map_err(|_| ContentError::IntegrityFailed)?;
		let operation: [u8; 16] = decode_hex(&record.operation_id)?;
		let request_id: [u8; 16] = decode_hex(&record.request_id)?;
		let nonce: [u8; 16] = decode_hex(&record.nonce)?;
		let host_key: [u8; 32] = decode_hex(&record.host_key_id)?;
		let provider: [u8; 32] = decode_hex(&record.provider)?;
		let chunk_hash = match record.effect {
			RecoveryEffect::Accepted | RecoveryEffect::Cancelled => {
				if !record.chunk_hash.is_empty() {
					return Err(ContentError::IntegrityFailed)
				}
				None
			},
			RecoveryEffect::Progress => {
				let encoded =
					record.chunk_hash.strip_prefix("0x").ok_or(ContentError::IntegrityFailed)?;
				decode_hex::<32>(encoded)?;
				Some(record.chunk_hash.as_str())
			},
		};
		if operation != request.operation_id ||
			request_id != request.request_id ||
			record.fingerprint != hex::encode(fingerprint(&request_bytes, &authority)) ||
			*stored_key != recovery_key(host_key, operation, record.generation, nonce) ||
			record.descriptor.operation_id != OperationId::from_bytes(operation) ||
			record.descriptor.bucket_id != BucketId::from_bytes(request.bucket_id) ||
			record.descriptor.expected_cid != request.cid.as_str() ||
			record.descriptor.object_len != request.object_len ||
			record.effect_hash !=
				hex::encode(effect_state_hash(
					record.effect,
					&record.descriptor,
					record.cursor,
					record.received_bytes,
					chunk_hash,
				)) {
			return Err(ContentError::IntegrityFailed)
		}
		let response = hex::decode(&record.response).map_err(|_| ContentError::IntegrityFailed)?;
		let expected_response = match record.effect {
			RecoveryEffect::Accepted => {
				if record.generation != 0 ||
					record.prior_cursor != 0 ||
					record.cursor != 0 ||
					record.received_bytes != 0 ||
					record.response_sequence != 0
				{
					return Err(ContentError::IntegrityFailed)
				}
				accepted_response(request_id)
			},
			RecoveryEffect::Progress => progress_response(
				request_id,
				record.response_sequence,
				record.received_bytes,
				record.cursor,
			),
			RecoveryEffect::Cancelled => {
				if record.prior_cursor != record.cursor {
					return Err(ContentError::IntegrityFailed)
				}
				terminal_operations.insert(operation_key(&record.descriptor));
				cancelled_response(request_id, record.response_sequence)
			},
		};
		if response != expected_response ||
			record.response_hash != hex::encode(Sha256::digest(&response))
		{
			return Err(ContentError::IntegrityFailed)
		}
		let (authority_expiry, authority_provider) =
			if let Ok(capability) = ProviderCapabilityV1::decode(&authority) {
				capability
					.verify_signature(decode_hex(&record.authority_public_key)?)
					.map_err(|_| ContentError::IntegrityFailed)?;
				(capability.expires_at, capability.provider)
			} else {
				let token =
					ResumeTokenV1::decode(&authority).map_err(|_| ContentError::IntegrityFailed)?;
				token
					.verify(decode_hex(&record.authority_public_key)?)
					.map_err(|_| ContentError::IntegrityFailed)?;
				if token.generation != record.generation || token.nonce != nonce {
					return Err(ContentError::IntegrityFailed)
				}
				(token.expires_at, token.provider)
			};
		if authority_provider != provider ||
			record.retain_until !=
				authority_expiry
					.checked_add(RECOVERY_TTL)
					.ok_or(ContentError::IntegrityFailed)?
		{
			return Err(ContentError::IntegrityFailed)
		}
		let successor = match (&record.effect, &record.successor_token) {
			(RecoveryEffect::Cancelled, None) => None,
			(RecoveryEffect::Accepted | RecoveryEffect::Progress, Some(encoded)) => {
				let bytes = hex::decode(encoded).map_err(|_| ContentError::IntegrityFailed)?;
				let token =
					ResumeTokenV1::decode(&bytes).map_err(|_| ContentError::IntegrityFailed)?;
				token
					.verify(decode_hex(&record.successor_public_key)?)
					.map_err(|_| ContentError::IntegrityFailed)?;
				if token.provider != provider ||
					token.operation_id != operation ||
					token.cursor != record.cursor ||
					token.generation !=
						record
							.generation
							.checked_add(1)
							.ok_or(ContentError::IntegrityFailed)?
				{
					return Err(ContentError::IntegrityFailed)
				}
				Some(token.canonical_bytes())
			},
			_ => return Err(ContentError::IntegrityFailed),
		};
		let entry = RecoveryEntryV1 {
			operation_id: operation,
			request_id,
			generation: record.generation,
			fingerprint: decode_hex(&record.fingerprint)?,
			nonce,
			prior_cursor: record.prior_cursor,
			new_cursor: record.cursor,
			response: response.clone(),
			successor_token: successor,
			response_hash: decode_hex(&record.response_hash)?,
			effect_state_hash: decode_hex(&record.effect_hash)?,
			acknowledged: record.acknowledged,
			retain_until: record.retain_until,
		};
		let entry_bytes = entry.canonical_bytes();
		if record.entry_cbor != hex::encode(&entry_bytes) {
			return Err(ContentError::IntegrityFailed)
		}
		RecoveryEntryV1::decode(&entry_bytes).map_err(|_| ContentError::IntegrityFailed)?;
		let operation_key = operation_key(&record.descriptor);
		chains.entry(operation_key.clone()).or_default().push(record);
		if state.operations.get(&operation_key).map(|item| &item.descriptor) !=
			Some(&record.descriptor)
		{
			return Err(ContentError::IntegrityFailed)
		}
		if record.effect == RecoveryEffect::Cancelled &&
			state.operations.get(&operation_key).map(|item| item.phase) != Some(Phase::Cancelled)
		{
			return Err(ContentError::IntegrityFailed)
		}
	}
	for (operation_key, chain) in &mut chains {
		chain.sort_by_key(|record| record.generation);
		let operation = state.operations.get(operation_key).ok_or(ContentError::IntegrityFailed)?;
		let first = chain.first().ok_or(ContentError::IntegrityFailed)?;
		if first.effect != RecoveryEffect::Accepted || first.generation != 0 {
			return Err(ContentError::IntegrityFailed)
		}
		for pair in chain.windows(2) {
			let previous = pair[0];
			let current = pair[1];
			if current.generation !=
				previous.generation.checked_add(1).ok_or(ContentError::IntegrityFailed)? ||
				current.authority !=
					previous
						.successor_token
						.as_deref()
						.ok_or(ContentError::IntegrityFailed)? ||
				current.prior_cursor != previous.cursor ||
				current.response_sequence !=
					previous
						.response_sequence
						.checked_add(1)
						.ok_or(ContentError::IntegrityFailed)? ||
				current.request != first.request ||
				current.request_id != first.request_id ||
				current.descriptor != first.descriptor
			{
				return Err(ContentError::IntegrityFailed)
			}
			match current.effect {
				RecoveryEffect::Progress
					if current.prior_cursor.checked_add(1) == Some(current.cursor) => {},
				RecoveryEffect::Cancelled if current.cursor == current.prior_cursor => {},
				_ => return Err(ContentError::IntegrityFailed),
			}
		}
		for record in chain.iter().filter(|record| record.effect == RecoveryEffect::Progress) {
			let cursor: usize =
				record.cursor.try_into().map_err(|_| ContentError::IntegrityFailed)?;
			if cursor == 0 ||
				cursor > operation.chunks.len() ||
				operation.chunks[cursor - 1].hash != record.chunk_hash
			{
				return Err(ContentError::IntegrityFailed)
			}
			let cumulative = operation.chunks[..cursor].iter().try_fold(0u64, |total, chunk| {
				total.checked_add(u64::from(chunk.length)).ok_or(ContentError::IntegrityFailed)
			})?;
			if record.received_bytes != cumulative {
				return Err(ContentError::IntegrityFailed)
			}
		}
		let latest = chain.last().ok_or(ContentError::IntegrityFailed)?;
		if latest.cursor != u32::from(operation.next_chunk) ||
			latest.received_bytes != operation.received_bytes ||
			operation.chunks.len() != usize::from(operation.next_chunk)
		{
			return Err(ContentError::IntegrityFailed)
		}
		if let Some(last_progress) =
			chain.iter().rev().find(|record| record.effect == RecoveryEffect::Progress)
		{
			if operation.chunks.last().map(|chunk| chunk.hash.as_str()) !=
				Some(last_progress.chunk_hash.as_str())
			{
				return Err(ContentError::IntegrityFailed)
			}
		} else if !operation.chunks.is_empty() {
			return Err(ContentError::IntegrityFailed)
		}
		match (operation.phase, latest.effect) {
			(Phase::Receiving, RecoveryEffect::Accepted | RecoveryEffect::Progress) |
			(Phase::Cancelled, RecoveryEffect::Cancelled) => {},
			_ => return Err(ContentError::IntegrityFailed),
		}
	}
	for (key, operation) in &state.operations {
		if operation.phase == Phase::Cancelled && !terminal_operations.contains(key) {
			return Err(ContentError::IntegrityFailed)
		}
	}
	for (key, replay) in &state.capability_replay {
		decode_hex::<32>(key)?;
		decode_hex::<32>(&replay.fingerprint)?;
		let recovery =
			state.recovery.get(&replay.recovery_key).ok_or(ContentError::IntegrityFailed)?;
		if recovery.effect != RecoveryEffect::Accepted ||
			recovery.fingerprint != replay.fingerprint ||
			recovery.retain_until != replay.retain_until
		{
			return Err(ContentError::IntegrityFailed)
		}
		let authority =
			hex::decode(&recovery.authority).map_err(|_| ContentError::IntegrityFailed)?;
		let capability =
			ProviderCapabilityV1::decode(&authority).map_err(|_| ContentError::IntegrityFailed)?;
		if *key != capability_replay_key(capability.grant_id, capability.nonce) {
			return Err(ContentError::IntegrityFailed)
		}
	}
	Ok(())
}

impl StreamingStore {
	pub(crate) fn accept_object_put(
		&self,
		request_bytes: &[u8],
		authority_bytes: &[u8],
		snapshot: &CapabilityAuthoritySnapshot,
		current_service_key: [u8; 32],
		signer: &dyn RecoverySigner,
		successor_nonce: [u8; 16],
	) -> Result<RecoveryResponseV1, RecoveryError> {
		let request = ObjectPutRequestV2::decode(request_bytes)?;
		let capability = ProviderCapabilityV1::decode(authority_bytes)?;
		let capability_request = CapabilityRequest {
			product_id: &request.product_id,
			bucket_id: request.bucket_id,
			agreement_id: capability.agreement_id,
			method: METHOD,
			cid: Some(&request.cid),
			bytes: request.object_len,
			requires_agreement: capability.agreement_id.is_some(),
		};
		verify_capability_identity(&capability, snapshot, &capability_request)?;
		if capability.grant_id != request.grant_id ||
			capability.provider != snapshot.local_provider ||
			capability.registry_sha256 != snapshot.registry_sha256 ||
			capability.genesis_hash != snapshot.genesis_hash ||
			capability.product_id != request.product_id ||
			capability.bucket_id != request.bucket_id ||
			capability.cid.as_ref() != Some(&request.cid) ||
			request.object_len > capability.max_bytes ||
			!capability.methods.contains(&METHOD) ||
			request.deadline > capability.expires_at
		{
			return Err(CapabilityError::GrantScopeDenied.into())
		}
		let key = recovery_key(capability.issuer_key_id, request.operation_id, 0, capability.nonce);
		let replay_key = capability_replay_key(capability.grant_id, capability.nonce);
		let fingerprint = fingerprint(request_bytes, authority_bytes);
		{
			let state = self.read_state()?;
			if let Some(record) = state.recovery.get(&key) {
				return recover(
					record,
					RecoveryEffect::Accepted,
					request_bytes,
					authority_bytes,
					fingerprint,
					capability.provider,
					u64::from(snapshot.finalized_number),
				)
			}
		}
		if signer.public_key() != current_service_key {
			return Err(RecoveryError::ResumeRevoked)
		}
		if u64::from(snapshot.finalized_number) >= request.deadline {
			return Err(CapabilityError::CapabilityExpired.into())
		}
		{
			let state = self.read_state()?;
			if state.capability_replay.contains_key(&replay_key) {
				return Err(CapabilityError::CapabilityNonceReplay.into())
			}
		}
		verify_capability(&capability, snapshot, capability_request, &Fresh)?;
		let descriptor = StreamingDescriptor {
			operation_id: OperationId::from_bytes(request.operation_id),
			bucket_id: BucketId::from_bytes(request.bucket_id),
			expected_cid: request.cid.as_str().into(),
			object_len: request.object_len,
		};
		validate_descriptor(&descriptor)?;
		let mut state = self.write_state()?;
		if let Some(record) = state.recovery.get(&key) {
			return recover(
				record,
				RecoveryEffect::Accepted,
				request_bytes,
				authority_bytes,
				fingerprint,
				capability.provider,
				u64::from(snapshot.finalized_number),
			)
		}
		if state.capability_replay.contains_key(&replay_key) {
			return Err(CapabilityError::CapabilityNonceReplay.into())
		}
		if state.recovery.len() >= MAX_STREAMING_OPERATIONS {
			return Err(ContentError::ProviderRecoveryTableFull.into())
		}
		let operation_key = operation_key(&descriptor);
		if state.operations.contains_key(&operation_key) {
			return Err(ContentError::IdempotencyConflict.into())
		}
		if state.operations.len() >= self.operation_limit {
			return Err(ContentError::ProviderRecoveryTableFull.into())
		}
		let path = self.root.join(STAGING).join(format!("{operation_key}.part"));
		let file = OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(&path)
			.map_err(|e| ContentError::Io(e.to_string()))?;
		file.sync_all().map_err(|e| ContentError::Io(e.to_string()))?;
		sync_dir(path.parent().expect("parent"))?;
		let response = accepted_response(request.request_id);
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		let token = ResumeTokenV1 {
			registry_sha256: capability.registry_sha256,
			genesis_hash: capability.genesis_hash,
			provider: capability.provider,
			host_key_id: capability.issuer_key_id,
			operation_id: request.operation_id,
			bucket_id: request.bucket_id,
			cid: request.cid.clone(),
			object_len: request.object_len,
			cursor: 0,
			generation: 1,
			issued_at: u64::from(snapshot.finalized_number),
			expires_at: capability.expires_at,
			nonce: successor_nonce,
			cancelled: false,
			signature: [0; 64],
		}
		.signed(signer);
		let token_bytes = token.canonical_bytes();
		let retain_until = capability
			.expires_at
			.checked_add(RECOVERY_TTL)
			.ok_or(ContentError::IntegrityFailed)?;
		let initial_effect_hash =
			effect_state_hash(RecoveryEffect::Accepted, &descriptor, 0, 0, None);
		let entry_cbor = RecoveryEntryV1 {
			operation_id: request.operation_id,
			request_id: request.request_id,
			generation: 0,
			fingerprint,
			nonce: capability.nonce,
			prior_cursor: 0,
			new_cursor: 0,
			response: response.clone(),
			successor_token: Some(token_bytes.clone()),
			response_hash,
			effect_state_hash: initial_effect_hash,
			acknowledged: false,
			retain_until,
		}
		.canonical_bytes();
		let record = RecoveryRecord {
			effect: RecoveryEffect::Accepted,
			request: hex::encode(request_bytes),
			authority: hex::encode(authority_bytes),
			authority_public_key: hex::encode(snapshot.delegation.issuer_public_key),
			successor_public_key: hex::encode(signer.public_key()),
			provider: hex::encode(capability.provider),
			operation_id: hex::encode(request.operation_id),
			request_id: hex::encode(request.request_id),
			generation: 0,
			fingerprint: hex::encode(fingerprint),
			nonce: hex::encode(capability.nonce),
			prior_cursor: 0,
			cursor: 0,
			received_bytes: 0,
			response_sequence: 0,
			response: hex::encode(&response),
			successor_token: Some(hex::encode(&token_bytes)),
			response_hash: hex::encode(response_hash),
			host_key_id: hex::encode(capability.issuer_key_id),
			acknowledged: false,
			retain_until,
			descriptor: descriptor.clone(),
			effect_hash: hex::encode(initial_effect_hash),
			chunk_hash: String::new(),
			entry_cbor: hex::encode(entry_cbor),
		};
		let mut next = state.clone();
		next.operations.insert(
			operation_key,
			OperationRecord {
				descriptor,
				phase: Phase::Receiving,
				install_sequence: None,
				next_chunk: 0,
				received_bytes: 0,
				chunks: Vec::<ChunkRecord>::new(),
				receipt: None,
			},
		);
		next.recovery.insert(key, record);
		next.capability_replay.insert(
			replay_key,
			CapabilityReplayRecord {
				recovery_key: recovery_key(
					capability.issuer_key_id,
					request.operation_id,
					0,
					capability.nonce,
				),
				fingerprint: hex::encode(fingerprint),
				retain_until,
			},
		);
		self.trip_fault(StreamingFault::BeforeRecoveryCommit)?;
		persist_state(&self.root, &next)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterRecoveryCommit)?;
		Ok(RecoveryResponseV1 { response, successor_token: Some(token_bytes), response_hash })
	}

	#[allow(clippy::too_many_arguments)]
	pub(crate) fn advance_object_put(
		&self,
		request_bytes: &[u8],
		token_bytes: &[u8],
		snapshot: &CapabilityAuthoritySnapshot,
		current_service_key: [u8; 32],
		cursor: u32,
		chunk: &[u8],
		signer: &dyn RecoverySigner,
		successor_nonce: [u8; 16],
	) -> Result<RecoveryResponseV1, RecoveryError> {
		let request = ObjectPutRequestV2::decode(request_bytes)?;
		let token = ResumeTokenV1::decode(token_bytes)?;
		let local_provider = snapshot.local_provider;
		let now = u64::from(snapshot.finalized_number);
		if token.provider != local_provider ||
			token.registry_sha256 != snapshot.registry_sha256 ||
			token.genesis_hash != snapshot.genesis_hash ||
			token.operation_id != request.operation_id ||
			token.bucket_id != request.bucket_id ||
			token.cid != request.cid ||
			token.object_len != request.object_len
		{
			return Err(RecoveryError::ResumeAudienceInvalid)
		}
		let key =
			recovery_key(token.host_key_id, token.operation_id, token.generation, token.nonce);
		let fingerprint = fingerprint(request_bytes, token_bytes);
		let chunk_digest = chunk_hash(chunk);
		{
			let state = self.read_state()?;
			if let Some(record) = state.recovery.get(&key) {
				if record.effect != RecoveryEffect::Progress || record.chunk_hash != chunk_digest {
					return Err(RecoveryError::ResumeReplay)
				}
				return recover(
					record,
					RecoveryEffect::Progress,
					request_bytes,
					token_bytes,
					fingerprint,
					local_provider,
					now,
				)
			}
		}
		if token.cancelled {
			return Err(RecoveryError::ResumeReplay)
		}
		let (prior_snapshot, accepted) = {
			let state = self.read_state()?;
			let prior = predecessor(&state, token_bytes)?.clone();
			let accepted = accepted_root(&state, &prior.descriptor)?.clone();
			(prior, accepted)
		};
		let recorded_service_key = decode_hex(&prior_snapshot.successor_public_key)?;
		token.verify(recorded_service_key)?;
		if recorded_service_key != current_service_key || signer.public_key() != current_service_key
		{
			return Err(RecoveryError::ResumeRevoked)
		}
		validate_fresh_resume_authority(&request, &token, &accepted, snapshot, now)?;
		if token.cursor.checked_add(1) != Some(cursor) {
			return Err(RecoveryError::ResumeCursorInvalid)
		}
		let internal_index: u16 =
			token.cursor.try_into().map_err(|_| RecoveryError::ResumeCursorInvalid)?;
		validate_chunk_len(request.object_len, internal_index, chunk.len())?;
		let mut state = self.write_state()?;
		if let Some(record) = state.recovery.get(&key) {
			if record.effect != RecoveryEffect::Progress || record.chunk_hash != chunk_digest {
				return Err(RecoveryError::ResumeReplay)
			}
			return recover(
				record,
				RecoveryEffect::Progress,
				request_bytes,
				token_bytes,
				fingerprint,
				local_provider,
				now,
			)
		}
		if state.recovery.len() >= MAX_STREAMING_OPERATIONS {
			return Err(ContentError::ProviderRecoveryTableFull.into())
		}
		let prior = predecessor(&state, token_bytes)?;
		if prior.cursor != token.cursor || prior.provider != hex::encode(local_provider) {
			return Err(RecoveryError::ResumeReplay)
		}
		let operation_key = operation_key(&prior.descriptor);
		let operation =
			state.operations.get(&operation_key).cloned().ok_or(ContentError::NotFound)?;
		if operation.phase != Phase::Receiving || operation.next_chunk != internal_index {
			return Err(RecoveryError::ResumeCursorInvalid)
		}
		let path = self.root.join(STAGING).join(format!("{operation_key}.part"));
		let mut file = OpenOptions::new()
			.append(true)
			.open(&path)
			.map_err(|error| ContentError::Io(error.to_string()))?;
		if file.metadata().map_err(|error| ContentError::Io(error.to_string()))?.len() !=
			operation.received_bytes
		{
			return Err(ContentError::IntegrityFailed.into())
		}
		file.write_all(chunk).map_err(|error| ContentError::Io(error.to_string()))?;
		file.sync_all().map_err(|error| ContentError::Io(error.to_string()))?;
		let received = operation
			.received_bytes
			.checked_add(chunk.len() as u64)
			.ok_or(ContentError::ObjectTooLarge)?;
		let sequence =
			prior.response_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		let response = progress_response(request.request_id, sequence, received, cursor);
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		let successor = ResumeTokenV1 {
			registry_sha256: token.registry_sha256,
			genesis_hash: token.genesis_hash,
			provider: token.provider,
			host_key_id: token.host_key_id,
			operation_id: token.operation_id,
			bucket_id: token.bucket_id,
			cid: token.cid.clone(),
			object_len: token.object_len,
			cursor,
			generation: token.generation.checked_add(1).ok_or(ContentError::IntegrityFailed)?,
			issued_at: now,
			expires_at: token.expires_at,
			nonce: successor_nonce,
			cancelled: false,
			signature: [0; 64],
		}
		.signed(signer);
		let successor_bytes = successor.canonical_bytes();
		let descriptor = prior.descriptor.clone();
		let effect_state_hash = effect_state_hash(
			RecoveryEffect::Progress,
			&descriptor,
			cursor,
			received,
			Some(&chunk_digest),
		);
		let retain_until = token
			.expires_at
			.checked_add(RECOVERY_TTL)
			.ok_or(ContentError::IntegrityFailed)?;
		let entry_cbor = RecoveryEntryV1 {
			operation_id: request.operation_id,
			request_id: request.request_id,
			generation: token.generation,
			fingerprint,
			nonce: token.nonce,
			prior_cursor: token.cursor,
			new_cursor: cursor,
			response: response.clone(),
			successor_token: Some(successor_bytes.clone()),
			response_hash,
			effect_state_hash,
			acknowledged: false,
			retain_until,
		}
		.canonical_bytes();
		let record = RecoveryRecord {
			effect: RecoveryEffect::Progress,
			request: hex::encode(request_bytes),
			authority: hex::encode(token_bytes),
			authority_public_key: hex::encode(signer.public_key()),
			successor_public_key: hex::encode(signer.public_key()),
			provider: hex::encode(local_provider),
			operation_id: hex::encode(request.operation_id),
			request_id: hex::encode(request.request_id),
			generation: token.generation,
			fingerprint: hex::encode(fingerprint),
			nonce: hex::encode(token.nonce),
			prior_cursor: token.cursor,
			cursor,
			received_bytes: received,
			response_sequence: sequence,
			response: hex::encode(&response),
			successor_token: Some(hex::encode(&successor_bytes)),
			response_hash: hex::encode(response_hash),
			host_key_id: hex::encode(token.host_key_id),
			acknowledged: false,
			retain_until,
			descriptor: descriptor.clone(),
			effect_hash: hex::encode(effect_state_hash),
			chunk_hash: chunk_digest,
			entry_cbor: hex::encode(entry_cbor),
		};
		let mut next = state.clone();
		let next_operation = next.operations.get_mut(&operation_key).expect("record exists");
		next_operation.received_bytes = received;
		next_operation.next_chunk =
			internal_index.checked_add(1).ok_or(ContentError::ObjectTooLarge)?;
		next_operation
			.chunks
			.push(ChunkRecord { length: chunk.len() as u32, hash: chunk_hash(chunk) });
		next.recovery.insert(key, record);
		self.trip_fault(StreamingFault::BeforeRecoveryCommit)?;
		persist_state(&self.root, &next)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterRecoveryCommit)?;
		Ok(RecoveryResponseV1 { response, successor_token: Some(successor_bytes), response_hash })
	}

	pub(crate) fn cancel_object_put(
		&self,
		request_bytes: &[u8],
		token_bytes: &[u8],
		snapshot: &CapabilityAuthoritySnapshot,
		current_service_key: [u8; 32],
		signer: &dyn RecoverySigner,
	) -> Result<RecoveryResponseV1, RecoveryError> {
		let request = ObjectPutRequestV2::decode(request_bytes)?;
		let token = ResumeTokenV1::decode(token_bytes)?;
		let local_provider = snapshot.local_provider;
		let now = u64::from(snapshot.finalized_number);
		if token.provider != local_provider ||
			token.registry_sha256 != snapshot.registry_sha256 ||
			token.genesis_hash != snapshot.genesis_hash ||
			token.operation_id != request.operation_id ||
			token.bucket_id != request.bucket_id ||
			token.cid != request.cid ||
			token.object_len != request.object_len
		{
			return Err(RecoveryError::ResumeAudienceInvalid)
		}
		let key =
			recovery_key(token.host_key_id, token.operation_id, token.generation, token.nonce);
		let fingerprint = fingerprint(request_bytes, token_bytes);
		{
			let state = self.read_state()?;
			if let Some(record) = state.recovery.get(&key).cloned() {
				if record.effect != RecoveryEffect::Cancelled {
					return Err(RecoveryError::ResumeReplay)
				}
				drop(state);
				self.cleanup_cancelled_staging(&record)?;
				return recover(
					&record,
					RecoveryEffect::Cancelled,
					request_bytes,
					token_bytes,
					fingerprint,
					local_provider,
					now,
				)
			}
		}
		if token.cancelled {
			return Err(RecoveryError::ResumeReplay)
		}
		let (prior_snapshot, accepted) = {
			let state = self.read_state()?;
			let prior = predecessor(&state, token_bytes)?.clone();
			let accepted = accepted_root(&state, &prior.descriptor)?.clone();
			(prior, accepted)
		};
		let recorded_service_key = decode_hex(&prior_snapshot.successor_public_key)?;
		token.verify(recorded_service_key)?;
		if recorded_service_key != current_service_key || signer.public_key() != current_service_key
		{
			return Err(RecoveryError::ResumeRevoked)
		}
		validate_fresh_resume_authority(&request, &token, &accepted, snapshot, now)?;
		let mut state = self.write_state()?;
		if let Some(record) = state.recovery.get(&key).cloned() {
			if record.effect != RecoveryEffect::Cancelled {
				return Err(RecoveryError::ResumeReplay)
			}
			drop(state);
			self.cleanup_cancelled_staging(&record)?;
			return recover(
				&record,
				RecoveryEffect::Cancelled,
				request_bytes,
				token_bytes,
				fingerprint,
				local_provider,
				now,
			)
		}
		if state.recovery.len() >= MAX_STREAMING_OPERATIONS {
			return Err(ContentError::ProviderRecoveryTableFull.into())
		}
		let encoded_token = hex::encode(token_bytes);
		let prior = state
			.recovery
			.values()
			.find(|record| record.successor_token.as_deref() == Some(encoded_token.as_str()))
			.ok_or(RecoveryError::ResumeReplay)?;
		if prior.cursor != token.cursor || prior.provider != hex::encode(local_provider) {
			return Err(RecoveryError::ResumeReplay)
		}
		let operation_key = operation_key(&prior.descriptor);
		let operation =
			state.operations.get(&operation_key).cloned().ok_or(ContentError::NotFound)?;
		let current_chunk: u16 =
			token.cursor.try_into().map_err(|_| RecoveryError::ResumeCursorInvalid)?;
		if operation.phase != Phase::Receiving || operation.next_chunk != current_chunk {
			return Err(RecoveryError::ResumeCursorInvalid)
		}
		let sequence =
			prior.response_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		let response = cancelled_response(request.request_id, sequence);
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		let effect_hash = effect_state_hash(
			RecoveryEffect::Cancelled,
			&prior.descriptor,
			token.cursor,
			operation.received_bytes,
			None,
		);
		let retain_until = token
			.expires_at
			.checked_add(RECOVERY_TTL)
			.ok_or(ContentError::IntegrityFailed)?;
		let entry_cbor = RecoveryEntryV1 {
			operation_id: request.operation_id,
			request_id: request.request_id,
			generation: token.generation,
			fingerprint,
			nonce: token.nonce,
			prior_cursor: token.cursor,
			new_cursor: token.cursor,
			response: response.clone(),
			successor_token: None,
			response_hash,
			effect_state_hash: effect_hash,
			acknowledged: false,
			retain_until,
		}
		.canonical_bytes();
		let record = RecoveryRecord {
			effect: RecoveryEffect::Cancelled,
			request: hex::encode(request_bytes),
			authority: encoded_token,
			authority_public_key: hex::encode(signer.public_key()),
			successor_public_key: hex::encode(signer.public_key()),
			provider: hex::encode(local_provider),
			operation_id: hex::encode(request.operation_id),
			request_id: hex::encode(request.request_id),
			generation: token.generation,
			fingerprint: hex::encode(fingerprint),
			nonce: hex::encode(token.nonce),
			prior_cursor: token.cursor,
			cursor: token.cursor,
			received_bytes: operation.received_bytes,
			response_sequence: sequence,
			response: hex::encode(&response),
			successor_token: None,
			response_hash: hex::encode(response_hash),
			host_key_id: hex::encode(token.host_key_id),
			acknowledged: false,
			retain_until,
			descriptor: prior.descriptor.clone(),
			effect_hash: hex::encode(effect_hash),
			chunk_hash: String::new(),
			entry_cbor: hex::encode(entry_cbor),
		};
		let mut next = state.clone();
		next.operations.get_mut(&operation_key).expect("operation exists").phase = Phase::Cancelled;
		next.recovery.insert(key, record.clone());
		self.trip_fault(StreamingFault::BeforeTerminalCommit)?;
		persist_state(&self.root, &next)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterTerminalCommit)?;
		drop(state);
		self.cleanup_cancelled_staging(&record)?;
		Ok(RecoveryResponseV1 { response, successor_token: None, response_hash })
	}

	pub(crate) fn gc_recovery(&self, finalized: u64, limit: usize) -> Result<usize, RecoveryError> {
		if limit == 0 || limit > MAX_STREAMING_OPERATIONS {
			return Err(ContentError::SchemaInvalid.into())
		}
		let mut state = self.write_state()?;
		let mut keys = Vec::new();
		for (expired_key, _) in state
			.operations
			.iter()
			.filter(|(_, operation)| matches!(operation.phase, Phase::Receiving | Phase::Cancelled))
		{
			let operation_keys = state
				.recovery
				.iter()
				.filter(|(_, record)| operation_key(&record.descriptor) == *expired_key)
				.map(|(key, record)| (key.clone(), record.retain_until))
				.collect::<Vec<_>>();
			if operation_keys.is_empty() ||
				operation_keys.iter().any(|(_, retain_until)| finalized <= *retain_until) ||
				keys.len().saturating_add(operation_keys.len()) > limit
			{
				continue
			}
			keys.extend(operation_keys.into_iter().map(|(key, _)| key));
		}
		if keys.is_empty() {
			return Ok(0)
		}
		let mut next = state.clone();
		for key in &keys {
			next.recovery.remove(key);
		}
		next.capability_replay.retain(|_, replay| {
			next.recovery.contains_key(&replay.recovery_key) || finalized <= replay.retain_until
		});
		let expired = next
			.operations
			.iter()
			.filter(|(_, operation)| matches!(operation.phase, Phase::Receiving | Phase::Cancelled))
			.map(|(key, _)| key.clone())
			.collect::<Vec<_>>();
		let mut removed_operations = Vec::new();
		for expired_key in expired {
			if !next
				.recovery
				.values()
				.any(|record| operation_key(&record.descriptor) == expired_key)
			{
				next.operations.remove(&expired_key);
				removed_operations.push(expired_key);
			}
		}
		self.trip_fault(StreamingFault::BeforeRecoveryGcCommit)?;
		persist_state(&self.root, &next)?;
		*state = next;
		self.trip_fault(StreamingFault::AfterRecoveryGcCommit)?;
		drop(state);
		for operation_key in removed_operations {
			let path = self.root.join(STAGING).join(format!("{operation_key}.part"));
			if path.exists() {
				fs::remove_file(&path).map_err(|error| ContentError::Io(error.to_string()))?;
				sync_dir(path.parent().expect("staging path has parent"))?;
			}
		}
		Ok(keys.len())
	}

	fn cleanup_cancelled_staging(&self, record: &RecoveryRecord) -> Result<(), RecoveryError> {
		let path = self
			.root
			.join(STAGING)
			.join(format!("{}.part", operation_key(&record.descriptor)));
		if path.exists() {
			fs::remove_file(&path).map_err(|error| ContentError::Io(error.to_string()))?;
			sync_dir(path.parent().expect("staging path has parent"))?;
		}
		Ok(())
	}

	pub(crate) fn acknowledge_response(&self, ack_bytes: &[u8]) -> Result<Vec<u8>, RecoveryError> {
		let ack = ResponseAckV1::decode(ack_bytes)?;
		let mut state = self.write_state()?;
		let key = state
			.recovery
			.iter()
			.find(|(_, r)| {
				r.request_id == hex::encode(ack.request_id) &&
					r.operation_id == hex::encode(ack.operation_id) &&
					r.generation == ack.generation
			})
			.map(|(k, _)| k.clone())
			.ok_or(ContentError::NotFound)?;
		let record = state.recovery.get(&key).expect("found");
		if record.response_hash != hex::encode(ack.response_hash) {
			return Err(ContentError::IdempotencyConflict.into())
		}
		let response = ack_response(&ack, true);
		if record.acknowledged {
			return Ok(response)
		}
		let mut next = state.clone();
		let record = next.recovery.get_mut(&key).expect("found");
		record.acknowledged = true;
		let mut entry = RecoveryEntryV1::decode(
			&hex::decode(&record.entry_cbor).map_err(|_| ContentError::IntegrityFailed)?,
		)
		.map_err(|_| ContentError::IntegrityFailed)?;
		entry.acknowledged = true;
		record.entry_cbor = hex::encode(entry.canonical_bytes());
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(response)
	}
}

fn predecessor<'a>(
	state: &'a super::JournalState,
	token_bytes: &[u8],
) -> Result<&'a RecoveryRecord, RecoveryError> {
	let encoded = hex::encode(token_bytes);
	state
		.recovery
		.values()
		.find(|record| record.successor_token.as_deref() == Some(encoded.as_str()))
		.ok_or(RecoveryError::ResumeReplay)
}

fn accepted_root<'a>(
	state: &'a super::JournalState,
	descriptor: &StreamingDescriptor,
) -> Result<&'a RecoveryRecord, RecoveryError> {
	state
		.recovery
		.values()
		.find(|record| {
			record.effect == RecoveryEffect::Accepted && record.descriptor == *descriptor
		})
		.ok_or_else(|| ContentError::IntegrityFailed.into())
}

fn validate_fresh_resume_authority(
	request: &ObjectPutRequestV2,
	token: &ResumeTokenV1,
	accepted: &RecoveryRecord,
	snapshot: &CapabilityAuthoritySnapshot,
	now: u64,
) -> Result<(), RecoveryError> {
	if now >= request.deadline {
		return Err(RecoveryError::ResumeExpired)
	}
	if now < token.issued_at || now >= token.expires_at {
		return Err(RecoveryError::ResumeExpired)
	}
	let authority = hex::decode(&accepted.authority).map_err(|_| ContentError::IntegrityFailed)?;
	let capability =
		ProviderCapabilityV1::decode(&authority).map_err(|_| ContentError::IntegrityFailed)?;
	if accepted.effect != RecoveryEffect::Accepted ||
		token.host_key_id != capability.issuer_key_id ||
		token.expires_at != capability.expires_at ||
		request.grant_id != capability.grant_id
	{
		return Err(RecoveryError::ResumeAudienceInvalid)
	}
	if snapshot.delegation.revoked_at.is_some() ||
		token.host_key_id != snapshot.delegation.issuer_key_id.0 ||
		capability.issuer_key_id != snapshot.delegation.issuer_key_id.0
	{
		return Err(RecoveryError::ResumeRevoked)
	}
	let capability_request = CapabilityRequest {
		product_id: &request.product_id,
		bucket_id: request.bucket_id,
		agreement_id: capability.agreement_id,
		method: METHOD,
		cid: Some(&request.cid),
		bytes: request.object_len,
		requires_agreement: capability.agreement_id.is_some(),
	};
	match verify_capability(&capability, snapshot, capability_request, &Fresh) {
		Err(CapabilityError::CapabilityIssuerRevoked) => return Err(RecoveryError::ResumeRevoked),
		result => {
			result?;
		},
	}
	Ok(())
}

fn recover(
	record: &RecoveryRecord,
	expected_effect: RecoveryEffect,
	request: &[u8],
	authority: &[u8],
	fingerprint: [u8; 32],
	provider: [u8; 32],
	now: u64,
) -> Result<RecoveryResponseV1, RecoveryError> {
	if record.effect != expected_effect ||
		record.request != hex::encode(request) ||
		record.authority != hex::encode(authority) ||
		record.fingerprint != hex::encode(fingerprint) ||
		record.provider != hex::encode(provider)
	{
		return if ProviderCapabilityV1::decode(authority).is_ok() {
			Err(CapabilityError::CapabilityNonceReplay.into())
		} else {
			Err(RecoveryError::ResumeReplay)
		}
	}
	if now > record.retain_until {
		return Err(RecoveryError::ResumeExpired)
	}
	let key: [u8; 32] = hex::decode(&record.authority_public_key)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	if let Ok(capability) = ProviderCapabilityV1::decode(authority) {
		capability.verify_signature(key)?;
	} else {
		ResumeTokenV1::decode(authority)?.verify(key)?;
	}
	let response = hex::decode(&record.response).map_err(|_| ContentError::IntegrityFailed)?;
	let successor_token = record
		.successor_token
		.as_ref()
		.map(|encoded| hex::decode(encoded).map_err(|_| ContentError::IntegrityFailed))
		.transpose()?;
	let response_hash = hex::decode(&record.response_hash)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)?;
	Ok(RecoveryResponseV1 { response, successor_token, response_hash })
}

fn accepted_response(request: [u8; 16]) -> Vec<u8> {
	map(vec![
		(0, uint(2)),
		(1, bstr(&request)),
		(2, uint(0)),
		(3, uint(0)),
		(4, Value::Map(vec![(uint(0), uint(1))])),
	])
}
fn ack_response(ack: &ResponseAckV1, accepted: bool) -> Vec<u8> {
	map(vec![
		(0, bstr(&ack.request_id)),
		(1, bstr(&ack.operation_id)),
		(2, uint(ack.generation)),
		(3, Value::Bool(accepted)),
	])
}

fn progress_response(request: [u8; 16], sequence: u32, bytes: u64, cursor: u32) -> Vec<u8> {
	map(vec![
		(0, uint(2)),
		(1, bstr(&request)),
		(2, uint(u64::from(sequence))),
		(3, uint(1)),
		(4, Value::Map(vec![(uint(0), uint(bytes)), (uint(1), uint(u64::from(cursor)))])),
	])
}
fn cancelled_response(request: [u8; 16], sequence: u32) -> Vec<u8> {
	map(vec![
		(0, uint(2)),
		(1, bstr(&request)),
		(2, uint(u64::from(sequence))),
		(3, uint(4)),
		(4, Value::Map(vec![(uint(0), uint(107))])),
	])
}
fn recovery_key(host: [u8; 32], operation: [u8; 16], generation: u64, nonce: [u8; 16]) -> String {
	let mut b = Vec::new();
	b.extend(host);
	b.extend(operation);
	b.extend(generation.to_be_bytes());
	b.extend(nonce);
	hex::encode(Sha256::digest(b))
}
fn capability_replay_key(grant_id: [u8; 32], nonce: [u8; 16]) -> String {
	let mut hash = Sha256::new();
	hash.update(b"cord.provider.capability-replay.v1");
	hash.update(grant_id);
	hash.update(nonce);
	hex::encode(hash.finalize())
}
fn fingerprint(request: &[u8], authority: &[u8]) -> [u8; 32] {
	let mut h = Sha256::new();
	h.update(request);
	h.update(authority);
	h.finalize().into()
}
fn effect_state_hash(
	effect: RecoveryEffect,
	descriptor: &StreamingDescriptor,
	cursor: u32,
	received: u64,
	chunk_hash: Option<&str>,
) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(b"cord.provider.recovery.effect.v1");
	hash.update([match effect {
		RecoveryEffect::Accepted => 0,
		RecoveryEffect::Progress => 1,
		RecoveryEffect::Cancelled => 2,
	}]);
	hash.update(serde_json::to_vec(descriptor).expect("descriptor serializes"));
	hash.update(cursor.to_be_bytes());
	hash.update(received.to_be_bytes());
	if let Some(chunk_hash) = chunk_hash {
		hash.update(chunk_hash.as_bytes());
	}
	hash.finalize().into()
}
fn map(entries: Vec<(u8, Value)>) -> Vec<u8> {
	let entries = entries.into_iter().map(|(k, v)| (uint(u64::from(k)), v)).collect();
	let mut out = Vec::new();
	ciborium::ser::into_writer(&Value::Map(entries), &mut out).expect("serializable");
	out
}
fn uint(v: u64) -> Value {
	Value::Integer(v.into())
}
fn bstr(v: &[u8]) -> Value {
	Value::Bytes(v.to_vec())
}
fn take<const N: usize>(f: &mut [Option<Value>; N], i: usize) -> Result<Value, RecoveryError> {
	f[i].take().ok_or(RecoveryError::WireSchemaInvalid)
}
fn integer(v: Value) -> Result<u64, RecoveryError> {
	let Value::Integer(v) = v else { return Err(RecoveryError::WireSchemaInvalid) };
	v.try_into().map_err(|_| RecoveryError::WireSchemaInvalid)
}
fn bytes_fixed<const N: usize>(v: Value) -> Result<[u8; N], RecoveryError> {
	let Value::Bytes(v) = v else { return Err(RecoveryError::WireSchemaInvalid) };
	v.try_into().map_err(|_| RecoveryError::WireSchemaInvalid)
}
fn bounded_bytes(v: Value, min: usize, max: usize) -> Result<Vec<u8>, RecoveryError> {
	let Value::Bytes(v) = v else { return Err(RecoveryError::WireSchemaInvalid) };
	if v.len() < min || v.len() > max {
		return Err(RecoveryError::WireSchemaInvalid)
	}
	Ok(v)
}
fn text(v: Value, max: usize) -> Result<String, RecoveryError> {
	let Value::Text(v) = v else { return Err(RecoveryError::WireSchemaInvalid) };
	if v.is_empty() || v.len() > max || !v.nfc().eq(v.chars()) {
		return Err(RecoveryError::WireSchemaInvalid)
	}
	Ok(v)
}

fn boolean(v: Value) -> Result<bool, RecoveryError> {
	let Value::Bool(v) = v else { return Err(RecoveryError::WireSchemaInvalid) };
	Ok(v)
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], ContentError> {
	if value.len() != N * 2 || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed)
	}
	hex::decode(value)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::CHUNK_BYTES;
	use orbis_storage_runtime_api::{
		AgreementInfo, AgreementStatus, BucketGrantInfo, BucketRole, ControlBucketInfo,
		HostDelegationInfo,
	};
	use sp_core::{crypto::AccountId32, H256};
	use std::fs;
	use tempfile::TempDir;

	fn fixture(
	) -> (ObjectPutRequestV2, ProviderCapabilityV1, CapabilityAuthoritySnapshot, ed25519::Pair) {
		let host = ed25519::Pair::from_seed(&[9; 32]);
		let service = ed25519::Pair::from_seed(&[7; 32]);
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(b"first"));
		let request = ObjectPutRequestV2 {
			request_id: [1; 16],
			product_id: "festival".into(),
			grant_id: [3; 32],
			operation_id: [4; 16],
			trace_context: None,
			deadline: 120,
			bucket_id: [5; 32],
			cid: cid.clone(),
			object_len: 5,
			mode: 0,
		};
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			genesis_hash: [2; 32],
			grant_id: [3; 32],
			issuer_key_id: [6; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([8; 32]),
			provider: [7; 32],
			methods: vec![METHOD],
			cid: Some(cid.clone()),
			max_bytes: 5,
			issued_at: 100,
			expires_at: 128,
			nonce: [9; 16],
			signature: [0; 64],
		};
		capability.signature = sp_core::Pair::sign(&host, &capability.signed_preimage()).0;
		let local = AccountId32::new([7; 32]);
		let snapshot = CapabilityAuthoritySnapshot {
			finalized_hash: format!("0x{}", "0a".repeat(32)),
			finalized_number: 110,
			genesis_hash: [2; 32],
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			local_provider: [7; 32],
			delegation: HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				issuance_nonce: 0,
				issuer_key_id: H256([6; 32]),
				issuer_public_key: host.public().0,
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![METHOD],
				cid: Some(cid.as_str().as_bytes().to_vec()),
				max_bytes: 5,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			},
			bucket: ControlBucketInfo {
				bucket_id: H256([5; 32]),
				owner: AccountId32::new([1; 32]),
				version: 1,
				policy: H256([1; 32]),
				primary: local.clone(),
				replicas: vec![],
				grants: vec![BucketGrantInfo {
					account: AccountId32::new([1; 32]),
					role: BucketRole::Admin,
				}],
				created_at: 1,
			},
			agreement: Some(AgreementInfo {
				agreement_id: H256([8; 32]),
				owner: AccountId32::new([1; 32]),
				bucket_id: H256([5; 32]),
				primary: local,
				replicas: vec![],
				bytes: 5,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}),
		};
		(request, capability, snapshot, service)
	}
	#[test]
	fn exact_request_ack_and_resume_vectors_decode_and_verify() {
		let request=hex::decode("a800020150555555555555555555555555555555550268666573746976616c031903f204582033333333333333333333333333333333333333333333333333333333333333330550444444444444444444444444444444440718e408a5005820222222222222222222222222222222222222222222222222222222222222222201783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c747072663772327102000300045044444444444444444444444444444444").unwrap();
		assert_eq!(ObjectPutRequestV2::decode(&request).unwrap().canonical_bytes(), request);
		let ack=hex::decode("a4005055555555555555555555555555555555015044444444444444444444444444444444020003582079048f62962cde1844e8cae186b36842d26314ddf1473b03995cc9ff99dd23f5").unwrap();
		assert_eq!(ResponseAckV1::decode(&ack).unwrap().canonical_bytes(), ack);
		let token=hex::decode("b000010158201111111111111111111111111111111111111111111111111111111111111111025820222222222222222222222222222222222222222222222222222222222222222203582011111111111111111111111111111111111111111111111111111111111111110458202222222222222222222222222222222222222222222222222222222222222222055044444444444444444444444444444444065820222222222222222222222222222222222222222222222222222222222222222207783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c7470726637723271081903e809030a010b18640c18e40d50666666666666666666666666666666660ef40f58407613a0769c3d7b3357bc847feaeb98b0dad6019f09f678a6a570523e14023df63fcaa59fd4529b45aba263a6e1911e6ee34ba5a6609a0d4c44757b14e5dd1b07").unwrap();
		let decoded = ResumeTokenV1::decode(&token).unwrap();
		assert_eq!(decoded.canonical_bytes(), token);
		decoded
			.verify(
				hex::decode("ee31f83c88a71219a6fcf9bee0da9bc22620588f5a15a6145553504df9649e5c")
					.unwrap()
					.try_into()
					.unwrap(),
			)
			.unwrap();
		assert_eq!(
			hex::encode(Sha256::digest(token)),
			"287e3850d82437fd58a80f2bc8767437767519261f81e2ea36275b3cde6ee002"
		);
		let vectors: serde_json::Value = serde_json::from_str(include_str!(
			"../../../../../../docs/specs/protocol-executable-v2.vectors.json"
		))
		.unwrap();
		let entry = vectors["vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|item| item["type"] == "RecoveryEntryV1")
			.unwrap();
		let bytes = hex::decode(entry["canonical_cbor_hex"].as_str().unwrap()).unwrap();
		assert_eq!(RecoveryEntryV1::decode(&bytes).unwrap().canonical_bytes(), bytes);
		assert_eq!(hex::encode(Sha256::digest(bytes)), entry["canonical_sha256"]);
	}

	#[test]
	fn accepted_progress_ack_restart_and_tamper_are_atomic() {
		let temp = TempDir::new().unwrap();
		let (request, capability, snapshot, service) = fixture();
		let request_bytes = request.canonical_bytes();
		let mut noncanonical = request_bytes.clone();
		noncanonical.splice(0..1, [0xb8, 0x08]);
		assert_eq!(ObjectPutRequestV2::decode(&noncanonical), Err(RecoveryError::WireNonCanonical));
		let authority = capability.canonical_bytes();
		let store = StreamingStore::open(temp.path()).unwrap();
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let mut changed = request.clone();
		changed.request_id = [2; 16];
		assert!(matches!(
			store.accept_object_put(
				&changed.canonical_bytes(),
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[10; 16]
			),
			Err(RecoveryError::Capability(CapabilityError::CapabilityNonceReplay))
		));
		assert_eq!(
			store
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[99; 16]
				)
				.unwrap(),
			accepted
		);
		let progress = store
			.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&snapshot,
				service.public().0,
				1,
				b"first",
				&service,
				[11; 16],
			)
			.unwrap();
		assert_eq!(
			hex::encode(&progress.response),
			"a500020150010101010101010101010101010101010201030104a200050101"
		);
		assert_eq!(
			hex::encode(progress.response_hash),
			"fe9af27dc0ab98ded95f65da05dca1f6092d13a7f6a336969519b21804da63b9"
		);
		assert_eq!(
			store
				.advance_object_put(
					&request_bytes,
					accepted.successor_token.as_deref().unwrap(),
					&snapshot,
					service.public().0,
					1,
					b"first",
					&service,
					[12; 16],
				)
				.unwrap(),
			progress
		);
		assert_eq!(
			store.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&snapshot,
				service.public().0,
				1,
				b"other",
				&service,
				[12; 16],
			),
			Err(RecoveryError::ResumeReplay)
		);
		let ack = ResponseAckV1 {
			request_id: request.request_id,
			operation_id: request.operation_id,
			generation: 0,
			response_hash: accepted.response_hash,
		}
		.canonical_bytes();
		let first_ack = store.acknowledge_response(&ack).unwrap();
		assert_eq!(store.acknowledge_response(&ack).unwrap(), first_ack);
		drop(store);
		let reopened = StreamingStore::open(temp.path()).unwrap();
		assert_eq!(reopened.acknowledge_response(&ack).unwrap(), first_ack);
		assert_eq!(
			reopened
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[13; 16]
				)
				.unwrap(),
			accepted
		);
		drop(reopened);
		let journal = temp.path().join("streaming-v1/journal.json");
		let mut value: serde_json::Value =
			serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
		let recovery = value["recovery"].as_object_mut().unwrap();
		let record = recovery.values_mut().next().unwrap();
		record["response_hash"] = serde_json::Value::String("aa".repeat(32));
		fs::write(&journal, serde_json::to_vec(&value).unwrap()).unwrap();
		assert!(StreamingStore::open(temp.path()).is_err());
	}

	#[test]
	fn fresh_authority_expiry_and_capacity_fail_closed_but_exact_recovery_survives() {
		let temp = TempDir::new().unwrap();
		let (request, capability, snapshot, service) = fixture();
		let request_bytes = request.canonical_bytes();
		let authority = capability.canonical_bytes();
		let store = StreamingStore::open(temp.path()).unwrap();
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let mut revoked = snapshot.clone();
		revoked.finalized_number = 130;
		revoked.delegation.revoked_at = Some(120);
		assert_eq!(
			store
				.accept_object_put(
					&request_bytes,
					&authority,
					&revoked,
					service.public().0,
					&service,
					[99; 16]
				)
				.unwrap(),
			accepted
		);
		let mut next = request.clone();
		next.operation_id = [12; 16];
		assert!(store
			.accept_object_put(
				&next.canonical_bytes(),
				&authority,
				&revoked,
				service.public().0,
				&service,
				[12; 16]
			)
			.is_err());
		let mut expired = snapshot.clone();
		expired.finalized_number = 128;
		assert_eq!(
			store.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&expired,
				service.public().0,
				1,
				b"first",
				&service,
				[11; 16],
			),
			Err(RecoveryError::ResumeExpired)
		);
		let mut rotated = snapshot.clone();
		rotated.delegation.issuer_key_id = H256([44; 32]);
		assert_eq!(
			store.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&rotated,
				service.public().0,
				1,
				b"first",
				&service,
				[11; 16],
			),
			Err(RecoveryError::ResumeRevoked)
		);
		assert_eq!(
			store.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&snapshot,
				[99; 32],
				1,
				b"first",
				&service,
				[11; 16],
			),
			Err(RecoveryError::ResumeRevoked)
		);
		let existing = store.state.read().unwrap().recovery.values().next().unwrap().clone();
		let mut state = store.state.write().unwrap();
		let mut index = 0u64;
		while state.recovery.len() < MAX_STREAMING_OPERATIONS {
			state
				.recovery
				.entry(format!("{index:064x}"))
				.or_insert_with(|| existing.clone());
			index += 1;
		}
		drop(state);
		assert_eq!(
			store
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[13; 16]
				)
				.unwrap(),
			accepted
		);
		assert_eq!(
			store.accept_object_put(
				&next.canonical_bytes(),
				&{
					let host = ed25519::Pair::from_seed(&[9; 32]);
					let mut next_capability = capability.clone();
					next_capability.nonce = [10; 16];
					next_capability.signature =
						sp_core::Pair::sign(&host, &next_capability.signed_preimage()).0;
					next_capability.canonical_bytes()
				},
				&snapshot,
				service.public().0,
				&service,
				[13; 16]
			),
			Err(RecoveryError::Content(ContentError::ProviderRecoveryTableFull))
		);
	}

	#[test]
	fn combined_commit_crash_points_recover_old_or_new_for_accept_and_progress() {
		for fault in [StreamingFault::BeforeRecoveryCommit, StreamingFault::AfterRecoveryCommit] {
			let temp = TempDir::new().unwrap();
			let (request, capability, snapshot, service) = fixture();
			let request_bytes = request.canonical_bytes();
			let authority = capability.canonical_bytes();
			let store = StreamingStore::open(temp.path()).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(store
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[10; 16]
				)
				.is_err());
			drop(store);
			let reopened = StreamingStore::open(temp.path()).unwrap();
			let accepted = reopened
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[10; 16],
				)
				.unwrap();
			reopened.inject_fault_once(fault).unwrap();
			assert!(reopened
				.advance_object_put(
					&request_bytes,
					accepted.successor_token.as_deref().unwrap(),
					&snapshot,
					service.public().0,
					1,
					b"first",
					&service,
					[11; 16],
				)
				.is_err());
			drop(reopened);
			let recovered = StreamingStore::open(temp.path()).unwrap();
			assert!(recovered
				.advance_object_put(
					&request_bytes,
					accepted.successor_token.as_deref().unwrap(),
					&snapshot,
					service.public().0,
					1,
					b"first",
					&service,
					[11; 16],
				)
				.is_ok());
		}
	}

	#[test]
	fn terminal_cancel_is_exact_ackable_restart_safe_and_gc_bounded() {
		let temp = TempDir::new().unwrap();
		let (request, capability, snapshot, service) = fixture();
		let request_bytes = request.canonical_bytes();
		let authority = capability.canonical_bytes();
		let store = StreamingStore::open(temp.path()).unwrap();
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let token = accepted.successor_token.as_deref().unwrap();
		let mut cancelled_token = ResumeTokenV1::decode(token).unwrap();
		cancelled_token.cancelled = true;
		cancelled_token.signature = [0; 64];
		let cancelled_token = cancelled_token.signed(&service).canonical_bytes();
		assert_eq!(
			store.cancel_object_put(
				&request_bytes,
				&cancelled_token,
				&snapshot,
				service.public().0,
				&service,
			),
			Err(RecoveryError::ResumeReplay)
		);
		let terminal = store
			.cancel_object_put(&request_bytes, token, &snapshot, service.public().0, &service)
			.unwrap();
		assert!(terminal.successor_token.is_none());
		assert_eq!(
			hex::encode(&terminal.response),
			"a500020150010101010101010101010101010101010201030404a100186b"
		);
		let expected_hash: [u8; 32] = Sha256::digest(&terminal.response).into();
		assert_eq!(terminal.response_hash, expected_hash);
		let staging = store.root.join(STAGING).join(format!(
			"{}.part",
			operation_key(&StreamingDescriptor {
				operation_id: OperationId::from_bytes(request.operation_id),
				bucket_id: BucketId::from_bytes(request.bucket_id),
				expected_cid: request.cid.as_str().into(),
				object_len: request.object_len,
			})
		));
		assert!(!staging.exists());
		assert_eq!(
			store.advance_object_put(
				&request_bytes,
				token,
				&snapshot,
				service.public().0,
				1,
				b"first",
				&service,
				[11; 16],
			),
			Err(RecoveryError::ResumeReplay)
		);
		let mut changed = request.clone();
		changed.request_id = [22; 16];
		assert_eq!(
			store.cancel_object_put(
				&changed.canonical_bytes(),
				token,
				&snapshot,
				service.public().0,
				&service,
			),
			Err(RecoveryError::ResumeReplay)
		);
		let ack = ResponseAckV1 {
			request_id: request.request_id,
			operation_id: request.operation_id,
			generation: 1,
			response_hash: terminal.response_hash,
		}
		.canonical_bytes();
		let acked = store.acknowledge_response(&ack).unwrap();
		assert_eq!(store.acknowledge_response(&ack).unwrap(), acked);
		let original_keys = store
			.state
			.read()
			.unwrap()
			.recovery
			.keys()
			.cloned()
			.collect::<std::collections::BTreeSet<_>>();
		{
			let mut state = store.state.write().unwrap();
			let existing = state.recovery.values().next().unwrap().clone();
			let mut index = 0u64;
			while state.recovery.len() < MAX_STREAMING_OPERATIONS {
				state
					.recovery
					.entry(format!("{index:064x}"))
					.or_insert_with(|| existing.clone());
				index += 1;
			}
		}
		assert_eq!(
			store
				.cancel_object_put(&request_bytes, token, &snapshot, service.public().0, &service,)
				.unwrap(),
			terminal
		);
		store
			.state
			.write()
			.unwrap()
			.recovery
			.retain(|key, _| original_keys.contains(key));
		drop(store);
		let reopened = StreamingStore::open(temp.path()).unwrap();
		let mut rotated = snapshot.clone();
		rotated.finalized_number = 200;
		rotated.delegation.issuer_key_id = H256([44; 32]);
		assert_eq!(
			reopened
				.cancel_object_put(&request_bytes, token, &rotated, [99; 32], &service)
				.unwrap(),
			terminal
		);
		assert_eq!(reopened.acknowledge_response(&ack).unwrap(), acked);
		assert_eq!(reopened.gc_recovery(384, MAX_STREAMING_OPERATIONS).unwrap(), 0);
		assert_eq!(reopened.gc_recovery(385, MAX_STREAMING_OPERATIONS).unwrap(), 2);
		assert!(reopened.state.read().unwrap().recovery.is_empty());
		assert!(reopened.state.read().unwrap().capability_replay.is_empty());
		drop(reopened);
		StreamingStore::open(temp.path()).unwrap();
	}

	#[test]
	fn terminal_and_gc_crash_seams_recover_old_or_new_atomically() {
		for fault in [StreamingFault::BeforeTerminalCommit, StreamingFault::AfterTerminalCommit] {
			let temp = TempDir::new().unwrap();
			let (request, capability, snapshot, service) = fixture();
			let request_bytes = request.canonical_bytes();
			let store = StreamingStore::open(temp.path()).unwrap();
			let accepted = store
				.accept_object_put(
					&request_bytes,
					&capability.canonical_bytes(),
					&snapshot,
					service.public().0,
					&service,
					[10; 16],
				)
				.unwrap();
			let token = accepted.successor_token.unwrap();
			store.inject_fault_once(fault).unwrap();
			assert!(store
				.cancel_object_put(&request_bytes, &token, &snapshot, service.public().0, &service,)
				.is_err());
			drop(store);
			let reopened = StreamingStore::open(temp.path()).unwrap();
			reopened
				.cancel_object_put(&request_bytes, &token, &snapshot, service.public().0, &service)
				.unwrap();
			drop(reopened);
			let reopened = StreamingStore::open(temp.path()).unwrap();
			let gc_fault = if fault == StreamingFault::BeforeTerminalCommit {
				StreamingFault::BeforeRecoveryGcCommit
			} else {
				StreamingFault::AfterRecoveryGcCommit
			};
			reopened.inject_fault_once(gc_fault).unwrap();
			assert!(reopened.gc_recovery(385, MAX_STREAMING_OPERATIONS).is_err());
			drop(reopened);
			let recovered = StreamingStore::open(temp.path()).unwrap();
			let removed = recovered.gc_recovery(385, MAX_STREAMING_OPERATIONS).unwrap();
			assert_eq!(
				removed,
				usize::from(gc_fault == StreamingFault::BeforeRecoveryGcCommit) * 2
			);
		}
	}

	#[test]
	fn request_bounds_trace_replay_deadline_service_key_and_operation_limit_fail_closed() {
		let (mut request, _, _, _) = fixture();
		request.trace_context = Some(vec![7; 64]);
		let request_bytes = request.canonical_bytes();
		assert_eq!(ObjectPutRequestV2::decode(&request_bytes).unwrap(), request);
		assert_eq!(
			ObjectPutRequestV2::decode(&vec![0; MAX_REQUEST_BYTES + 1]),
			Err(RecoveryError::WireSchemaInvalid)
		);
		assert_eq!(
			ResumeTokenV1::decode(&vec![0; MAX_RESUME_TOKEN_BYTES + 1]),
			Err(RecoveryError::WireSchemaInvalid)
		);
		assert_eq!(
			ResponseAckV1::decode(&vec![0; MAX_ACK_BYTES + 1]),
			Err(RecoveryError::WireSchemaInvalid)
		);
		request.product_id = "festiv\u{0065}\u{0301}".into();
		assert_eq!(
			ObjectPutRequestV2::decode(&request.canonical_bytes()),
			Err(RecoveryError::WireSchemaInvalid)
		);

		let temp = TempDir::new().unwrap();
		let (request, capability, snapshot, service) = fixture();
		let store = StreamingStore::open_with_operation_limit(temp.path(), 1).unwrap();
		let request_bytes = request.canonical_bytes();
		let authority = capability.canonical_bytes();
		assert_eq!(
			store.accept_object_put(
				&request_bytes,
				&authority,
				&snapshot,
				[99; 32],
				&service,
				[10; 16],
			),
			Err(RecoveryError::ResumeRevoked)
		);
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let mut changed_request = request.clone();
		changed_request.operation_id = [33; 16];
		assert_eq!(
			store.accept_object_put(
				&changed_request.canonical_bytes(),
				&authority,
				&snapshot,
				service.public().0,
				&service,
				[11; 16],
			),
			Err(RecoveryError::Capability(CapabilityError::CapabilityNonceReplay))
		);
		let host = ed25519::Pair::from_seed(&[9; 32]);
		let mut next_capability = capability.clone();
		next_capability.nonce = [23; 16];
		next_capability.signature =
			sp_core::Pair::sign(&host, &next_capability.signed_preimage()).0;
		assert_eq!(
			store.accept_object_put(
				&changed_request.canonical_bytes(),
				&next_capability.canonical_bytes(),
				&snapshot,
				service.public().0,
				&service,
				[11; 16],
			),
			Err(RecoveryError::Content(ContentError::ProviderRecoveryTableFull))
		);
		let mut past_deadline = changed_request;
		past_deadline.operation_id = [34; 16];
		past_deadline.deadline = u64::from(snapshot.finalized_number);
		next_capability.nonce = [24; 16];
		next_capability.signature =
			sp_core::Pair::sign(&host, &next_capability.signed_preimage()).0;
		assert!(matches!(
			store.accept_object_put(
				&past_deadline.canonical_bytes(),
				&next_capability.canonical_bytes(),
				&snapshot,
				service.public().0,
				&service,
				[12; 16],
			),
			Err(RecoveryError::Capability(CapabilityError::CapabilityExpired))
		));
		assert_eq!(
			store
				.accept_object_put(
					&request_bytes,
					&authority,
					&snapshot,
					service.public().0,
					&service,
					[99; 16],
				)
				.unwrap(),
			accepted
		);
	}

	#[test]
	fn gc_retires_expired_receiving_chain_and_staged_bytes() {
		let temp = TempDir::new().unwrap();
		let (request, capability, snapshot, service) = fixture();
		let store = StreamingStore::open(temp.path()).unwrap();
		store
			.accept_object_put(
				&request.canonical_bytes(),
				&capability.canonical_bytes(),
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		assert_eq!(store.gc_recovery(384, MAX_STREAMING_OPERATIONS).unwrap(), 0);
		assert_eq!(store.gc_recovery(385, MAX_STREAMING_OPERATIONS).unwrap(), 1);
		let state = store.state.read().unwrap();
		assert!(state.operations.is_empty());
		assert!(state.recovery.is_empty());
		assert!(state.capability_replay.is_empty());
		drop(state);
		assert!(fs::read_dir(store.root.join(STAGING)).unwrap().next().is_none());
		drop(store);
		StreamingStore::open(temp.path()).unwrap();
	}

	#[test]
	fn reopen_rejects_recomputed_middle_progress_not_bound_to_operation_chunks() {
		let temp = TempDir::new().unwrap();
		let (mut request, mut capability, mut snapshot, service) = fixture();
		request.object_len = CHUNK_BYTES as u64 + 1;
		capability.max_bytes = request.object_len;
		snapshot.delegation.max_bytes = request.object_len;
		snapshot.agreement.as_mut().unwrap().bytes = request.object_len;
		let host = ed25519::Pair::from_seed(&[9; 32]);
		capability.signature = sp_core::Pair::sign(&host, &capability.signed_preimage()).0;
		let request_bytes = request.canonical_bytes();
		let store = StreamingStore::open(temp.path()).unwrap();
		let accepted = store
			.accept_object_put(
				&request_bytes,
				&capability.canonical_bytes(),
				&snapshot,
				service.public().0,
				&service,
				[10; 16],
			)
			.unwrap();
		let first_chunk = vec![7; CHUNK_BYTES];
		let first = store
			.advance_object_put(
				&request_bytes,
				accepted.successor_token.as_deref().unwrap(),
				&snapshot,
				service.public().0,
				1,
				&first_chunk,
				&service,
				[11; 16],
			)
			.unwrap();
		store
			.advance_object_put(
				&request_bytes,
				first.successor_token.as_deref().unwrap(),
				&snapshot,
				service.public().0,
				2,
				&[8],
				&service,
				[12; 16],
			)
			.unwrap();
		let mut state = store.state.write().unwrap();
		let record = state
			.recovery
			.values_mut()
			.find(|record| record.effect == RecoveryEffect::Progress && record.generation == 1)
			.unwrap();
		record.chunk_hash = chunk_hash(b"wrong");
		let changed_effect = effect_state_hash(
			RecoveryEffect::Progress,
			&record.descriptor,
			record.cursor,
			record.received_bytes,
			Some(&record.chunk_hash),
		);
		record.effect_hash = hex::encode(changed_effect);
		let mut entry = RecoveryEntryV1::decode(&hex::decode(&record.entry_cbor).unwrap()).unwrap();
		entry.effect_state_hash = changed_effect;
		record.entry_cbor = hex::encode(entry.canonical_bytes());
		persist_state(&store.root, &state).unwrap();
		drop(state);
		drop(store);
		assert!(StreamingStore::open(temp.path()).is_err());
	}
}
