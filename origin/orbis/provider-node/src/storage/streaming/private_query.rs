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

//! Private, unrouted provider byte-plane queries pinned to finalized Commons authority.

use std::{
	array,
	ops::Bound::{Excluded, Unbounded},
};

use ciborium::value::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sp_core::{ed25519, Pair as _};
use unicode_normalization::UnicodeNormalization;

use super::{persist_state, OperationRecord, Phase, StreamingStore};
use crate::{
	capability::{
		verify_capability, verify_capability_identity, CapabilityError, CapabilityReplayInspection,
		CapabilityReplayInspector, CapabilityRequest, ProviderCapabilityV1,
	},
	chain::ReplicationTopologySnapshot,
	storage::bucket_mmr::BucketMmrStore,
	BucketId, CanonicalCid, CapabilityAuthoritySnapshot, ContentError, MAX_RANGE_BYTES,
	MAX_STREAMING_OPERATIONS,
};

const GET: u16 = 1011;
const RANGE: u16 = 1012;
const DELETE: u16 = 1013;
const STATUS: u16 = 1014;
const MAX_REQUEST_BYTES: usize = 4096;
const QUERY_TTL: u64 = 256;
const PRIVATE_QUERY_GC_BATCH: usize = 64;
const QUERY_ID_DOMAIN: &[u8] = b"cord/provider/private-object-query/v1";
const QUERY_RESPONSE_DOMAIN: &[u8] = b"cord/provider/private-object-response/v1";

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PrivateQueryError {
	#[error("{0}")]
	Capability(#[from] CapabilityError),
	#[error("{0}")]
	Content(#[from] ContentError),
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid,
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical,
	#[error("CHAIN_FINALITY_REQUIRED")]
	ChainFinalityRequired,
	#[error("FINALIZED_VIEW_INVALID")]
	FinalizedViewInvalid,
	#[error("VERIFIED_OFFSET_INVALID")]
	VerifiedOffsetInvalid,
}

/// Exact closed RequestV2 shape for private object GET, RANGE, or provider-local STATUS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateObjectRequestV2 {
	pub(crate) request_id: [u8; 16],
	pub(crate) product_id: String,
	pub(crate) method: u16,
	pub(crate) grant_id: [u8; 32],
	pub(crate) trace_context: Option<Vec<u8>>,
	pub(crate) deadline: u64,
	pub(crate) bucket_id: [u8; 32],
	pub(crate) cid: CanonicalCid,
	pub(crate) range: Option<(u64, u64)>,
}

impl PrivateObjectRequestV2 {
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, PrivateQueryError> {
		if bytes.len() > MAX_REQUEST_BYTES {
			return Err(PrivateQueryError::WireSchemaInvalid)
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| PrivateQueryError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(PrivateQueryError::WireSchemaInvalid) };
		let mut fields: [Option<Value>; 9] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key = integer(key)? as usize;
			if key > 8 || fields[key].replace(value).is_some() {
				return Err(PrivateQueryError::WireSchemaInvalid)
			}
		}
		let method = fields[3]
			.clone()
			.map(integer)
			.transpose()?
			.ok_or(PrivateQueryError::WireSchemaInvalid)?;
		if method == u64::from(DELETE) {
			// Deletion is a Commons runtime tombstone/finality transition. Provider workers consume
			// finalized deletion duties; the private byte dispatcher never creates delete state.
			return Err(PrivateQueryError::ChainFinalityRequired)
		}
		let method: u16 = method.try_into().map_err(|_| PrivateQueryError::WireSchemaInvalid)?;
		if !matches!(method, GET | RANGE | STATUS) || fields[5].is_some() {
			return Err(PrivateQueryError::WireSchemaInvalid)
		}
		for key in [0usize, 1, 2, 3, 4, 7, 8] {
			if fields[key].is_none() {
				return Err(PrivateQueryError::WireSchemaInvalid)
			}
		}
		if integer(take(&mut fields, 0)?)? != 2 {
			return Err(PrivateQueryError::WireSchemaInvalid)
		}
		let request_id = bytes_fixed(take(&mut fields, 1)?)?;
		let product_id = text(take(&mut fields, 2)?, 128)?;
		let _ = take(&mut fields, 3)?;
		let grant_id = bytes_fixed(take(&mut fields, 4)?)?;
		let trace_context =
			fields[6].take().map(|value| bounded_bytes(value, 1, 64)).transpose()?;
		let deadline = integer(take(&mut fields, 7)?)?;
		let Value::Map(body) = take(&mut fields, 8)? else {
			return Err(PrivateQueryError::WireSchemaInvalid)
		};
		let expected = if method == RANGE { 4 } else { 2 };
		let mut body_fields: [Option<Value>; 4] = array::from_fn(|_| None);
		for (key, value) in body {
			let key = integer(key)? as usize;
			if key >= expected || body_fields[key].replace(value).is_some() {
				return Err(PrivateQueryError::WireSchemaInvalid)
			}
		}
		if body_fields[..expected].iter().any(Option::is_none) {
			return Err(PrivateQueryError::WireSchemaInvalid)
		}
		let bucket_id = bytes_fixed(take(&mut body_fields, 0)?)?;
		let cid = CanonicalCid::parse(&text(take(&mut body_fields, 1)?, 128)?)
			.map_err(|_| PrivateQueryError::WireSchemaInvalid)?;
		let range = if method == RANGE {
			Some((integer(take(&mut body_fields, 2)?)?, integer(take(&mut body_fields, 3)?)?))
		} else {
			None
		};
		let request = Self {
			request_id,
			product_id,
			method,
			grant_id,
			trace_context,
			deadline,
			bucket_id,
			cid,
			range,
		};
		if request.canonical_bytes() != bytes {
			return Err(PrivateQueryError::WireNonCanonical)
		}
		Ok(request)
	}

	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		let mut fields = vec![
			(0, uint(2)),
			(1, bstr(&self.request_id)),
			(2, Value::Text(self.product_id.clone())),
			(3, uint(u64::from(self.method))),
			(4, bstr(&self.grant_id)),
		];
		if let Some(trace) = &self.trace_context {
			fields.push((6, bstr(trace)));
		}
		let mut body = vec![
			(uint(0), bstr(&self.bucket_id)),
			(uint(1), Value::Text(self.cid.as_str().into())),
		];
		if let Some((start, length)) = self.range {
			body.push((uint(2), uint(start)));
			body.push((uint(3), uint(length)));
		}
		fields.extend([(7, uint(self.deadline)), (8, Value::Map(body))]);
		map(fields)
	}
}

