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

//! Private PUT transition adapter over the durable recovery kernel.
//!
//! Process framing and socket lifecycle are intentionally outside this module.

use std::array;

use blake2::{digest::consts::U32, Blake2b, Digest};
use ciborium::value::Value;

use super::{
	recovery::{
		ObjectPutRequestV2, RecoveryError, RecoveryResponseV1, RecoverySigner, ResumeTokenV1,
	},
	StreamingStore,
};
use crate::{CapabilityAuthoritySnapshot, CHUNK_BYTES};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum LocalPutSessionError {
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid,
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical,
	#[error("STORAGE_CHUNK_OUT_OF_ORDER")]
	ChunkOutOfOrder,
	#[error("REQUEST_CANCELLED")]
	Cancelled,
	#[error("REQUEST_ALREADY_TERMINAL")]
	Terminal,
	#[error(transparent)]
	Recovery(#[from] RecoveryError),
}

/// Exact canonical `ProviderTransferChunkV1`; bytes are never represented by a second DTO.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProviderTransferChunkV1 {
	pub(crate) operation_id: [u8; 16],
	pub(crate) index: u16,
	pub(crate) bytes: Vec<u8>,
	pub(crate) hash: [u8; 32],
}

impl ProviderTransferChunkV1 {
	pub(crate) fn decode(exact: &[u8]) -> Result<Self, LocalPutSessionError> {
		if exact.len() > CHUNK_BYTES + 128 {
			return Err(LocalPutSessionError::WireSchemaInvalid)
		}
		let value: Value = ciborium::de::from_reader(exact)
			.map_err(|_| LocalPutSessionError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else {
			return Err(LocalPutSessionError::WireSchemaInvalid)
		};
		let mut fields: [Option<Value>; 5] = array::from_fn(|_| None);
		for (key, value) in entries {
			let key: usize =
				uint(key)?.try_into().map_err(|_| LocalPutSessionError::WireSchemaInvalid)?;
			if key > 4 || fields[key].replace(value).is_some() {
				return Err(LocalPutSessionError::WireSchemaInvalid)
			}
		}
		if fields.iter().any(Option::is_none) || uint(take(&mut fields, 0)?)? != 1 {
			return Err(LocalPutSessionError::WireSchemaInvalid)
		}
		let operation_id = fixed_bytes(take(&mut fields, 1)?)?;
		let index = uint(take(&mut fields, 2)?)?
			.try_into()
			.map_err(|_| LocalPutSessionError::WireSchemaInvalid)?;
		let bytes = bounded_bytes(take(&mut fields, 3)?, CHUNK_BYTES)?;
		let hash = fixed_bytes(take(&mut fields, 4)?)?;
		let chunk = Self { operation_id, index, bytes, hash };
		if chunk.hash != Blake2b::<U32>::digest(&chunk.bytes).as_slice() {
			return Err(LocalPutSessionError::WireSchemaInvalid)
		}
		if chunk.canonical_bytes() != exact {
			return Err(LocalPutSessionError::WireNonCanonical)
		}
		Ok(chunk)
	}

	fn canonical_bytes(&self) -> Vec<u8> {
		canonical_map(vec![
			(0, Value::Integer(1.into())),
			(1, Value::Bytes(self.operation_id.to_vec())),
			(2, Value::Integer(u64::from(self.index).into())),
			(3, Value::Bytes(self.bytes.clone())),
			(4, Value::Bytes(self.hash.to_vec())),
		])
	}
}

/// One authenticated PUT stream. All effects and resume cursors remain owned by `StreamingStore`.
pub(crate) struct LocalObjectPutSession<'a> {
	store: &'a StreamingStore,
	snapshot: CapabilityAuthoritySnapshot,
	current_service_key: [u8; 32],
	signer: &'a dyn RecoverySigner,
	request_bytes: Vec<u8>,
	operation_id: [u8; 16],
	token: Vec<u8>,
	next_index: u16,
	terminal: bool,
	cancelled: bool,
}

