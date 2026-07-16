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
	collections::BTreeMap,
	fs::{self, File, OpenOptions},
	io::Write,
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
const RECOVERY_BLOCKS: u64 = 256;
const MAX_RECORDS: usize = 4_096;
const MAX_TOTAL_ENCRYPTED_BYTES: u64 = 268_435_456;
const MAX_ENCRYPTED_RECORD_BYTES: usize = 4_456_448;
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_AUTHORITY_BYTES: usize = 4_096;
const ROOT: &str = "host-outbox-v1";
const QUARANTINE: &str = "quarantine";
const EXTENSION: &str = ".outbox";
const RECORD_DOMAIN: &[u8] = b"cord/host-outbox/private-record/v1";

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
			exact_authority_bytes: bounded_bytes(take(&mut fields, 4)?, 1, MAX_AUTHORITY_BYTES)?,
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
		if self.exact_request_bytes.len() > MAX_REQUEST_BYTES
			|| self.exact_authority_bytes.is_empty()
			|| self.exact_authority_bytes.len() > MAX_AUTHORITY_BYTES
			|| self.authority_expires_at < self.created_at
			|| self.recover_until
				< self.created_at.checked_add(RECOVERY_BLOCKS).ok_or(HostOutboxError::Corrupt)?
			|| self.request_fingerprint
				!= request_fingerprint(&self.exact_request_bytes, &self.exact_authority_bytes)
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
	pub(crate) fingerprint: [u8; 32],
}

