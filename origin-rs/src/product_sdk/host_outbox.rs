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

//! Private encrypted pre-send outbox for provider-bound host requests.
//!
//! This is the durable protocol kernel. Browser and desktop persistence adapters remain non-public
//! until the product-path phase binds them to their real transports.

use std::{
	array,
	collections::{BTreeMap, BTreeSet},
	fs::{self, File, OpenOptions},
	io::{Read, Write},
	path::{Path, PathBuf},
	sync::RwLock,
};

use chacha20poly1305::{
	aead::{Aead, Payload},
	KeyInit, XChaCha20Poly1305, XNonce,
};
use ciborium::value::Value;
use codec::{Decode, Encode};
use sha2::{Digest, Sha256};

const VERSION: u8 = 1;
const ENVELOPE_VERSION: u8 = 1;
const AUTHORITY_BLOCKS: u64 = 128;
const RECOVERY_BLOCKS: u64 = 256;
const MAX_RECOVERY_BLOCKS: u64 = AUTHORITY_BLOCKS + RECOVERY_BLOCKS;
const MAX_RECORDS: usize = 4_096;
const MAX_TOTAL_ENCRYPTED_BYTES: u64 = 268_435_456;
const MAX_ENCRYPTED_RECORD_BYTES: usize = 4_456_448;
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_AUTHORITY_BYTES: usize = 4_096;
const MAX_TRANSFER_CHUNK_BYTES: usize = 262_144;
const MAX_TRANSFER_PAYLOAD_BYTES: usize = MAX_TRANSFER_CHUNK_BYTES + 128;
const ROOT: &str = "host-outbox-v1";
const QUARANTINE: &str = "quarantine";
const EXTENSION: &str = ".outbox";
const RECORD_DOMAIN: &[u8] = b"cord/host-outbox/private-record/v1";
const UPLOAD_MANIFEST_DOMAIN: &[u8] = b"cord/host-outbox/upload-manifest/v1";
const UPLOAD_CHUNK_DOMAIN: &[u8] = b"cord/host-outbox/upload-chunk/v1";
const UPLOAD_NONCE_DOMAIN: &[u8] = b"cord/host-outbox/upload-nonce/v1";

/// Stable host outbox failures from the normative registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[repr(u16)]
pub(crate) enum HostOutboxError {
	/// Input does not match the closed `HostOutboxEntryV1` schema.
	#[error("WIRE_SCHEMA_INVALID")]
	WireSchemaInvalid = 100,
	/// Input is valid CBOR but not its unique deterministic encoding.
	#[error("WIRE_NON_CANONICAL")]
	WireNonCanonical = 101,
	/// Input uses an unsupported host outbox entry version.
	#[error("WIRE_VERSION_MISMATCH")]
	WireVersionMismatch = 102,
	/// Durable storage or a required key is unavailable before a send.
	#[error("HOST_OUTBOX_UNAVAILABLE")]
	Unavailable = 113,
	/// Admission would exceed a record or byte bound.
	#[error("HOST_OUTBOX_FULL")]
	Full = 114,
	/// An encrypted or decoded record failed authentication or validation.
	#[error("HOST_OUTBOX_CORRUPT")]
	Corrupt = 115,
	/// The byte-identical retry window has closed.
	#[error("HOST_OUTBOX_EXPIRED")]
	Expired = 116,
	/// The requested state transition is invalid for the durable record.
	#[error("HOST_OUTBOX_STATE_INVALID")]
	StateInvalid,
	/// The supplied response or acknowledgement does not match the durable hash.
	#[error("HOST_OUTBOX_RESPONSE_MISMATCH")]
	ResponseMismatch,
}

impl HostOutboxError {
	/// Numeric registry code when this is a public protocol failure.
	pub(crate) const fn code(self) -> Option<u16> {
		match self {
			Self::WireSchemaInvalid => Some(100),
			Self::WireNonCanonical => Some(101),
			Self::WireVersionMismatch => Some(102),
			Self::Unavailable => Some(113),
			Self::Full => Some(114),
			Self::Corrupt => Some(115),
			Self::Expired => Some(116),
			Self::StateInvalid | Self::ResponseMismatch => None,
		}
	}
}

/// Durable lifecycle encoded in key two of `HostOutboxEntryV1`.
#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum HostOutboxStateV1 {
	/// Exact request and authority are durable and may be sent.
	Prepared = 0,
	/// A send was attempted; retry still uses the Prepared bytes.
	Sent = 1,
	/// Exact authenticated response and any successor are durable.
	ResponseInstalled = 2,
	/// Provider acknowledgement confirmation is durable.
	AckConfirmed = 3,
	/// Recovery ended and authority bytes were erased.
	Expired = 4,
	/// Corrupt ciphertext was moved aside without any send.
	Quarantined = 5,
}

impl TryFrom<u64> for HostOutboxStateV1 {
	type Error = HostOutboxError;

	fn try_from(value: u64) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::Prepared),
			1 => Ok(Self::Sent),
			2 => Ok(Self::ResponseInstalled),
			3 => Ok(Self::AckConfirmed),
			4 => Ok(Self::Expired),
			5 => Ok(Self::Quarantined),
			_ => Err(HostOutboxError::Corrupt),
		}
	}
}

/// Exact closed-map record from `origin-host-registry-v2.cddl`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxEntryV1 {
	pub(crate) outbox_id: [u8; 16],
	pub(crate) state: HostOutboxStateV1,
	pub(crate) exact_request_bytes: Vec<u8>,
	pub(crate) exact_authority_bytes: Vec<u8>,
	pub(crate) request_fingerprint: [u8; 32],
	pub(crate) request_id: [u8; 16],
	pub(crate) operation_id: [u8; 16],
	pub(crate) generation: u64,
	pub(crate) intended_cursor: u32,
	pub(crate) registry_hash: [u8; 32],
	pub(crate) genesis_hash: [u8; 32],
	pub(crate) negotiated_tuple: [u8; 32],
	pub(crate) provider_id: [u8; 32],
	pub(crate) provider_endpoint_hash: [u8; 32],
	pub(crate) expected_response_kind: u16,
	pub(crate) prior_response_hash: Option<[u8; 32]>,
	pub(crate) created_at: u64,
	pub(crate) authority_expires_at: u64,
	pub(crate) recover_until: u64,
	pub(crate) key_version: u32,
}

impl HostOutboxEntryV1 {
	/// Decode only the unique deterministic CBOR encoding of the closed map.
	pub(crate) fn decode(bytes: &[u8]) -> Result<Self, HostOutboxError> {
		if bytes.len() > MAX_ENCRYPTED_RECORD_BYTES {
			return Err(HostOutboxError::Corrupt);
		}
		let value: Value =
			ciborium::de::from_reader(bytes).map_err(|_| HostOutboxError::WireSchemaInvalid)?;
		let Value::Map(entries) = value else { return Err(HostOutboxError::WireSchemaInvalid) };
		let mut fields: [Option<Value>; 21] = array::from_fn(|_| None);
		for (key, value) in entries {
			let index: usize = value_u64(key)?.try_into().map_err(|_| HostOutboxError::Corrupt)?;
			if index > 20 || fields[index].replace(value).is_some() {
				return Err(HostOutboxError::Corrupt);
			}
		}
		for index in 0..=20 {
			if index != 16 && fields[index].is_none() {
				return Err(HostOutboxError::Corrupt);
			}
		}
		if value_u64(take(&mut fields, 0)?)? != u64::from(VERSION) {
			return Err(HostOutboxError::WireVersionMismatch);
		}
		let entry = Self {
			outbox_id: fixed_bytes(take(&mut fields, 1)?)?,
			state: value_u64(take(&mut fields, 2)?)?.try_into()?,
			exact_request_bytes: bounded_bytes(take(&mut fields, 3)?, 0, MAX_REQUEST_BYTES)?,
			exact_authority_bytes: bounded_bytes(take(&mut fields, 4)?, 0, MAX_AUTHORITY_BYTES)?,
			request_fingerprint: fixed_bytes(take(&mut fields, 5)?)?,
			request_id: fixed_bytes(take(&mut fields, 6)?)?,
			operation_id: fixed_bytes(take(&mut fields, 7)?)?,
			generation: value_u64(take(&mut fields, 8)?)?,
			intended_cursor: value_u64(take(&mut fields, 9)?)?
				.try_into()
				.map_err(|_| HostOutboxError::Corrupt)?,
			registry_hash: fixed_bytes(take(&mut fields, 10)?)?,
			genesis_hash: fixed_bytes(take(&mut fields, 11)?)?,
			negotiated_tuple: fixed_bytes(take(&mut fields, 12)?)?,
			provider_id: fixed_bytes(take(&mut fields, 13)?)?,
			provider_endpoint_hash: fixed_bytes(take(&mut fields, 14)?)?,
			expected_response_kind: value_u64(take(&mut fields, 15)?)?
				.try_into()
				.map_err(|_| HostOutboxError::Corrupt)?,
			prior_response_hash: fields[16].take().map(fixed_bytes).transpose()?,
			created_at: value_u64(take(&mut fields, 17)?)?,
			authority_expires_at: value_u64(take(&mut fields, 18)?)?,
			recover_until: value_u64(take(&mut fields, 19)?)?,
			key_version: value_u64(take(&mut fields, 20)?)?
				.try_into()
				.map_err(|_| HostOutboxError::Corrupt)?,
		};
		entry.validate()?;
		if entry.canonical_bytes() != bytes {
			return Err(HostOutboxError::WireNonCanonical);
		}
		Ok(entry)
	}

	/// Encode the unique deterministic CBOR map.
	pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
		let mut entries = vec![
			(0, uint(VERSION.into())),
			(1, bstr(&self.outbox_id)),
			(2, uint(self.state as u8 as u64)),
			(3, bstr(&self.exact_request_bytes)),
			(4, bstr(&self.exact_authority_bytes)),
			(5, bstr(&self.request_fingerprint)),
			(6, bstr(&self.request_id)),
			(7, bstr(&self.operation_id)),
			(8, uint(self.generation)),
			(9, uint(u64::from(self.intended_cursor))),
			(10, bstr(&self.registry_hash)),
			(11, bstr(&self.genesis_hash)),
			(12, bstr(&self.negotiated_tuple)),
			(13, bstr(&self.provider_id)),
			(14, bstr(&self.provider_endpoint_hash)),
			(15, uint(u64::from(self.expected_response_kind))),
		];
		if let Some(hash) = self.prior_response_hash {
			entries.push((16, bstr(&hash)));
		}
		entries.extend([
			(17, uint(self.created_at)),
			(18, uint(self.authority_expires_at)),
			(19, uint(self.recover_until)),
			(20, uint(u64::from(self.key_version))),
		]);
		map(entries)
	}

	fn validate(&self) -> Result<(), HostOutboxError> {
		let max_authority_expiry =
			self.created_at.checked_add(AUTHORITY_BLOCKS).ok_or(HostOutboxError::Corrupt)?;
		let min_recovery =
			self.created_at.checked_add(RECOVERY_BLOCKS).ok_or(HostOutboxError::Corrupt)?;
		let max_recovery = self
			.created_at
			.checked_add(MAX_RECOVERY_BLOCKS)
			.ok_or(HostOutboxError::Corrupt)?;
		let authority_valid = match self.state {
			HostOutboxStateV1::Prepared |
			HostOutboxStateV1::Sent |
			HostOutboxStateV1::ResponseInstalled =>
				!self.exact_authority_bytes.is_empty() &&
					self.request_fingerprint ==
						request_fingerprint(
							&self.exact_request_bytes,
							&self.exact_authority_bytes,
						),
			HostOutboxStateV1::AckConfirmed => self.exact_authority_bytes.is_empty(),
			HostOutboxStateV1::Expired | HostOutboxStateV1::Quarantined => false,
		};
		if self.exact_request_bytes.len() > MAX_REQUEST_BYTES ||
			self.exact_authority_bytes.len() > MAX_AUTHORITY_BYTES ||
			self.authority_expires_at < self.created_at ||
			self.authority_expires_at > max_authority_expiry ||
			self.recover_until < min_recovery ||
			self.recover_until > max_recovery ||
			!authority_valid
		{
			return Err(HostOutboxError::Corrupt);
		}
		Ok(())
	}
}

/// Fixed authority and chain binding for one host profile outbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxContextV1 {
	pub(crate) profile_id: [u8; 32],
	pub(crate) registry_hash: [u8; 32],
	pub(crate) genesis_hash: [u8; 32],
}

/// Versioned host-profile keys supplied by the OS or hardware keystore adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxKeyRingV1 {
	active_version: u32,
	keys: BTreeMap<u32, [u8; 32]>,
}

impl HostOutboxKeyRingV1 {
	pub(crate) fn new(
		active_version: u32,
		keys: BTreeMap<u32, [u8; 32]>,
	) -> Result<Self, HostOutboxError> {
		if !keys.contains_key(&active_version) || keys.is_empty() {
			return Err(HostOutboxError::Unavailable);
		}
		Ok(Self { active_version, keys })
	}

	fn active(&self) -> Result<(u32, [u8; 32]), HostOutboxError> {
		self.keys
			.get(&self.active_version)
			.copied()
			.map(|key| (self.active_version, key))
			.ok_or(HostOutboxError::Unavailable)
	}
}

/// Inputs fixed before provider-visible bytes can leave the host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrepareHostOutboxV1 {
	pub(crate) outbox_id: [u8; 16],
	pub(crate) exact_request_bytes: Vec<u8>,
	pub(crate) exact_authority_bytes: Vec<u8>,
	pub(crate) exact_payload_bytes: Option<Vec<u8>>,
	pub(crate) request_id: [u8; 16],
	pub(crate) operation_id: [u8; 16],
	pub(crate) generation: u64,
	pub(crate) intended_cursor: u32,
	pub(crate) negotiated_tuple: [u8; 32],
	pub(crate) provider_id: [u8; 32],
	pub(crate) provider_endpoint_hash: [u8; 32],
	pub(crate) expected_response_kind: u16,
	pub(crate) created_at: u64,
	pub(crate) authority_expires_at: u64,
}

/// Byte-exact material that may be retransmitted after durable preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxRetryV1 {
	pub(crate) request: Vec<u8>,
	pub(crate) authority: Vec<u8>,
	pub(crate) payload: Option<Vec<u8>>,
	pub(crate) fingerprint: [u8; 32],
}

/// Atomically installed provider response and continuation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxInstalledResponseV1 {
	pub(crate) response: Vec<u8>,
	pub(crate) response_hash: [u8; 32],
	/// Canonical `ResponseAckV1` bytes durably installed before any acknowledgement send.
	pub(crate) response_ack: Vec<u8>,
	pub(crate) successor_authority: Option<Vec<u8>>,
	pub(crate) successor_cursor: Option<u32>,
	pub(crate) terminal: bool,
	pub(crate) recover_until: u64,
}