/// One byte-identical durable response batch. `next_verified_offset` resumes only after emitted
/// bytes have passed full-file and touched-chunk verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateObjectResponseV2 {
	pub(crate) operation_id: [u8; 16],
	pub(crate) frames: Vec<Vec<u8>>,
	pub(crate) next_verified_offset: Option<u64>,
	pub(crate) response_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrivateQueryRecord {
	request: String,
	authority: String,
	provider: String,
	method: u16,
	request_id: String,
	operation_id: String,
	nonce: String,
	fingerprint: String,
	verified_offset: u64,
	next_verified_offset: Option<u64>,
	frames: Vec<String>,
	response_hash: String,
	checkpoint_root: String,
	checkpoint_start: u64,
	checkpoint_leaves: u64,
	checkpoint_block: u32,
	finalized_hash: String,
	finalized_number: u32,
	retain_until: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrivateQueryReplayRecord {
	operation_id: String,
	fingerprint: String,
	retain_until: u64,
}

#[derive(Clone, Copy)]
struct Fresh;
impl CapabilityReplayInspector for Fresh {
	fn inspect(&self, _: &[u8; 32], _: &[u8; 16], _: &[u8; 32]) -> CapabilityReplayInspection {
		CapabilityReplayInspection::Fresh
	}
}

#[derive(Clone, Copy)]
struct FinalizedCheckpoint {
	root: [u8; 32],
	start: u64,
	leaves: u64,
	block: u32,
	finalized_hash: [u8; 32],
	finalized_number: u32,
	replicas: u32,
}

impl StreamingStore {
	/// Execute one private provider GET, RANGE, or local STATUS request. This has no public route.
	pub(crate) fn private_object_query(
		&self,
		request_bytes: &[u8],
		authority_bytes: &[u8],
		authority: &CapabilityAuthoritySnapshot,
		topology: &ReplicationTopologySnapshot,
		mmr: &BucketMmrStore,
		local_service_key: [u8; 32],
		verified_offset: u64,
	) -> Result<PrivateObjectResponseV2, PrivateQueryError> {
		let request = PrivateObjectRequestV2::decode(request_bytes)?;
		if request.method == STATUS && verified_offset != 0 {
			return Err(PrivateQueryError::VerifiedOffsetInvalid)
		}
		let capability = ProviderCapabilityV1::decode(authority_bytes)?;
		let record = self.local_record(request.bucket_id, &request.cid)?;
		let authorized_bytes = match request.method {
			GET => record
				.as_ref()
				.filter(|record| record.phase == Phase::Installed)
				.map(|record| record.descriptor.object_len)
				.ok_or(ContentError::NotFound)?,
			RANGE => {
				let (start, length) = request.range.ok_or(PrivateQueryError::WireSchemaInvalid)?;
				if length == 0 || start.checked_add(length).is_none() {
					return Err(ContentError::RangeInvalid.into())
				}
				length
			},
			STATUS => 0,
			_ => return Err(PrivateQueryError::WireSchemaInvalid),
		};
		let capability_request = CapabilityRequest {
			product_id: &request.product_id,
			bucket_id: request.bucket_id,
			agreement_id: capability.agreement_id,
			method: request.method,
			cid: Some(&request.cid),
			bytes: authorized_bytes,
			requires_agreement: capability.agreement_id.is_some(),
		};
		verify_capability_identity(&capability, authority, &capability_request)?;
		if capability.grant_id != request.grant_id ||
			capability.provider != authority.local_provider ||
			request.deadline > capability.expires_at
		{
			return Err(CapabilityError::GrantScopeDenied.into())
		}
		let operation_id = derive_operation_id(request.method, request.request_id);
		let fingerprint = fingerprint(request_bytes, authority_bytes);
		let query_key = query_key(operation_id, verified_offset);
		let replay_key = replay_key(capability.grant_id, capability.nonce);
		let now = u64::from(authority.finalized_number);
		self.prune_private_queries(now)?;
		{
			let state = self.read_state()?;
			if let Some(stored) = state.private_queries.get(&query_key) {
				return recover(
					stored,
					request_bytes,
					authority_bytes,
					fingerprint,
					capability.provider,
					now,
				)
			}
		}
		let object_len = record.as_ref().map(|record| record.descriptor.object_len).unwrap_or(0);
		let require_membership = matches!(request.method, GET | RANGE) ||
			record.as_ref().is_some_and(|record| record.phase == Phase::Installed);
		let checkpoint = finalized_checkpoint(
			mmr,
			topology,
			authority,
			request.bucket_id,
			&request.cid,
			object_len,
			require_membership,
			local_service_key,
		)?;
		{
			let state = self.read_state()?;
			if let Some(replay) = state.private_query_replay.get(&replay_key) {
				if replay.operation_id != hex::encode(operation_id) ||
					replay.fingerprint != hex::encode(fingerprint) ||
					now > replay.retain_until
				{
					return Err(CapabilityError::CapabilityNonceReplay.into())
				}
			}
			// Durable query replay owns nonce/fingerprint admission. Every new effect, including a
			// continuation, must still pass the complete current finalized authority decision.
			// Only the byte-identical stored query-key branch above bypasses mutable liveness.
			verify_capability(&capability, authority, capability_request, &Fresh)?;
			if !continuation_authorized(&state, operation_id, fingerprint, verified_offset) {
				return Err(PrivateQueryError::VerifiedOffsetInvalid)
			}
		}
		if now >= request.deadline {
			return Err(CapabilityError::CapabilityExpired.into())
		}

		let (frames, next_verified_offset) = match request.method {
			GET => self.get_frames(&request, record.as_ref(), checkpoint, verified_offset)?,
			RANGE => self.range_frames(&request, record.as_ref(), checkpoint, verified_offset)?,
			STATUS => self.status_frames(
				&request,
				record.as_ref(),
				checkpoint,
				authority.local_provider,
				local_service_key,
			)?,
			_ => return Err(PrivateQueryError::WireSchemaInvalid),
		};
		let response_hash = response_hash(&frames, next_verified_offset);
		let retain_until = capability
			.expires_at
			.checked_add(QUERY_TTL)
			.ok_or(ContentError::IntegrityFailed)?;
		let stored = PrivateQueryRecord {
			request: hex::encode(request_bytes),
			authority: hex::encode(authority_bytes),
			provider: hex::encode(capability.provider),
			method: request.method,
			request_id: hex::encode(request.request_id),
			operation_id: hex::encode(operation_id),
			nonce: hex::encode(capability.nonce),
			fingerprint: hex::encode(fingerprint),
			verified_offset,
			next_verified_offset,
			frames: frames.iter().map(hex::encode).collect(),
			response_hash: hex::encode(response_hash),
			checkpoint_root: hex::encode(checkpoint.root),
			checkpoint_start: checkpoint.start,
			checkpoint_leaves: checkpoint.leaves,
			checkpoint_block: checkpoint.block,
			finalized_hash: hex::encode(checkpoint.finalized_hash),
			finalized_number: checkpoint.finalized_number,
			retain_until,
		};
		let mut state = self.write_state()?;
		if let Some(existing) = state.private_queries.get(&query_key) {
			return recover(
				existing,
				request_bytes,
				authority_bytes,
				fingerprint,
				capability.provider,
				now,
			)
		}
		ensure_private_query_capacity(&state, &replay_key)?;
		let mut next = state.clone();
		match next.private_query_replay.get(&replay_key) {
			Some(replay)
				if replay.operation_id == hex::encode(operation_id) &&
					replay.fingerprint == hex::encode(fingerprint) => {},
			Some(_) => return Err(CapabilityError::CapabilityNonceReplay.into()),
			None => {
				next.private_query_replay.insert(
					replay_key,
					PrivateQueryReplayRecord {
						operation_id: hex::encode(operation_id),
						fingerprint: hex::encode(fingerprint),
						retain_until,
					},
				);
			},
		}
		if !continuation_authorized(&next, operation_id, fingerprint, verified_offset) {
			return Err(PrivateQueryError::VerifiedOffsetInvalid)
		}
		next.private_queries.insert(query_key, stored);
		self.trip_fault(super::StreamingFault::BeforePrivateQueryCommit)?;
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(PrivateObjectResponseV2 { operation_id, frames, next_verified_offset, response_hash })
	}

	fn local_record(
		&self,
		bucket_id: [u8; 32],
		cid: &CanonicalCid,
	) -> Result<Option<OperationRecord>, ContentError> {
		let state = self.read_state()?;
		let exact = |record: &&OperationRecord| {
			record.descriptor.bucket_id == BucketId::from_bytes(bucket_id) &&
				record.descriptor.expected_cid == cid.as_str()
		};
		Ok(state
			.operations
			.values()
			.filter(exact)
			.find(|record| record.phase == Phase::Installed)
			.or_else(|| state.operations.values().filter(exact).next())
			.cloned())
	}

	fn prune_private_queries(&self, now: u64) -> Result<(), ContentError> {
		let mut state = self.write_state()?;
		let mut next = state.clone();
		prune_private_query_state(&mut next, now, PRIVATE_QUERY_GC_BATCH)?;
		if next.private_queries != state.private_queries ||
			next.private_query_replay != state.private_query_replay ||
			next.private_query_gc_cursor != state.private_query_gc_cursor
		{
			persist_state(&self.root, &next)?;
			*state = next;
		}
		Ok(())
	}

	fn get_frames(
		&self,
		request: &PrivateObjectRequestV2,
		record: Option<&OperationRecord>,
		checkpoint: FinalizedCheckpoint,
		verified_offset: u64,
	) -> Result<(Vec<Vec<u8>>, Option<u64>), PrivateQueryError> {
		let record = record
			.filter(|record| record.phase == Phase::Installed)
			.ok_or(ContentError::NotFound)?;
		let total = record.descriptor.object_len;
		if verified_offset > total {
			return Err(PrivateQueryError::VerifiedOffsetInvalid)
		}
		self.verify_installed(request.cid.as_str())?;
		let end = total.min(verified_offset.saturating_add(MAX_RANGE_BYTES));
		let bytes = self.read_range_verified(request.cid.as_str(), verified_offset, end)?;
		let mut frames = Vec::new();
		let mut sequence = (verified_offset / MAX_RANGE_BYTES) as u32;
		if verified_offset == 0 {
			frames.push(event(request.request_id, sequence, 0, accepted()));
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		} else {
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		}
		if !bytes.is_empty() {
			frames.push(event(
				request.request_id,
				sequence,
				1,
				Value::Map(vec![(uint(0), uint(verified_offset)), (uint(1), bstr(&bytes))]),
			));
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		}
		if end == total {
			frames.push(event(
				request.request_id,
				sequence,
				2,
				Value::Map(vec![
					(uint(0), Value::Text(request.cid.as_str().into())),
					(uint(1), uint(total)),
					(uint(2), checkpoint_value(checkpoint)),
				]),
			));
			Ok((frames, None))
		} else {
			Ok((frames, Some(end)))
		}
	}

	fn range_frames(
		&self,
		request: &PrivateObjectRequestV2,
		record: Option<&OperationRecord>,
		checkpoint: FinalizedCheckpoint,
		verified_offset: u64,
	) -> Result<(Vec<Vec<u8>>, Option<u64>), PrivateQueryError> {
		let record = record
			.filter(|record| record.phase == Phase::Installed)
			.ok_or(ContentError::NotFound)?;
		let (start, length) = request.range.ok_or(PrivateQueryError::WireSchemaInvalid)?;
		let range_end = start.checked_add(length).ok_or(ContentError::RangeInvalid)?;
		if length == 0 || range_end > record.descriptor.object_len {
			return Err(ContentError::RangeInvalid.into())
		}
		if verified_offset > length {
			return Err(PrivateQueryError::VerifiedOffsetInvalid)
		}
		self.verify_installed(request.cid.as_str())?;
		let absolute = start.checked_add(verified_offset).ok_or(ContentError::RangeInvalid)?;
		let relative_end = length.min(verified_offset.saturating_add(MAX_RANGE_BYTES));
		let absolute_end = start.checked_add(relative_end).ok_or(ContentError::RangeInvalid)?;
		let bytes = self.read_range_verified(request.cid.as_str(), absolute, absolute_end)?;
		let mut frames = Vec::new();
		let mut sequence = (verified_offset / MAX_RANGE_BYTES) as u32;
		if verified_offset == 0 {
			frames.push(event(request.request_id, sequence, 0, accepted()));
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		} else {
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		}
		if !bytes.is_empty() {
			frames.push(event(
				request.request_id,
				sequence,
				1,
				Value::Map(vec![(uint(0), uint(absolute)), (uint(1), bstr(&bytes))]),
			));
			sequence = sequence.checked_add(1).ok_or(PrivateQueryError::WireSchemaInvalid)?;
		}
		if relative_end == length {
			frames.push(event(
				request.request_id,
				sequence,
				2,
				Value::Map(vec![
					(uint(0), Value::Text(request.cid.as_str().into())),
					(uint(1), uint(record.descriptor.object_len)),
					(uint(2), uint(start)),
					(uint(3), uint(range_end)),
					(uint(4), checkpoint_value(checkpoint)),
				]),
			));
			Ok((frames, None))
		} else {
			Ok((frames, Some(relative_end)))
		}
	}

	fn status_frames(
		&self,
		request: &PrivateObjectRequestV2,
		record: Option<&OperationRecord>,
		checkpoint: FinalizedCheckpoint,
		local_provider: [u8; 32],
		local_service_key: [u8; 32],
	) -> Result<(Vec<Vec<u8>>, Option<u64>), PrivateQueryError> {
		let mut receipt = None;
		let status = if self.is_quarantined(request.cid.as_str())? {
			4
		} else {
			match record.map(|record| record.phase) {
				Some(Phase::Installed) => {
					self.verify_installed(request.cid.as_str())?;
					if let Some(encoded) =
						record.and_then(|record| record.provider_receipt.as_ref())
					{
						let bytes =
							hex::decode(encoded).map_err(|_| ContentError::IntegrityFailed)?;
						let decoded = super::recovery::ProviderReceiptV1::decode(&bytes)
							.map_err(|_| ContentError::IntegrityFailed)?;
						verify_provider_receipt(&decoded, local_service_key)?;
						if decoded.provider != local_provider ||
							decoded.cid != request.cid.as_str() ||
							decoded.stored_bytes !=
								record.expect("installed record").descriptor.object_len
						{
							return Err(ContentError::IntegrityFailed.into())
						}
						receipt = Some(value(&bytes)?);
					}
					2
				},
				Some(Phase::Receiving | Phase::Finalizing) => 1,
				Some(Phase::Cancelled) | None => 0,
			}
		};
		let mut result = vec![(uint(0), uint(status))];
		if let Some(receipt) = receipt {
			result.push((uint(1), receipt));
		}
		result.extend([
			(uint(2), checkpoint_value(checkpoint)),
			(uint(3), uint(u64::from(checkpoint.replicas))),
			(uint(4), Value::Bool(false)),
			(uint(5), finality_value(checkpoint)),
		]);
		Ok((
			vec![
				event(request.request_id, 0, 0, accepted()),
				event(request.request_id, 1, 2, Value::Map(result)),
			],
			None,
		))
	}
}

pub(super) fn validate_private_query_state(
	state: &super::JournalState,
) -> Result<(), ContentError> {
	if state.private_query_gc_cursor.as_ref().is_some_and(|cursor| cursor.len() > 64) {
		return Err(ContentError::IntegrityFailed)
	}
	for (key, record) in &state.private_queries {
		let request_bytes =
			hex::decode(&record.request).map_err(|_| ContentError::IntegrityFailed)?;
		let authority =
			hex::decode(&record.authority).map_err(|_| ContentError::IntegrityFailed)?;
		let request = PrivateObjectRequestV2::decode(&request_bytes)
			.map_err(|_| ContentError::IntegrityFailed)?;
		let capability =
			ProviderCapabilityV1::decode(&authority).map_err(|_| ContentError::IntegrityFailed)?;
		let operation = derive_operation_id(request.method, request.request_id);
		let frames = record
			.frames
			.iter()
			.map(|frame| hex::decode(frame).map_err(|_| ContentError::IntegrityFailed))
			.collect::<Result<Vec<_>, _>>()?;
		if *key != query_key(operation, record.verified_offset) ||
			record.method != request.method ||
			record.request_id != hex::encode(request.request_id) ||
			record.operation_id != hex::encode(operation) ||
			record.nonce != hex::encode(capability.nonce) ||
			record.provider != hex::encode(capability.provider) ||
			record.fingerprint != hex::encode(fingerprint(&request_bytes, &authority)) ||
			record.response_hash !=
				hex::encode(response_hash(&frames, record.next_verified_offset)) ||
			record.frames.is_empty()
		{
			return Err(ContentError::IntegrityFailed)
		}
		let replay = state
			.private_query_replay
			.get(&replay_key(capability.grant_id, capability.nonce))
			.ok_or(ContentError::IntegrityFailed)?;
		if replay.operation_id != record.operation_id ||
			replay.fingerprint != record.fingerprint ||
			replay.retain_until != record.retain_until
		{
			return Err(ContentError::IntegrityFailed)
		}
	}
	for (key, replay) in &state.private_query_replay {
		let roots = state.private_queries.values().filter(|record| {
			record.operation_id == replay.operation_id && record.fingerprint == replay.fingerprint
		});
		let mut found = false;
		for record in roots {
			let authority =
				hex::decode(&record.authority).map_err(|_| ContentError::IntegrityFailed)?;
			let capability = ProviderCapabilityV1::decode(&authority)
				.map_err(|_| ContentError::IntegrityFailed)?;
			if *key == replay_key(capability.grant_id, capability.nonce) &&
				replay.retain_until == record.retain_until
			{
				found = true;
				break
			}
		}
		if !found {
			return Err(ContentError::IntegrityFailed)
		}
	}
	Ok(())
}

fn prune_private_query_state(
	state: &mut super::JournalState,
	now: u64,
	batch: usize,
) -> Result<(), ContentError> {
	if batch == 0 {
		return Ok(())
	}
	let mut keys = match &state.private_query_gc_cursor {
		Some(cursor) => state
			.private_queries
			.range((Excluded(cursor.clone()), Unbounded))
			.map(|(key, _)| key.clone())
			.take(batch.saturating_add(1))
			.collect::<Vec<_>>(),
		None => state
			.private_queries
			.keys()
			.take(batch.saturating_add(1))
			.cloned()
			.collect::<Vec<_>>(),
	};
	if keys.is_empty() {
		state.private_query_gc_cursor = None;
		return Ok(())
	}
	let has_more = keys.len() > batch;
	keys.truncate(batch);
	let last = keys.last().cloned();
	let mut replay_candidates = Vec::new();
	for key in &keys {
		let Some(record) = state.private_queries.get(key) else { continue };
		if now <= record.retain_until {
			continue
		}
		let authority =
			hex::decode(&record.authority).map_err(|_| ContentError::IntegrityFailed)?;
		let capability =
			ProviderCapabilityV1::decode(&authority).map_err(|_| ContentError::IntegrityFailed)?;
		replay_candidates.push(replay_key(capability.grant_id, capability.nonce));
		state.private_queries.remove(key);
	}
	replay_candidates.sort();
	replay_candidates.dedup();
	for key in replay_candidates {
		let Some(replay) = state.private_query_replay.get(&key) else {
			return Err(ContentError::IntegrityFailed)
		};
		if now > replay.retain_until &&
			!state.private_queries.values().any(|record| {
				record.operation_id == replay.operation_id &&
					record.fingerprint == replay.fingerprint
			}) {
			state.private_query_replay.remove(&key);
		}
	}
	state.private_query_gc_cursor = if has_more { last } else { None };
	Ok(())
}

fn finalized_checkpoint(
	mmr: &BucketMmrStore,
	topology: &ReplicationTopologySnapshot,
	authority: &CapabilityAuthoritySnapshot,
	bucket: [u8; 32],
	cid: &CanonicalCid,
	object_len: u64,
	require_membership: bool,
	local_service_key: [u8; 32],
) -> Result<FinalizedCheckpoint, PrivateQueryError> {
	topology
		.validate(authority.local_provider, local_service_key)
		.map_err(|_| PrivateQueryError::FinalizedViewInvalid)?;
	let authority_hash = decode_hash_text(&authority.finalized_hash)
		.ok_or(PrivateQueryError::FinalizedViewInvalid)?;
	if topology.genesis_hash != authority.genesis_hash ||
		topology.finalized_hash != authority_hash ||
		topology.finalized_number != authority.finalized_number ||
		topology.bucket_id != bucket
	{
		return Err(PrivateQueryError::FinalizedViewInvalid)
	}
	let current = topology
		.current_checkpoint
		.as_ref()
		.ok_or(PrivateQueryError::FinalizedViewInvalid)?;
	if current.bucket_id.as_bytes() != &bucket ||
		current.checkpoint_block > topology.finalized_number ||
		topology
			.governed_finalized_checkpoint
			.is_none_or(|finalized| current.checkpoint_block > finalized)
	{
		return Err(PrivateQueryError::FinalizedViewInvalid)
	}
	let member = mmr
		.finalized_object_membership(
			BucketId::from_bytes(bucket),
			current.commitment.mmr_root,
			current.commitment.start_seq,
			current.commitment.leaf_count,
			cid,
			object_len,
		)
		.map_err(|_| PrivateQueryError::FinalizedViewInvalid)?;
	if require_membership && !member {
		return Err(PrivateQueryError::FinalizedViewInvalid)
	}
	let replicas: u32 = current
		.replica_confirmations
		.len()
		.try_into()
		.map_err(|_| PrivateQueryError::FinalizedViewInvalid)?;
	Ok(FinalizedCheckpoint {
		root: *current.commitment.mmr_root.as_fixed_bytes(),
		start: current.commitment.start_seq,
		leaves: current.commitment.leaf_count,
		block: current.checkpoint_block,
		finalized_hash: topology.finalized_hash,
		finalized_number: topology.finalized_number,
		replicas,
	})
}

fn recover(
	record: &PrivateQueryRecord,
	request: &[u8],
	authority: &[u8],
	fingerprint: [u8; 32],
	provider: [u8; 32],
	now: u64,
) -> Result<PrivateObjectResponseV2, PrivateQueryError> {
	if record.request != hex::encode(request) ||
		record.authority != hex::encode(authority) ||
		record.fingerprint != hex::encode(fingerprint) ||
		record.provider != hex::encode(provider)
	{
		return Err(CapabilityError::CapabilityNonceReplay.into())
	}
	if now > record.retain_until {
		return Err(CapabilityError::CapabilityExpired.into())
	}
	let frames = record
		.frames
		.iter()
		.map(|frame| hex::decode(frame).map_err(|_| ContentError::IntegrityFailed.into()))
		.collect::<Result<Vec<_>, PrivateQueryError>>()?;
	let operation_id = decode_hex(&record.operation_id)?;
	let response_hash = decode_hex(&record.response_hash)?;
	if response_hash != response_hash_fn(&frames, record.next_verified_offset) {
		return Err(ContentError::IntegrityFailed.into())
	}
	Ok(PrivateObjectResponseV2 {
		operation_id,
		frames,
		next_verified_offset: record.next_verified_offset,
		response_hash,
	})
}

fn derive_operation_id(method: u16, request_id: [u8; 16]) -> [u8; 16] {
	let mut hash = Sha256::new();
	hash.update(QUERY_ID_DOMAIN);
	hash.update(method.to_be_bytes());
	hash.update(request_id);
	let digest: [u8; 32] = hash.finalize().into();
	digest[..16].try_into().expect("fixed prefix")
}

fn fingerprint(request: &[u8], authority: &[u8]) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update((request.len() as u64).to_be_bytes());
	hash.update(request);
	hash.update((authority.len() as u64).to_be_bytes());
	hash.update(authority);
	hash.finalize().into()
}