/// Atomically installed provider response and continuation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostOutboxInstalledResponseV1 {
	pub(crate) response: Vec<u8>,
	pub(crate) response_hash: [u8; 32],
	pub(crate) successor_authority: Option<Vec<u8>>,
	pub(crate) terminal: bool,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct LiveRecordV1 {
	entry_cbor: Vec<u8>,
	response: Option<Vec<u8>>,
	successor_authority: Option<Vec<u8>>,
	terminal: bool,
	record_hash: [u8; 32],
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
struct TombstoneV1 {
	outbox_id: [u8; 16],
	state: HostOutboxStateV1,
	request_fingerprint: [u8; 32],
	prior_response_hash: Option<[u8; 32]>,
	recover_until: u64,
	key_version: u32,
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
enum DurableRecordV1 {
	Live(LiveRecordV1),
	Tombstone(TombstoneV1),
}

#[derive(Clone, Debug)]
struct LoadedRecordV1 {
	record: DurableRecordV1,
	encrypted_bytes: usize,
}

/// Deterministic atomic-write seams used by recovery tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostOutboxFault {
	BeforeTempFsync,
	AfterTempFsync,
	AfterRename,
	AfterDirectoryFsync,
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
	/// Open and authenticate every durable entry. One corrupt entry is quarantined and fails open.
	pub(crate) fn open(
		root: impl AsRef<Path>,
		context: HostOutboxContextV1,
		keys: HostOutboxKeyRingV1,
	) -> Result<Self, HostOutboxError> {
		Self::open_with_limits(root, context, keys, MAX_RECORDS, MAX_TOTAL_ENCRYPTED_BYTES)
	}

	fn open_with_limits(
		root: impl AsRef<Path>,
		context: HostOutboxContextV1,
		keys: HostOutboxKeyRingV1,
		record_limit: usize,
		byte_limit: u64,
	) -> Result<Self, HostOutboxError> {
		if record_limit == 0
			|| record_limit > MAX_RECORDS
			|| byte_limit == 0
			|| byte_limit > MAX_TOTAL_ENCRYPTED_BYTES
		{
			return Err(HostOutboxError::Unavailable);
		}
		let root = root.as_ref().join(ROOT);
		let quarantine = root.join(QUARANTINE);
		ensure_directory(&root)?;
		ensure_directory(&quarantine)?;
		let mut records = BTreeMap::new();
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
			let bytes = fs::read(item.path()).map_err(|_| HostOutboxError::Unavailable)?;
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
		let (key_version, _) = self.keys.active()?;
		let recover_until = input
			.authority_expires_at
			.checked_add(RECOVERY_BLOCKS)
			.ok_or(HostOutboxError::Corrupt)?;
		let fingerprint =
			request_fingerprint(&input.exact_request_bytes, &input.exact_authority_bytes);
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
		let record = live_record(entry.clone(), None, None, false);
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		if records.contains_key(&entry.outbox_id) {
			return Err(HostOutboxError::StateInvalid);
		}
		self.persist_new(&records, entry.outbox_id, record, nonce)?;
		let bytes = fs::metadata(self.path(entry.outbox_id))
			.map_err(|_| HostOutboxError::Unavailable)?
			.len()
			.try_into()
			.map_err(|_| HostOutboxError::Full)?;
		records.insert(
			entry.outbox_id,
			LoadedRecordV1 {
				record: live_record(entry.clone(), None, None, false),
				encrypted_bytes: bytes,
			},
		);
		Ok(HostOutboxRetryV1 {
			request: entry.exact_request_bytes,
			authority: entry.exact_authority_bytes,
			fingerprint,
		})
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
			fingerprint: entry.request_fingerprint,
		})
	}

	/// Persist the advisory Sent state; the exact Prepared bytes remain authoritative.
	pub(crate) fn mark_sent(
		&self,
		outbox_id: [u8; 16],
		nonce: [u8; 24],
	) -> Result<(), HostOutboxError> {
		self.rewrite_live(outbox_id, nonce, |entry, _, _, _| {
			if entry.state != HostOutboxStateV1::Prepared {
				return Err(HostOutboxError::StateInvalid);
			}
			entry.state = HostOutboxStateV1::Sent;
			Ok((None, None, false))
		})
	}

	/// Install exact authenticated response bytes before an acknowledgement may be sent.
	pub(crate) fn install_response(
		&self,
		outbox_id: [u8; 16],
		response: Vec<u8>,
		successor_authority: Option<Vec<u8>>,
		terminal_block: Option<u64>,
		nonce: [u8; 24],
	) -> Result<[u8; 32], HostOutboxError> {
		let response_hash: [u8; 32] = Sha256::digest(&response).into();
		self.rewrite_live(outbox_id, nonce, move |entry, _, _, _| {
			if !matches!(entry.state, HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent) {
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
			if let Some(block) = terminal_block {
				entry.recover_until =
					block.checked_add(RECOVERY_BLOCKS).ok_or(HostOutboxError::Corrupt)?;
			}
			Ok((Some(response), successor_authority, terminal_block.is_some()))
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
		Ok(HostOutboxInstalledResponseV1 {
			response,
			response_hash,
			successor_authority: live.successor_authority.clone(),
			terminal: live.terminal,
		})
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
			return if tombstone.state == HostOutboxStateV1::AckConfirmed
				&& tombstone.prior_response_hash == Some(response_hash)
			{
				Ok(())
			} else {
				Err(HostOutboxError::ResponseMismatch)
			};
		}
		let existing = live(&loaded)?.clone();
		let mut entry = decode_entry(&existing)?;
		if entry.state == HostOutboxStateV1::AckConfirmed
			&& entry.prior_response_hash == Some(response_hash)
		{
			return Ok(());
		}
		if entry.state != HostOutboxStateV1::ResponseInstalled
			|| entry.prior_response_hash != Some(response_hash)
		{
			return Err(HostOutboxError::ResponseMismatch);
		}
		let active_version = self.keys.active()?.0;
		let record = if existing.terminal {
			DurableRecordV1::Tombstone(TombstoneV1 {
				outbox_id,
				state: HostOutboxStateV1::AckConfirmed,
				request_fingerprint: entry.request_fingerprint,
				prior_response_hash: Some(response_hash),
				recover_until: entry.recover_until,
				key_version: active_version,
			})
		} else {
			entry.state = HostOutboxStateV1::AckConfirmed;
			entry.key_version = active_version;
			live_record(entry, existing.response, existing.successor_authority, false)
		};
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
		if finalized < entry.recover_until {
			return Err(HostOutboxError::StateInvalid);
		}
		let tombstone = TombstoneV1 {
			outbox_id,
			state: HostOutboxStateV1::Expired,
			request_fingerprint: entry.request_fingerprint,
			prior_response_hash: entry.prior_response_hash,
			recover_until: entry.recover_until,
			key_version: self.keys.active()?.0,
		};
		let record = DurableRecordV1::Tombstone(tombstone);
		let encrypted_bytes = self.persist_replace(&records, outbox_id, record.clone(), nonce)?;
		records.insert(outbox_id, LoadedRecordV1 { record, encrypted_bytes });
		Ok(())
	}

	/// Remove only terminal/expired records whose full recovery window has closed.
	pub(crate) fn gc(&self, finalized: u64, limit: usize) -> Result<usize, HostOutboxError> {
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let mut selected = Vec::new();
		for (id, loaded) in records.iter() {
			if selected.len() >= limit {
				break;
			}
			let eligible = match &loaded.record {
				DurableRecordV1::Tombstone(tombstone) => finalized >= tombstone.recover_until,
				DurableRecordV1::Live(record) => {
					let entry = decode_entry(record)?;
					record.terminal
						&& entry.state == HostOutboxStateV1::AckConfirmed
						&& finalized >= entry.recover_until
				},
			};
			if eligible {
				selected.push(*id);
			}
		}
		for id in &selected {
			fs::remove_file(self.path(*id)).map_err(|_| HostOutboxError::Unavailable)?;
		}
		if !selected.is_empty() {
			sync_dir(&self.root)?;
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
			bool,
		) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>, bool), HostOutboxError>,
	{
		let mut records = self.records.write().map_err(|_| HostOutboxError::Unavailable)?;
		let loaded = records.get(&outbox_id).cloned().ok_or(HostOutboxError::StateInvalid)?;
		let existing = live(&loaded)?.clone();
		let mut entry = decode_entry(&existing)?;
		entry.key_version = self.keys.active()?.0;
		let (response, successor, terminal) = transition(
			&mut entry,
			existing.response,
			existing.successor_authority,
			existing.terminal,
		)?;
		let record = live_record(entry, response, successor, terminal);
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
	response: Option<Vec<u8>>,
	successor_authority: Option<Vec<u8>>,
	terminal: bool,
) -> DurableRecordV1 {
	let mut live = LiveRecordV1 {
		entry_cbor: entry.canonical_bytes(),
		response,
		successor_authority,
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
	}
}

fn decode_entry(record: &LiveRecordV1) -> Result<HostOutboxEntryV1, HostOutboxError> {
	if record_hash(record) != record.record_hash {
		return Err(HostOutboxError::Corrupt);
	}
	let entry =
		HostOutboxEntryV1::decode(&record.entry_cbor).map_err(|_| HostOutboxError::Corrupt)?;
	match (entry.state, record.response.as_ref()) {
		(HostOutboxStateV1::Prepared | HostOutboxStateV1::Sent, None) => {},
		(
			HostOutboxStateV1::ResponseInstalled | HostOutboxStateV1::AckConfirmed,
			Some(response),
		) if entry.prior_response_hash == Some(Sha256::digest(response).into()) => {},
		_ => return Err(HostOutboxError::Corrupt),
	}
	Ok(entry)
}

fn record_hash(record: &LiveRecordV1) -> [u8; 32] {
	let mut hash = Sha256::new();
	hash.update(RECORD_DOMAIN);
	hash.update((record.entry_cbor.len() as u64).to_be_bytes());
	hash.update(&record.entry_cbor);
	for bytes in [record.response.as_ref(), record.successor_authority.as_ref()] {
		match bytes {
			Some(bytes) => {
				hash.update([1]);
				hash.update((bytes.len() as u64).to_be_bytes());
				hash.update(bytes);
			},
			None => hash.update([0]),
		}
	}
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
			if entry.outbox_id != id
				|| entry.registry_hash != context.registry_hash
				|| entry.genesis_hash != context.genesis_hash
				|| entry.key_version != key_version
				|| (!record.terminal && entry.recover_until != initial_recovery)
				|| record
					.successor_authority
					.as_ref()
					.is_some_and(|bytes| bytes.is_empty() || bytes.len() > MAX_AUTHORITY_BYTES)
			{
				return Err(HostOutboxError::Corrupt);
			}
		},
		DurableRecordV1::Tombstone(tombstone) => {
			if tombstone.outbox_id != id
				|| !matches!(
					tombstone.state,
					HostOutboxStateV1::AckConfirmed | HostOutboxStateV1::Expired
				) || tombstone.key_version != key_version
			{
				return Err(HostOutboxError::Corrupt);
			}
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

fn take(fields: &mut [Option<Value>; 21], index: usize) -> Result<Value, HostOutboxError> {
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
		let hash = store.install_response(id, response.clone(), None, Some(300), [3; 24]).unwrap();
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let installed = store.installed_response(id).unwrap();
		assert_eq!(installed.response, response);
		assert_eq!(installed.response_hash, hash);
		assert_eq!(installed.successor_authority, None);
		assert!(installed.terminal);
		assert_eq!(store.confirm_ack(id, [9; 32], [4; 24]), Err(HostOutboxError::ResponseMismatch));
		store.confirm_ack(id, hash, [5; 24]).unwrap();
		assert_eq!(store.confirm_ack(id, hash, [6; 24]), Ok(()));
		assert_eq!(store.installed_response(id), Err(HostOutboxError::Expired));
		assert_eq!(store.gc(555, 16).unwrap(), 0);
		assert_eq!(store.gc(556, 16).unwrap(), 1);
	}

	#[test]
	fn nonterminal_successor_is_durable_and_never_terminal_gc_eligible() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		store.prepare(input, [1; 24]).unwrap();
		let successor = b"signed-successor".to_vec();
		let hash = store
			.install_response(id, b"progress".to_vec(), Some(successor.clone()), None, [2; 24])
			.unwrap();
		store.confirm_ack(id, hash, [3; 24]).unwrap();
		drop(store);
		let reopened = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		assert_eq!(reopened.installed_response(id).unwrap().successor_authority, Some(successor));
		assert_eq!(reopened.gc(u64::MAX, 1).unwrap(), 0);
	}

	#[test]
	fn expiry_erases_authority_and_corrupt_ciphertext_is_quarantined() {
		let temp = tempfile::tempdir().unwrap();
		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
		let input = prepared();
		let id = input.outbox_id;
		store.prepare(input, [1; 24]).unwrap();
		assert_eq!(store.retry_request(id, 484), Err(HostOutboxError::Expired));
		store.expire(id, 484, [2; 24]).unwrap();
		assert_eq!(store.retry_request(id, 484), Err(HostOutboxError::Expired));
		drop(store);

		let store = HostOutboxStoreV1::open(temp.path(), context(), keyring()).unwrap();
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
}