impl<'a> LocalObjectPutSession<'a> {
	pub(crate) fn accept(
		store: &'a StreamingStore,
		request_bytes: &[u8],
		authority_bytes: &[u8],
		snapshot: CapabilityAuthoritySnapshot,
		current_service_key: [u8; 32],
		signer: &'a dyn RecoverySigner,
		successor_nonce: [u8; 16],
	) -> Result<(Self, RecoveryResponseV1), LocalPutSessionError> {
		let request = ObjectPutRequestV2::decode(request_bytes)?;
		let response = store.accept_object_put(
			request_bytes,
			authority_bytes,
			&snapshot,
			current_service_key,
			signer,
			successor_nonce,
		)?;
		let token = response.successor_token.clone().ok_or(LocalPutSessionError::Terminal)?;
		Ok((
			Self {
				store,
				snapshot,
				current_service_key,
				signer,
				request_bytes: request_bytes.to_vec(),
				operation_id: request.operation_id,
				token,
				next_index: 0,
				terminal: false,
				cancelled: false,
			},
			response,
		))
	}

	pub(crate) fn resume(
		store: &'a StreamingStore,
		request_bytes: &[u8],
		token_bytes: &[u8],
		snapshot: CapabilityAuthoritySnapshot,
		current_service_key: [u8; 32],
		signer: &'a dyn RecoverySigner,
	) -> Result<Self, LocalPutSessionError> {
		let request = ObjectPutRequestV2::decode(request_bytes)?;
		let token = ResumeTokenV1::decode(token_bytes)?;
		if token.operation_id != request.operation_id ||
			token.bucket_id != request.bucket_id ||
			token.cid != request.cid ||
			token.object_len != request.object_len
		{
			return Err(LocalPutSessionError::ChunkOutOfOrder)
		}
		if token.cancelled {
			return Err(LocalPutSessionError::Cancelled)
		}
		let next_index =
			token.cursor.try_into().map_err(|_| LocalPutSessionError::ChunkOutOfOrder)?;
		Ok(Self {
			store,
			snapshot,
			current_service_key,
			signer,
			request_bytes: request_bytes.to_vec(),
			operation_id: request.operation_id,
			token: token_bytes.to_vec(),
			next_index,
			terminal: false,
			cancelled: false,
		})
	}

	pub(crate) fn push_chunk(
		&mut self,
		exact_chunk: &[u8],
		successor_nonce: [u8; 16],
	) -> Result<RecoveryResponseV1, LocalPutSessionError> {
		self.ensure_open()?;
		let chunk = ProviderTransferChunkV1::decode(exact_chunk)?;
		if chunk.operation_id != self.operation_id || chunk.index != self.next_index {
			return Err(LocalPutSessionError::ChunkOutOfOrder)
		}
		let cursor = u32::from(chunk.index)
			.checked_add(1)
			.ok_or(LocalPutSessionError::ChunkOutOfOrder)?;
		let response = self.store.advance_object_put(
			&self.request_bytes,
			&self.token,
			&self.snapshot,
			self.current_service_key,
			cursor,
			&chunk.bytes,
			self.signer,
			successor_nonce,
		)?;
		self.token = response.successor_token.clone().ok_or(LocalPutSessionError::Terminal)?;
		self.next_index =
			self.next_index.checked_add(1).ok_or(LocalPutSessionError::ChunkOutOfOrder)?;
		Ok(response)
	}

	pub(crate) fn finalize(&mut self) -> Result<RecoveryResponseV1, LocalPutSessionError> {
		self.ensure_open()?;
		let response = self.store.finalize_object_put(
			&self.request_bytes,
			&self.token,
			&self.snapshot,
			self.current_service_key,
			self.signer,
		)?;
		self.terminal = true;
		Ok(response)
	}

	pub(crate) fn cancel(&mut self) -> Result<RecoveryResponseV1, LocalPutSessionError> {
		self.ensure_open()?;
		let response = self.store.cancel_object_put(
			&self.request_bytes,
			&self.token,
			&self.snapshot,
			self.current_service_key,
			self.signer,
		)?;
		self.cancelled = true;
		Ok(response)
	}

	pub(crate) fn acknowledge(
		&self,
		authenticated_host_key_id: [u8; 32],
		exact_ack: &[u8],
	) -> Result<Vec<u8>, LocalPutSessionError> {
		Ok(self.store.acknowledge_response(authenticated_host_key_id, exact_ack)?)
	}