fn response_hash(frames: &[Vec<u8>], next: Option<u64>) -> [u8; 32] {
	response_hash_fn(frames, next)
}

fn response_hash_fn(frames: &[Vec<u8>], next: Option<u64>) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(QUERY_RESPONSE_DOMAIN);
	for frame in frames {
		hash.update((frame.len() as u64).to_be_bytes());
		hash.update(frame);
	}
	match next {
		Some(offset) => {
			hash.update([1]);
			hash.update(offset.to_be_bytes());
		},
		None => hash.update([0]),
	}
	hash.finalize().into()
}

fn query_key(operation_id: [u8; 16], offset: u64) -> String {
	format!("{}:{offset:016x}", hex::encode(operation_id))
}

fn continuation_authorized(
	state: &super::JournalState,
	operation_id: [u8; 16],
	fingerprint: [u8; 32],
	offset: u64,
) -> bool {
	if offset == 0 {
		return true
	}
	let operation_id = hex::encode(operation_id);
	let fingerprint = hex::encode(fingerprint);
	state.private_queries.values().any(|record| {
		record.operation_id == operation_id &&
			record.fingerprint == fingerprint &&
			record.next_verified_offset == Some(offset)
	})
}

fn ensure_private_query_capacity(
	state: &super::JournalState,
	replay_key: &str,
) -> Result<(), PrivateQueryError> {
	if state.private_queries.len() >= MAX_STREAMING_OPERATIONS ||
		(!state.private_query_replay.contains_key(replay_key) &&
			state.private_query_replay.len() >= MAX_STREAMING_OPERATIONS)
	{
		Err(ContentError::ProviderRecoveryTableFull.into())
	} else {
		Ok(())
	}
}

