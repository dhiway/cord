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

//! Private durable bucket-local MMR commitments over verified streaming installations.
//!
//! Immutable per-install records are the append commit points. Small global, bucket, and
//! idempotency metadata are derived indexes: a restart validates or completes a stale index from
//! the immutable records. Corrupt bytes make only their bucket unavailable. Reconciliation stops
//! at the first bad global suffix source, without invalidating already complete healthy buckets.

use std::{
	collections::{BTreeSet, HashMap},
	fs::{self, File, OpenOptions},
	io::{Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::Encode;
use pallet_orbis_storage_provider::{CommitmentV1, MmrLeafV1};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::H256;
use sp_crypto_hashing::blake2_256;

use super::streaming::{StreamingStore, VerifiedInstallation};
use crate::{
	peer::{PeerMmrCommitmentV1, PeerObjectV1, PeerPageCursorV1},
	BucketId, CanonicalCid, ContentError, OperationId, MAX_STREAMING_OPERATIONS,
};

const VERSION: u16 = 3;
const ROOT: &str = "bucket-mmr-v3";
const LOG: &str = "leaves.v1.log";
const META: &str = "meta.v1";
const MAX_FRAME_PAYLOAD: usize = 2_048;
const MAX_META_BYTES: usize = 8_192;
const CHECKSUM_BYTES: usize = 32;
const LENGTH_BYTES: usize = 4;
const MAX_FRAME_BYTES: usize = LENGTH_BYTES + MAX_FRAME_PAYLOAD + CHECKSUM_BYTES;
const MAX_LOG_BYTES: u64 = MAX_STREAMING_OPERATIONS as u64 * MAX_FRAME_BYTES as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BucketMmrFault {
	PartialFrame,
	AfterLogSync,
	AfterMetaTempSync,
	AfterMetaRename,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BucketMeta {
	version: u16,
	entry_count: u64,
	confirmed_log_bytes: u64,
	peaks: Vec<String>,
	total_size: u64,
	root: Option<String>,
}

impl Default for BucketMeta {
	fn default() -> Self {
		Self {
			version: VERSION,
			entry_count: 0,
			confirmed_log_bytes: 0,
			peaks: Vec::new(),
			total_size: 0,
			root: None,
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
	version: u16,
	bucket_id: String,
	sequence: u64,
	install_sequence: u64,
	operation_id: String,
	cid: String,
	data_size: u64,
	total_size: u64,
	leaf_hash: String,
}

#[derive(Default)]
struct BucketRuntime {
	entries: Vec<Entry>,
	meta: BucketMeta,
	unavailable: bool,
	blocked_at: Option<u64>,
	source_order: Vec<u64>,
}

#[derive(Default)]
struct State {
	buckets: HashMap<BucketId, BucketRuntime>,
	known_sources: HashMap<u64, VerifiedInstallation>,
	committed_sources: HashMap<u64, (BucketId, usize)>,
	operations: HashMap<(BucketId, OperationId), u64>,
	next_new_install_sequence: u64,
}

/// Private commitment substrate. It is deliberately not attached to any public route.
pub(crate) struct BucketMmrStore {
	root: PathBuf,
	state: RwLock<State>,
	fault: RwLock<Option<BucketMmrFault>>,
}

impl BucketMmrStore {
	/// Open every bucket independently and reconcile each contiguous verified bucket suffix.
	pub(crate) fn open(
		root: impl AsRef<Path>,
		streaming: &StreamingStore,
	) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let records = streaming.installation_records()?;
		let mut state = State::default();
		for record in records {
			state.next_new_install_sequence = state
				.next_new_install_sequence
				.max(record.install_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?);
			state
				.buckets
				.entry(record.bucket_id)
				.or_default()
				.source_order
				.push(record.install_sequence);
			state.known_sources.insert(record.install_sequence, record);
		}
		let mut bucket_ids = state.buckets.keys().copied().collect::<BTreeSet<_>>();
		for item in fs::read_dir(&root).map_err(io_error)? {
			let item = item.map_err(io_error)?;
			if !item.file_type().map_err(io_error)?.is_dir() {
				return Err(ContentError::IntegrityFailed);
			}
			bucket_ids.insert(BucketId::parse(&item.file_name().to_string_lossy())?);
		}
		for bucket_id in bucket_ids {
			let bucket = state.buckets.entry(bucket_id).or_default();
			if open_bucket(&root, bucket_id, bucket).is_err() {
				bucket.unavailable = true;
			}
		}
		index_confirmed_entries(&mut state)?;
		for (bucket_id, bucket) in &mut state.buckets {
			if bucket.unavailable {
				continue;
			}
			for (index, entry) in bucket.entries.iter().enumerate() {
				let Some(source) = state.known_sources.get(&entry.install_sequence) else {
					bucket.unavailable = true;
					break;
				};
				if bucket.source_order.get(index).copied() != Some(entry.install_sequence)
					|| !entry_matches_installation(entry, source)?
					|| source.bucket_id != *bucket_id
				{
					bucket.unavailable = true;
					break;
				}
				if streaming.verified_installation(source.bucket_id, source.operation_id).is_err() {
					bucket.unavailable = true;
					bucket.blocked_at = Some(source.install_sequence);
					break;
				}
			}
		}
		let store = Self { root, state: RwLock::new(state), fault: RwLock::new(None) };
		let bucket_ids = store
			.state
			.read()
			.map_err(|_| lock_error())?
			.buckets
			.keys()
			.copied()
			.collect::<Vec<_>>();
		for bucket_id in bucket_ids {
			loop {
				let next = {
					let state = store.state.read().map_err(|_| lock_error())?;
					let bucket = state.buckets.get(&bucket_id).expect("bucket exists");
					if bucket.unavailable || bucket.entries.len() >= bucket.source_order.len() {
						None
					} else {
						let sequence = bucket.source_order[bucket.entries.len()];
						state.known_sources.get(&sequence).cloned()
					}
				};
				let Some(source) = next else { break };
				if store.append_verified(streaming, source.bucket_id, source.operation_id).is_err()
				{
					let mut state = store.state.write().map_err(|_| lock_error())?;
					let bucket = state.buckets.get_mut(&bucket_id).expect("bucket exists");
					bucket.unavailable = true;
					bucket.blocked_at = Some(source.install_sequence);
					break;
				}
			}
		}
		Ok(store)
	}

	#[doc(hidden)]
	pub(crate) fn inject_fault_once(&self, fault: BucketMmrFault) -> Result<(), ContentError> {
		*self.fault.write().map_err(|_| lock_error())? = Some(fault);
		Ok(())
	}

	/// Append one exact next source for its bucket, or accept an exact committed replay.
	pub(crate) fn append_verified(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		operation_id: OperationId,
	) -> Result<(), ContentError> {
		let installation = streaming.verified_installation(bucket_id, operation_id)?;
		let mut state = self.state.write().map_err(|_| lock_error())?;
		if let Some(sequence) = state.operations.get(&(bucket_id, operation_id)).copied() {
			let (committed_bucket, index) = state
				.committed_sources
				.get(&sequence)
				.copied()
				.ok_or(ContentError::IntegrityFailed)?;
			let entry = &state.buckets[&committed_bucket].entries[index];
			return if entry_matches_installation(entry, &installation)? {
				Ok(())
			} else {
				Err(ContentError::IdempotencyConflict)
			};
		}
		if let Some(known) = state.known_sources.get(&installation.install_sequence) {
			if known != &installation {
				return Err(ContentError::IdempotencyConflict);
			}
		} else {
			if installation.install_sequence != state.next_new_install_sequence {
				return Err(ContentError::IntegrityFailed);
			}
			state.next_new_install_sequence = installation
				.install_sequence
				.checked_add(1)
				.ok_or(ContentError::IntegrityFailed)?;
			state
				.buckets
				.entry(bucket_id)
				.or_default()
				.source_order
				.push(installation.install_sequence);
			state.known_sources.insert(installation.install_sequence, installation.clone());
		}
		let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::IntegrityFailed)?;
		if bucket.unavailable
			|| bucket.meta.entry_count >= MAX_STREAMING_OPERATIONS as u64
			|| bucket.source_order.get(bucket.entries.len()).copied()
				!= Some(installation.install_sequence)
		{
			return Err(ContentError::IntegrityFailed);
		}
		let entry = build_entry(&installation, &bucket.meta)?;
		let frame = encode_frame(&entry)?;
		let next_meta = advance_meta(&bucket.meta, &entry, frame.len() as u64)?;
		let result = self.persist_append(bucket_id, &bucket.meta, &next_meta, &frame);
		if let Err(error) = result {
			state.buckets.get_mut(&bucket_id).expect("bucket exists").unavailable = true;
			return Err(error);
		}
		let bucket = state.buckets.get_mut(&bucket_id).expect("bucket exists");
		let index = bucket.entries.len();
		bucket.entries.push(entry.clone());
		bucket.meta = next_meta;
		bucket.blocked_at = None;
		state
			.operations
			.insert((bucket_id, operation_id), installation.install_sequence);
		state
			.committed_sources
			.insert(installation.install_sequence, (bucket_id, index));
		Ok(())
	}

	/// Revalidate every committed entry after a quarantined object repair and make the bucket
	/// available only when the complete in-memory index still matches durable streaming state.
	pub(crate) fn revalidate_repaired_bucket(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
	) -> Result<(), ContentError> {
		let snapshot = {
			let state = self.state.read().map_err(|_| lock_error())?;
			let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
			(bucket.entries.clone(), bucket.source_order.clone(), bucket.meta.clone())
		};
		let (entries, source_order, meta) = snapshot;
		if entries.len() != source_order.len() || entries.len() as u64 != meta.entry_count {
			return Err(ContentError::IntegrityFailed);
		}
		for (index, entry) in entries.iter().enumerate() {
			if source_order.get(index) != Some(&entry.install_sequence) {
				return Err(ContentError::IntegrityFailed);
			}
			let installation = streaming
				.verified_installation(bucket_id, OperationId::parse(&entry.operation_id)?)?;
			if !entry_matches_installation(entry, &installation)? {
				return Err(ContentError::IntegrityFailed);
			}
		}
		let mut state = self.state.write().map_err(|_| lock_error())?;
		let bucket = state.buckets.get_mut(&bucket_id).ok_or(ContentError::NotFound)?;
		if bucket.entries != entries || bucket.source_order != source_order || bucket.meta != meta {
			return Err(ContentError::IdempotencyConflict);
		}
		bucket.unavailable = false;
		bucket.blocked_at = None;
		Ok(())
	}

	/// Return whether the exact logical replication leaf already occupies its sequence.
	/// An absent next suffix slot is appendable; a gap or occupied mismatch fails closed.
	pub(crate) fn replication_slot_structurally_matches(
		&self,
		bucket_id: BucketId,
		object: &PeerObjectV1,
	) -> Result<bool, ContentError> {
		let (length, sequence, cumulative_total) = object.position();
		self.replication_position_structurally_matches(
			bucket_id,
			sequence,
			object.cid(),
			length,
			cumulative_total,
		)
	}

	/// Return whether an admitted empty object already occupies its exact logical position.
	pub(crate) fn zero_replication_slot_structurally_matches(
		&self,
		bucket_id: BucketId,
		sequence: u64,
		cid: &str,
		cumulative_total: u64,
	) -> Result<bool, ContentError> {
		self.replication_position_structurally_matches(
			bucket_id,
			sequence,
			cid,
			0,
			cumulative_total,
		)
	}

	fn replication_position_structurally_matches(
		&self,
		bucket_id: BucketId,
		sequence: u64,
		cid: &str,
		length: u64,
		cumulative_total: u64,
	) -> Result<bool, ContentError> {
		let state = self.state.read().map_err(|_| lock_error())?;
		let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
		let index: usize = sequence.try_into().map_err(|_| ContentError::IntegrityFailed)?;
		if index == bucket.entries.len() {
			return Ok(false);
		}
		let entry = bucket.entries.get(index).ok_or(ContentError::IntegrityFailed)?;
		if entry.sequence != sequence
			|| entry.cid != cid
			|| entry.data_size != length
			|| entry.total_size != cumulative_total
		{
			return Err(ContentError::IdempotencyConflict);
		}
		Ok(true)
	}

	/// Reverify the bytes behind a structurally matching logical replication slot.
	pub(crate) fn replication_slot_matches(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		object: &PeerObjectV1,
	) -> Result<bool, ContentError> {
		let (length, sequence, cumulative_total) = object.position();
		let entry = {
			let state = self.state.read().map_err(|_| lock_error())?;
			let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
			if bucket.unavailable {
				return Err(ContentError::IntegrityFailed);
			}
			let index: usize = sequence.try_into().map_err(|_| ContentError::IntegrityFailed)?;
			if index == bucket.entries.len() {
				return Ok(false);
			}
			bucket.entries.get(index).cloned().ok_or(ContentError::IntegrityFailed)?
		};
		if entry.sequence != sequence
			|| entry.cid != object.cid()
			|| entry.data_size != length
			|| entry.total_size != cumulative_total
		{
			return Err(ContentError::IdempotencyConflict);
		}
		let installed = streaming
			.verified_replication_object(bucket_id, OperationId::parse(&entry.operation_id)?)?;
		if installed.cid.as_str() != object.cid()
			|| installed.stored_bytes != length
			|| installed.chunk_hashes != object.chunk_hashes()
		{
			return Err(ContentError::IdempotencyConflict);
		}
		Ok(true)
	}

	/// Build the exact runtime commitment fields after re-verifying only this bucket's bytes.
	pub(crate) fn commitment_candidate(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		expected_start_seq: u64,
	) -> Result<CommitmentV1<H256>, ContentError> {
		let structural = streaming.installation_records()?;
		let structural_count =
			structural.iter().filter(|source| source.bucket_id == bucket_id).count();
		let state = self.state.read().map_err(|_| lock_error())?;
		let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
		if bucket.unavailable
			|| bucket.entries.len() != bucket.source_order.len()
			|| bucket.entries.len() != structural_count
		{
			return Err(ContentError::IntegrityFailed);
		}
		for entry in &bucket.entries {
			let source = streaming
				.verified_installation(bucket_id, OperationId::parse(&entry.operation_id)?)?;
			if !entry_matches_installation(entry, &source)? {
				return Err(ContentError::IntegrityFailed);
			}
		}
		let leaf_count = bucket
			.meta
			.entry_count
			.checked_sub(expected_start_seq)
			.ok_or(ContentError::IntegrityFailed)?;
		if leaf_count == 0 {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(CommitmentV1 {
			mmr_root: decode_hash(
				bucket.meta.root.as_deref().ok_or(ContentError::IntegrityFailed)?,
			)?,
			start_seq: expected_start_seq,
			leaf_count,
		})
	}

	/// Return the exact cumulative total immediately before a candidate range after rebuilding the
	/// committed MMR structure. This deliberately does not verify object bytes or claim that an
	/// unavailable bucket is publishable; it only exposes predecessor size evidence when the
	/// durable entry structure itself is exact.
	pub(crate) fn commitment_predecessor_total(
		&self,
		bucket_id: BucketId,
		expected_start_seq: u64,
	) -> Result<u64, ContentError> {
		let state = self.state.read().map_err(|_| lock_error())?;
		let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
		if expected_start_seq > bucket.meta.entry_count
			|| bucket.entries.len() as u64 != bucket.meta.entry_count
		{
			return Err(ContentError::IntegrityFailed);
		}
		let mut rebuilt = BucketMeta::default();
		for (index, entry) in bucket.entries.iter().enumerate() {
			if entry.version != VERSION
				|| entry.bucket_id != bucket_id.to_string()
				|| entry.sequence != index as u64
				|| bucket.source_order.get(index) != Some(&entry.install_sequence)
			{
				return Err(ContentError::IntegrityFailed);
			}
			let frame = encode_frame(entry)?;
			rebuilt = advance_meta(&rebuilt, entry, frame.len() as u64)?;
		}
		if rebuilt != bucket.meta {
			return Err(ContentError::IntegrityFailed);
		}
		if expected_start_seq == 0 {
			return Ok(0);
		}
		let predecessor: usize =
			(expected_start_seq - 1).try_into().map_err(|_| ContentError::IntegrityFailed)?;
		bucket
			.entries
			.get(predecessor)
			.filter(|entry| entry.sequence == expected_start_seq - 1)
			.map(|entry| entry.total_size)
			.ok_or(ContentError::IntegrityFailed)
	}

	/// Build one bounded, contiguous replication page from fully reverified installed objects.
	pub(crate) fn replication_page(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		commitment: PeerMmrCommitmentV1,
		cursor: Option<PeerPageCursorV1>,
		limit: u16,
	) -> Result<(Vec<PeerObjectV1>, Option<PeerPageCursorV1>), ContentError> {
		const MAX_REPLICATION_PAGE_ITEMS: usize = 128;
		if limit == 0 || usize::from(limit) > MAX_REPLICATION_PAGE_ITEMS {
			return Err(ContentError::SchemaInvalid);
		}
		commitment.validate()?;
		let (start, end) = commitment.sequence_range();
		let candidate = self.commitment_candidate(streaming, bucket_id, start)?;
		if candidate.mmr_root != H256::from(commitment.mmr_root())
			|| candidate.start_seq != start
			|| candidate.leaf_count != end - start
		{
			return Err(ContentError::IntegrityFailed);
		}

		let (entries, mut sequence, mut prior_total) = {
			let state = self.state.read().map_err(|_| lock_error())?;
			let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
			let entry_count: u64 =
				bucket.entries.len().try_into().map_err(|_| ContentError::IntegrityFailed)?;
			if bucket.unavailable || entry_count != end || bucket.meta.entry_count != end {
				return Err(ContentError::IntegrityFailed);
			}
			let predecessor_total = if start == 0 {
				0
			} else {
				let predecessor: usize =
					(start - 1).try_into().map_err(|_| ContentError::IntegrityFailed)?;
				bucket.entries.get(predecessor).ok_or(ContentError::IntegrityFailed)?.total_size
			};
			if predecessor_total != commitment.predecessor_total_size() {
				return Err(ContentError::IntegrityFailed);
			}
			let (sequence, prior_total) = match cursor {
				None => (start, predecessor_total),
				Some(cursor) => {
					let (last_sequence, cumulative_total) = cursor.position();
					let next = last_sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
					let last: usize =
						last_sequence.try_into().map_err(|_| ContentError::IntegrityFailed)?;
					let stored_total = bucket
						.entries
						.get(last)
						.filter(|entry| entry.sequence == last_sequence)
						.map(|entry| entry.total_size)
						.ok_or(ContentError::IntegrityFailed)?;
					if last_sequence < start || next >= end || cumulative_total != stored_total {
						return Err(ContentError::IntegrityFailed);
					}
					(next, cumulative_total)
				},
			};
			let remaining: usize =
				(end - sequence).try_into().map_err(|_| ContentError::IntegrityFailed)?;
			let take = usize::from(limit).min(remaining);
			let from: usize = sequence.try_into().map_err(|_| ContentError::IntegrityFailed)?;
			let to = from.checked_add(take).ok_or(ContentError::IntegrityFailed)?;
			let entries =
				bucket.entries.get(from..to).ok_or(ContentError::IntegrityFailed)?.to_vec();
			(entries, sequence, prior_total)
		};

		let mut items = Vec::with_capacity(entries.len());
		for entry in entries {
			if entry.sequence != sequence {
				return Err(ContentError::IntegrityFailed);
			}
			let expected_total =
				prior_total.checked_add(entry.data_size).ok_or(ContentError::IntegrityFailed)?;
			if entry.total_size != expected_total {
				return Err(ContentError::IntegrityFailed);
			}
			let operation_id = OperationId::parse(&entry.operation_id)?;
			let source = streaming.verified_replication_object(bucket_id, operation_id)?;
			if source.bucket_id != bucket_id
				|| source.operation_id != operation_id
				|| source.cid.as_str() != entry.cid
				|| source.stored_bytes != entry.data_size
			{
				return Err(ContentError::IntegrityFailed);
			}
			let leaf = MmrLeafV1 {
				data_root: H256::from(source.cid.digest()),
				data_size: source.stored_bytes,
				total_size: entry.total_size,
			};
			if decode_hash(&entry.leaf_hash)? != hash_leaf(&leaf) {
				return Err(ContentError::IntegrityFailed);
			}
			items.push(PeerObjectV1::new(
				&source.cid,
				source.stored_bytes,
				entry.sequence,
				entry.total_size,
				source.chunk_hashes,
			)?);
			sequence = sequence.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
			prior_total = entry.total_size;
		}
		if items.is_empty() {
			return Err(ContentError::IntegrityFailed);
		}
		let next_cursor = if sequence == end {
			None
		} else {
			Some(PeerPageCursorV1::new(sequence - 1, prior_total))
		};
		Ok((items, next_cursor))
	}

	/// Prove that a target-signed chunk descriptor is the exact committed object at its sequence.
	pub(crate) fn verify_replication_object(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		commitment: PeerMmrCommitmentV1,
		object: &PeerObjectV1,
	) -> Result<(), ContentError> {
		let (_, sequence, _) = object.position();
		let (start, end) = commitment.sequence_range();
		if sequence < start || sequence >= end {
			return Err(ContentError::IntegrityFailed);
		}
		let cursor = if sequence == start {
			None
		} else {
			let state = self.state.read().map_err(|_| lock_error())?;
			let bucket = state.buckets.get(&bucket_id).ok_or(ContentError::NotFound)?;
			let predecessor = sequence.checked_sub(1).ok_or(ContentError::IntegrityFailed)?;
			let index: usize = predecessor.try_into().map_err(|_| ContentError::IntegrityFailed)?;
			let total = bucket
				.entries
				.get(index)
				.filter(|entry| entry.sequence == predecessor)
				.map(|entry| entry.total_size)
				.ok_or(ContentError::IntegrityFailed)?;
			Some(PeerPageCursorV1::new(predecessor, total))
		};
		let (items, _) = self.replication_page(streaming, bucket_id, commitment, cursor, 1)?;
		if items.as_slice() != [object.clone()] {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(())
	}

	fn persist_append(
		&self,
		bucket_id: BucketId,
		current: &BucketMeta,
		next: &BucketMeta,
		frame: &[u8],
	) -> Result<(), ContentError> {
		let directory = self.root.join(bucket_id.to_string());
		fs::create_dir_all(&directory).map_err(io_error)?;
		let log_path = directory.join(LOG);
		let mut log = OpenOptions::new()
			.create(true)
			.read(true)
			.write(true)
			.open(&log_path)
			.map_err(io_error)?;
		let length = log.metadata().map_err(io_error)?.len();
		if length != current.confirmed_log_bytes {
			return Err(ContentError::IntegrityFailed);
		}
		log.seek(SeekFrom::End(0)).map_err(io_error)?;
		if self.take_fault(BucketMmrFault::PartialFrame)? {
			let partial = (frame.len() / 2).max(1);
			log.write_all(&frame[..partial]).map_err(io_error)?;
			log.sync_all().map_err(io_error)?;
			return Err(ContentError::Io("injected partial bucket MMR frame".into()));
		}
		log.write_all(frame).map_err(io_error)?;
		log.sync_all().map_err(io_error)?;
		self.trip_fault(BucketMmrFault::AfterLogSync)?;
		let bytes = encode_json(next, MAX_META_BYTES)?;
		let meta_path = directory.join(META);
		let temporary =
			directory.join(format!("{META}.tmp-{}-{}", std::process::id(), next.entry_count));
		let mut file = File::create(&temporary).map_err(io_error)?;
		file.write_all(&bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		self.trip_fault(BucketMmrFault::AfterMetaTempSync)?;
		fs::rename(temporary, meta_path).map_err(io_error)?;
		self.trip_fault(BucketMmrFault::AfterMetaRename)?;
		sync_dir(&directory)
	}

	fn take_fault(&self, point: BucketMmrFault) -> Result<bool, ContentError> {
		let mut fault = self.fault.write().map_err(|_| lock_error())?;
		if fault.as_ref() == Some(&point) {
			*fault = None;
			Ok(true)
		} else {
			Ok(false)
		}
	}

	fn trip_fault(&self, point: BucketMmrFault) -> Result<(), ContentError> {
		if self.take_fault(point)? {
			Err(ContentError::Io(format!("injected bucket MMR fault: {point:?}")))
		} else {
			Ok(())
		}
	}
}

fn open_bucket(
	root: &Path,
	bucket_id: BucketId,
	bucket: &mut BucketRuntime,
) -> Result<(), ContentError> {
	let directory = root.join(bucket_id.to_string());
	fs::create_dir_all(&directory).map_err(io_error)?;
	remove_meta_temps(&directory)?;
	let log_path = directory.join(LOG);
	if !log_path.exists() {
		File::create(&log_path).and_then(|file| file.sync_all()).map_err(io_error)?;
		sync_dir(&directory)?;
	}
	let log_len = fs::metadata(&log_path).map_err(io_error)?.len();
	if log_len > MAX_LOG_BYTES {
		bucket.unavailable = true;
		return Ok(());
	}
	let meta_path = directory.join(META);
	let meta = if meta_path.exists() {
		match read_json::<BucketMeta>(&meta_path, MAX_META_BYTES) {
			Ok(meta)
				if meta.version == VERSION
					&& meta.peaks.len() <= 64
					&& meta.entry_count <= MAX_STREAMING_OPERATIONS as u64
					&& meta.confirmed_log_bytes <= MAX_LOG_BYTES =>
			{
				meta
			},
			_ => {
				bucket.unavailable = true;
				return Ok(());
			},
		}
	} else {
		BucketMeta::default()
	};
	if meta.confirmed_log_bytes > log_len {
		bucket.unavailable = true;
		return Ok(());
	}
	let mut parsed_frames = 0usize;
	let confirmed = match read_confirmed_frames(
		&log_path,
		meta.confirmed_log_bytes,
		meta.entry_count,
		&mut parsed_frames,
	) {
		Ok(entries) => entries,
		Err(_) => {
			bucket.unavailable = true;
			return Ok(());
		},
	};
	let mut rebuilt = BucketMeta::default();
	for (index, entry) in confirmed.iter().enumerate() {
		if entry.version != VERSION
			|| entry.bucket_id != bucket_id.to_string()
			|| entry.sequence != index as u64
		{
			bucket.unavailable = true;
			return Ok(());
		}
		let next =
			encode_frame(entry).and_then(|frame| advance_meta(&rebuilt, entry, frame.len() as u64));
		match next {
			Ok(next) => rebuilt = next,
			Err(_) => {
				bucket.unavailable = true;
				return Ok(());
			},
		}
	}
	if rebuilt != meta {
		bucket.unavailable = true;
		return Ok(());
	}
	if log_len > meta.confirmed_log_bytes {
		let file = OpenOptions::new().write(true).open(&log_path).map_err(io_error)?;
		file.set_len(meta.confirmed_log_bytes).map_err(io_error)?;
		file.sync_all().map_err(io_error)?;
		sync_dir(&directory)?;
	}
	if !meta_path.exists() {
		persist_meta(&directory, &meta)?;
	}
	bucket.entries = confirmed;
	bucket.meta = meta;
	Ok(())
}

fn index_confirmed_entries(state: &mut State) -> Result<(), ContentError> {
	let mut duplicate_sources: HashMap<u64, BucketId> = HashMap::new();
	let mut duplicate_operations: HashMap<(BucketId, OperationId), BucketId> = HashMap::new();
	let mut unavailable = BTreeSet::new();
	for (bucket_id, bucket) in &state.buckets {
		if bucket.unavailable {
			continue;
		}
		for entry in &bucket.entries {
			let operation = match OperationId::parse(&entry.operation_id) {
				Ok(operation) => operation,
				Err(_) => {
					unavailable.insert(*bucket_id);
					continue;
				},
			};
			if let Some(previous) = duplicate_sources.insert(entry.install_sequence, *bucket_id) {
				unavailable.insert(previous);
				unavailable.insert(*bucket_id);
			}
			if let Some(previous) = duplicate_operations.insert((*bucket_id, operation), *bucket_id)
			{
				unavailable.insert(previous);
				unavailable.insert(*bucket_id);
			}
		}
	}
	for bucket_id in unavailable {
		state.buckets.get_mut(&bucket_id).expect("bucket exists").unavailable = true;
	}
	for (bucket_id, bucket) in &state.buckets {
		if bucket.unavailable {
			continue;
		}
		for (index, entry) in bucket.entries.iter().enumerate() {
			let operation = OperationId::parse(&entry.operation_id)
				.map_err(|_| ContentError::IntegrityFailed)?;
			state.committed_sources.insert(entry.install_sequence, (*bucket_id, index));
			state.operations.insert((*bucket_id, operation), entry.install_sequence);
		}
	}
	Ok(())
}

fn build_entry(
	installation: &VerifiedInstallation,
	meta: &BucketMeta,
) -> Result<Entry, ContentError> {
	let total_size = meta
		.total_size
		.checked_add(installation.stored_bytes)
		.ok_or(ContentError::IntegrityFailed)?;
	let leaf = MmrLeafV1 {
		data_root: H256::from(installation.cid.digest()),
		data_size: installation.stored_bytes,
		total_size,
	};
	Ok(Entry {
		version: VERSION,
		bucket_id: installation.bucket_id.to_string(),
		sequence: meta.entry_count,
		install_sequence: installation.install_sequence,
		operation_id: installation.operation_id.to_string(),
		cid: installation.cid.as_str().into(),
		data_size: installation.stored_bytes,
		total_size,
		leaf_hash: encode_hash(hash_leaf(&leaf)),
	})
}

fn advance_meta(
	meta: &BucketMeta,
	entry: &Entry,
	frame_bytes: u64,
) -> Result<BucketMeta, ContentError> {
	validate_entry(entry, meta)?;
	let (peaks, root) = append_peak(&meta.peaks, meta.entry_count, decode_hash(&entry.leaf_hash)?)?;
	Ok(BucketMeta {
		version: VERSION,
		entry_count: meta.entry_count.checked_add(1).ok_or(ContentError::IntegrityFailed)?,
		confirmed_log_bytes: meta
			.confirmed_log_bytes
			.checked_add(frame_bytes)
			.ok_or(ContentError::IntegrityFailed)?,
		peaks: peaks.into_iter().map(encode_hash).collect(),
		total_size: entry.total_size,
		root: Some(encode_hash(root)),
	})
}

fn validate_entry(entry: &Entry, meta: &BucketMeta) -> Result<(), ContentError> {
	if entry.sequence != meta.entry_count
		|| entry.total_size
			!= meta
				.total_size
				.checked_add(entry.data_size)
				.ok_or(ContentError::IntegrityFailed)?
	{
		return Err(ContentError::IntegrityFailed);
	}
	let cid = CanonicalCid::parse(&entry.cid)?;
	let leaf = MmrLeafV1 {
		data_root: H256::from(cid.digest()),
		data_size: entry.data_size,
		total_size: entry.total_size,
	};
	if entry.leaf_hash != encode_hash(hash_leaf(&leaf)) {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn entry_matches_installation(
	entry: &Entry,
	installation: &VerifiedInstallation,
) -> Result<bool, ContentError> {
	Ok(entry.install_sequence == installation.install_sequence
		&& entry.bucket_id == installation.bucket_id.to_string()
		&& entry.operation_id == installation.operation_id.to_string()
		&& entry.cid == installation.cid.as_str()
		&& entry.data_size == installation.stored_bytes)
}

fn encode_frame(entry: &Entry) -> Result<Vec<u8>, ContentError> {
	let payload = encode_json(entry, MAX_FRAME_PAYLOAD)?;
	let length: u32 = payload.len().try_into().map_err(|_| ContentError::IntegrityFailed)?;
	let mut frame = Vec::with_capacity(LENGTH_BYTES + payload.len() + CHECKSUM_BYTES);
	frame.extend_from_slice(&length.to_le_bytes());
	frame.extend_from_slice(&payload);
	frame.extend_from_slice(&Sha256::digest(&payload));
	Ok(frame)
}

fn read_confirmed_frames(
	path: &Path,
	confirmed_bytes: u64,
	expected_entries: u64,
	parsed_frames: &mut usize,
) -> Result<Vec<Entry>, ContentError> {
	if confirmed_bytes > MAX_LOG_BYTES || expected_entries > MAX_STREAMING_OPERATIONS as u64 {
		return Err(ContentError::IntegrityFailed);
	}
	let expected_entries: usize =
		expected_entries.try_into().map_err(|_| ContentError::IntegrityFailed)?;
	let mut file = File::open(path).map_err(io_error)?;
	let mut consumed = 0u64;
	let mut entries = Vec::with_capacity(expected_entries);
	while consumed < confirmed_bytes {
		if *parsed_frames >= expected_entries || *parsed_frames >= MAX_STREAMING_OPERATIONS {
			return Err(ContentError::IntegrityFailed);
		}
		let mut length = [0u8; LENGTH_BYTES];
		file.read_exact(&mut length).map_err(|_| ContentError::IntegrityFailed)?;
		let length = u32::from_le_bytes(length) as usize;
		if length == 0 || length > MAX_FRAME_PAYLOAD {
			return Err(ContentError::IntegrityFailed);
		}
		let frame_len = LENGTH_BYTES
			.checked_add(length)
			.and_then(|value| value.checked_add(CHECKSUM_BYTES))
			.ok_or(ContentError::IntegrityFailed)? as u64;
		consumed = consumed.checked_add(frame_len).ok_or(ContentError::IntegrityFailed)?;
		if consumed > confirmed_bytes {
			return Err(ContentError::IntegrityFailed);
		}
		let mut payload = vec![0u8; length];
		file.read_exact(&mut payload).map_err(|_| ContentError::IntegrityFailed)?;
		let mut checksum = [0u8; CHECKSUM_BYTES];
		file.read_exact(&mut checksum).map_err(|_| ContentError::IntegrityFailed)?;
		if Sha256::digest(&payload).as_slice() != checksum {
			return Err(ContentError::IntegrityFailed);
		}
		entries.push(serde_json::from_slice(&payload).map_err(|_| ContentError::IntegrityFailed)?);
		*parsed_frames += 1;
	}
	if consumed != confirmed_bytes || *parsed_frames != expected_entries {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(entries)
}

fn persist_meta(directory: &Path, meta: &BucketMeta) -> Result<(), ContentError> {
	let bytes = encode_json(meta, MAX_META_BYTES)?;
	let temporary = directory.join(format!("{META}.tmp-{}", std::process::id()));
	let mut file = File::create(&temporary).map_err(io_error)?;
	file.write_all(&bytes).map_err(io_error)?;
	file.sync_all().map_err(io_error)?;
	fs::rename(temporary, directory.join(META)).map_err(io_error)?;
	sync_dir(directory)
}

fn remove_meta_temps(directory: &Path) -> Result<(), ContentError> {
	let mut changed = false;
	for item in fs::read_dir(directory).map_err(io_error)? {
		let item = item.map_err(io_error)?;
		let name = item.file_name().to_string_lossy().into_owned();
		if name.starts_with(&format!("{META}.tmp-")) {
			fs::remove_file(item.path()).map_err(io_error)?;
			changed = true;
		} else if name != LOG && name != META {
			return Err(ContentError::IntegrityFailed);
		}
	}
	if changed {
		sync_dir(directory)?;
	}
	Ok(())
}

fn encode_json<T: Serialize>(value: &T, bound: usize) -> Result<Vec<u8>, ContentError> {
	let bytes = serde_json::to_vec(value).map_err(io_error)?;
	if bytes.len() > bound {
		return Err(ContentError::IntegrityFailed);
	}
	Ok(bytes)
}

fn read_json<T: DeserializeOwned>(path: &Path, bound: usize) -> Result<T, ContentError> {
	let bytes = fs::read(path).map_err(io_error)?;
	if bytes.len() > bound {
		return Err(ContentError::IntegrityFailed);
	}
	serde_json::from_slice(&bytes).map_err(|_| ContentError::IntegrityFailed)
}

fn append_peak(
	ordered_peaks: &[String],
	leaf_count: u64,
	mut current: H256,
) -> Result<(Vec<H256>, H256), ContentError> {
	let mut slots = vec![None; 64];
	let mut persisted = ordered_peaks.iter();
	for height in (0usize..64).rev() {
		if leaf_count & (1u64 << height) != 0 {
			slots[height] =
				Some(decode_hash(persisted.next().ok_or(ContentError::IntegrityFailed)?)?);
		}
	}
	if persisted.next().is_some() {
		return Err(ContentError::IntegrityFailed);
	}
	let mut height = 0usize;
	loop {
		if height == slots.len() {
			return Err(ContentError::IntegrityFailed);
		}
		match slots[height].take() {
			Some(left) => {
				current = hash_parent(left, current);
				height += 1;
			},
			None => {
				slots[height] = Some(current);
				break;
			},
		}
	}
	let peaks = slots.into_iter().rev().flatten().collect::<Vec<_>>();
	let root = peaks
		.iter()
		.rev()
		.fold(None, |right, peak| Some(right.map_or(*peak, |value| hash_parent(*peak, value))))
		.ok_or(ContentError::IntegrityFailed)?;
	Ok((peaks, root))
}

fn hash_leaf(leaf: &MmrLeafV1<H256>) -> H256 {
	H256::from(blake2_256(&leaf.encode()))
}

fn hash_parent(left: H256, right: H256) -> H256 {
	H256::from(blake2_256(&(left, right).encode()))
}

fn encode_hash(hash: H256) -> String {
	hex::encode(hash.as_bytes())
}

fn decode_hash(value: &str) -> Result<H256, ContentError> {
	if value.len() != 64 || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
		return Err(ContentError::IntegrityFailed);
	}
	let bytes = hex::decode(value).map_err(|_| ContentError::IntegrityFailed)?;
	Ok(H256::from_slice(&bytes))
}

fn sync_dir(path: &Path) -> Result<(), ContentError> {
	File::open(path).and_then(|directory| directory.sync_all()).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> ContentError {
	ContentError::Io(error.to_string())
}

fn lock_error() -> ContentError {
	ContentError::Io("bucket MMR state lock poisoned".into())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::storage::{StreamingDescriptor, StreamingStore};
	use tempfile::TempDir;

	fn install(
		root: &Path,
		store: &StreamingStore,
		bucket: u8,
		operation: u8,
		bytes: &[u8],
	) -> (BucketId, OperationId, CanonicalCid) {
		let bucket_id = BucketId::from_bytes([bucket; 32]);
		let operation_id = OperationId::from_bytes([operation; 16]);
		let cid = CanonicalCid::from_digest(blake2_256(bytes));
		store
			.put_chunks(
				StreamingDescriptor {
					operation_id,
					bucket_id,
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				[bytes.to_vec()],
			)
			.unwrap();
		assert!(root.join("streaming-v1").exists());
		(bucket_id, operation_id, cid)
	}

	fn peer_commitment(
		mmr: &BucketMmrStore,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		start: u64,
	) -> PeerMmrCommitmentV1 {
		let candidate = mmr.commitment_candidate(streaming, bucket_id, start).unwrap();
		let state = mmr.state.read().unwrap();
		let bucket = state.buckets.get(&bucket_id).unwrap();
		let predecessor =
			if start == 0 { 0 } else { bucket.entries[(start - 1) as usize].total_size };
		PeerMmrCommitmentV1::new(
			candidate.mmr_root.0,
			candidate.start_seq,
			candidate.leaf_count,
			predecessor,
		)
		.unwrap()
	}

	#[test]
	fn runtime_scale_hashes_and_incremental_right_bagging_are_fixed() {
		let leaves = (1u8..=5)
			.map(|value| {
				hash_leaf(&MmrLeafV1 {
					data_root: H256::repeat_byte(value),
					data_size: u64::from(value),
					total_size: u64::from(value) * u64::from(value + 1) / 2,
				})
			})
			.collect::<Vec<_>>();
		assert_eq!(
			encode_hash(leaves[0]),
			"adcbaa3f6b801cf206aa63c8afd2607f2fb717b51803a6f095e2bd9442e4184d"
		);
		let expected = [
			"adcbaa3f6b801cf206aa63c8afd2607f2fb717b51803a6f095e2bd9442e4184d",
			"6b4f9b5a21fd137869c9a8561e3e34f853a6f9097dede747b3bf84edac734b70",
			"93c8c0d7c0e8f06177602ff1b83886544fd10ac8a17cdfa05687117c36e9e5ec",
			"8887a98cd8949e50e33713cea9ea92087a49bd41afbad20466764a0b64fdfcb0",
			"f8834153b4b86a917233960176970659158448828dddb050c81627dcfd9d1d5b",
		];
		let mut peaks = Vec::new();
		for (index, leaf) in leaves.into_iter().enumerate() {
			let (next, root) = append_peak(&peaks, index as u64, leaf).unwrap();
			assert_eq!(encode_hash(root), expected[index]);
			peaks = next.into_iter().map(encode_hash).collect();
		}
	}

	#[test]
	fn append_is_bucket_local_exact_and_uses_bounded_logs() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, operation_a, _) = install(temp.path(), &streaming, 1, 1, b"alpha");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 2, 2, b"bravo");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let log_path = temp.path().join(ROOT).join(bucket_a.to_string()).join(LOG);
		let prefix = fs::read(&log_path).unwrap();
		let (_, operation_c, _) = install(temp.path(), &streaming, 1, 3, b"charlie");
		mmr.append_verified(&streaming, bucket_a, operation_c).unwrap();
		let appended = fs::read(&log_path).unwrap();
		assert!(appended.len() > prefix.len());
		assert_eq!(&appended[..prefix.len()], prefix.as_slice());
		assert_eq!(mmr.commitment_candidate(&streaming, bucket_a, 0).unwrap().leaf_count, 2);
		assert_eq!(mmr.commitment_candidate(&streaming, bucket_b, 0).unwrap().leaf_count, 1);
		assert_eq!(
			mmr.commitment_candidate(&streaming, bucket_a, 2),
			Err(ContentError::IntegrityFailed)
		);
		mmr.append_verified(&streaming, bucket_a, operation_a).unwrap();
		let state = mmr.state.read().unwrap();
		assert_eq!(state.operations.len(), 3);
		assert!(state.buckets.values().all(|bucket| bucket.meta.peaks.len() <= 64));
		for (bucket, runtime) in &state.buckets {
			let log = temp.path().join(ROOT).join(bucket.to_string()).join(LOG);
			assert_eq!(fs::metadata(log).unwrap().len(), runtime.meta.confirmed_log_bytes);
		}
	}

	#[test]
	fn replication_pages_enforce_zero_one_127_and_128_bounds_and_cursors() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let mut bucket = None;
		for value in 1u8..=128 {
			let (installed_bucket, _, _) = install(temp.path(), &streaming, 30, value, &[value]);
			bucket = Some(installed_bucket);
		}
		let bucket = bucket.unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let commitment = peer_commitment(&mmr, &streaming, bucket, 0);
		assert_eq!(commitment.sequence_range(), (0, 128));
		assert_eq!(
			mmr.replication_page(&streaming, bucket, commitment, None, 0),
			Err(ContentError::SchemaInvalid)
		);
		assert_eq!(
			mmr.replication_page(&streaming, bucket, commitment, None, 129),
			Err(ContentError::SchemaInvalid)
		);

		let (one, cursor) = mmr.replication_page(&streaming, bucket, commitment, None, 1).unwrap();
		assert_eq!(one.len(), 1);
		assert_eq!(one[0].position(), (1, 0, 1));
		assert_eq!(cursor.unwrap().position(), (0, 1));

		let (prefix, cursor) =
			mmr.replication_page(&streaming, bucket, commitment, None, 127).unwrap();
		assert_eq!(prefix.len(), 127);
		assert_eq!(prefix.last().unwrap().position(), (1, 126, 127));
		assert_eq!(cursor.unwrap().position(), (126, 127));
		let (tail, terminal) =
			mmr.replication_page(&streaming, bucket, commitment, cursor, 128).unwrap();
		assert_eq!(tail.len(), 1);
		assert_eq!(tail[0].position(), (1, 127, 128));
		assert_eq!(terminal, None);

		let (full, terminal) =
			mmr.replication_page(&streaming, bucket, commitment, None, 128).unwrap();
		assert_eq!(full.len(), 128);
		assert_eq!(terminal, None);
		let suffix = peer_commitment(&mmr, &streaming, bucket, 1);
		let (suffix_items, terminal) =
			mmr.replication_page(&streaming, bucket, suffix, None, 128).unwrap();
		assert_eq!(suffix_items.len(), 127);
		assert_eq!(suffix_items[0].position(), (1, 1, 2));
		assert_eq!(terminal, None);
	}

	#[test]
	fn zero_byte_installation_is_a_canonical_terminal_replication_page() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let bucket_id = BucketId::from_bytes([32; 32]);
		let operation_id = OperationId::from_bytes([32; 16]);
		let cid = CanonicalCid::from_digest(blake2_256(&[]));
		streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id,
					bucket_id,
					expected_cid: cid.as_str().into(),
					object_len: 0,
				},
				std::iter::empty::<Vec<u8>>(),
			)
			.unwrap();

		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let commitment = peer_commitment(&mmr, &streaming, bucket_id, 0);
		assert_eq!(commitment.sequence_range(), (0, 1));
		assert_eq!(commitment.predecessor_total_size(), 0);
		let (items, cursor) =
			mmr.replication_page(&streaming, bucket_id, commitment, None, 1).unwrap();
		assert_eq!(items.len(), 1);
		assert_eq!(items[0].cid(), cid.as_str());
		assert_eq!(items[0].position(), (0, 0, 0));
		assert!(items[0].chunk_hashes().is_empty());
		assert_eq!(cursor, None);
	}

	#[test]
	fn replication_pages_reject_commitment_cursor_entry_and_source_tamper() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket, _, first_cid) = install(temp.path(), &streaming, 31, 1, b"first");
		install(temp.path(), &streaming, 31, 2, b"second");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let commitment = peer_commitment(&mmr, &streaming, bucket, 0);
		let (start, end) = commitment.sequence_range();

		let wrong_root = PeerMmrCommitmentV1::new([99; 32], start, end - start, 0).unwrap();
		assert!(mmr.replication_page(&streaming, bucket, wrong_root, None, 1).is_err());
		let wrong_count =
			PeerMmrCommitmentV1::new(commitment.mmr_root(), start, end - start - 1, 0).unwrap();
		assert!(mmr.replication_page(&streaming, bucket, wrong_count, None, 1).is_err());
		let suffix = peer_commitment(&mmr, &streaming, bucket, 1);
		let wrong_total =
			PeerMmrCommitmentV1::new(suffix.mmr_root(), 1, 1, suffix.predecessor_total_size() + 1)
				.unwrap();
		assert!(mmr.replication_page(&streaming, bucket, wrong_total, None, 1).is_err());
		assert!(mmr
			.replication_page(&streaming, bucket, commitment, Some(PeerPageCursorV1::new(0, 99)), 1,)
			.is_err());

		{
			let mut state = mmr.state.write().unwrap();
			state.buckets.get_mut(&bucket).unwrap().entries[0].total_size += 1;
		}
		assert!(mmr.replication_page(&streaming, bucket, commitment, None, 1).is_err());
		{
			let mut state = mmr.state.write().unwrap();
			state.buckets.get_mut(&bucket).unwrap().entries[0].total_size -= 1;
		}

		let object = temp.path().join("streaming-v1").join("objects").join(first_cid.as_str());
		let original = fs::read(&object).unwrap();
		fs::write(&object, b"xxxxx").unwrap();
		assert!(mmr.replication_page(&streaming, bucket, commitment, None, 1).is_err());
		assert_eq!(
			streaming.verify_installed(first_cid.as_str()),
			Err(ContentError::IntegrityFailed)
		);
		fs::write(&object, original).unwrap();
		assert!(mmr.replication_page(&streaming, bucket, commitment, None, 1).is_err());
	}

	#[test]
	fn corrupt_confirmed_bucket_log_is_isolated_across_restart() {
		let temp = TempDir::new().unwrap();
		let legacy = temp.path().join("provider-index-v5.json");
		fs::write(&legacy, b"legacy-byte-for-byte").unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, _, _) = install(temp.path(), &streaming, 3, 3, b"corrupt-me");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 4, 4, b"meta-ahead");
		let (bucket_c, _, _) = install(temp.path(), &streaming, 8, 8, b"healthy");
		drop(BucketMmrStore::open(temp.path(), &streaming).unwrap());
		let log = temp.path().join(ROOT).join(bucket_a.to_string()).join(LOG);
		let mut bytes = fs::read(&log).unwrap();
		bytes[LENGTH_BYTES + 1] ^= 0x01;
		fs::write(&log, bytes).unwrap();
		let meta_path = temp.path().join(ROOT).join(bucket_b.to_string()).join(META);
		let mut meta = read_json::<BucketMeta>(&meta_path, MAX_META_BYTES).unwrap();
		meta.confirmed_log_bytes += 1;
		fs::write(&meta_path, encode_json(&meta, MAX_META_BYTES).unwrap()).unwrap();
		let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert_eq!(
			reopened.commitment_candidate(&streaming, bucket_a, 0),
			Err(ContentError::IntegrityFailed)
		);
		assert_eq!(
			reopened.commitment_candidate(&streaming, bucket_b, 0),
			Err(ContentError::IntegrityFailed)
		);
		assert_eq!(reopened.commitment_candidate(&streaming, bucket_c, 0).unwrap().leaf_count, 1);
		assert_eq!(fs::read(legacy).unwrap(), b"legacy-byte-for-byte");
	}

	#[test]
	fn corrupt_object_blocks_only_its_bucket_after_restart() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, _, cid_a) = install(temp.path(), &streaming, 5, 5, b"bad-object");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 6, 6, b"good-object");
		drop(BucketMmrStore::open(temp.path(), &streaming).unwrap());
		fs::write(
			temp.path().join("streaming-v1").join("objects").join(cid_a.as_str()),
			b"corruption",
		)
		.unwrap();
		let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert!(reopened.commitment_candidate(&streaming, bucket_a, 0).is_err());
		assert!(reopened.commitment_candidate(&streaming, bucket_b, 0).is_ok());
	}

	#[test]
	fn persistence_faults_are_old_or_new_and_poison_until_reopen() {
		for fault in [
			BucketMmrFault::PartialFrame,
			BucketMmrFault::AfterLogSync,
			BucketMmrFault::AfterMetaTempSync,
			BucketMmrFault::AfterMetaRename,
		] {
			let temp = TempDir::new().unwrap();
			let streaming = StreamingStore::open(temp.path()).unwrap();
			let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
			let (bucket, operation, _) = install(temp.path(), &streaming, 7, 7, b"faulted");
			mmr.inject_fault_once(fault).unwrap();
			assert!(mmr.append_verified(&streaming, bucket, operation).is_err());
			assert!(mmr.commitment_candidate(&streaming, bucket, 0).is_err());
			let directory = temp.path().join(ROOT).join(bucket.to_string());
			let persisted = directory.join(META);
			if fault == BucketMmrFault::AfterMetaRename {
				assert_eq!(
					read_json::<BucketMeta>(&persisted, MAX_META_BYTES).unwrap().entry_count,
					1
				);
			} else {
				assert!(!persisted.exists());
			}
			drop(mmr);
			let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
			assert_eq!(reopened.commitment_candidate(&streaming, bucket, 0).unwrap().leaf_count, 1);
			let meta = read_json::<BucketMeta>(&directory.join(META), MAX_META_BYTES).unwrap();
			assert_eq!(fs::metadata(directory.join(LOG)).unwrap().len(), meta.confirmed_log_bytes);
			assert!(!fs::read_dir(&directory).unwrap().any(|item| item
				.unwrap()
				.file_name()
				.to_string_lossy()
				.contains(".tmp-")));
		}
	}

	#[test]
	fn max_plus_one_valid_frames_are_rejected_before_parse_and_isolated() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, _, _) = install(temp.path(), &streaming, 9, 9, b"bounded");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 10, 10, b"healthy");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let healthy = mmr.commitment_candidate(&streaming, bucket_b, 0).unwrap();
		drop(mmr);
		let directory = temp.path().join(ROOT).join(bucket_a.to_string());
		let frame = fs::read(directory.join(LOG)).unwrap();
		let mut oversized = File::create(directory.join(LOG)).unwrap();
		for _ in 0..=MAX_STREAMING_OPERATIONS {
			oversized.write_all(&frame).unwrap();
		}
		oversized.sync_all().unwrap();
		let mut meta = read_json::<BucketMeta>(&directory.join(META), MAX_META_BYTES).unwrap();
		meta.entry_count = MAX_STREAMING_OPERATIONS as u64 + 1;
		meta.confirmed_log_bytes = frame.len() as u64 * meta.entry_count;
		fs::write(directory.join(META), encode_json(&meta, MAX_META_BYTES).unwrap()).unwrap();
		let mut parsed = 0usize;
		assert!(read_confirmed_frames(
			&directory.join(LOG),
			meta.confirmed_log_bytes,
			meta.entry_count,
			&mut parsed,
		)
		.is_err());
		assert_eq!(parsed, 0);
		let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert!(reopened.commitment_candidate(&streaming, bucket_a, 0).is_err());
		assert_eq!(reopened.commitment_candidate(&streaming, bucket_b, 0).unwrap(), healthy);
	}

	#[test]
	fn unexpected_bucket_file_is_namespace_local() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, _, _) = install(temp.path(), &streaming, 11, 11, b"namespace");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 12, 12, b"healthy");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		let healthy = mmr.commitment_candidate(&streaming, bucket_b, 0).unwrap();
		drop(mmr);
		fs::write(temp.path().join(ROOT).join(bucket_a.to_string()).join("unexpected"), b"tamper")
			.unwrap();
		let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert!(reopened.commitment_candidate(&streaming, bucket_a, 0).is_err());
		assert_eq!(reopened.commitment_candidate(&streaming, bucket_b, 0).unwrap(), healthy);
	}

	#[test]
	fn changed_known_source_is_never_accepted() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket, operation, cid) = install(temp.path(), &streaming, 13, 13, b"blocked");
		let object = temp.path().join("streaming-v1").join("objects").join(cid.as_str());
		fs::write(&object, b"corrupt").unwrap();
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		fs::write(&object, b"blocked").unwrap();
		{
			let mut state = mmr.state.write().unwrap();
			state.known_sources.get_mut(&0).unwrap().cid = CanonicalCid::from_digest([99; 32]);
		}
		assert_eq!(
			mmr.append_verified(&streaming, bucket, operation),
			Err(ContentError::IdempotencyConflict)
		);
	}
}