/// Byte-exact acknowledgement that may be transmitted only after durable response installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxResponseAckV1 {
	pub(crate) bytes: Vec<u8>,
	pub(crate) response_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxBindingV1 {
	pub(crate) registry_hash: [u8; 32],
	pub(crate) genesis_hash: [u8; 32],
	pub(crate) negotiated_tuple: [u8; 32],
	pub(crate) provider_id: [u8; 32],
	pub(crate) provider_endpoint_hash: [u8; 32],
	pub(crate) request_id: [u8; 16],
	pub(crate) operation_id: [u8; 16],
	pub(crate) expected_response_kind: u16,
	pub(crate) intended_cursor: u32,
	pub(crate) generation: u64,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct LiveRecordV1 {
	entry_cbor: Vec<u8>,
	exact_payload_bytes: Option<Vec<u8>>,
	response: Option<Vec<u8>>,
	response_ack: Option<Vec<u8>>,
	successor_authority: Option<Vec<u8>>,
	successor_cursor: Option<u32>,
	ack_send_attempted: bool,
	terminal: bool,
	record_hash: [u8; 32],
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct TombstoneV1 {
	outbox_id: [u8; 16],
	operation_id: Option<[u8; 16]>,
	state: HostOutboxStateV1,
	request_fingerprint: [u8; 32],
	prior_response_hash: Option<[u8; 32]>,
	recover_until: u64,
	key_version: u32,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct UploadManifestV1 {
	operation_id: [u8; 16],
	exact_request_bytes: Vec<u8>,
	request_hash: [u8; 32],
	cid: Vec<u8>,
	object_len: u64,
	chunk_count: u32,
	chunk_ids: Vec<[u8; 16]>,
	chunk_commitment: [u8; 32],
	created_at: u64,
	recover_until: u64,
	key_version: u32,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct UploadChunkV1 {
	operation_id: [u8; 16],
	request_hash: [u8; 32],
	index: u32,
	exact_payload_bytes: Vec<u8>,
	payload_hash: [u8; 32],
	key_version: u32,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
enum DurableRecordV1 {
	Live(LiveRecordV1),
	Tombstone(TombstoneV1),
	UploadManifest(UploadManifestV1),
	UploadChunk(UploadChunkV1),
}

#[derive(Clone, Debug)]
struct LoadedRecordV1 {
	record: DurableRecordV1,
	encrypted_bytes: usize,
}

/// Deterministic atomic-write seams used by recovery tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostOutboxFault {
	BeforeUploadErase,
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
	BeforeGcRemove,
	AfterGcRemove,
	AfterGcDirectoryFsync,
}

/// File-backed encrypted host outbox kernel.
pub(crate) struct HostOutboxStoreV1 {
	root: PathBuf,
	context: HostOutboxContextV1,
	keys: HostOutboxKeyRingV1,
	records: RwLock<BTreeMap<[u8; 16], LoadedRecordV1>>,
	fault: RwLock<Option<HostOutboxFault>>,
	record_limit: usize,
	byte_limit: u64,
}

impl HostOutboxStoreV1 {
	pub(crate) fn context_binding(&self) -> ([u8; 32], [u8; 32]) {
		(self.context.registry_hash, self.context.genesis_hash)
	}

	pub(crate) fn binding(
		&self,
		outbox_id: [u8; 16],
	) -> Result<HostOutboxBindingV1, HostOutboxError> {
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		let live = live(records.get(&outbox_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let entry = decode_entry(live)?;
		Ok(HostOutboxBindingV1 {
			registry_hash: entry.registry_hash,
			genesis_hash: entry.genesis_hash,
			negotiated_tuple: entry.negotiated_tuple,
			provider_id: entry.provider_id,
			provider_endpoint_hash: entry.provider_endpoint_hash,
			request_id: entry.request_id,
			operation_id: entry.operation_id,
			expected_response_kind: entry.expected_response_kind,
			intended_cursor: entry.intended_cursor,
			generation: entry.generation,
		})
	}

	/// Open and authenticate every durable entry. One corrupt entry is quarantined and fails open.
	pub(crate) fn open(
		root: impl AsRef<Path>,
		context: HostOutboxContextV1,
		keys: HostOutboxKeyRingV1,
	) -> Result<Self, HostOutboxError> {
		Self::open_with_limits(root, context, keys, MAX_RECORDS, MAX_TOTAL_ENCRYPTED_BYTES)
	}

	pub(crate) fn open_with_limits(
		root: impl AsRef<Path>,
		context: HostOutboxContextV1,
		keys: HostOutboxKeyRingV1,
		record_limit: usize,
		byte_limit: u64,
	) -> Result<Self, HostOutboxError> {
		if record_limit == 0 ||
			record_limit > MAX_RECORDS ||
			byte_limit == 0 ||
			byte_limit > MAX_TOTAL_ENCRYPTED_BYTES
		{
			return Err(HostOutboxError::Unavailable);
		}
		let root = root.as_ref().join(ROOT);
		let quarantine = root.join(QUARANTINE);
		ensure_directory(&root)?;
		ensure_directory(&quarantine)?;
		let mut records = BTreeMap::new();
		let mut loaded_bytes = 0u64;
		let mut removed_temporary = false;
		for item in fs::read_dir(&root).map_err(|_| HostOutboxError::Unavailable)? {
			let item = item.map_err(|_| HostOutboxError::Unavailable)?;
			let file_type = item.file_type().map_err(|_| HostOutboxError::Unavailable)?;
			let name = item.file_name().into_string().map_err(|_| HostOutboxError::Corrupt)?;
			if file_type.is_dir() {
				if name == QUARANTINE {
					continue;
				}
				return Err(HostOutboxError::Corrupt);
			}
			if file_type.is_symlink() {
				Self::quarantine_file(&item.path(), &quarantine)?;
				return Err(HostOutboxError::Corrupt);
			}
			if name.contains(".tmp-") {
				fs::remove_file(item.path()).map_err(|_| HostOutboxError::Unavailable)?;
				removed_temporary = true;
				continue;
			}
			if !name.ends_with(EXTENSION) {
				Self::quarantine_file(&item.path(), &quarantine)?;
				return Err(HostOutboxError::Corrupt);
			}
			let id = decode_id(&name[..name.len() - EXTENSION.len()])?;
			if records.len() >= record_limit {
				return Err(HostOutboxError::Corrupt);
			}
			let file = File::open(item.path()).map_err(|_| HostOutboxError::Unavailable)?;
			let encrypted_len = file.metadata().map_err(|_| HostOutboxError::Unavailable)?.len();
			if encrypted_len > MAX_ENCRYPTED_RECORD_BYTES as u64 {
				drop(file);
				Self::quarantine_file(&item.path(), &quarantine)?;
				return Err(HostOutboxError::Corrupt);
			}
			if loaded_bytes.checked_add(encrypted_len).ok_or(HostOutboxError::Corrupt)? > byte_limit
			{
				return Err(HostOutboxError::Corrupt);
			}
			let mut bytes = Vec::with_capacity(encrypted_len as usize);
			let mut bounded = file.take(MAX_ENCRYPTED_RECORD_BYTES as u64 + 1);
			bounded.read_to_end(&mut bytes).map_err(|_| HostOutboxError::Unavailable)?;
			drop(bounded);
			if bytes.len() > MAX_ENCRYPTED_RECORD_BYTES {
				Self::quarantine_file(&item.path(), &quarantine)?;
				return Err(HostOutboxError::Corrupt);
			}
			loaded_bytes =
				loaded_bytes.checked_add(bytes.len() as u64).ok_or(HostOutboxError::Corrupt)?;
			if loaded_bytes > byte_limit {
				return Err(HostOutboxError::Corrupt);
			}
			let loaded = match decrypt_record(&bytes, id, &context, &keys) {
				Ok(record) => record,
				Err(error) => {
					Self::quarantine_file(&item.path(), &quarantine)?;
					return Err(error);
				},
			};
			if records.insert(id, loaded).is_some() {
				return Err(HostOutboxError::Corrupt);
			}
		}
		if removed_temporary {
			sync_dir(&root)?;
		}
		if reconcile_upload_spools(&root, &mut records)? {
			sync_dir(&root)?;
		}
		validate_unique_live_operation_generations(&records)?;
		if records.len() > record_limit || total_bytes(&records)? > byte_limit {
			return Err(HostOutboxError::Corrupt);
		}
		Ok(Self {
			root,
			context,
			keys,
			records: RwLock::new(records),
			fault: RwLock::new(None),
			record_limit,
			byte_limit,
		})
	}

	/// Persist a byte-exact request and authority before any transport may send them.
	pub(crate) fn prepare(
		&self,
		input: PrepareHostOutboxV1,
		nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, HostOutboxError> {
		if input.generation != 0 {
			return Err(HostOutboxError::StateInvalid);
		}
		let (key_version, _) = self.keys.active()?;
		let recover_until = input
			.authority_expires_at
			.checked_add(RECOVERY_BLOCKS)
			.ok_or(HostOutboxError::Corrupt)?;
		let fingerprint =
			request_fingerprint(&input.exact_request_bytes, &input.exact_authority_bytes);
		validate_exact_payload(
			&input.exact_request_bytes,
			input.operation_id,
			input.generation,
			input.intended_cursor,
			input.exact_payload_bytes.as_deref(),
		)?;
		let exact_payload_bytes = input.exact_payload_bytes;
		let entry = HostOutboxEntryV1 {
			outbox_id: input.outbox_id,
			state: HostOutboxStateV1::Prepared,
			exact_request_bytes: input.exact_request_bytes,
			exact_authority_bytes: input.exact_authority_bytes,
			request_fingerprint: fingerprint,
			request_id: input.request_id,
			operation_id: input.operation_id,
			generation: input.generation,
			intended_cursor: input.intended_cursor,
			registry_hash: self.context.registry_hash,
			genesis_hash: self.context.genesis_hash,
			negotiated_tuple: input.negotiated_tuple,
			provider_id: input.provider_id,
			provider_endpoint_hash: input.provider_endpoint_hash,
			expected_response_kind: input.expected_response_kind,
			prior_response_hash: None,
			created_at: input.created_at,
			authority_expires_at: input.authority_expires_at,
			recover_until,
			key_version,
		};
		entry.validate()?;
		let record = live_record(
			entry.clone(),
			exact_payload_bytes.clone(),
			None,
			None,
			None,
			None,
			false,
			false,
		);
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		if records.contains_key(&entry.outbox_id) {
			return Err(HostOutboxError::StateInvalid);
		}
		ensure_live_operation_generation_available(&records, entry.operation_id, entry.generation)?;
		self.persist_new(&records, entry.outbox_id, record, nonce)?;
		let bytes = fs::metadata(self.path(entry.outbox_id))
			.map_err(|_| HostOutboxError::Unavailable)?
			.len()
			.try_into()
			.map_err(|_| HostOutboxError::Full)?;
		records.insert(
			entry.outbox_id,
			LoadedRecordV1 {
				record: live_record(
					entry.clone(),
					exact_payload_bytes.clone(),
					None,
					None,
					None,
					None,
					false,
					false,
				),
				encrypted_bytes: bytes,
			},
		);
		Ok(HostOutboxRetryV1 {
			request: entry.exact_request_bytes,
			authority: entry.exact_authority_bytes,
			payload: exact_payload_bytes,
			fingerprint,
		})
	}

	/// Commit every canonical PUT chunk under the same encrypted record and byte ceilings before
	/// the initial provider request can be sent. The manifest is the last durable commit marker.
	pub(crate) fn stage_upload(
		&self,
		exact_request_bytes: &[u8],
		operation_id: [u8; 16],
		exact_chunks: Vec<Vec<u8>>,
		created_at: u64,
		authority_expires_at: u64,
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		let binding = put_request_binding(exact_request_bytes, operation_id)?;
		let recover_until = authority_expires_at
			.checked_add(RECOVERY_BLOCKS)
			.ok_or(HostOutboxError::Corrupt)?;
		if authority_expires_at < created_at ||
			authority_expires_at >
				created_at.checked_add(AUTHORITY_BLOCKS).ok_or(HostOutboxError::Corrupt)?
		{
			return Err(HostOutboxError::Corrupt);
		}
		if exact_chunks.len() != binding.chunk_count as usize {
			return Err(HostOutboxError::Corrupt);
		}
		let request_hash: [u8; 32] = Sha256::digest(exact_request_bytes).into();
		let key_version = self.keys.active()?.0;
		let mut chunks = Vec::with_capacity(exact_chunks.len());
		let mut commitment = Sha256::new();
		for (position, exact_payload_bytes) in exact_chunks.into_iter().enumerate() {
			let index: u32 = position.try_into().map_err(|_| HostOutboxError::Full)?;
			let payload = parse_transfer_chunk(&exact_payload_bytes)?;
			let expected_len =
				expected_upload_chunk_len(binding.object_len, index, binding.chunk_count)?;
			if payload.operation_id != operation_id ||
				payload.index != index ||
				payload.bytes_len != expected_len
			{
				return Err(HostOutboxError::Corrupt);
			}
			let payload_hash: [u8; 32] = Sha256::digest(&exact_payload_bytes).into();
			commitment.update(payload_hash);
			chunks.push(UploadChunkV1 {
				operation_id,
				request_hash,
				index,
				exact_payload_bytes,
				payload_hash,
				key_version,
			});
		}
		let chunk_ids = (0..binding.chunk_count)
			.map(|index| upload_chunk_id(operation_id, index))
			.collect::<Vec<_>>();
		let manifest = UploadManifestV1 {
			operation_id,
			exact_request_bytes: exact_request_bytes.to_vec(),
			request_hash,
			cid: binding.cid,
			object_len: binding.object_len,
			chunk_count: binding.chunk_count,
			chunk_ids: chunk_ids.clone(),
			chunk_commitment: commitment.finalize().into(),
			created_at,
			recover_until,
			key_version,
		};
		let manifest_id = upload_manifest_id(operation_id);
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		if let Some(existing) = records.get(&manifest_id) {
			let DurableRecordV1::UploadManifest(existing_manifest) = &existing.record else {
				return Err(HostOutboxError::StateInvalid);
			};
			if existing_manifest != &manifest {
				return Err(HostOutboxError::StateInvalid);
			}
			for (id, expected) in chunk_ids.iter().zip(&chunks) {
				let Some(loaded) = records.get(id) else { return Err(HostOutboxError::Corrupt) };
				if loaded.record != DurableRecordV1::UploadChunk(expected.clone()) {
					return Err(HostOutboxError::StateInvalid);
				}
			}
			return Ok(());
		}
		if records.contains_key(&manifest_id) || chunk_ids.iter().any(|id| records.contains_key(id))
		{
			return Err(HostOutboxError::StateInvalid);
		}
		let mut pending = Vec::with_capacity(chunks.len() + 1);
		for chunk in chunks {
			let index = chunk.index;
			let id = upload_chunk_id(operation_id, index);
			let record = DurableRecordV1::UploadChunk(chunk);
			let bytes = encrypt_record(
				&record,
				id,
				&self.context,
				&self.keys,
				upload_nonce(nonce, Some(index)),
			)?;
			pending.push((id, record, bytes));
		}
		let manifest_record = DurableRecordV1::UploadManifest(manifest);
		let manifest_bytes = encrypt_record(
			&manifest_record,
			manifest_id,
			&self.context,
			&self.keys,
			upload_nonce(nonce, None),
		)?;
		pending.push((manifest_id, manifest_record, manifest_bytes));
		let new_bytes = pending.iter().try_fold(0u64, |total, (_, _, bytes)| {
			total.checked_add(bytes.len() as u64).ok_or(HostOutboxError::Full)
		})?;
		if records.len().checked_add(pending.len()).ok_or(HostOutboxError::Full)? >
			self.record_limit ||
			total_bytes(&records)?.checked_add(new_bytes).ok_or(HostOutboxError::Full)? >
				self.byte_limit
		{
			return Err(HostOutboxError::Full);
		}
		let mut written = Vec::new();
		for (id, record, bytes) in pending {
			if let Err(error) = self.persist_bytes(id, &bytes, true) {
				let durable = self.path(id).exists();
				if durable {
					records.insert(id, LoadedRecordV1 { record, encrypted_bytes: bytes.len() });
					written.push(id);
				}
				let temporary =
					self.root.join(format!("{}.tmp-{}", hex::encode(id), std::process::id()));
				if temporary.exists() {
					fs::remove_file(temporary).map_err(|_| HostOutboxError::Unavailable)?;
				}
				if id == manifest_id && durable {
					return Err(error);
				}
				for written_id in written {
					if self.path(written_id).exists() {
						fs::remove_file(self.path(written_id))
							.map_err(|_| HostOutboxError::Unavailable)?;
					}
					records.remove(&written_id);
				}
				sync_dir(&self.root)?;
				return Err(error);
			}
			records.insert(id, LoadedRecordV1 { record, encrypted_bytes: bytes.len() });
			written.push(id);
		}
		Ok(())
	}

	/// Resolve the exact committed PUT payload for one continuation cursor. The finalize cursor
	/// returns no payload; no caller can substitute bytes after initial admission.
	pub(crate) fn staged_upload_payload(
		&self,
		exact_request_bytes: &[u8],
		operation_id: [u8; 16],
		cursor: u32,
	) -> Result<Option<Vec<u8>>, HostOutboxError> {
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		if !has_retryable_upload_operation(&records, operation_id)? {
			return Err(HostOutboxError::StateInvalid);
		}
		let manifest_id = upload_manifest_id(operation_id);
		let loaded = records.get(&manifest_id).ok_or(HostOutboxError::StateInvalid)?;
		let DurableRecordV1::UploadManifest(manifest) = &loaded.record else {
			return Err(HostOutboxError::Corrupt);
		};
		if manifest.exact_request_bytes != exact_request_bytes || cursor > manifest.chunk_count {
			return Err(HostOutboxError::StateInvalid);
		}
		if cursor == manifest.chunk_count {
			return Ok(None);
		}
		let id = manifest.chunk_ids.get(cursor as usize).ok_or(HostOutboxError::Corrupt)?;
		let loaded = records.get(id).ok_or(HostOutboxError::Corrupt)?;
		let DurableRecordV1::UploadChunk(chunk) = &loaded.record else {
			return Err(HostOutboxError::Corrupt);
		};
		Ok(Some(chunk.exact_payload_bytes.clone()))
	}

	/// Erase a committed upload and all authenticated chunk records. The manifest commit marker is
	/// removed first, so a crash can only leave uncommitted chunks that open-time recovery deletes.
	pub(crate) fn erase_upload(&self, operation_id: [u8; 16]) -> Result<(), HostOutboxError> {
		self.trip(HostOutboxFault::BeforeUploadErase)?;
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let manifest_id = upload_manifest_id(operation_id);
		let chunk_ids = match records.get(&manifest_id) {
			Some(LoadedRecordV1 { record: DurableRecordV1::UploadManifest(manifest), .. }) =>
				manifest.chunk_ids.clone(),
			Some(_) => return Err(HostOutboxError::Corrupt),
			None => records
				.iter()
				.filter_map(|(id, loaded)| match &loaded.record {
					DurableRecordV1::UploadChunk(chunk) if chunk.operation_id == operation_id =>
						Some(*id),
					_ => None,
				})
				.collect(),
		};
		if records.contains_key(&manifest_id) {
			fs::remove_file(self.path(manifest_id)).map_err(|_| HostOutboxError::Unavailable)?;
			records.remove(&manifest_id);
			sync_dir(&self.root)?;
		}
		for id in chunk_ids {
			fs::remove_file(self.path(id)).map_err(|_| HostOutboxError::Unavailable)?;
			records.remove(&id);
		}
		sync_dir(&self.root)
	}

	/// Return only the exact durable Prepared bytes and never regenerate authority.
	pub(crate) fn retry_request(
		&self,
		outbox_id: [u8; 16],
		finalized: u64,
	) -> Result<HostOutboxRetryV1, HostOutboxError> {
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		let live = live(records.get(&outbox_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let entry = decode_entry(live)?;
		if finalized >= entry.recover_until {
			return Err(HostOutboxError::Expired);
		}
		if !matches!(entry.state, HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent) {
			return Err(HostOutboxError::StateInvalid);
		}
		Ok(HostOutboxRetryV1 {
			request: entry.exact_request_bytes,
			authority: entry.exact_authority_bytes,
			payload: live.exact_payload_bytes.clone(),
			fingerprint: entry.request_fingerprint,
		})
	}

	/// Persist the advisory Sent state; the exact Prepared bytes remain authoritative.
	pub(crate) fn mark_sent(
		&self,
		outbox_id: [u8; 16],
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		self.rewrite_live(outbox_id, nonce, |entry, payload, _, _, _, _, _, _| {
			if entry.state != HostOutboxStateV1::Prepared {
				return Err(HostOutboxError::StateInvalid);
			}
			entry.state = HostOutboxStateV1::Sent;
			Ok((payload, None, None, None, None, false, false))
		})
	}

	/// Replace a live request with its byte-exact terminal cancel before the cancel may be sent.
	pub(crate) fn prepare_cancel(
		&self,
		outbox_id: [u8; 16],
		exact_cancel_bytes: Vec<u8>,
		next_sequence: u32,
		nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, HostOutboxError> {
		if exact_cancel_bytes.is_empty() || exact_cancel_bytes.len() > MAX_REQUEST_BYTES {
			return Err(HostOutboxError::Corrupt);
		}
		let cancel = exact_cancel_bytes.clone();
		self.rewrite_live(
			outbox_id,
			nonce,
			move |entry, _, response, ack, successor, cursor, attempted, terminal| {
				if !matches!(entry.state, HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent) ||
					response.is_some() ||
					ack.is_some() || terminal
				{
					return Err(HostOutboxError::StateInvalid);
				}
				entry.exact_request_bytes = cancel;
				entry.request_fingerprint =
					request_fingerprint(&entry.exact_request_bytes, &entry.exact_authority_bytes);
				entry.intended_cursor = next_sequence;
				entry.expected_response_kind = 4;
				entry.state = HostOutboxStateV1::Prepared;
				Ok((None, None, None, successor, cursor, attempted, false))
			},
		)?;
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		let live = live(records.get(&outbox_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let entry = decode_entry(live)?;
		Ok(HostOutboxRetryV1 {
			request: entry.exact_request_bytes,
			authority: entry.exact_authority_bytes,
			payload: None,
			fingerprint: entry.request_fingerprint,
		})
	}

	/// Install exact authenticated response bytes before an acknowledgement may be sent.
	pub(crate) fn install_response(
		&self,
		outbox_id: [u8; 16],
		response: Vec<u8>,
		successor_authority: Option<Vec<u8>>,
		successor_cursor: Option<u32>,
		terminal_block: Option<u64>,
		nonce: [u8; 24],
	) -> Result<[u8; 32], HostOutboxError> {
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		let terminal_recover_until = terminal_block
			.map(|block| block.checked_add(RECOVERY_BLOCKS).ok_or(HostOutboxError::Corrupt))
			.transpose()?;
		if successor_authority.is_some() != successor_cursor.is_some() {
			return Err(HostOutboxError::StateInvalid);
		}
		if let Ok(installed) = self.installed_response(outbox_id) {
			return if installed.response == response &&
				installed.successor_authority == successor_authority &&
				installed.successor_cursor == successor_cursor &&
				installed.terminal == terminal_block.is_some() &&
				terminal_recover_until.is_none_or(|until| installed.recover_until == until)
			{
				Ok(response_hash)
			} else {
				Err(HostOutboxError::ResponseMismatch)
			};
		}
		self.rewrite_live(outbox_id, nonce, move |entry, payload, _, _, _, _, _, _| {
			if !matches!(entry.state, HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent) {
				return Err(HostOutboxError::StateInvalid);
			}
			if terminal_block
				.is_some_and(|block| block < entry.created_at || block > entry.authority_expires_at)
			{
				return Err(HostOutboxError::StateInvalid);
			}
			if successor_authority
				.as_ref()
				.is_some_and(|bytes| bytes.is_empty() || bytes.len() > MAX_AUTHORITY_BYTES)
			{
				return Err(HostOutboxError::Corrupt);
			}
			if terminal_block.is_some() && successor_authority.is_some() {
				return Err(HostOutboxError::StateInvalid);
			}
			entry.state = HostOutboxStateV1::ResponseInstalled;
			entry.prior_response_hash = Some(response_hash);
			let response_ack = response_ack_bytes(
				entry.request_id,
				entry.operation_id,
				entry.generation,
				response_hash,
			);
			if let Some(until) = terminal_recover_until {
				entry.recover_until = until;
			}
			Ok((
				payload,
				Some(response),
				Some(response_ack),
				successor_authority,
				successor_cursor,
				false,
				terminal_block.is_some(),
			))
		})?;
		Ok(response_hash)
	}

	/// Return exact installed response material after restart.
	pub(crate) fn installed_response(
		&self,
		outbox_id: [u8; 16],
	) -> Result<HostOutboxInstalledResponseV1, HostOutboxError> {
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		let live = live(records.get(&outbox_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let entry = decode_entry(live)?;
		if !matches!(
			entry.state,
			HostOutboxStateV1::ResponseInstalled | HostOutboxStateV1::AckConfirmed
		) {
			return Err(HostOutboxError::StateInvalid);
		}
		let response = live.response.clone().ok_or(HostOutboxError::Corrupt)?;
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		if entry.prior_response_hash != Some(response_hash) {
			return Err(HostOutboxError::Corrupt);
		}
		let response_ack = live.response_ack.clone().ok_or(HostOutboxError::Corrupt)?;
		if response_ack !=
			response_ack_bytes(
				entry.request_id,
				entry.operation_id,
				entry.generation,
				response_hash,
			) {
			return Err(HostOutboxError::Corrupt);
		}
		Ok(HostOutboxInstalledResponseV1 {
			response,
			response_hash,
			response_ack,
			successor_authority: live.successor_authority.clone(),
			successor_cursor: live.successor_cursor,
			terminal: live.terminal,
			recover_until: entry.recover_until,
		})
	}

	/// Return the exact canonical acknowledgement committed by `install_response`.
	pub(crate) fn retry_response_ack(
		&self,
		outbox_id: [u8; 16],
	) -> Result<HostOutboxResponseAckV1, HostOutboxError> {
		let installed = self.installed_response(outbox_id)?;
		Ok(HostOutboxResponseAckV1 {
			bytes: installed.response_ack,
			response_hash: installed.response_hash,
		})
	}

	/// Report whether this exact response already has a durable provider confirmation. This lets
	/// restart cleanup continue without replaying `mark_ack_sent`, whose transition is
	/// intentionally restricted to `ResponseInstalled`.
	pub(crate) fn is_ack_confirmed(
		&self,
		outbox_id: [u8; 16],
		response_hash: [u8; 32],
	) -> Result<bool, HostOutboxError> {
		let records = self.records.read().map_err(|_| HostOutboxError::Unavailable)?;
		let live = live(records.get(&outbox_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let entry = decode_entry(live)?;
		if entry.prior_response_hash != Some(response_hash) {
			return Err(HostOutboxError::ResponseMismatch);
		}
		Ok(entry.state == HostOutboxStateV1::AckConfirmed)
	}

	/// Persist the advisory acknowledgement-send attempt without changing its durable bytes.
	pub(crate) fn mark_ack_sent(
		&self,
		outbox_id: [u8; 16],
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		self.rewrite_live(
			outbox_id,
			nonce,
			|entry, payload, response, ack, successor, cursor, _, terminal| {
				if entry.state != HostOutboxStateV1::ResponseInstalled {
					return Err(HostOutboxError::StateInvalid);
				}
				Ok((payload, response, ack, successor, cursor, true, terminal))
			},
		)
	}

	/// Persist provider acknowledgement confirmation before authority cleanup or GC.
	pub(crate) fn confirm_ack(
		&self,
		outbox_id: [u8; 16],
		response_hash: [u8; 32],
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let loaded = records.get(&outbox_id).cloned().ok_or(HostOutboxError::StateInvalid)?;
		if let DurableRecordV1::Tombstone(tombstone) = &loaded.record {
			return if tombstone.state == HostOutboxStateV1::AckConfirmed &&
				tombstone.prior_response_hash == Some(response_hash)
			{
				Ok(())
			} else {
				Err(HostOutboxError::ResponseMismatch)
			};
		}
		let existing = live(&loaded)?.clone();
		let mut entry = decode_entry(&existing)?;
		if entry.state == HostOutboxStateV1::AckConfirmed &&
			entry.prior_response_hash == Some(response_hash)
		{
			return Ok(());
		}
		if entry.state != HostOutboxStateV1::ResponseInstalled ||
			entry.prior_response_hash != Some(response_hash)
		{
			return Err(HostOutboxError::ResponseMismatch);
		}
		let active_version = self.keys.active()?.0;
		entry.state = HostOutboxStateV1::AckConfirmed;
		entry.key_version = active_version;
		entry.exact_authority_bytes.clear();
		entry.validate()?;
		let record = live_record(
			entry,
			None,
			existing.response,
			existing.response_ack,
			existing.successor_authority,
			existing.successor_cursor,
			existing.ack_send_attempted,
			existing.terminal,
		);
		validate_durable_record(&record, outbox_id, &self.context, active_version)?;
		let encrypted_bytes = self.persist_replace(&records, outbox_id, record.clone(), nonce)?;
		records.insert(outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		Ok(())
	}

	/// Prepare generation N+1 from the exact successor installed for an acknowledged predecessor.
	/// The returned bytes are the only continuation bytes allowed to reach the transport.
	pub(crate) fn prepare_successor(
		&self,
		predecessor_id: [u8; 16],
		input: PrepareHostOutboxV1,
		nonce: [u8; 24],
	) -> Result<HostOutboxRetryV1, HostOutboxError> {
		let (key_version, _) = self.keys.active()?;
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let predecessor = live(records.get(&predecessor_id).ok_or(HostOutboxError::StateInvalid)?)?;
		let predecessor_entry = decode_entry(predecessor)?;
		if predecessor_entry.state != HostOutboxStateV1::AckConfirmed || predecessor.terminal {
			return Err(HostOutboxError::StateInvalid);
		}
		let successor_authority =
			predecessor.successor_authority.as_ref().ok_or(HostOutboxError::StateInvalid)?;
		let successor_cursor = predecessor.successor_cursor.ok_or(HostOutboxError::StateInvalid)?;
		let successor_generation = predecessor_entry
			.generation
			.checked_add(1)
			.ok_or(HostOutboxError::StateInvalid)?;
		if input.exact_authority_bytes != *successor_authority ||
			input.operation_id != predecessor_entry.operation_id ||
			input.generation != successor_generation ||
			input.intended_cursor != successor_cursor ||
			input.negotiated_tuple != predecessor_entry.negotiated_tuple ||
			input.provider_id != predecessor_entry.provider_id ||
			input.provider_endpoint_hash != predecessor_entry.provider_endpoint_hash ||
			input.expected_response_kind != predecessor_entry.expected_response_kind
		{
			return Err(HostOutboxError::StateInvalid);
		}
		let recover_until = input
			.authority_expires_at
			.checked_add(RECOVERY_BLOCKS)
			.ok_or(HostOutboxError::Corrupt)?;
		let fingerprint =
			request_fingerprint(&input.exact_request_bytes, &input.exact_authority_bytes);
		validate_exact_payload(
			&input.exact_request_bytes,
			input.operation_id,
			input.generation,
			input.intended_cursor,
			input.exact_payload_bytes.as_deref(),
		)?;
		if let Some(existing) = records.get(&input.outbox_id) {
			let existing = live(existing)?;
			let entry = decode_entry(existing)?;
			if entry.state == HostOutboxStateV1::Prepared &&
				entry.exact_request_bytes == input.exact_request_bytes &&
				entry.exact_authority_bytes == input.exact_authority_bytes &&
				existing.exact_payload_bytes == input.exact_payload_bytes &&
				entry.request_fingerprint == fingerprint &&
				entry.request_id == input.request_id &&
				entry.operation_id == input.operation_id &&
				entry.generation == input.generation &&
				entry.intended_cursor == input.intended_cursor &&
				entry.registry_hash == self.context.registry_hash &&
				entry.genesis_hash == self.context.genesis_hash &&
				entry.negotiated_tuple == input.negotiated_tuple &&
				entry.provider_id == input.provider_id &&
				entry.provider_endpoint_hash == input.provider_endpoint_hash &&
				entry.expected_response_kind == input.expected_response_kind &&
				entry.prior_response_hash == predecessor_entry.prior_response_hash &&
				entry.created_at == input.created_at &&
				entry.authority_expires_at == input.authority_expires_at &&
				entry.recover_until == recover_until
			{
				return Ok(HostOutboxRetryV1 {
					request: entry.exact_request_bytes,
					authority: entry.exact_authority_bytes,
					payload: existing.exact_payload_bytes.clone(),
					fingerprint: entry.request_fingerprint,
				});
			}
			return Err(HostOutboxError::StateInvalid);
		}
		ensure_live_operation_generation_available(&records, input.operation_id, input.generation)?;
		let entry = HostOutboxEntryV1 {
			outbox_id: input.outbox_id,
			state: HostOutboxStateV1::Prepared,
			exact_request_bytes: input.exact_request_bytes,
			exact_authority_bytes: input.exact_authority_bytes,
			request_fingerprint: fingerprint,
			request_id: input.request_id,
			operation_id: input.operation_id,
			generation: input.generation,
			intended_cursor: input.intended_cursor,
			registry_hash: self.context.registry_hash,
			genesis_hash: self.context.genesis_hash,
			negotiated_tuple: input.negotiated_tuple,
			provider_id: input.provider_id,
			provider_endpoint_hash: input.provider_endpoint_hash,
			expected_response_kind: input.expected_response_kind,
			prior_response_hash: predecessor_entry.prior_response_hash,
			created_at: input.created_at,
			authority_expires_at: input.authority_expires_at,
			recover_until,
			key_version,
		};
		entry.validate()?;
		let exact_payload_bytes = input.exact_payload_bytes;
		let record = live_record(
			entry.clone(),
			exact_payload_bytes.clone(),
			None,
			None,
			None,
			None,
			false,
			false,
		);
		self.persist_new(&records, entry.outbox_id, record.clone(), nonce)?;
		let encrypted_bytes = fs::metadata(self.path(entry.outbox_id))
			.map_err(|_| HostOutboxError::Unavailable)?
			.len()
			.try_into()
			.map_err(|_| HostOutboxError::Full)?;
		records.insert(entry.outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		Ok(HostOutboxRetryV1 {
			request: entry.exact_request_bytes,
			authority: entry.exact_authority_bytes,
			payload: exact_payload_bytes,
			fingerprint,
		})
	}

	/// Erase an acknowledged body only after terminal durability or a durable N+1 Prepared entry.
	pub(crate) fn compact_acknowledged(
		&self,
		outbox_id: [u8; 16],
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let loaded = records.get(&outbox_id).cloned().ok_or(HostOutboxError::StateInvalid)?;
		if matches!(&loaded.record, DurableRecordV1::Tombstone(_)) {
			return Ok(());
		}
		let existing = live(&loaded)?;
		let entry = decode_entry(existing)?;
		if entry.state != HostOutboxStateV1::AckConfirmed {
			return Err(HostOutboxError::StateInvalid);
		}
		if !existing.terminal && !has_durable_successor(&records, &entry, existing)? {
			return Err(HostOutboxError::StateInvalid);
		}
		let record = DurableRecordV1::Tombstone(TombstoneV1 {
			outbox_id,
			operation_id: Some(entry.operation_id),
			state: HostOutboxStateV1::AckConfirmed,
			request_fingerprint: entry.request_fingerprint,
			prior_response_hash: entry.prior_response_hash,
			recover_until: entry.recover_until,
			key_version: self.keys.active()?.0,
		});
		let encrypted_bytes = self.persist_replace(&records, outbox_id, record.clone(), nonce)?;
		records.insert(outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		Ok(())
	}

	/// Erase live authority at the recovery bound and retain only a non-authorizing tombstone.
	pub(crate) fn expire(
		&self,
		outbox_id: [u8; 16],
		finalized: u64,
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let loaded = records.get(&outbox_id).cloned().ok_or(HostOutboxError::StateInvalid)?;
		let live = live(&loaded)?;
		let entry = decode_entry(live)?;
		let operation_id = entry.operation_id;
		if finalized < entry.recover_until {
			return Err(HostOutboxError::StateInvalid);
		}
		let tombstone = TombstoneV1 {
			outbox_id,
			operation_id: Some(entry.operation_id),
			state: HostOutboxStateV1::Expired,
			request_fingerprint: entry.request_fingerprint,
			prior_response_hash: entry.prior_response_hash,
			recover_until: entry.recover_until,
			key_version: self.keys.active()?.0,
		};
		let record = DurableRecordV1::Tombstone(tombstone);
		let encrypted_bytes = self.persist_replace(&records, outbox_id, record.clone(), nonce)?;
		records.insert(outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		drop(records);
		self.erase_upload(operation_id)
	}

	/// Remove only terminal/expired records whose full recovery window has closed.
	pub(crate) fn gc(&self, finalized: u64, limit: usize) -> Result<usize, HostOutboxError> {
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let mut selected = Vec::new();
		for (id, loaded) in records.iter() {
			let DurableRecordV1::UploadManifest(manifest) = &loaded.record else { continue };
			let group_len =
				manifest.chunk_ids.len().checked_add(1).ok_or(HostOutboxError::Corrupt)?;
			if finalized >= manifest.recover_until &&
				!has_live_operation(&records, manifest.operation_id)? &&
				selected.len().checked_add(group_len).is_some_and(|next| next <= limit)
			{
				selected.push(*id);
				selected.extend(manifest.chunk_ids.iter().copied());
			}
		}
		for (id, loaded) in records.iter() {
			if selected.len() >= limit {
				break;
			}
			if selected.contains(id) {
				continue;
			}
			let eligible = match &loaded.record {
				DurableRecordV1::Tombstone(tombstone) => finalized >= tombstone.recover_until,
				DurableRecordV1::Live(record) => {
					let entry = decode_entry(record)?;
					record.terminal &&
						entry.state == HostOutboxStateV1::AckConfirmed &&
						finalized >= entry.recover_until
				},
				DurableRecordV1::UploadManifest(_) | DurableRecordV1::UploadChunk(_) => false,
			};
			if eligible {
				selected.push(*id);
			}
		}
		for id in &selected {
			self.trip(HostOutboxFault::BeforeGcRemove)?;
			fs::remove_file(self.path(*id)).map_err(|_| HostOutboxError::Unavailable)?;
			self.trip(HostOutboxFault::AfterGcRemove)?;
		}
		if !selected.is_empty() {
			sync_dir(&self.root)?;
			self.trip(HostOutboxFault::AfterGcDirectoryFsync)?;
		}
		for id in &selected {
			records.remove(id);
		}
		Ok(selected.len())
	}

	#[doc(hidden)]
	pub(crate) fn inject_fault_once(&self, fault: HostOutboxFault) -> Result<(), HostOutboxError> {
		*self.fault.write().map_err(|_| HostOutboxError::Unavailable)? = Some(fault);
		Ok(())
	}

	fn rewrite_live<F>(
		&self,
		outbox_id: [u8; 16],
		nonce: [u8; 24],
		transition: F,
	) -> Result<(), HostOutboxError>
	where
		F: FnOnce(
			&mut HostOutboxEntryV1,
			Option<Vec<u8>>,
			Option<Vec<u8>>,
			Option<Vec<u8>>,
			Option<Vec<u8>>,
			Option<u32>,
			bool,
			bool,
		) -> Result<
			(
				Option<Vec<u8>>,
				Option<Vec<u8>>,
				Option<Vec<u8>>,
				Option<Vec<u8>>,
				Option<u32>,
				bool,
				bool,
			),
			HostOutboxError,
		>,
	{
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let loaded = records.get(&outbox_id).cloned().ok_or(HostOutboxError::StateInvalid)?;
		let existing = live(&loaded)?.clone();
		let mut entry = decode_entry(&existing)?;
		entry.key_version = self.keys.active()?.0;
		let (
			exact_payload_bytes,
			response,
			response_ack,
			successor,
			successor_cursor,
			ack_send_attempted,
			terminal,
		) = transition(
			&mut entry,
			existing.exact_payload_bytes,
			existing.response,
			existing.response_ack,
			existing.successor_authority,
			existing.successor_cursor,
			existing.ack_send_attempted,
			existing.terminal,
		)?;
		entry.validate()?;
		let key_version = entry.key_version;
		let record = live_record(
			entry,
			exact_payload_bytes,
			response,
			response_ack,
			successor,
			successor_cursor,
			ack_send_attempted,
			terminal,
		);
		validate_durable_record(&record, outbox_id, &self.context, key_version)?;
		let encrypted_bytes = self.persist_replace(&records, outbox_id, record.clone(), nonce)?;
		records.insert(outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		Ok(())
	}

	fn persist_new(
		&self,
		records: &BTreeMap<[u8; 16], LoadedRecordV1>,
		id: [u8; 16],
		record: DurableRecordV1,
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		if records.len() >= self.record_limit {
			return Err(HostOutboxError::Full);
		}
		validate_durable_record(&record, id, &self.context, self.keys.active()?.0)?;
		let bytes = encrypt_record(&record, id, &self.context, &self.keys, nonce)?;
		ensure_capacity(records, None, bytes.len(), self.byte_limit)?;
		self.persist_bytes(id, &bytes, true)
	}

	fn persist_replace(
		&self,
		records: &BTreeMap<[u8; 16], LoadedRecordV1>,
		id: [u8; 16],
		record: DurableRecordV1,
		nonce: [u8; 24],
	) -> Result<usize, HostOutboxError> {
		validate_durable_record(&record, id, &self.context, self.keys.active()?.0)?;
		let bytes = encrypt_record(&record, id, &self.context, &self.keys, nonce)?;
		ensure_capacity(
			records,
			records.get(&id).map(|item| item.encrypted_bytes),
			bytes.len(),
			self.byte_limit,
		)?;
		self.persist_bytes(id, &bytes, false)?;
		Ok(bytes.len())
	}

	fn persist_bytes(
		&self,
		id: [u8; 16],
		bytes: &[u8],
		create_new: bool,
	) -> Result<(), HostOutboxError> {
		let destination = self.path(id);
		let temporary = self.root.join(format!("{}.tmp-{}", hex::encode(id), std::process::id()));
		let mut options = OpenOptions::new();
		options.write(true).create_new(true);
		let mut file = options.open(&temporary).map_err(|_| HostOutboxError::Unavailable)?;
		file.write_all(bytes).map_err(|_| HostOutboxError::Unavailable)?;
		self.trip(HostOutboxFault::BeforeTempFsync)?;
		file.sync_all().map_err(|_| HostOutboxError::Unavailable)?;
		self.trip(HostOutboxFault::AfterTempFsync)?;
		if create_new && destination.exists() {
			let _ = fs::remove_file(&temporary);
			return Err(HostOutboxError::StateInvalid);
		}
		fs::rename(&temporary, &destination).map_err(|_| HostOutboxError::Unavailable)?;
		self.trip(HostOutboxFault::AfterRename)?;
		sync_dir(&self.root)?;
		self.trip(HostOutboxFault::AfterDirectoryFsync)
	}

	fn trip(&self, point: HostOutboxFault) -> Result<(), HostOutboxError> {
		let mut fault = self.fault.write().map_err(|_| HostOutboxError::Unavailable)?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			return Err(HostOutboxError::Unavailable);
		}
		Ok(())
	}

	fn quarantine_file(path: &Path, quarantine: &Path) -> Result<(), HostOutboxError> {
		let name = path.file_name().ok_or(HostOutboxError::Corrupt)?;
		let destination = quarantine.join(format!("{}.corrupt", name.to_string_lossy()));
		fs::rename(path, destination).map_err(|_| HostOutboxError::Unavailable)?;
		sync_dir(quarantine)?;
		sync_dir(path.parent().ok_or(HostOutboxError::Corrupt)?)
	}

	fn path(&self, id: [u8; 16]) -> PathBuf {
		self.root.join(format!("{}{}", hex::encode(id), EXTENSION))
	}
}

fn live_record(
	entry: HostOutboxEntryV1,
	exact_payload_bytes: Option<Vec<u8>>,
	response: Option<Vec<u8>>,
	response_ack: Option<Vec<u8>>,
	successor_authority: Option<Vec<u8>>,
	successor_cursor: Option<u32>,
	ack_send_attempted: bool,
	terminal: bool,
) -> DurableRecordV1 {
	let mut live = LiveRecordV1 {
		entry_cbor: entry.canonical_bytes(),
		exact_payload_bytes,
		response,
		response_ack,
		successor_authority,
		successor_cursor,
		ack_send_attempted,
		terminal,
		record_hash: [0; 32],
	};
	live.record_hash = record_hash(&live);
	DurableRecordV1::Live(live)
}

fn live(loaded: &LoadedRecordV1) -> Result<&LiveRecordV1, HostOutboxError> {
	match &loaded.record {
		DurableRecordV1::Live(record) => Ok(record),
		DurableRecordV1::Tombstone(_) => Err(HostOutboxError::Expired),
		DurableRecordV1::UploadManifest(_) | DurableRecordV1::UploadChunk(_) =>
			Err(HostOutboxError::StateInvalid),
	}
}

fn decode_entry(record: &LiveRecordV1) -> Result<HostOutboxEntryV1, HostOutboxError> {
	if record_hash(record) != record.record_hash {
		return Err(HostOutboxError::Corrupt);
	}
	let entry =
		HostOutboxEntryV1::decode(&record.entry_cbor).map_err(|_| HostOutboxError::Corrupt)?;
	if entry.state == HostOutboxStateV1::AckConfirmed {
		if record.exact_payload_bytes.is_some() {
			return Err(HostOutboxError::Corrupt);
		}
	} else {
		validate_exact_payload(
			&entry.exact_request_bytes,
			entry.operation_id,
			entry.generation,
			entry.intended_cursor,
			record.exact_payload_bytes.as_deref(),
		)?;
	}
	match (entry.state, record.response.as_ref(), record.response_ack.as_ref()) {
		(HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent, None, None)
			if record.successor_authority.is_none() &&
				record.successor_cursor.is_none() &&
				!record.ack_send_attempted &&
				!record.terminal => {},
		(
			HostOutboxStateV1::ResponseInstalled | HostOutboxStateV1::AckConfirmed,
			Some(response),
			Some(response_ack),
		) if entry.prior_response_hash == Some(Sha256::digest(response).into()) &&
			*response_ack ==
				response_ack_bytes(
					entry.request_id,
					entry.operation_id,
					entry.generation,
					entry.prior_response_hash.expect("matched response hash"),
				) && record.successor_authority.is_some() == record.successor_cursor.is_some() &&
			!(record.terminal && record.successor_authority.is_some()) => {},
		_ => return Err(HostOutboxError::Corrupt),
	}
	Ok(entry)
}

fn record_hash(record: &LiveRecordV1) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(RECORD_DOMAIN);
	hash.update((record.entry_cbor.len() as u64).to_be_bytes());
	hash.update(&record.entry_cbor);
	for bytes in [
		record.exact_payload_bytes.as_ref(),
		record.response.as_ref(),
		record.response_ack.as_ref(),
		record.successor_authority.as_ref(),
	] {
		match bytes {
			Some(bytes) => {
				hash.update([1]);
				hash.update((bytes.len() as u64).to_be_bytes());
				hash.update(bytes);
			},
			None => hash.update([0]),
		}
	}
	match record.successor_cursor {
		Some(cursor) => {
			hash.update([1]);
			hash.update(cursor.to_be_bytes());
		},
		None => hash.update([0]),
	}
	hash.update([u8::from(record.ack_send_attempted)]);
	hash.update([u8::from(record.terminal)]);
	hash.finalize().into()
}

fn encrypt_record(
	record: &DurableRecordV1,
	id: [u8; 16],
	context: &HostOutboxContextV1,
	keys: &HostOutboxKeyRingV1,
	nonce: [u8; 24],
) -> Result<Vec<u8>, HostOutboxError> {
	let (key_version, key) = keys.active()?;
	let plaintext = record.encode();
	encrypt(&plaintext, &key, nonce, &aad(context, id, key_version))
}

fn decrypt_record(
	bytes: &[u8],
	id: [u8; 16],
	context: &HostOutboxContextV1,
	keys: &HostOutboxKeyRingV1,
) -> Result<LoadedRecordV1, HostOutboxError> {
	if bytes.len() > MAX_ENCRYPTED_RECORD_BYTES {
		return Err(HostOutboxError::Corrupt);
	}
	for (version, key) in &keys.keys {
		let Ok(plaintext) = decrypt(bytes, key, &aad(context, id, *version)) else { continue };
		let mut input = plaintext.as_slice();
		let record = DurableRecordV1::decode(&mut input).map_err(|_| HostOutboxError::Corrupt)?;
		if !input.is_empty() {
			return Err(HostOutboxError::Corrupt);
		}
		validate_durable_record(&record, id, context, *version)?;
		return Ok(LoadedRecordV1 { record, encrypted_bytes: bytes.len() });
	}
	Err(HostOutboxError::Corrupt)
}

fn validate_durable_record(
	record: &DurableRecordV1,
	id: [u8; 16],
	context: &HostOutboxContextV1,
	key_version: u32,
) -> Result<(), HostOutboxError> {
	match record {
		DurableRecordV1::Live(record) => {
			let entry = decode_entry(record)?;
			let initial_recovery = entry
				.authority_expires_at
				.checked_add(RECOVERY_BLOCKS)
				.ok_or(HostOutboxError::Corrupt)?;
			let recovery_valid = if record.terminal {
				entry.recover_until.checked_sub(RECOVERY_BLOCKS).is_some_and(|terminal_block| {
					terminal_block >= entry.created_at &&
						terminal_block <= entry.authority_expires_at
				})
			} else {
				entry.recover_until == initial_recovery
			};
			if entry.outbox_id != id ||
				entry.registry_hash != context.registry_hash ||
				entry.genesis_hash != context.genesis_hash ||
				entry.key_version != key_version ||
				!recovery_valid ||
				record
					.successor_authority
					.as_ref()
					.is_some_and(|bytes| bytes.is_empty() || bytes.len() > MAX_AUTHORITY_BYTES)
			{
				return Err(HostOutboxError::Corrupt);
			}
		},
		DurableRecordV1::Tombstone(tombstone) => {
			if tombstone.outbox_id != id ||
				!matches!(
					tombstone.state,
					HostOutboxStateV1::AckConfirmed | HostOutboxStateV1::Expired
				) || tombstone.key_version != key_version
			{
				return Err(HostOutboxError::Corrupt);
			}
		},
		DurableRecordV1::UploadManifest(manifest) => {
			validate_upload_manifest(manifest, id, key_version)?;
		},
		DurableRecordV1::UploadChunk(chunk) => {
			validate_upload_chunk(chunk, id, key_version)?;
		},
	}
	Ok(())
}

fn encrypt(
	plaintext: &[u8],
	key: &[u8; 32],
	nonce: [u8; 24],
	aad: &[u8],
) -> Result<Vec<u8>, HostOutboxError> {
	let cipher =
		XChaCha20Poly1305::new_from_slice(key).map_err(|_| HostOutboxError::Unavailable)?;
	let ciphertext = cipher
		.encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad })
		.map_err(|_| HostOutboxError::Unavailable)?;
	let mut envelope = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
	envelope.push(ENVELOPE_VERSION);
	envelope.extend_from_slice(&nonce);
	envelope.extend(ciphertext);
	if envelope.len() > MAX_ENCRYPTED_RECORD_BYTES {
		return Err(HostOutboxError::Full);
	}
	Ok(envelope)
}

fn decrypt(bytes: &[u8], key: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>, HostOutboxError> {
	if bytes.len() < 1 + 24 + 16 || bytes[0] != ENVELOPE_VERSION {
		return Err(HostOutboxError::Corrupt);
	}
	let cipher =
		XChaCha20Poly1305::new_from_slice(key).map_err(|_| HostOutboxError::Unavailable)?;
	cipher
		.decrypt(XNonce::from_slice(&bytes[1..25]), Payload { msg: &bytes[25..], aad })
		.map_err(|_| HostOutboxError::Corrupt)
}

fn validate_exact_payload(
	request: &[u8],
	operation_id: [u8; 16],
	generation: u64,
	intended_cursor: u32,
	payload: Option<&[u8]>,
) -> Result<(), HostOutboxError> {
	let operation = request_operation(request)?;
	if operation != 1010 {
		return if payload.is_none() { Ok(()) } else { Err(HostOutboxError::Corrupt) };
	}
	let object_len = put_object_len(request)?;
	let chunks = put_chunk_count(request)?;
	if generation == 0 {
		return if payload.is_none() { Ok(()) } else { Err(HostOutboxError::Corrupt) };
	}
	if intended_cursor > chunks || payload.is_some() != (intended_cursor < chunks) {
		return Err(HostOutboxError::Corrupt);
	}
	let Some(payload) = payload else { return Ok(()) };
	if payload.len() > MAX_TRANSFER_PAYLOAD_BYTES {
		return Err(HostOutboxError::Corrupt);
	}
	let value: Value = ciborium::de::from_reader(payload).map_err(|_| HostOutboxError::Corrupt)?;
	let Value::Map(entries) = value else { return Err(HostOutboxError::Corrupt) };
	let mut fields: [Option<Value>; 5] = array::from_fn(|_| None);
	for (key, value) in entries {
		let key: usize = value_u64(key)?.try_into().map_err(|_| HostOutboxError::Corrupt)?;
		if key > 4 || fields[key].replace(value).is_some() {
			return Err(HostOutboxError::Corrupt);
		}
	}
	if fields.iter().any(Option::is_none) || value_u64(take(&mut fields, 0)?)? != 1 {
		return Err(HostOutboxError::Corrupt);
	}
	let payload_operation: [u8; 16] = fixed_bytes(take(&mut fields, 1)?)?;
	let index: u32 = value_u64(take(&mut fields, 2)?)?
		.try_into()
		.map_err(|_| HostOutboxError::Corrupt)?;
	let bytes = bounded_bytes(take(&mut fields, 3)?, 0, MAX_TRANSFER_CHUNK_BYTES)?;
	let digest: [u8; 32] = fixed_bytes(take(&mut fields, 4)?)?;
	if payload_operation != operation_id ||
		index != intended_cursor ||
		index >= 256 ||
		bytes.len() != expected_upload_chunk_len(object_len, index, chunks)? ||
		digest != sp_crypto_hashing::blake2_256(&bytes)
	{
		return Err(HostOutboxError::Corrupt);
	}
	let canonical = map(vec![
		(0, uint(1)),
		(1, bstr(&payload_operation)),
		(2, uint(u64::from(index))),
		(3, bstr(&bytes)),
		(4, bstr(&digest)),
	]);
	if canonical != payload {
		return Err(HostOutboxError::WireNonCanonical);
	}
	Ok(())
}

struct PutRequestBindingV1 {
	cid: Vec<u8>,
	object_len: u64,
	chunk_count: u32,
}

struct ParsedTransferChunkV1 {
	operation_id: [u8; 16],
	index: u32,
	bytes_len: usize,
}

fn put_request_binding(
	request: &[u8],
	operation_id: [u8; 16],
) -> Result<PutRequestBindingV1, HostOutboxError> {
	let value: Value = ciborium::de::from_reader(request).map_err(|_| HostOutboxError::Corrupt)?;
	let Value::Map(entries) = value else { return Err(HostOutboxError::Corrupt) };
	let mut operation = None;
	let mut envelope_operation_id = None;
	let mut body = None;
	for (key, value) in entries {
		match value_u64(key)? {
			3 => operation = Some(value_u64(value)?),
			5 => envelope_operation_id = Some(fixed_bytes::<16>(value)?),
			8 => body = Some(value),
			_ => {},
		}
	}
	if operation != Some(1010) || envelope_operation_id != Some(operation_id) {
		return Err(HostOutboxError::Corrupt);
	}
	let Value::Map(entries) = body.ok_or(HostOutboxError::Corrupt)? else {
		return Err(HostOutboxError::Corrupt);
	};
	let mut cid = None;
	let mut object_len = None;
	let mut body_operation_id = None;
	for (key, value) in entries {
		match value_u64(key)? {
			1 => {
				let Value::Text(value) = value else { return Err(HostOutboxError::Corrupt) };
				if value.is_empty() || value.len() > 128 {
					return Err(HostOutboxError::Corrupt);
				}
				cid = Some(value.into_bytes());
			},
			2 => object_len = Some(value_u64(value)?),
			4 => body_operation_id = Some(fixed_bytes::<16>(value)?),
			_ => {},
		}
	}
	if body_operation_id != Some(operation_id) {
		return Err(HostOutboxError::Corrupt);
	}
	let object_len = object_len.ok_or(HostOutboxError::Corrupt)?;
	let chunk_count = object_len
		.checked_add(MAX_TRANSFER_CHUNK_BYTES as u64 - 1)
		.ok_or(HostOutboxError::Corrupt)? /
		MAX_TRANSFER_CHUNK_BYTES as u64;
	let chunk_count: u32 = chunk_count.try_into().map_err(|_| HostOutboxError::Corrupt)?;
	if chunk_count > 256 {
		return Err(HostOutboxError::Full);
	}
	Ok(PutRequestBindingV1 { cid: cid.ok_or(HostOutboxError::Corrupt)?, object_len, chunk_count })
}

fn parse_transfer_chunk(payload: &[u8]) -> Result<ParsedTransferChunkV1, HostOutboxError> {
	if payload.len() > MAX_TRANSFER_PAYLOAD_BYTES {
		return Err(HostOutboxError::Corrupt);
	}
	let value: Value = ciborium::de::from_reader(payload).map_err(|_| HostOutboxError::Corrupt)?;
	let Value::Map(entries) = value else { return Err(HostOutboxError::Corrupt) };
	let mut fields: [Option<Value>; 5] = array::from_fn(|_| None);
	for (key, value) in entries {
		let key: usize = value_u64(key)?.try_into().map_err(|_| HostOutboxError::Corrupt)?;
		if key > 4 || fields[key].replace(value).is_some() {
			return Err(HostOutboxError::Corrupt);
		}
	}
	if fields.iter().any(Option::is_none) || value_u64(take(&mut fields, 0)?)? != 1 {
		return Err(HostOutboxError::Corrupt);
	}
	let operation_id: [u8; 16] = fixed_bytes(take(&mut fields, 1)?)?;
	let index: u32 = value_u64(take(&mut fields, 2)?)?
		.try_into()
		.map_err(|_| HostOutboxError::Corrupt)?;
	let bytes = bounded_bytes(take(&mut fields, 3)?, 0, MAX_TRANSFER_CHUNK_BYTES)?;
	let digest: [u8; 32] = fixed_bytes(take(&mut fields, 4)?)?;
	if index >= 256 || digest != sp_crypto_hashing::blake2_256(&bytes) {
		return Err(HostOutboxError::Corrupt);
	}
	let canonical = map(vec![
		(0, uint(1)),
		(1, bstr(&operation_id)),
		(2, uint(u64::from(index))),
		(3, bstr(&bytes)),
		(4, bstr(&digest)),
	]);
	if canonical != payload {
		return Err(HostOutboxError::WireNonCanonical);
	}
	Ok(ParsedTransferChunkV1 { operation_id, index, bytes_len: bytes.len() })
}

fn expected_upload_chunk_len(
	object_len: u64,
	index: u32,
	chunk_count: u32,
) -> Result<usize, HostOutboxError> {
	if index >= chunk_count || chunk_count > 256 {
		return Err(HostOutboxError::Corrupt);
	}
	let offset = u64::from(index)
		.checked_mul(MAX_TRANSFER_CHUNK_BYTES as u64)
		.ok_or(HostOutboxError::Corrupt)?;
	let remaining = object_len.checked_sub(offset).ok_or(HostOutboxError::Corrupt)?;
	usize::try_from(remaining.min(MAX_TRANSFER_CHUNK_BYTES as u64))
		.map_err(|_| HostOutboxError::Corrupt)
}

fn upload_manifest_id(operation_id: [u8; 16]) -> [u8; 16] {
	upload_record_id(UPLOAD_MANIFEST_DOMAIN, operation_id, None)
}

fn upload_chunk_id(operation_id: [u8; 16], index: u32) -> [u8; 16] {
	upload_record_id(UPLOAD_CHUNK_DOMAIN, operation_id, Some(index))
}

fn upload_record_id(domain: &[u8], operation_id: [u8; 16], index: Option<u32>) -> [u8; 16] {
	let mut digest = Sha256::new();
	digest.update(domain);
	digest.update(operation_id);
	if let Some(index) = index {
		digest.update(index.to_be_bytes());
	}
	let digest = digest.finalize();
	let mut id = [0; 16];
	id.copy_from_slice(&digest[..16]);
	id
}

fn upload_nonce(base: [u8; 24], index: Option<u32>) -> [u8; 24] {
	let mut digest = Sha256::new();
	digest.update(UPLOAD_NONCE_DOMAIN);
	digest.update(base);
	match index {
		Some(index) => {
			digest.update([1]);
			digest.update(index.to_be_bytes());
		},
		None => digest.update([0]),
	}
	let digest = digest.finalize();
	let mut nonce = [0; 24];
	nonce.copy_from_slice(&digest[..24]);
	nonce
}

fn validate_upload_manifest(
	manifest: &UploadManifestV1,
	id: [u8; 16],
	key_version: u32,
) -> Result<(), HostOutboxError> {
	let binding = put_request_binding(&manifest.exact_request_bytes, manifest.operation_id)?;
	let request_hash: [u8; 32] = Sha256::digest(&manifest.exact_request_bytes).into();
	let min_recovery = manifest
		.created_at
		.checked_add(RECOVERY_BLOCKS)
		.ok_or(HostOutboxError::Corrupt)?;
	let max_recovery = manifest
		.created_at
		.checked_add(MAX_RECOVERY_BLOCKS)
		.ok_or(HostOutboxError::Corrupt)?;
	if id != upload_manifest_id(manifest.operation_id) ||
		manifest.request_hash != request_hash ||
		manifest.cid != binding.cid ||
		manifest.object_len != binding.object_len ||
		manifest.chunk_count != binding.chunk_count ||
		manifest.chunk_ids.len() != manifest.chunk_count as usize ||
		manifest.recover_until < min_recovery ||
		manifest.recover_until > max_recovery ||
		manifest.key_version != key_version
	{
		return Err(HostOutboxError::Corrupt);
	}
	for (index, id) in manifest.chunk_ids.iter().enumerate() {
		if *id != upload_chunk_id(manifest.operation_id, index as u32) {
			return Err(HostOutboxError::Corrupt);
		}
	}
	Ok(())
}

fn validate_upload_chunk(
	chunk: &UploadChunkV1,
	id: [u8; 16],
	key_version: u32,
) -> Result<(), HostOutboxError> {
	let payload = parse_transfer_chunk(&chunk.exact_payload_bytes)?;
	let payload_hash: [u8; 32] = Sha256::digest(&chunk.exact_payload_bytes).into();
	if id != upload_chunk_id(chunk.operation_id, chunk.index) ||
		chunk.payload_hash != payload_hash ||
		payload.operation_id != chunk.operation_id ||
		payload.index != chunk.index ||
		chunk.key_version != key_version
	{
		return Err(HostOutboxError::Corrupt);
	}
	Ok(())
}

fn reconcile_upload_spools(
	root: &Path,
	records: &mut BTreeMap<[u8; 16], LoadedRecordV1>,
) -> Result<bool, HostOutboxError> {
	let manifests = records
		.iter()
		.filter_map(|(id, loaded)| match &loaded.record {
			DurableRecordV1::UploadManifest(manifest) => Some((*id, manifest.clone())),
			_ => None,
		})
		.collect::<Vec<_>>();
	let mut committed_chunks = BTreeSet::new();
	for (_, manifest) in &manifests {
		let mut commitment = Sha256::new();
		for (index, id) in manifest.chunk_ids.iter().enumerate() {
			let Some(loaded) = records.get(id) else { return Err(HostOutboxError::Corrupt) };
			let DurableRecordV1::UploadChunk(chunk) = &loaded.record else {
				return Err(HostOutboxError::Corrupt);
			};
			if chunk.operation_id != manifest.operation_id ||
				chunk.request_hash != manifest.request_hash ||
				chunk.index != index as u32 ||
				parse_transfer_chunk(&chunk.exact_payload_bytes)?.bytes_len !=
					expected_upload_chunk_len(
						manifest.object_len,
						chunk.index,
						manifest.chunk_count,
					)? {
				return Err(HostOutboxError::Corrupt);
			}
			commitment.update(chunk.payload_hash);
			if !committed_chunks.insert(*id) {
				return Err(HostOutboxError::Corrupt);
			}
		}
		if <[u8; 32]>::from(commitment.finalize()) != manifest.chunk_commitment {
			return Err(HostOutboxError::Corrupt);
		}
	}
	let mut retired_operations = BTreeSet::new();
	for loaded in records.values() {
		match &loaded.record {
			DurableRecordV1::Tombstone(tombstone)
				if matches!(
					tombstone.state,
					HostOutboxStateV1::Expired | HostOutboxStateV1::AckConfirmed
				) =>
				if let Some(operation_id) = tombstone.operation_id {
					retired_operations.insert(operation_id);
				},
			DurableRecordV1::Live(record) if record.terminal => {
				let entry = decode_entry(record)?;
				if entry.state == HostOutboxStateV1::AckConfirmed {
					retired_operations.insert(entry.operation_id);
				}
			},
			_ => {},
		}
	}
	let retired_manifests = manifests
		.iter()
		.filter(|(_, manifest)| retired_operations.contains(&manifest.operation_id))
		.collect::<Vec<_>>();
	let mut removals = retired_manifests.iter().map(|(id, _)| *id).collect::<Vec<_>>();
	removals.extend(
		retired_manifests
			.iter()
			.flat_map(|(_, manifest)| manifest.chunk_ids.iter().copied()),
	);
	removals.extend(records.iter().filter_map(|(id, loaded)| {
		matches!(loaded.record, DurableRecordV1::UploadChunk(_))
			.then_some(*id)
			.filter(|id| !committed_chunks.contains(id))
	}));
	let mut unique = BTreeSet::new();
	removals.retain(|id| unique.insert(*id));
	for id in &removals {
		fs::remove_file(root.join(format!("{}{}", hex::encode(id), EXTENSION)))
			.map_err(|_| HostOutboxError::Unavailable)?;
		records.remove(id);
	}
	Ok(!removals.is_empty())
}

fn request_operation(request: &[u8]) -> Result<u16, HostOutboxError> {
	let value: Value = ciborium::de::from_reader(request).map_err(|_| HostOutboxError::Corrupt)?;
	let Value::Map(entries) = value else { return Err(HostOutboxError::Corrupt) };
	let operation = entries
		.into_iter()
		.find_map(|(key, value)| (value_u64(key).ok() == Some(3)).then_some(value));
	value_u64(operation.ok_or(HostOutboxError::Corrupt)?)?
		.try_into()
		.map_err(|_| HostOutboxError::Corrupt)
}

fn put_chunk_count(request: &[u8]) -> Result<u32, HostOutboxError> {
	let object_len = put_object_len(request)?;
	let chunks = object_len
		.checked_add(MAX_TRANSFER_CHUNK_BYTES as u64 - 1)
		.ok_or(HostOutboxError::Corrupt)? /
		MAX_TRANSFER_CHUNK_BYTES as u64;
	if chunks > 256 {
		return Err(HostOutboxError::Corrupt);
	}
	chunks.try_into().map_err(|_| HostOutboxError::Corrupt)
}

fn put_object_len(request: &[u8]) -> Result<u64, HostOutboxError> {
	let value: Value = ciborium::de::from_reader(request).map_err(|_| HostOutboxError::Corrupt)?;
	let Value::Map(entries) = value else { return Err(HostOutboxError::Corrupt) };
	let body = entries
		.into_iter()
		.find_map(|(key, value)| (value_u64(key).ok() == Some(8)).then_some(value));
	let Value::Map(entries) = body.ok_or(HostOutboxError::Corrupt)? else {
		return Err(HostOutboxError::Corrupt);
	};
	let object_len = entries
		.into_iter()
		.find_map(|(key, value)| (value_u64(key).ok() == Some(2)).then_some(value));
	value_u64(object_len.ok_or(HostOutboxError::Corrupt)?)
}

fn aad(context: &HostOutboxContextV1, id: [u8; 16], key_version: u32) -> Vec<u8> {
	map(vec![
		(0, bstr(&context.profile_id)),
		(1, bstr(&id)),
		(2, uint(u64::from(key_version))),
		(3, bstr(&context.registry_hash)),
		(4, bstr(&context.genesis_hash)),
	])
}

fn request_fingerprint(request: &[u8], authority: &[u8]) -> [u8; 32] {
	let mut digest = Sha256::new();
	digest.update(request);
	digest.update(authority);
	digest.finalize().into()
}

fn response_ack_bytes(
	request_id: [u8; 16],
	operation_id: [u8; 16],
	generation: u64,
	response_hash: [u8; 32],
) -> Vec<u8> {
	map(vec![
		(0, bstr(&request_id)),
		(1, bstr(&operation_id)),
		(2, uint(generation)),
		(3, bstr(&response_hash)),
	])
}

fn ensure_live_operation_generation_available(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
	operation_id: [u8; 16],
	generation: u64,
) -> Result<(), HostOutboxError> {
	for loaded in records.values() {
		let DurableRecordV1::Live(record) = &loaded.record else { continue };
		let entry = decode_entry(record)?;
		if entry.operation_id == operation_id && entry.generation == generation {
			return Err(HostOutboxError::StateInvalid);
		}
	}
	Ok(())
}

fn has_live_operation(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
	operation_id: [u8; 16],
) -> Result<bool, HostOutboxError> {
	for loaded in records.values() {
		let DurableRecordV1::Live(record) = &loaded.record else { continue };
		if decode_entry(record)?.operation_id == operation_id {
			return Ok(true);
		}
	}
	Ok(false)
}

fn has_retryable_upload_operation(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
	operation_id: [u8; 16],
) -> Result<bool, HostOutboxError> {
	for loaded in records.values() {
		let DurableRecordV1::Live(record) = &loaded.record else { continue };
		let entry = decode_entry(record)?;
		if entry.operation_id == operation_id &&
			(matches!(
				entry.state,
				HostOutboxStateV1::Prepared |
					HostOutboxStateV1::Sent |
					HostOutboxStateV1::ResponseInstalled
			) || (entry.state == HostOutboxStateV1::AckConfirmed &&
				!record.terminal &&
				record.successor_authority.is_some()))
		{
			return Ok(true);
		}
	}
	Ok(false)
}

fn validate_unique_live_operation_generations(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
) -> Result<(), HostOutboxError> {
	let mut identities = BTreeSet::new();
	for loaded in records.values() {
		let DurableRecordV1::Live(record) = &loaded.record else { continue };
		let entry = decode_entry(record)?;
		if !identities.insert((entry.operation_id, entry.generation)) {
			return Err(HostOutboxError::Corrupt);
		}
	}
	Ok(())
}

fn has_durable_successor(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
	predecessor: &HostOutboxEntryV1,
	live_predecessor: &LiveRecordV1,
) -> Result<bool, HostOutboxError> {
	let Some(authority) = live_predecessor.successor_authority.as_ref() else { return Ok(false) };
	let Some(cursor) = live_predecessor.successor_cursor else { return Ok(false) };
	let Some(generation) = predecessor.generation.checked_add(1) else { return Ok(false) };
	for loaded in records.values() {
		let DurableRecordV1::Live(candidate) = &loaded.record else { continue };
		let entry = decode_entry(candidate)?;
		if matches!(entry.state, HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent) &&
			entry.operation_id == predecessor.operation_id &&
			entry.generation == generation &&
			entry.intended_cursor == cursor &&
			entry.registry_hash == predecessor.registry_hash &&
			entry.genesis_hash == predecessor.genesis_hash &&
			entry.negotiated_tuple == predecessor.negotiated_tuple &&
			entry.provider_id == predecessor.provider_id &&
			entry.provider_endpoint_hash == predecessor.provider_endpoint_hash &&
			entry.expected_response_kind == predecessor.expected_response_kind &&
			entry.exact_authority_bytes == *authority &&
			entry.prior_response_hash == predecessor.prior_response_hash
		{
			return Ok(true);
		}
	}
	Ok(false)
}

fn ensure_capacity(
	records: &BTreeMap<[u8; 16], LoadedRecordV1>,
	replaced: Option<usize>,
	new_bytes: usize,
	byte_limit: u64,
) -> Result<(), HostOutboxError> {
	if new_bytes > MAX_ENCRYPTED_RECORD_BYTES {
		return Err(HostOutboxError::Full);
	}
	let current = total_bytes(records)?;
	let replaced: u64 =
		replaced.unwrap_or_default().try_into().map_err(|_| HostOutboxError::Full)?;
	let next = current
		.checked_sub(replaced)
		.and_then(|value| value.checked_add(new_bytes as u64))
		.ok_or(HostOutboxError::Full)?;
	if next > byte_limit {
		return Err(HostOutboxError::Full);
	}
	Ok(())
}

fn total_bytes(records: &BTreeMap<[u8; 16], LoadedRecordV1>) -> Result<u64, HostOutboxError> {
	records.values().try_fold(0u64, |total, record| {
		total.checked_add(record.encrypted_bytes as u64).ok_or(HostOutboxError::Corrupt)
	})
}

fn decode_id(value: &str) -> Result<[u8; 16], HostOutboxError> {
	let bytes = hex::decode(value).map_err(|_| HostOutboxError::Corrupt)?;
	bytes.try_into().map_err(|_| HostOutboxError::Corrupt)
}

fn sync_dir(path: &Path) -> Result<(), HostOutboxError> {
	File::open(path)
		.and_then(|directory| directory.sync_all())
		.map_err(|_| HostOutboxError::Unavailable)
}

fn ensure_directory(path: &Path) -> Result<(), HostOutboxError> {
	fs::create_dir_all(path).map_err(|_| HostOutboxError::Unavailable)?;
	let metadata = fs::symlink_metadata(path).map_err(|_| HostOutboxError::Unavailable)?;
	if metadata.file_type().is_symlink() || !metadata.is_dir() {
		return Err(HostOutboxError::Corrupt);
	}
	Ok(())
}

fn map(entries: Vec<(u8, Value)>) -> Vec<u8> {
	let mut bytes = Vec::new();
	let value = Value::Map(
		entries
			.into_iter()
			.map(|(key, value)| (Value::Integer(key.into()), value))
			.collect(),
	);
	ciborium::ser::into_writer(&value, &mut bytes)
		.expect("bounded host outbox CBOR is serializable");
	bytes
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
) -> Result<Value, HostOutboxError> {
	fields[index].take().ok_or(HostOutboxError::Corrupt)
}

fn value_u64(value: Value) -> Result<u64, HostOutboxError> {
	let Value::Integer(value) = value else { return Err(HostOutboxError::Corrupt) };
	value.try_into().map_err(|_| HostOutboxError::Corrupt)
}

fn fixed_bytes<const N: usize>(value: Value) -> Result<[u8; N], HostOutboxError> {
	let Value::Bytes(bytes) = value else { return Err(HostOutboxError::Corrupt) };
	bytes.try_into().map_err(|_| HostOutboxError::Corrupt)
}

fn bounded_bytes(value: Value, min: usize, max: usize) -> Result<Vec<u8>, HostOutboxError> {
	let Value::Bytes(bytes) = value else { return Err(HostOutboxError::Corrupt) };
	if bytes.len() < min || bytes.len() > max {
		return Err(HostOutboxError::Corrupt);
	}
	Ok(bytes)
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::Value as JsonValue;

	const VECTORS: &str = include_str!("../../../docs/specs/host-outbox-v1.vectors.json");
	const PROTOCOL_VECTORS: &str =
		include_str!("../../../docs/specs/protocol-executable-v2.vectors.json");

	fn vector() -> JsonValue {
		serde_json::from_str::<JsonValue>(VECTORS).expect("outbox vectors are JSON")
	}

	fn hex_field(value: &JsonValue, name: &str) -> Vec<u8> {
		hex::decode(value[name].as_str().expect("hex field")).expect("valid hex")
	}

	fn context() -> HostOutboxContextV1 {
		HostOutboxContextV1 {
			profile_id: [0x11; 32],
			registry_hash: [0x11; 32],
			genesis_hash: [0x22; 32],
		}
	}

	fn keyring() -> HostOutboxKeyRingV1 {
		HostOutboxKeyRingV1::new(1, BTreeMap::from([(1, [0x8a; 32])])).unwrap()
	}

	fn prepared() -> PrepareHostOutboxV1 {
		let base = &vector()["base_vector"];
		let entry = HostOutboxEntryV1::decode(&hex_field(base, "canonical_cbor_hex")).unwrap();
		PrepareHostOutboxV1 {
			outbox_id: entry.outbox_id,
			exact_request_bytes: entry.exact_request_bytes,
			exact_authority_bytes: entry.exact_authority_bytes,
			exact_payload_bytes: None,
			request_id: entry.request_id,
			operation_id: entry.operation_id,
			generation: entry.generation,
			intended_cursor: entry.intended_cursor,
			negotiated_tuple: entry.negotiated_tuple,
			provider_id: entry.provider_id,
			provider_endpoint_hash: entry.provider_endpoint_hash,
			expected_response_kind: entry.expected_response_kind,
			created_at: entry.created_at,
			authority_expires_at: entry.authority_expires_at,
		}
	}

	fn successor(authority: Vec<u8>, cursor: u32) -> PrepareHostOutboxV1 {
		let mut input = prepared();
		input.outbox_id = [0x45; 16];
		input.exact_authority_bytes = authority;
		input.generation = 1;
		input.intended_cursor = cursor;
		input.created_at = 201;
		input.authority_expires_at = 300;
		input
	}

	fn put_request_with_length(request: &[u8], object_len: u64) -> Vec<u8> {
		let mut value: Value = ciborium::de::from_reader(request).unwrap();
		let Value::Map(fields) = &mut value else { unreachable!() };
		let Value::Map(body) = &mut fields.iter_mut().find(|(key, _)| *key == uint(8)).unwrap().1
		else {
			unreachable!()
		};
		body.iter_mut().find(|(key, _)| *key == uint(2)).unwrap().1 = uint(object_len);
		let mut exact = Vec::new();
		ciborium::ser::into_writer(&value, &mut exact).unwrap();
		exact
	}

	fn transfer_payload(operation_id: [u8; 16], index: u32, bytes: Vec<u8>) -> Vec<u8> {
		map(vec![
			(0, uint(1)),
			(1, bstr(&operation_id)),
			(2, uint(u64::from(index))),
			(3, bstr(&bytes)),
			(4, bstr(&sp_crypto_hashing::blake2_256(&bytes))),
		])
	}

	fn durable_side(fault: HostOutboxFault) -> bool {
		matches!(fault, HostOutboxFault::AfterRename | HostOutboxFault::AfterDirectoryFsync)
	}

	#[test]
	fn exact_registry_entry_and_envelope_vector_are_reproduced() {
		let root = vector();
		let base = &root["base_vector"];
		let canonical = hex_field(base, "canonical_cbor_hex");
		let entry = HostOutboxEntryV1::decode(&canonical).unwrap();
		assert_eq!(entry.canonical_bytes(), canonical);
		assert_eq!(hex::encode(Sha256::digest(&canonical)), base["canonical_sha256"]);
		assert_eq!(
			HostOutboxEntryV1::decode(&hex_field(base, "noncanonical_cbor_hex")),
			Err(HostOutboxError::WireNonCanonical)
		);
		let crypto = &base["crypto"];
		let nonce: [u8; 24] = hex_field(crypto, "nonce_hex").try_into().unwrap();
		let key: [u8; 32] = hex_field(crypto, "key_hex").try_into().unwrap();
		let expected_aad = hex_field(crypto, "aad_cbor_hex");
		assert_eq!(aad(&context(), entry.outbox_id, entry.key_version), expected_aad);
		let envelope = encrypt(&canonical, &key, nonce, &expected_aad).unwrap();
		assert_eq!(hex::encode(&envelope), crypto["envelope_hex"]);
		assert_eq!(decrypt(&envelope, &key, &expected_aad).unwrap(), canonical);
		let mut corrupt = envelope;
		*corrupt.last_mut().unwrap() ^= 1;
		assert_eq!(decrypt(&corrupt, &key, &expected_aad), Err(HostOutboxError::Corrupt));
		assert_eq!(HostOutboxError::Corrupt.code(), Some(115));
		assert_eq!(HostOutboxError::WireNonCanonical.code(), Some(101));

		let protocol: JsonValue = serde_json::from_str(PROTOCOL_VECTORS).unwrap();
		let ack = protocol["vectors"]
			.as_array()
			.unwrap()
			.iter()
			.find(|item| item["id"] == "provider-response-ack-v1")
			.unwrap();
		let response_hash: [u8; 32] =
			hex::decode("79048f62962cde1844e8cae186b36842d26314ddf1473b03995cc9ff99dd23f5")
				.unwrap()
				.try_into()
				.unwrap();
		assert_eq!(
			hex::encode(response_ack_bytes([0x55; 16], [0x44; 16], 0, response_hash)),
			ack["canonical_cbor_hex"]
		);
	}

	#[test]
	fn encrypted_payload_attachment_is_closed_bound_and_finalize_aware() {
		let input = prepared();
		let request = put_request_with_length(&input.exact_request_bytes, 15);
		let payload = transfer_payload(input.operation_id, 0, b"CORD byte plane".to_vec());
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, Some(&payload)),
			Ok(())
		);
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, None),
			Err(HostOutboxError::Corrupt)
		);
		assert_eq!(validate_exact_payload(&request, input.operation_id, 2, 1, None), Ok(()));
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 2, 1, Some(&payload)),
			Err(HostOutboxError::Corrupt)
		);
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 0, 0, Some(&payload)),
			Err(HostOutboxError::Corrupt)
		);

		let mut wrong_operation = payload.clone();
		let mut value: Value = ciborium::de::from_reader(wrong_operation.as_slice()).unwrap();
		let Value::Map(fields) = &mut value else { unreachable!() };
		fields.iter_mut().find(|(key, _)| *key == uint(1)).unwrap().1 = bstr(&[9; 16]);
		wrong_operation.clear();
		ciborium::ser::into_writer(&value, &mut wrong_operation).unwrap();
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, Some(&wrong_operation)),
			Err(HostOutboxError::Corrupt)
		);

		let wrong_index = transfer_payload(input.operation_id, 1, b"CORD byte plane".to_vec());
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, Some(&wrong_index)),
			Err(HostOutboxError::Corrupt)
		);
		let over_index_request = put_request_with_length(
			&input.exact_request_bytes,
			257 * MAX_TRANSFER_CHUNK_BYTES as u64,
		);
		let over_index = transfer_payload(input.operation_id, 256, vec![0]);
		assert_eq!(
			validate_exact_payload(
				&over_index_request,
				input.operation_id,
				257,
				256,
				Some(&over_index),
			),
			Err(HostOutboxError::Corrupt)
		);
		let mut wrong_digest = payload.clone();
		*wrong_digest.last_mut().unwrap() ^= 1;
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, Some(&wrong_digest)),
			Err(HostOutboxError::Corrupt)
		);
		let mut noncanonical: Value = ciborium::de::from_reader(payload.as_slice()).unwrap();
		let Value::Map(fields) = &mut noncanonical else { unreachable!() };
		fields.reverse();
		let mut noncanonical_bytes = Vec::new();
		ciborium::ser::into_writer(&noncanonical, &mut noncanonical_bytes).unwrap();
		assert_eq!(
			validate_exact_payload(&request, input.operation_id, 1, 0, Some(&noncanonical_bytes),),
			Err(HostOutboxError::WireNonCanonical)
		);
		assert_eq!(
			validate_exact_payload(
				&request,
				input.operation_id,
				1,
				0,
				Some(&vec![0; MAX_TRANSFER_PAYLOAD_BYTES + 1]),
			),
			Err(HostOutboxError::Corrupt)
		);

		for operation in [1011, 1012, 1014] {
			let mut query: Value = ciborium::de::from_reader(request.as_slice()).unwrap();
			let Value::Map(fields) = &mut query else { unreachable!() };
			fields.iter_mut().find(|(key, _)| *key == uint(3)).unwrap().1 = uint(operation);
			let mut query_bytes = Vec::new();
			ciborium::ser::into_writer(&query, &mut query_bytes).unwrap();
			assert!(validate_exact_payload(&query_bytes, input.operation_id, 1, 0, None).is_ok());
			assert_eq!(
				validate_exact_payload(&query_bytes, input.operation_id, 1, 0, Some(&payload)),
				Err(HostOutboxError::Corrupt)
			);
		}
	}

	#[test]
	fn put_payload_lengths_are_exact_and_capped_at_256_chunks() {
		let input = prepared();
		let check = |object_len: u64, index: u32, bytes: Vec<u8>| {
			let request = put_request_with_length(&input.exact_request_bytes, object_len);
			let payload = transfer_payload(input.operation_id, index, bytes);
			validate_exact_payload(
				&request,
				input.operation_id,
				u64::from(index) + 1,
				index,
				Some(&payload),
			)
		};
		let empty = put_request_with_length(&input.exact_request_bytes, 0);
		assert_eq!(validate_exact_payload(&empty, input.operation_id, 0, 0, None), Ok(()));
		assert_eq!(validate_exact_payload(&empty, input.operation_id, 1, 0, None), Ok(()));

		assert_eq!(check(1, 0, vec![1]), Ok(()));
		assert_eq!(check(1, 0, vec![]), Err(HostOutboxError::Corrupt));
		assert_eq!(check(262_143, 0, vec![1; 262_143]), Ok(()));
		assert_eq!(check(262_144, 0, vec![1; 262_144]), Ok(()));
		assert_eq!(check(262_145, 0, vec![1; 262_144]), Ok(()));
		assert_eq!(check(262_145, 1, vec![1]), Ok(()));
		assert_eq!(check(262_145, 1, vec![]), Err(HostOutboxError::Corrupt));
		assert_eq!(check(262_145, 1, vec![1, 2]), Err(HostOutboxError::Corrupt));

		let max_len = 256 * MAX_TRANSFER_CHUNK_BYTES as u64;
		let max = put_request_with_length(&input.exact_request_bytes, max_len);
		assert_eq!(validate_exact_payload(&max, input.operation_id, 0, 0, None), Ok(()));
		assert_eq!(check(max_len, 255, vec![1; MAX_TRANSFER_CHUNK_BYTES]), Ok(()));
		let over = put_request_with_length(&input.exact_request_bytes, max_len + 1);
		assert_eq!(
			validate_exact_payload(&over, input.operation_id, 0, 0, None),
			Err(HostOutboxError::Corrupt)
		);
	}

	#[test]
	fn upload_spool_survives_preprepare_failure_and_terminal_ack_erases_without_revival() {
		let root = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		let mut input = prepared();
		input.exact_request_bytes = put_request_with_length(&input.exact_request_bytes, 15);
		let payload = transfer_payload(input.operation_id, 0, b"CORD byte plane".to_vec());
		store
			.stage_upload(
				&input.exact_request_bytes,
				input.operation_id,
				vec![payload.clone()],
				input.created_at,
				input.authority_expires_at,
				[41; 24],
			)
			.unwrap();
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		store.inject_fault_once(HostOutboxFault::BeforeTempFsync).unwrap();
		assert_eq!(store.prepare(input.clone(), [42; 24]), Err(HostOutboxError::Unavailable));
		drop(store);

		let store = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		let recover_until = input.authority_expires_at + RECOVERY_BLOCKS;
		assert_eq!(store.gc(recover_until - 1, MAX_RECORDS).unwrap(), 0);
		assert_eq!(store.gc(recover_until, MAX_RECORDS).unwrap(), 2);
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		store
			.stage_upload(
				&input.exact_request_bytes,
				input.operation_id,
				vec![payload.clone()],
				input.created_at,
				input.authority_expires_at,
				[48; 24],
			)
			.unwrap();
		store.prepare(input.clone(), [43; 24]).unwrap();
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Ok(Some(payload.clone()))
		);
		assert_eq!(store.gc(recover_until, MAX_RECORDS).unwrap(), 0);
		store.mark_sent(input.outbox_id, [44; 24]).unwrap();
		let response_hash = store
			.install_response(
				input.outbox_id,
				b"authenticated terminal".to_vec(),
				None,
				None,
				Some(input.created_at),
				[45; 24],
			)
			.unwrap();
		store.confirm_ack(input.outbox_id, response_hash, [46; 24]).unwrap();
		store.erase_upload(input.operation_id).unwrap();
		store.compact_acknowledged(input.outbox_id, [47; 24]).unwrap();
		{
			let records = store.records.read().unwrap();
			let DurableRecordV1::Tombstone(tombstone) = &records[&input.outbox_id].record else {
				panic!("terminal acknowledgement did not retain a tombstone")
			};
			assert_eq!(tombstone.state, HostOutboxStateV1::AckConfirmed);
			assert_eq!(tombstone.prior_response_hash, Some(response_hash));
		}
		drop(store);

		let reopened = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		assert_eq!(
			reopened.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		let records = reopened.records.read().unwrap();
		let DurableRecordV1::Tombstone(tombstone) = &records[&input.outbox_id].record else {
			panic!("terminal tombstone revived as live state")
		};
		assert_eq!(tombstone.prior_response_hash, Some(response_hash));
	}

	#[test]
	fn partial_upload_stage_is_cleaned_and_durable_manifest_is_exactly_retryable() {
		let root = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		let mut input = prepared();
		input.exact_request_bytes = put_request_with_length(&input.exact_request_bytes, 15);
		let payload = transfer_payload(input.operation_id, 0, b"CORD byte plane".to_vec());
		store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
		assert_eq!(
			store.stage_upload(
				&input.exact_request_bytes,
				input.operation_id,
				vec![payload],
				input.created_at,
				input.authority_expires_at,
				[51; 24],
			),
			Err(HostOutboxError::Unavailable)
		);
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		drop(store);
		let store = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		assert_eq!(
			store.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);

		let empty = prepared();
		store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
		assert_eq!(
			store.stage_upload(
				&empty.exact_request_bytes,
				empty.operation_id,
				Vec::new(),
				empty.created_at,
				empty.authority_expires_at,
				[52; 24],
			),
			Err(HostOutboxError::Unavailable)
		);
		assert_eq!(
			store.staged_upload_payload(&empty.exact_request_bytes, empty.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		store
			.stage_upload(
				&empty.exact_request_bytes,
				empty.operation_id,
				Vec::new(),
				empty.created_at,
				empty.authority_expires_at,
				[53; 24],
			)
			.unwrap();
		store.prepare(empty.clone(), [54; 24]).unwrap();
		assert_eq!(
			store.staged_upload_payload(&empty.exact_request_bytes, empty.operation_id, 0),
			Ok(None)
		);
		drop(store);
		let reopened = HostOutboxStoreV1::open(root.path(), context(), keyring()).unwrap();
		assert_eq!(
			reopened.staged_upload_payload(&empty.exact_request_bytes, empty.operation_id, 0),
			Ok(None)
		);
	}

	#[test]
	fn reopen_reconciles_uploads_after_expiry_or_terminal_ack_commit_crash() {
		let expiry_root = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(expiry_root.path(), context(), keyring()).unwrap();
		let input = prepared();
		store
			.stage_upload(
				&input.exact_request_bytes,
				input.operation_id,
				Vec::new(),
				input.created_at,
				input.authority_expires_at,
				[61; 24],
			)
			.unwrap();
		store.prepare(input.clone(), [62; 24]).unwrap();
		store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
		assert_eq!(
			store.expire(input.outbox_id, input.authority_expires_at + RECOVERY_BLOCKS, [63; 24],),
			Err(HostOutboxError::Unavailable)
		);
		drop(store);
		let reopened = HostOutboxStoreV1::open(expiry_root.path(), context(), keyring()).unwrap();
		assert_eq!(
			reopened.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		assert_eq!(
			reopened.expire(
				input.outbox_id,
				input.authority_expires_at + RECOVERY_BLOCKS,
				[64; 24],
			),
			Err(HostOutboxError::Expired)
		);
		assert!(!reopened.records.read().unwrap().values().any(|record| matches!(
			&record.record,
			DurableRecordV1::UploadManifest(_) | DurableRecordV1::UploadChunk(_)
		)));

		let terminal_root = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(terminal_root.path(), context(), keyring()).unwrap();
		let input = prepared();
		store
			.stage_upload(
				&input.exact_request_bytes,
				input.operation_id,
				Vec::new(),
				input.created_at,
				input.authority_expires_at,
				[65; 24],
			)
			.unwrap();
		store.prepare(input.clone(), [66; 24]).unwrap();
		store.mark_sent(input.outbox_id, [67; 24]).unwrap();
		let response_hash = store
			.install_response(
				input.outbox_id,
				b"terminal".to_vec(),
				None,
				None,
				Some(input.created_at),
				[68; 24],
			)
			.unwrap();
		store.inject_fault_once(HostOutboxFault::AfterDirectoryFsync).unwrap();
		assert_eq!(
			store.confirm_ack(input.outbox_id, response_hash, [69; 24]),
			Err(HostOutboxError::Unavailable)
		);
		drop(store);
		let reopened = HostOutboxStoreV1::open(terminal_root.path(), context(), keyring()).unwrap();
		assert_eq!(
			reopened.staged_upload_payload(&input.exact_request_bytes, input.operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		let records = reopened.records.read().unwrap();
		let live = live(&records[&input.outbox_id]).unwrap();
		let entry = decode_entry(live).unwrap();
		assert!(live.terminal);
		assert_eq!(entry.state, HostOutboxStateV1::AckConfirmed);
		assert_eq!(entry.prior_response_hash, Some(response_hash));
		assert!(!records.values().any(|record| matches!(
			&record.record,
			DurableRecordV1::UploadManifest(_) | DurableRecordV1::UploadChunk(_)
		)));
	}

	#[test]
	fn prepared_response_ack_restart_and_terminal_gc_are_byte_exact() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		let request = input.exact_request_bytes.clone();
		let authority = input.exact_authority_bytes.clone();
		let expected_fingerprint = request_fingerprint(&request, &authority);
		let prepared = store.prepare(input, [1; 24]).unwrap();
		assert_eq!(prepared.request, request);
		assert_eq!(prepared.authority, authority);
		assert_eq!(prepared.fingerprint, expected_fingerprint);
		store.mark_sent(id, [2; 24]).unwrap();
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		assert_eq!(store.retry_request(id, 200).unwrap(), prepared);
		let response = b"provider-accepted".to_vec();
		assert_eq!(
			store.install_response(
				id,
				response.clone(),
				Some(b"forbidden-terminal-successor".to_vec()),
				Some(1),
				Some(200),
				[3; 24],
			),
			Err(HostOutboxError::StateInvalid)
		);
		let hash = store
			.install_response(id, response.clone(), None, None, Some(200), [3; 24])
			.unwrap();
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let installed = store.installed_response(id).unwrap();
		assert_eq!(installed.response, response);
		assert_eq!(installed.response_hash, hash);
		assert_eq!(installed.response_ack, response_ack_bytes([0x55; 16], [0x44; 16], 0, hash));
		assert_eq!(installed.successor_authority, None);
		assert_eq!(installed.successor_cursor, None);
		assert!(installed.terminal);
		let exact_ack = store.retry_response_ack(id).unwrap();
		store.mark_ack_sent(id, [4; 24]).unwrap();
		assert_eq!(store.retry_response_ack(id).unwrap(), exact_ack);
		assert_eq!(store.confirm_ack(id, [9; 32], [5; 24]), Err(HostOutboxError::ResponseMismatch));
		store.confirm_ack(id, hash, [6; 24]).unwrap();
		assert_eq!(store.confirm_ack(id, hash, [6; 24]), Ok(()));
		assert_eq!(store.retry_response_ack(id).unwrap(), exact_ack);
		store.compact_acknowledged(id, [7; 24]).unwrap();
		assert_eq!(store.installed_response(id), Err(HostOutboxError::Expired));
		assert_eq!(store.gc(455, 16).unwrap(), 0);
		assert_eq!(store.gc(456, 16).unwrap(), 1);
	}

	#[test]
	fn nonterminal_successor_requires_its_own_prepared_generation_before_cleanup() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		store.prepare(input, [1; 24]).unwrap();
		let successor_token = b"signed-successor".to_vec();
		let cursor = 0;
		let hash = store
			.install_response(
				id,
				b"progress".to_vec(),
				Some(successor_token.clone()),
				Some(cursor),
				None,
				[2; 24],
			)
			.unwrap();
		assert_eq!(store.retry_request(id, 200), Err(HostOutboxError::StateInvalid));
		store.confirm_ack(id, hash, [3; 24]).unwrap();
		let records = store.records.read().unwrap();
		let predecessor = live(records.get(&id).unwrap()).unwrap();
		assert!(decode_entry(predecessor).unwrap().exact_authority_bytes.is_empty());
		drop(records);
		assert_eq!(store.compact_acknowledged(id, [4; 24]), Err(HostOutboxError::StateInvalid));
		let next = successor(successor_token.clone(), cursor);
		let next_id = next.outbox_id;
		assert_eq!(store.prepare(next.clone(), [5; 24]), Err(HostOutboxError::StateInvalid));
		let prepared_next = store.prepare_successor(id, next, [5; 24]).unwrap();
		assert_eq!(prepared_next.authority, successor_token);
		store.compact_acknowledged(id, [6; 24]).unwrap();
		drop(store);
		let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		assert_eq!(reopened.installed_response(id), Err(HostOutboxError::Expired));
		assert_eq!(reopened.retry_request(next_id, 201).unwrap(), prepared_next);
		assert_eq!(reopened.gc(483, 1).unwrap(), 0);
		assert_eq!(reopened.gc(484, 1).unwrap(), 1);
	}

	#[test]
	fn expiry_erases_authority_and_corrupt_ciphertext_is_quarantined() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		let operation_id = input.operation_id;
		let exact_request = input.exact_request_bytes.clone();
		store
			.stage_upload(
				&exact_request,
				operation_id,
				Vec::new(),
				input.created_at,
				input.authority_expires_at,
				[9; 24],
			)
			.unwrap();
		store.prepare(input, [1; 24]).unwrap();
		assert_eq!(store.retry_request(id, 484), Err(HostOutboxError::Expired));
		store.expire(id, 484, [2; 24]).unwrap();
		assert_eq!(store.retry_request(id, 484), Err(HostOutboxError::Expired));
		assert_eq!(
			store.staged_upload_payload(&exact_request, operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		assert_eq!(
			store.staged_upload_payload(&exact_request, operation_id, 0),
			Err(HostOutboxError::StateInvalid)
		);
		assert_eq!(store.gc(484, 1).unwrap(), 1);
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		store.prepare(input, [3; 24]).unwrap();
		drop(store);
		let path = temp.path().join(ROOT).join(format!("{}{}", hex::encode(id), EXTENSION));
		let mut bytes = fs::read(&path).unwrap();
		*bytes.last_mut().unwrap() ^= 1;
		fs::write(&path, bytes).unwrap();
		assert!(matches!(
			HostOutboxStoreV1::open(temp.path(), context(), keyring()),
			Err(HostOutboxError::Corrupt)
		));
		assert_eq!(fs::read_dir(temp.path().join(ROOT).join(QUARANTINE)).unwrap().count(), 1);
	}

	#[test]
	fn crash_after_rename_recovers_new_durable_state_without_memory_claim() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		store.inject_fault_once(HostOutboxFault::AfterRename).unwrap();
		assert_eq!(store.prepare(input, [1; 24]), Err(HostOutboxError::Unavailable));
		drop(store);
		let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		assert_eq!(
			reopened.retry_request(id, 200).unwrap().fingerprint,
			[
				0x3c, 0x24, 0xed, 0x13, 0xfd, 0xdd, 0x42, 0xa0, 0xf5, 0x11, 0x46, 0xd2, 0xc0, 0x0c,
				0x98, 0xfc, 0x90, 0xd4, 0x0b, 0xa7, 0x87, 0x44, 0x80, 0xbf, 0xe5, 0xfd, 0x1f, 0xe0,
				0xda, 0x5a, 0x33, 0x2c
			]
		);
	}

	#[test]
	fn every_atomic_prepare_boundary_recovers_old_or_new_without_regeneration() {
		for (fault, durable) in [
			(HostOutboxFault::BeforeTempFsync, false),
			(HostOutboxFault::AfterTempFsync, false),
			(HostOutboxFault::AfterRename, true),
			(HostOutboxFault::AfterDirectoryFsync, true),
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			let expected = HostOutboxRetryV1 {
				request: input.exact_request_bytes.clone(),
				authority: input.exact_authority_bytes.clone(),
				payload: input.exact_payload_bytes.clone(),
				fingerprint: request_fingerprint(
					&input.exact_request_bytes,
					&input.exact_authority_bytes,
				),
			};
			store.inject_fault_once(fault).unwrap();
			assert_eq!(store.prepare(input, [7; 24]), Err(HostOutboxError::Unavailable));
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			if durable {
				assert_eq!(reopened.retry_request(id, 200).unwrap(), expected);
			} else {
				assert_eq!(reopened.retry_request(id, 200), Err(HostOutboxError::StateInvalid));
			}
		}
	}

	#[test]
	fn response_install_and_ack_send_boundaries_preserve_exact_ack() {
		for fault in [
			HostOutboxFault::BeforeTempFsync,
			HostOutboxFault::AfterTempFsync,
			HostOutboxFault::AfterRename,
			HostOutboxFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			let expected_retry = store.prepare(input, [1; 24]).unwrap();
			let response = b"durable-progress".to_vec();
			let expected_hash: [u8; 32] = Sha256::digest(&response).into();
			let expected_ack = response_ack_bytes([0x55; 16], [0x44; 16], 0, expected_hash);
			store.inject_fault_once(fault).unwrap();
			assert_eq!(
				store.install_response(
					id,
					response,
					Some(b"next-authority".to_vec()),
					Some(0),
					None,
					[2; 24],
				),
				Err(HostOutboxError::Unavailable)
			);
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			if durable_side(fault) {
				assert_eq!(reopened.retry_response_ack(id).unwrap().bytes, expected_ack);
				assert_eq!(reopened.retry_request(id, 200), Err(HostOutboxError::StateInvalid));
			} else {
				assert_eq!(reopened.retry_request(id, 200).unwrap(), expected_retry);
				assert_eq!(reopened.retry_response_ack(id), Err(HostOutboxError::StateInvalid));
			}
		}

		for fault in [
			HostOutboxFault::BeforeTempFsync,
			HostOutboxFault::AfterTempFsync,
			HostOutboxFault::AfterRename,
			HostOutboxFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			store.prepare(input, [1; 24]).unwrap();
			store
				.install_response(id, b"progress".to_vec(), None, None, Some(200), [2; 24])
				.unwrap();
			let expected_ack = store.retry_response_ack(id).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert_eq!(store.mark_ack_sent(id, [3; 24]), Err(HostOutboxError::Unavailable));
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			assert_eq!(reopened.retry_response_ack(id).unwrap(), expected_ack);
		}
	}

	#[test]
	fn ack_confirmation_successor_and_cleanup_boundaries_recover_old_or_new() {
		for fault in [
			HostOutboxFault::BeforeTempFsync,
			HostOutboxFault::AfterTempFsync,
			HostOutboxFault::AfterRename,
			HostOutboxFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			store.prepare(input, [1; 24]).unwrap();
			let hash = store
				.install_response(
					id,
					b"progress".to_vec(),
					Some(b"next-authority".to_vec()),
					Some(0),
					None,
					[2; 24],
				)
				.unwrap();
			let exact_ack = store.retry_response_ack(id).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert_eq!(store.confirm_ack(id, hash, [3; 24]), Err(HostOutboxError::Unavailable));
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			assert_eq!(reopened.retry_response_ack(id).unwrap(), exact_ack);
			reopened.confirm_ack(id, hash, [4; 24]).unwrap();
		}

		for fault in [
			HostOutboxFault::BeforeTempFsync,
			HostOutboxFault::AfterTempFsync,
			HostOutboxFault::AfterRename,
			HostOutboxFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			store.prepare(input, [1; 24]).unwrap();
			let token = b"next-authority".to_vec();
			let hash = store
				.install_response(
					id,
					b"progress".to_vec(),
					Some(token.clone()),
					Some(0),
					None,
					[2; 24],
				)
				.unwrap();
			store.confirm_ack(id, hash, [3; 24]).unwrap();
			let next = successor(token, 0);
			let next_id = next.outbox_id;
			store.inject_fault_once(fault).unwrap();
			assert_eq!(
				store.prepare_successor(id, next, [4; 24]),
				Err(HostOutboxError::Unavailable)
			);
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			if durable_side(fault) {
				assert!(reopened.retry_request(next_id, 201).is_ok());
				reopened.compact_acknowledged(id, [5; 24]).unwrap();
			} else {
				assert_eq!(
					reopened.retry_request(next_id, 201),
					Err(HostOutboxError::StateInvalid)
				);
				assert_eq!(
					reopened.compact_acknowledged(id, [5; 24]),
					Err(HostOutboxError::StateInvalid)
				);
			}
		}

		for fault in [
			HostOutboxFault::BeforeTempFsync,
			HostOutboxFault::AfterTempFsync,
			HostOutboxFault::AfterRename,
			HostOutboxFault::AfterDirectoryFsync,
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			store.prepare(input, [1; 24]).unwrap();
			let hash = store
				.install_response(id, b"cancelled".to_vec(), None, None, Some(200), [2; 24])
				.unwrap();
			store.confirm_ack(id, hash, [3; 24]).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert_eq!(store.compact_acknowledged(id, [4; 24]), Err(HostOutboxError::Unavailable));
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			if durable_side(fault) {
				assert_eq!(reopened.installed_response(id), Err(HostOutboxError::Expired));
			} else {
				assert_eq!(reopened.retry_response_ack(id).unwrap().response_hash, hash);
				reopened.compact_acknowledged(id, [5; 24]).unwrap();
			}
		}
	}

	#[test]
	fn terminal_tombstone_gc_boundaries_are_restart_safe() {
		for (fault, removed) in [
			(HostOutboxFault::BeforeGcRemove, false),
			(HostOutboxFault::AfterGcRemove, true),
			(HostOutboxFault::AfterGcDirectoryFsync, true),
		] {
			let temp = tempfile::tempdir().unwrap();
			let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			let input = prepared();
			let id = input.outbox_id;
			store.prepare(input, [1; 24]).unwrap();
			let hash = store
				.install_response(id, b"terminal".to_vec(), None, None, Some(200), [2; 24])
				.unwrap();
			store.confirm_ack(id, hash, [3; 24]).unwrap();
			store.compact_acknowledged(id, [4; 24]).unwrap();
			store.inject_fault_once(fault).unwrap();
			assert_eq!(store.gc(456, 1), Err(HostOutboxError::Unavailable));
			drop(store);
			let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
			if removed {
				assert!(reopened.records.read().unwrap().get(&id).is_none());
			} else {
				assert_eq!(reopened.gc(456, 1).unwrap(), 1);
			}
		}
	}

	#[test]
	fn reopen_rejects_oversized_files_and_record_count_before_unbounded_work() {
		let oversized = tempfile::tempdir().unwrap();
		drop(HostOutboxStoreV1::open(oversized.path(), context(), keyring()).unwrap());
		let path =
			oversized
				.path()
				.join(ROOT)
				.join(format!("{}{}", hex::encode([0x71; 16]), EXTENSION));
		File::create(path)
			.unwrap()
			.set_len(MAX_ENCRYPTED_RECORD_BYTES as u64 + 1)
			.unwrap();
		assert!(matches!(
			HostOutboxStoreV1::open(oversized.path(), context(), keyring()),
			Err(HostOutboxError::Corrupt)
		));
		assert_eq!(fs::read_dir(oversized.path().join(ROOT).join(QUARANTINE)).unwrap().count(), 1);

		let counted = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open_with_limits(
			counted.path(),
			context(),
			keyring(),
			2,
			MAX_TOTAL_ENCRYPTED_BYTES,
		)
		.unwrap();
		store.prepare(prepared(), [1; 24]).unwrap();
		let mut second = prepared();
		second.outbox_id = [0x72; 16];
		second.operation_id = [0x73; 16];
		store.prepare(second, [2; 24]).unwrap();
		drop(store);
		assert!(matches!(
			HostOutboxStoreV1::open_with_limits(
				counted.path(),
				context(),
				keyring(),
				1,
				MAX_TOTAL_ENCRYPTED_BYTES,
			),
			Err(HostOutboxError::Corrupt)
		));
		assert!(matches!(
			HostOutboxStoreV1::open_with_limits(counted.path(), context(), keyring(), 2, 1),
			Err(HostOutboxError::Corrupt)
		));
	}

	#[test]
	fn authority_and_terminal_lifetimes_are_exact_and_fail_closed() {
		let excessive = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(excessive.path(), context(), keyring()).unwrap();
		let mut input = prepared();
		input.authority_expires_at = input.created_at + AUTHORITY_BLOCKS + 1;
		assert_eq!(store.prepare(input, [1; 24]), Err(HostOutboxError::Corrupt));
		let base = &vector()["base_vector"];
		let mut invalid_recovery =
			HostOutboxEntryV1::decode(&hex_field(base, "canonical_cbor_hex")).unwrap();
		invalid_recovery.recover_until = invalid_recovery.created_at + MAX_RECOVERY_BLOCKS + 1;
		assert_eq!(
			HostOutboxEntryV1::decode(&invalid_recovery.canonical_bytes()),
			Err(HostOutboxError::Corrupt)
		);
		let mut non_normative =
			HostOutboxEntryV1::decode(&hex_field(base, "canonical_cbor_hex")).unwrap();
		non_normative.recover_until = non_normative.created_at + RECOVERY_BLOCKS + 1;
		let non_normative_id = non_normative.outbox_id;
		let record = live_record(non_normative, None, None, None, None, None, false, false);
		assert_eq!(
			validate_durable_record(&record, non_normative_id, &context(), 1),
			Err(HostOutboxError::Corrupt)
		);

		let bounded = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(bounded.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		store.prepare(input, [2; 24]).unwrap();
		assert_eq!(
			store.install_response(id, b"stale".to_vec(), None, None, Some(99), [3; 24]),
			Err(HostOutboxError::StateInvalid)
		);
		assert_eq!(
			store.install_response(id, b"late".to_vec(), None, None, Some(229), [4; 24]),
			Err(HostOutboxError::StateInvalid)
		);
		drop(store);
		let reopened = HostOutboxStoreV1::open(bounded.path(), context(), keyring()).unwrap();
		assert!(reopened.retry_request(id, 200).is_ok());
		let hash = reopened
			.install_response(id, b"bounded".to_vec(), None, None, Some(228), [5; 24])
			.unwrap();
		assert_eq!(
			reopened.installed_response(id).unwrap().recover_until,
			100 + MAX_RECOVERY_BLOCKS
		);
		let expected_hash: [u8; 32] = Sha256::digest(b"bounded").into();
		assert_eq!(hash, expected_hash);
	}

	#[test]
	fn successor_replay_and_cleanup_are_bound_to_the_exact_predecessor() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let first = prepared();
		let first_id = first.outbox_id;
		store.prepare(first.clone(), [1; 24]).unwrap();
		let mut duplicate_identity = first;
		duplicate_identity.outbox_id = [0x72; 16];
		duplicate_identity.request_id = [0x73; 16];
		assert_eq!(store.prepare(duplicate_identity, [2; 24]), Err(HostOutboxError::StateInvalid));
		let mut second = prepared();
		second.outbox_id = [0x74; 16];
		second.request_id = [0x75; 16];
		second.operation_id = [0x76; 16];
		let second_id = second.outbox_id;
		let second_operation = second.operation_id;
		store.prepare(second, [3; 24]).unwrap();

		let authority = b"shared-successor-authority".to_vec();
		let cursor = 0;
		for (id, nonce) in [(first_id, [4; 24]), (second_id, [5; 24])] {
			let hash = store
				.install_response(
					id,
					b"shared-response".to_vec(),
					Some(authority.clone()),
					Some(cursor),
					None,
					nonce,
				)
				.unwrap();
			store.confirm_ack(id, hash, [nonce[0] + 2; 24]).unwrap();
		}

		let first_successor = successor(authority.clone(), cursor);
		let successor_id = first_successor.outbox_id;
		store.prepare_successor(first_id, first_successor, [8; 24]).unwrap();
		let mut duplicate_successor = successor(authority.clone(), cursor);
		duplicate_successor.outbox_id = [0x78; 16];
		assert_eq!(
			store.prepare_successor(first_id, duplicate_successor, [9; 24]),
			Err(HostOutboxError::StateInvalid)
		);
		let mut cross_predecessor_replay = successor(authority, cursor);
		cross_predecessor_replay.outbox_id = successor_id;
		cross_predecessor_replay.operation_id = second_operation;
		assert_eq!(
			store.prepare_successor(second_id, cross_predecessor_replay, [10; 24]),
			Err(HostOutboxError::StateInvalid)
		);
		assert_eq!(
			store.compact_acknowledged(second_id, [11; 24]),
			Err(HostOutboxError::StateInvalid)
		);
		store.compact_acknowledged(first_id, [12; 24]).unwrap();

		let left = tempfile::tempdir().unwrap();
		let left_store = HostOutboxStoreV1::open(left.path(), context(), keyring()).unwrap();
		let first = prepared();
		left_store.prepare(first.clone(), [13; 24]).unwrap();
		drop(left_store);
		let right = tempfile::tempdir().unwrap();
		let right_store = HostOutboxStoreV1::open(right.path(), context(), keyring()).unwrap();
		let mut collision = first;
		collision.outbox_id = [0x77; 16];
		right_store.prepare(collision.clone(), [14; 24]).unwrap();
		drop(right_store);
		let collision_name = format!("{}{}", hex::encode(collision.outbox_id), EXTENSION);
		fs::copy(
			right.path().join(ROOT).join(&collision_name),
			left.path().join(ROOT).join(collision_name),
		)
		.unwrap();
		assert!(matches!(
			HostOutboxStoreV1::open(left.path(), context(), keyring()),
			Err(HostOutboxError::Corrupt)
		));
	}

	#[test]
	fn admission_bounds_and_key_rotation_fail_closed() {
		assert_eq!(HostOutboxKeyRingV1::new(1, BTreeMap::new()), Err(HostOutboxError::Unavailable));
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open_with_limits(
			temp.path(),
			context(),
			keyring(),
			1,
			MAX_TOTAL_ENCRYPTED_BYTES,
		)
		.unwrap();
		let first = prepared();
		let id = first.outbox_id;
		store.prepare(first, [1; 24]).unwrap();
		let mut second = prepared();
		second.outbox_id = [0x45; 16];
		second.operation_id = [0x46; 16];
		assert_eq!(store.prepare(second, [2; 24]), Err(HostOutboxError::Full));
		drop(store);

		let rotated =
			HostOutboxKeyRingV1::new(2, BTreeMap::from([(1, [0x8a; 32]), (2, [0x9a; 32])]))
				.unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), rotated.clone()).unwrap();
		store.mark_sent(id, [3; 24]).unwrap();
		drop(store);
		let reopened = HostOutboxStoreV1::open(temp.path(), context(), rotated).unwrap();
		assert_eq!(
			reopened.retry_request(id, 200).unwrap().fingerprint,
			[
				0x3c, 0x24, 0xed, 0x13, 0xfd, 0xdd, 0x42, 0xa0, 0xf5, 0x11, 0x46, 0xd2, 0xc0, 0x0c,
				0x98, 0xfc, 0x90, 0xd4, 0x0b, 0xa7, 0x87, 0x44, 0x80, 0xbf, 0xe5, 0xfd, 0x1f, 0xe0,
				0xda, 0x5a, 0x33, 0x2c,
			]
		);

		let tiny = tempfile::tempdir().unwrap();
		let store =
			HostOutboxStoreV1::open_with_limits(tiny.path(), context(), keyring(), MAX_RECORDS, 1)
				.unwrap();
		assert_eq!(store.prepare(prepared(), [4; 24]), Err(HostOutboxError::Full));
	}

	#[test]
	fn host_v2_conformance() {
		exact_registry_entry_and_envelope_vector_are_reproduced();
		prepared_response_ack_restart_and_terminal_gc_are_byte_exact();
		expiry_erases_authority_and_corrupt_ciphertext_is_quarantined();
		crash_after_rename_recovers_new_durable_state_without_memory_claim();
		every_atomic_prepare_boundary_recovers_old_or_new_without_regeneration();
		response_install_and_ack_send_boundaries_preserve_exact_ack();
		ack_confirmation_successor_and_cleanup_boundaries_recover_old_or_new();
		terminal_tombstone_gc_boundaries_are_restart_safe();
		reopen_rejects_oversized_files_and_record_count_before_unbounded_work();
		authority_and_terminal_lifetimes_are_exact_and_fail_closed();
		successor_replay_and_cleanup_are_bound_to_the_exact_predecessor();
	}
}