fn replay_key(grant: [u8; 32], nonce: [u8; 16]) -> String {
	format!("{}:{}", hex::encode(grant), hex::encode(nonce))
}

fn event(request: [u8; 16], sequence: u32, kind: u8, payload: Value) -> Vec<u8> {
	map(vec![
		(0, uint(2)),
		(1, bstr(&request)),
		(2, uint(u64::from(sequence))),
		(3, uint(u64::from(kind))),
		(4, payload),
	])
}

fn accepted() -> Value {
	Value::Map(vec![(uint(0), uint(1))])
}

fn checkpoint_value(checkpoint: FinalizedCheckpoint) -> Value {
	Value::Map(vec![
		(uint(0), bstr(&checkpoint.root)),
		(uint(1), uint(checkpoint.start)),
		(uint(2), uint(checkpoint.leaves)),
		(uint(3), uint(u64::from(checkpoint.block))),
	])
}

fn finality_value(checkpoint: FinalizedCheckpoint) -> Value {
	Value::Map(vec![
		(uint(0), uint(u64::from(checkpoint.finalized_number))),
		(uint(1), bstr(&checkpoint.finalized_hash)),
	])
}

fn verify_provider_receipt(
	receipt: &super::recovery::ProviderReceiptV1,
	service_key: [u8; 32],
) -> Result<(), ContentError> {
	let mut message = b"cord.provider.storage.receipt.v1".to_vec();
	message.extend(map(vec![
		(0, bstr(&receipt.provider)),
		(1, Value::Text(receipt.cid.clone())),
		(2, uint(receipt.stored_bytes)),
	]));
	if ed25519::Pair::verify(
		&ed25519::Signature::from_raw(receipt.signature),
		&message,
		&ed25519::Public::from_raw(service_key),
	) {
		Ok(())
	} else {
		Err(ContentError::IntegrityFailed)
	}
}