	fn ensure_open(&self) -> Result<(), LocalPutSessionError> {
		if self.cancelled {
			Err(LocalPutSessionError::Cancelled)
		} else if self.terminal {
			Err(LocalPutSessionError::Terminal)
		} else {
			Ok(())
		}
	}
}

fn take(fields: &mut [Option<Value>], key: usize) -> Result<Value, LocalPutSessionError> {
	fields[key].take().ok_or(LocalPutSessionError::WireSchemaInvalid)
}

fn uint(value: Value) -> Result<u64, LocalPutSessionError> {
	let Value::Integer(value) = value else { return Err(LocalPutSessionError::WireSchemaInvalid) };
	u64::try_from(value).map_err(|_| LocalPutSessionError::WireSchemaInvalid)
}

fn fixed_bytes<const N: usize>(value: Value) -> Result<[u8; N], LocalPutSessionError> {
	let Value::Bytes(value) = value else { return Err(LocalPutSessionError::WireSchemaInvalid) };
	value.try_into().map_err(|_| LocalPutSessionError::WireSchemaInvalid)
}

fn bounded_bytes(value: Value, maximum: usize) -> Result<Vec<u8>, LocalPutSessionError> {
	let Value::Bytes(value) = value else { return Err(LocalPutSessionError::WireSchemaInvalid) };
	if value.len() > maximum {
		return Err(LocalPutSessionError::WireSchemaInvalid)
	}
	Ok(value)
}

fn canonical_map(entries: Vec<(u64, Value)>) -> Vec<u8> {
	let value = Value::Map(
		entries
			.into_iter()
			.map(|(key, value)| (Value::Integer(key.into()), value))
			.collect(),
	);
	let mut bytes = Vec::new();
	ciborium::ser::into_writer(&value, &mut bytes).expect("bounded canonical CBOR is infallible");
	bytes
}

#[cfg(test)]
mod tests {
	use super::*;

	fn chunk(operation_id: [u8; 16], index: u16, bytes: Vec<u8>) -> Vec<u8> {
		ProviderTransferChunkV1 {
			operation_id,
			index,
			hash: Blake2b::<U32>::digest(&bytes).into(),
			bytes,
		}
		.canonical_bytes()
	}

	#[test]
	fn transfer_chunk_is_exact_canonical_bounded_and_hash_bound() {
		let fixture: serde_json::Value = serde_json::from_str(include_str!(
			"../../../../../../docs/specs/provider-transfer-chunk-v1.vectors.json"
		))
		.expect("shared transfer fixture is JSON");
		assert_eq!(fixture["algorithm"], "BLAKE2b-256");
		let vector = &fixture["vectors"][0];
		let exact = hex::decode(vector["canonical_cbor_hex"].as_str().unwrap()).unwrap();
		let decoded = ProviderTransferChunkV1::decode(&exact).unwrap();
		assert_eq!(hex::encode(decoded.operation_id), vector["operation_id_hex"]);
		assert_eq!(u64::from(decoded.index), vector["index"]);
		assert_eq!(hex::encode(&decoded.bytes), vector["bytes_hex"]);
		assert_eq!(hex::encode(decoded.hash), vector["digest_hex"]);
		assert_eq!(hex::encode(Blake2b::<U32>::digest(&decoded.bytes)), vector["digest_hex"]);

		let mut wrong_hash: Value = ciborium::de::from_reader(exact.as_slice()).unwrap();
		let Value::Map(fields) = &mut wrong_hash else { unreachable!() };
		fields.iter_mut().find(|(key, _)| *key == Value::Integer(4.into())).unwrap().1 =
			Value::Bytes([0; 32].to_vec());
		let mut wrong_hash_bytes = Vec::new();
		ciborium::ser::into_writer(&wrong_hash, &mut wrong_hash_bytes).unwrap();
		assert_eq!(
			ProviderTransferChunkV1::decode(&wrong_hash_bytes),
			Err(LocalPutSessionError::WireSchemaInvalid)
		);
		assert!(ProviderTransferChunkV1::decode(&chunk([7; 16], 0, vec![0; CHUNK_BYTES])).is_ok());
		assert_eq!(
			ProviderTransferChunkV1::decode(&chunk([7; 16], 0, vec![0; CHUNK_BYTES + 1])),
			Err(LocalPutSessionError::WireSchemaInvalid)
		);
	}
}