fn map(entries: Vec<(u8, Value)>) -> Vec<u8> {
	let entries = entries.into_iter().map(|(key, value)| (uint(u64::from(key)), value)).collect();
	let mut bytes = Vec::new();
	ciborium::ser::into_writer(&Value::Map(entries), &mut bytes).expect("serializable");
	bytes
}

fn value(bytes: &[u8]) -> Result<Value, PrivateQueryError> {
	ciborium::de::from_reader(bytes).map_err(|_| PrivateQueryError::WireSchemaInvalid)
}

fn uint(value: u64) -> Value {
	Value::Integer(value.into())
}

fn bstr(value: &[u8]) -> Value {
	Value::Bytes(value.to_vec())
}

fn take<const N: usize>(
	fields: &mut [Option<Value>; N],
	index: usize,
) -> Result<Value, PrivateQueryError> {
	fields[index].take().ok_or(PrivateQueryError::WireSchemaInvalid)
}

fn integer(value: Value) -> Result<u64, PrivateQueryError> {
	let Value::Integer(value) = value else { return Err(PrivateQueryError::WireSchemaInvalid) };
	value.try_into().map_err(|_| PrivateQueryError::WireSchemaInvalid)
}

fn bytes_fixed<const N: usize>(value: Value) -> Result<[u8; N], PrivateQueryError> {
	let Value::Bytes(value) = value else { return Err(PrivateQueryError::WireSchemaInvalid) };
	value.try_into().map_err(|_| PrivateQueryError::WireSchemaInvalid)
}

fn bounded_bytes(value: Value, min: usize, max: usize) -> Result<Vec<u8>, PrivateQueryError> {
	let Value::Bytes(value) = value else { return Err(PrivateQueryError::WireSchemaInvalid) };
	if value.len() < min || value.len() > max {
		return Err(PrivateQueryError::WireSchemaInvalid)
	}
	Ok(value)
}

fn text(value: Value, max: usize) -> Result<String, PrivateQueryError> {
	let Value::Text(value) = value else { return Err(PrivateQueryError::WireSchemaInvalid) };
	if value.is_empty() || value.len() > max || !value.nfc().eq(value.chars()) {
		return Err(PrivateQueryError::WireSchemaInvalid)
	}
	Ok(value)
}

fn decode_hash_text(value: &str) -> Option<[u8; 32]> {
	let value = value.strip_prefix("0x").unwrap_or(value);
	hex::decode(value).ok()?.try_into().ok()
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], PrivateQueryError> {
	hex::decode(value)
		.map_err(|_| ContentError::IntegrityFailed)?
		.try_into()
		.map_err(|_| ContentError::IntegrityFailed.into())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{
		chain::{ReplicationProviderSnapshot, ReplicationTopologySnapshot},
		OperationId, StreamingDescriptor, CHUNK_BYTES,
	};
	use codec::Encode;
	use orbis_storage_runtime_api::{
		AgreementInfo, AgreementStatus, BucketGrantInfo, BucketRole, CheckpointInfo,
		CommitmentInfo, ControlBucketInfo, HostDelegationInfo,
	};
	use sp_core::{crypto::AccountId32, ed25519, H256};
	use tempfile::TempDir;

	const GET_VECTOR: &str = "a700020150111111111111111111111111111111110268666573746976616c031903f3045820222222222222222222222222222222222222222222222222222222222222222207186408a2005820222222222222222222222222222222222222222222222222222222222222222201783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c7470726637723271";
	const RANGE_VECTOR: &str = "a700020150111111111111111111111111111111110268666573746976616c031903f4045820222222222222222222222222222222222222222222222222222222222222222207186408a4005820222222222222222222222222222222222222222222222222222222222222222201783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c747072663772327102010301";
	const DELETE_VECTOR: &str = "a800020150111111111111111111111111111111110268666573746976616c031903f5045820222222222222222222222222222222222222222222222222222222222222222205503333333333333333333333333333333307186408a3005820222222222222222222222222222222222222222222222222222222222222222201783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c74707266377232710201";
	const STATUS_VECTOR: &str = "a700020150111111111111111111111111111111110268666573746976616c031903f6045820222222222222222222222222222222222222222222222222222222222222222207186408a2005820222222222222222222222222222222222222222222222222222222222222222201783e6261666b32627a6163656168666f756f6165337375686d7869766d786c6179657a336b7135647a6f376935337936353468376b76756c7470726637723271";

	#[test]
	fn frozen_positive_requests_decode_and_round_trip_exactly() {
		for (wire, method) in [(GET_VECTOR, GET), (RANGE_VECTOR, RANGE), (STATUS_VECTOR, STATUS)] {
			let bytes = hex::decode(wire).unwrap();
			let request = PrivateObjectRequestV2::decode(&bytes).unwrap();
			assert_eq!(request.method, method);
			assert_eq!(request.canonical_bytes(), bytes);
		}
	}

	#[test]
	fn private_dispatcher_refuses_delete_without_state() {
		let error =
			PrivateObjectRequestV2::decode(&hex::decode(DELETE_VECTOR).unwrap()).unwrap_err();
		assert_eq!(error, PrivateQueryError::ChainFinalityRequired);
	}

	#[test]
	fn derived_operation_id_is_method_and_request_domain_separated() {
		let request = [7; 16];
		assert_eq!(derive_operation_id(GET, request), derive_operation_id(GET, request));
		assert_ne!(derive_operation_id(GET, request), derive_operation_id(RANGE, request));
		assert_ne!(derive_operation_id(GET, request), derive_operation_id(GET, [8; 16]));
	}

	struct QueryFixture {
		temp: TempDir,
		streaming: StreamingStore,
		mmr: BucketMmrStore,
		authority: CapabilityAuthoritySnapshot,
		topology: ReplicationTopologySnapshot,
		host: ed25519::Pair,
		service: ed25519::Pair,
		cid: CanonicalCid,
		bytes: Vec<u8>,
	}

	fn query_fixture(bytes: Vec<u8>) -> QueryFixture {
		let temp = TempDir::new().unwrap();
		let bucket = [5; 32];
		let operation = OperationId::from_bytes([4; 16]);
		let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(&bytes));
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: operation,
					bucket_id: BucketId::from_bytes(bucket),
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				bytes.chunks(CHUNK_BYTES).map(<[u8]>::to_vec).collect::<Vec<_>>(),
			)
			.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		mmr.append_verified(&streaming, BucketId::from_bytes(bucket), operation)
			.unwrap();
		let commitment =
			mmr.commitment_candidate(&streaming, BucketId::from_bytes(bucket), 0).unwrap();
		let host = ed25519::Pair::from_seed(&[9; 32]);
		let service = ed25519::Pair::from_seed(&[8; 32]);
		let local = AccountId32::new([7; 32]);
		let authority = CapabilityAuthoritySnapshot {
			finalized_hash: format!("0x{}", hex::encode([10; 32])),
			finalized_number: 110,
			genesis_hash: [2; 32],
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			local_provider: [7; 32],
			delegation: HostDelegationInfo {
				grant_id: H256([3; 32]),
				bucket_id: H256(bucket),
				owner: AccountId32::new([1; 32]),
				issuance_nonce: 0,
				issuer_key_id: H256([6; 32]),
				issuer_public_key: host.public().0,
				key_version: 1,
				state_version: 1,
				key_activated_at: 90,
				product_id: b"festival".to_vec(),
				methods: vec![GET, RANGE, STATUS],
				cid: Some(cid.as_str().as_bytes().to_vec()),
				max_bytes: bytes.len() as u64,
				issued_at: 90,
				expires_at: 200,
				revoked_at: None,
			},
			bucket: ControlBucketInfo {
				bucket_id: H256(bucket),
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
				agreement_id: H256([11; 32]),
				owner: AccountId32::new([1; 32]),
				bucket_id: H256(bucket),
				primary: local,
				replicas: vec![],
				bytes: bytes.len() as u64,
				created_at: 90,
				expires_at: 180,
				release_at: None,
				state_version: 1,
				status: AgreementStatus::Active,
			}),
		};
		let endpoint = b"https://origin-provider.invalid".to_vec();
		let provider = ReplicationProviderSnapshot {
			provider: [7; 32],
			order: 0,
			primary: true,
			record_present: true,
			endpoint_hash: Some(sp_crypto_hashing::blake2_256(&endpoint)),
			endpoint: Some(endpoint),
			active_service_key: Some(service.public().0),
			active_service_key_version: Some(1),
			status_active: true,
			organization_valid: true,
			authority_validated_at: Some(108),
			overdue_challenges: 0,
			eligible: true,
			usable: true,
			exclusions: Vec::new(),
			confirmed_checkpoint: Some(108),
		};
		let mut topology = ReplicationTopologySnapshot {
			genesis_hash: authority.genesis_hash,
			finalized_hash: [10; 32],
			finalized_number: 110,
			governed_finalized_checkpoint: Some(108),
			bucket_id: bucket,
			bucket_version: 1,
			primary: [7; 32],
			replicas: vec![],
			providers: vec![provider],
			current_checkpoint: Some(CheckpointInfo {
				bucket_id: H256(bucket),
				commitment: CommitmentInfo {
					mmr_root: commitment.mmr_root,
					start_seq: commitment.start_seq,
					leaf_count: commitment.leaf_count,
				},
				checkpoint_block: 108,
				primary_signers: 1,
				commitment_nonce: 108,
				replica_confirmations: vec![],
			}),
			snapshot_hash: [0; 32],
		};
		seal(&mut topology);
		QueryFixture { temp, streaming, mmr, authority, topology, host, service, cid, bytes }
	}

	fn seal(topology: &mut ReplicationTopologySnapshot) {
		topology.snapshot_hash = [0; 32];
		let mut input = b"cord/provider/replication-topology/v1".to_vec();
		topology.encode_to(&mut input);
		topology.snapshot_hash = sp_crypto_hashing::blake2_256(&input);
	}

	fn request(fixture: &QueryFixture, method: u16, range: Option<(u64, u64)>) -> Vec<u8> {
		PrivateObjectRequestV2 {
			request_id: [method as u8; 16],
			product_id: "festival".into(),
			method,
			grant_id: [3; 32],
			trace_context: None,
			deadline: 120,
			bucket_id: [5; 32],
			cid: fixture.cid.clone(),
			range,
		}
		.canonical_bytes()
	}

	fn capability(fixture: &QueryFixture, method: u16, bytes: u64, nonce: u8) -> Vec<u8> {
		let mut capability = ProviderCapabilityV1 {
			version: 1,
			registry_sha256: crate::capability::NORMATIVE_REGISTRY_SHA256,
			genesis_hash: fixture.authority.genesis_hash,
			grant_id: [3; 32],
			issuer_key_id: [6; 32],
			product_id: "festival".into(),
			bucket_id: [5; 32],
			agreement_id: Some([11; 32]),
			provider: [7; 32],
			methods: vec![method],
			cid: Some(fixture.cid.clone()),
			max_bytes: bytes,
			issued_at: 100,
			expires_at: 128,
			nonce: [nonce; 16],
			signature: [0; 64],
		};
		capability.signature = fixture.host.sign(&capability.signed_preimage()).0;
		capability.canonical_bytes()
	}

	#[test]
	fn get_and_range_emit_exact_verified_offsets_and_replay_across_restart() {
		let fixture = query_fixture(b"0123456789".to_vec());
		let get = request(&fixture, GET, None);
		let get_capability = capability(&fixture, GET, fixture.bytes.len() as u64, 1);
		let first = fixture
			.streaming
			.private_object_query(
				&get,
				&get_capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		assert_eq!(first.next_verified_offset, None);
		assert_eq!(first.frames.len(), 3);
		let first_frames = first.frames.clone();
		let reopened = StreamingStore::open(fixture.temp.path()).unwrap();
		let reopened_mmr = BucketMmrStore::open(fixture.temp.path(), &reopened).unwrap();
		let replay = reopened
			.private_object_query(
				&get,
				&get_capability,
				&fixture.authority,
				&fixture.topology,
				&reopened_mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		assert_eq!(replay, first);
		assert_eq!(replay.frames, first_frames);

		let range = request(&fixture, RANGE, Some((2, 4)));
		let range_capability = capability(&fixture, RANGE, 4, 2);
		let response = reopened
			.private_object_query(
				&range,
				&range_capability,
				&fixture.authority,
				&fixture.topology,
				&reopened_mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		let progress: Value = ciborium::de::from_reader(response.frames[1].as_slice()).unwrap();
		let Value::Map(event) = progress else { panic!("event") };
		let Value::Map(payload) = &event[4].1 else { panic!("payload") };
		assert_eq!(payload[0].1, uint(2));
		assert_eq!(payload[1].1, bstr(b"2345"));
		let result: Value = ciborium::de::from_reader(response.frames[2].as_slice()).unwrap();
		let Value::Map(event) = result else { panic!("event") };
		assert_eq!(event[3].1, uint(2));
		let Value::Map(payload) = &event[4].1 else { panic!("payload") };
		assert_eq!(payload.len(), 5);
		assert_eq!(payload[0].1, Value::Text(fixture.cid.as_str().into()));
		assert_eq!(payload[1].1, uint(10));
		assert_eq!(payload[2].1, uint(2));
		assert_eq!(payload[3].1, uint(6));
		assert_eq!(
			payload[4].1,
			checkpoint_value(
				finalized_checkpoint(
					&reopened_mmr,
					&fixture.topology,
					&fixture.authority,
					[5; 32],
					&fixture.cid,
					10,
					true,
					fixture.service.public().0,
				)
				.unwrap()
			)
		);
		let range_reopened = StreamingStore::open(fixture.temp.path()).unwrap();
		let range_reopened_mmr =
			BucketMmrStore::open(fixture.temp.path(), &range_reopened).unwrap();
		let range_replay = range_reopened
			.private_object_query(
				&range,
				&range_capability,
				&fixture.authority,
				&fixture.topology,
				&range_reopened_mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		assert_eq!(range_replay, response);
	}

	#[test]
	fn capability_scope_and_finalized_checkpoint_mismatch_fail_closed() {
		let fixture = query_fixture(b"bytes".to_vec());
		let get = request(&fixture, GET, None);
		let wrong_method = capability(&fixture, RANGE, fixture.bytes.len() as u64, 3);
		assert!(matches!(
			fixture.streaming.private_object_query(
				&get,
				&wrong_method,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			),
			Err(PrivateQueryError::Capability(CapabilityError::GrantScopeDenied))
		));
		let mut wrong_audience =
			ProviderCapabilityV1::decode(&capability(&fixture, GET, fixture.bytes.len() as u64, 6))
				.unwrap();
		wrong_audience.provider = [99; 32];
		wrong_audience.signature = fixture.host.sign(&wrong_audience.signed_preimage()).0;
		assert!(matches!(
			fixture.streaming.private_object_query(
				&get,
				&wrong_audience.canonical_bytes(),
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			),
			Err(PrivateQueryError::Capability(CapabilityError::CapabilityAudienceInvalid))
		));
		let mut wrong_cid =
			ProviderCapabilityV1::decode(&capability(&fixture, GET, fixture.bytes.len() as u64, 7))
				.unwrap();
		wrong_cid.cid = Some(CanonicalCid::from_digest([42; 32]));
		wrong_cid.signature = fixture.host.sign(&wrong_cid.signed_preimage()).0;
		assert!(matches!(
			fixture.streaming.private_object_query(
				&get,
				&wrong_cid.canonical_bytes(),
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			),
			Err(PrivateQueryError::Capability(CapabilityError::CapabilityContentInvalid))
		));
		let mut mismatched = fixture.topology.clone();
		mismatched.current_checkpoint.as_mut().unwrap().commitment.mmr_root = H256([99; 32]);
		seal(&mut mismatched);
		let valid = capability(&fixture, GET, fixture.bytes.len() as u64, 4);
		assert_eq!(
			fixture
				.streaming
				.private_object_query(
					&get,
					&valid,
					&fixture.authority,
					&mismatched,
					&fixture.mmr,
					fixture.service.public().0,
					0,
				)
				.unwrap_err(),
			PrivateQueryError::FinalizedViewInvalid
		);
	}

	#[test]
	fn corrupt_get_and_range_never_emit_or_persist_bytes() {
		let fixture = query_fixture(b"verified".to_vec());
		std::fs::write(
			fixture
				.temp
				.path()
				.join("streaming-v1")
				.join("objects")
				.join(fixture.cid.as_str()),
			b"corrupt!",
		)
		.unwrap();
		let get = request(&fixture, GET, None);
		let get_capability = capability(&fixture, GET, fixture.bytes.len() as u64, 5);
		assert!(fixture
			.streaming
			.private_object_query(
				&get,
				&get_capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.is_err());
		let range = request(&fixture, RANGE, Some((1, 3)));
		let range_capability = capability(&fixture, RANGE, 3, 9);
		assert!(fixture
			.streaming
			.private_object_query(
				&range,
				&range_capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.is_err());
		assert!(fixture.streaming.read_state().unwrap().private_queries.is_empty());
	}

	#[test]
	fn range_rejects_zero_length_overflow_and_out_of_object() {
		let fixture = query_fixture(b"0123456789".to_vec());
		for (start, length, nonce) in [(2, 0, 10), (u64::MAX, 1, 11), (8, 3, 12)] {
			let range = request(&fixture, RANGE, Some((start, length)));
			let authority = capability(&fixture, RANGE, length, nonce);
			assert_eq!(
				fixture
					.streaming
					.private_object_query(
						&range,
						&authority,
						&fixture.authority,
						&fixture.topology,
						&fixture.mmr,
						fixture.service.public().0,
						0,
					)
					.unwrap_err(),
				PrivateQueryError::Content(ContentError::RangeInvalid)
			);
		}
		assert!(fixture.streaming.read_state().unwrap().private_queries.is_empty());
	}

	#[test]
	fn get_resume_accepts_only_the_previously_verified_offset() {
		let fixture = query_fixture(vec![21; MAX_RANGE_BYTES as usize + 3]);
		let get = request(&fixture, GET, None);
		let capability = capability(&fixture, GET, fixture.bytes.len() as u64, 8);
		assert_eq!(
			fixture
				.streaming
				.private_object_query(
					&get,
					&capability,
					&fixture.authority,
					&fixture.topology,
					&fixture.mmr,
					fixture.service.public().0,
					1,
				)
				.unwrap_err(),
			PrivateQueryError::VerifiedOffsetInvalid
		);
		let first = fixture
			.streaming
			.private_object_query(
				&get,
				&capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		assert_eq!(first.next_verified_offset, Some(MAX_RANGE_BYTES));
		assert_eq!(
			fixture
				.streaming
				.private_object_query(
					&get,
					&capability,
					&fixture.authority,
					&fixture.topology,
					&fixture.mmr,
					fixture.service.public().0,
					MAX_RANGE_BYTES + 1,
				)
				.unwrap_err(),
			PrivateQueryError::VerifiedOffsetInvalid
		);
		let tail = fixture
			.streaming
			.private_object_query(
				&get,
				&capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				MAX_RANGE_BYTES,
			)
			.unwrap();
		assert_eq!(tail.next_verified_offset, None);
		assert_eq!(tail.frames.len(), 2);
		let retain_until = fixture
			.streaming
			.read_state()
			.unwrap()
			.private_queries
			.values()
			.next()
			.unwrap()
			.retain_until;
		fixture.streaming.prune_private_queries(retain_until + 1).unwrap();
		let state = fixture.streaming.read_state().unwrap();
		assert!(state.private_queries.is_empty());
		assert!(state.private_query_replay.is_empty());
	}

	#[test]
	fn stored_page_replays_after_rotation_and_finality_advance_but_continuation_fails() {
		let fixture = query_fixture(vec![31; MAX_RANGE_BYTES as usize + 1]);
		let get = request(&fixture, GET, None);
		let capability = capability(&fixture, GET, fixture.bytes.len() as u64, 13);
		let first = fixture
			.streaming
			.private_object_query(
				&get,
				&capability,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		let mut advanced_authority = fixture.authority.clone();
		advanced_authority.finalized_hash = format!("0x{}", hex::encode([12; 32]));
		advanced_authority.finalized_number = 111;
		advanced_authority.delegation.revoked_at = Some(111);
		advanced_authority.delegation.issuer_key_id = H256([44; 32]);
		let mut advanced_topology = fixture.topology.clone();
		advanced_topology.finalized_hash = [12; 32];
		advanced_topology.finalized_number = 111;
		seal(&mut advanced_topology);
		let replay = fixture
			.streaming
			.private_object_query(
				&get,
				&capability,
				&advanced_authority,
				&advanced_topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		assert_eq!(replay, first);
		assert_eq!(
			fixture
				.streaming
				.private_object_query(
					&get,
					&capability,
					&advanced_authority,
					&advanced_topology,
					&fixture.mmr,
					fixture.service.public().0,
					MAX_RANGE_BYTES,
				)
				.unwrap_err(),
			PrivateQueryError::Capability(CapabilityError::CapabilityIssuerRevoked)
		);
	}

	#[test]
	fn cross_bucket_same_cid_is_not_a_query_record() {
		let fixture = query_fixture(b"same-cid".to_vec());
		let mut cross_bucket =
			PrivateObjectRequestV2::decode(&request(&fixture, GET, None)).unwrap();
		cross_bucket.bucket_id = [66; 32];
		let authority = capability(&fixture, GET, fixture.bytes.len() as u64, 14);
		assert_eq!(
			fixture
				.streaming
				.private_object_query(
					&cross_bucket.canonical_bytes(),
					&authority,
					&fixture.authority,
					&fixture.topology,
					&fixture.mmr,
					fixture.service.public().0,
					0,
				)
				.unwrap_err(),
			PrivateQueryError::Content(ContentError::NotFound)
		);
	}

	#[test]
	fn earlier_checkpoint_member_survives_suffix_advance_and_unfinalized_local_append() {
		let fixture = query_fixture(b"checkpoint-zero".to_vec());
		let append = |operation: [u8; 16], bytes: &[u8]| {
			let cid = CanonicalCid::from_digest(sp_crypto_hashing::blake2_256(bytes));
			let operation = OperationId::from_bytes(operation);
			fixture
				.streaming
				.put_chunks(
					StreamingDescriptor {
						operation_id: operation,
						bucket_id: BucketId::from_bytes([5; 32]),
						expected_cid: cid.as_str().into(),
						object_len: bytes.len() as u64,
					},
					vec![bytes.to_vec()],
				)
				.unwrap();
			fixture
				.mmr
				.append_verified(&fixture.streaming, BucketId::from_bytes([5; 32]), operation)
				.unwrap();
		};
		append([51; 16], b"checkpoint-one");
		let suffix = fixture
			.mmr
			.commitment_candidate(&fixture.streaming, BucketId::from_bytes([5; 32]), 1)
			.unwrap();
		let mut advanced = fixture.topology.clone();
		let checkpoint = advanced.current_checkpoint.as_mut().unwrap();
		checkpoint.commitment = CommitmentInfo {
			mmr_root: suffix.mmr_root,
			start_seq: suffix.start_seq,
			leaf_count: suffix.leaf_count,
		};
		checkpoint.checkpoint_block = 109;
		advanced.governed_finalized_checkpoint = Some(109);
		seal(&mut advanced);
		append([52; 16], b"not-finalized-yet");

		let get = request(&fixture, GET, None);
		let authority = capability(&fixture, GET, fixture.bytes.len() as u64, 19);
		assert!(fixture
			.streaming
			.private_object_query(
				&get,
				&authority,
				&fixture.authority,
				&advanced,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.is_ok());
	}

	#[test]
	fn quarantined_status_uses_structural_checkpoint_and_reports_phase_four() {
		let fixture = query_fixture(b"status-bytes".to_vec());
		std::fs::write(
			fixture
				.temp
				.path()
				.join("streaming-v1")
				.join("objects")
				.join(fixture.cid.as_str()),
			b"broken-bytes",
		)
		.unwrap();
		assert_eq!(
			fixture.streaming.verify_installed(fixture.cid.as_str()).unwrap_err(),
			ContentError::IntegrityFailed
		);
		let status = request(&fixture, STATUS, None);
		let authority = capability(&fixture, STATUS, 0, 15);
		let response = fixture
			.streaming
			.private_object_query(
				&status,
				&authority,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		let result: Value = ciborium::de::from_reader(response.frames[1].as_slice()).unwrap();
		let Value::Map(event) = result else { panic!("event") };
		let Value::Map(payload) = &event[4].1 else { panic!("payload") };
		assert_eq!(payload[0].1, uint(4));
	}

	#[test]
	fn query_commit_failure_never_exposes_non_durable_replay_state() {
		let fixture = query_fixture(b"durable".to_vec());
		let get = request(&fixture, GET, None);
		let authority = capability(&fixture, GET, fixture.bytes.len() as u64, 16);
		fixture
			.streaming
			.inject_fault_once(super::super::StreamingFault::BeforePrivateQueryCommit)
			.unwrap();
		assert!(matches!(
			fixture.streaming.private_object_query(
				&get,
				&authority,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			),
			Err(PrivateQueryError::Content(ContentError::Io(_)))
		));
		{
			let state = fixture.streaming.read_state().unwrap();
			assert!(state.private_queries.is_empty());
			assert!(state.private_query_replay.is_empty());
		}
		let reopened = StreamingStore::open(fixture.temp.path()).unwrap();
		let reopened_mmr = BucketMmrStore::open(fixture.temp.path(), &reopened).unwrap();
		assert!(reopened.read_state().unwrap().private_queries.is_empty());
		assert!(reopened
			.private_object_query(
				&get,
				&authority,
				&fixture.authority,
				&fixture.topology,
				&reopened_mmr,
				fixture.service.public().0,
				0,
			)
			.is_ok());
	}

	#[test]
	fn bounded_gc_retains_live_then_removes_expired_record_and_replay() {
		let fixture = query_fixture(b"gc".to_vec());
		let get = request(&fixture, GET, None);
		let authority = capability(&fixture, GET, fixture.bytes.len() as u64, 17);
		fixture
			.streaming
			.private_object_query(
				&get,
				&authority,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		let retain_until = fixture
			.streaming
			.read_state()
			.unwrap()
			.private_queries
			.values()
			.next()
			.unwrap()
			.retain_until;
		fixture.streaming.prune_private_queries(retain_until).unwrap();
		assert_eq!(fixture.streaming.read_state().unwrap().private_queries.len(), 1);
		fixture.streaming.prune_private_queries(retain_until + 1).unwrap();
		{
			let state = fixture.streaming.read_state().unwrap();
			assert!(state.private_queries.is_empty());
			assert!(state.private_query_replay.is_empty());
			validate_private_query_state(&state).unwrap();
		}
		let reopened = StreamingStore::open(fixture.temp.path()).unwrap();
		let state = reopened.read_state().unwrap();
		assert!(state.private_queries.is_empty());
		assert!(state.private_query_replay.is_empty());
	}

	#[test]
	fn bounded_gc_restores_capacity_without_pruning_live_records() {
		let fixture = query_fixture(b"capacity".to_vec());
		let get = request(&fixture, GET, None);
		let authority = capability(&fixture, GET, fixture.bytes.len() as u64, 18);
		fixture
			.streaming
			.private_object_query(
				&get,
				&authority,
				&fixture.authority,
				&fixture.topology,
				&fixture.mmr,
				fixture.service.public().0,
				0,
			)
			.unwrap();
		let mut state = fixture.streaming.read_state().unwrap().clone();
		let template = state.private_queries.values().next().unwrap().clone();
		state.private_queries.clear();
		for index in 0..MAX_STREAMING_OPERATIONS {
			let mut record = template.clone();
			record.retain_until = 1;
			state.private_queries.insert(format!("{index:064x}"), record);
		}
		let replay_key = state.private_query_replay.keys().next().unwrap().clone();
		assert_eq!(
			ensure_private_query_capacity(&state, &replay_key).unwrap_err(),
			PrivateQueryError::Content(ContentError::ProviderRecoveryTableFull)
		);
		prune_private_query_state(&mut state, 2, PRIVATE_QUERY_GC_BATCH).unwrap();
		assert_eq!(state.private_queries.len(), MAX_STREAMING_OPERATIONS - PRIVATE_QUERY_GC_BATCH);
		ensure_private_query_capacity(&state, &replay_key).unwrap();
	}
}
