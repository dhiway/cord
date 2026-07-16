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
//! Opening is globally fail-closed because the single immutable installation sequence cannot skip
//! a corrupt source during suffix reconciliation. Once open, candidate verification is bucket-local
//! so later corruption in an unrelated bucket does not invalidate an already complete commitment.

use std::{
	collections::{BTreeMap, BTreeSet},
	fs::{self, File},
	io::Write,
	path::{Path, PathBuf},
	sync::RwLock,
};

use codec::Encode;
use pallet_orbis_storage_provider::{CommitmentV1, MmrLeafV1};
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_crypto_hashing::blake2_256;

use super::streaming::{StreamingStore, VerifiedInstallation};
use crate::{BucketId, CanonicalCid, ContentError, OperationId};

const VERSION: u16 = 1;
const ROOT: &str = "bucket-mmr-v1";
const JOURNAL: &str = "journal.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalState {
	version: u16,
	next_install_sequence: u64,
	buckets: BTreeMap<String, BucketState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BucketState {
	entries: Vec<Entry>,
	peaks: Vec<String>,
	total_size: u64,
	root: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
	sequence: u64,
	install_sequence: u64,
	operation_id: String,
	cid: String,
	data_size: u64,
	total_size: u64,
	leaf_hash: String,
}

/// Private commitment substrate. It is deliberately not attached to any public route.
pub(crate) struct BucketMmrStore {
	root: PathBuf,
	state: RwLock<JournalState>,
}

impl BucketMmrStore {
	/// Open, fully verify, and deterministically reconcile any missing installed suffix.
	pub(crate) fn open(
		root: impl AsRef<Path>,
		streaming: &StreamingStore,
	) -> Result<Self, ContentError> {
		let root = root.as_ref().join(ROOT);
		fs::create_dir_all(&root).map_err(io_error)?;
		let journal = root.join(JOURNAL);
		let mut state = if journal.exists() {
			let bytes = fs::read(&journal).map_err(io_error)?;
			let state: JournalState = serde_json::from_slice(&bytes).map_err(io_error)?;
			if state.version != VERSION {
				return Err(ContentError::IntegrityFailed);
			}
			state
		} else {
			JournalState { version: VERSION, next_install_sequence: 0, buckets: BTreeMap::new() }
		};
		validate_state(&state)?;
		let installations = streaming.verified_installations()?;
		validate_prefix(&state, &installations)?;
		let mut changed = !journal.exists();
		for installation in installations.iter().skip(state.next_install_sequence as usize) {
			append_to_state(&mut state, installation)?;
			changed = true;
		}
		if changed {
			persist_state(&root, &state)?;
		}
		Ok(Self { root, state: RwLock::new(state) })
	}

	/// Append one exact verified install, or accept only an exact immutable replay.
	pub(crate) fn append_verified(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		operation_id: OperationId,
	) -> Result<(), ContentError> {
		let installation = streaming.verified_installation(bucket_id, operation_id)?;
		let mut state = self.state.write().map_err(|_| lock_error())?;
		if installation.install_sequence < state.next_install_sequence {
			let (bucket, existing) =
				entry_by_install_sequence(&state, installation.install_sequence)
					.ok_or(ContentError::IntegrityFailed)?;
			if bucket != installation.bucket_id.to_string()
				|| !entry_matches_installation(existing, &installation)?
			{
				return Err(ContentError::IdempotencyConflict);
			}
			return Ok(());
		}
		if installation.install_sequence != state.next_install_sequence {
			return Err(ContentError::IntegrityFailed);
		}
		let mut next = state.clone();
		append_to_state(&mut next, &installation)?;
		persist_state(&self.root, &next)?;
		*state = next;
		Ok(())
	}

	/// Build only the exact runtime commitment fields over a non-empty verified suffix.
	pub(crate) fn commitment_candidate(
		&self,
		streaming: &StreamingStore,
		bucket_id: BucketId,
		expected_start_seq: u64,
	) -> Result<CommitmentV1<H256>, ContentError> {
		let state = self.state.read().map_err(|_| lock_error())?;
		validate_state(&state)?;
		let bucket = state.buckets.get(&bucket_id.to_string()).ok_or(ContentError::NotFound)?;
		let installations = streaming.verified_bucket_installations(bucket_id)?;
		if bucket.entries.len() != installations.len()
			|| bucket.entries.iter().zip(&installations).any(|(entry, installation)| {
				entry.install_sequence != installation.install_sequence
					|| !entry_matches_installation(entry, installation).unwrap_or(false)
			}) {
			return Err(ContentError::IntegrityFailed);
		}
		let end = bucket.entries.len() as u64;
		let leaf_count =
			end.checked_sub(expected_start_seq).ok_or(ContentError::IntegrityFailed)?;
		if leaf_count == 0 {
			return Err(ContentError::IntegrityFailed);
		}
		Ok(CommitmentV1 {
			mmr_root: decode_hash(&bucket.root)?,
			start_seq: expected_start_seq,
			leaf_count,
		})
	}
}

fn append_to_state(
	state: &mut JournalState,
	installation: &VerifiedInstallation,
) -> Result<(), ContentError> {
	if installation.install_sequence != state.next_install_sequence {
		return Err(ContentError::IntegrityFailed);
	}
	let key = installation.bucket_id.to_string();
	if state.buckets.values().any(|bucket| {
		bucket
			.entries
			.iter()
			.any(|entry| entry.install_sequence == installation.install_sequence)
	}) || state.buckets.get(&key).is_some_and(|bucket| {
		bucket
			.entries
			.iter()
			.any(|entry| entry.operation_id == installation.operation_id.to_string())
	}) {
		return Err(ContentError::IdempotencyConflict);
	}
	let bucket = state.buckets.entry(key).or_insert_with(|| BucketState {
		entries: Vec::new(),
		peaks: Vec::new(),
		total_size: 0,
		root: String::new(),
	});
	let sequence = bucket.entries.len() as u64;
	let total_size = bucket
		.total_size
		.checked_add(installation.stored_bytes)
		.ok_or(ContentError::IntegrityFailed)?;
	let leaf = leaf(installation, total_size);
	let leaf_hash = hash_leaf(&leaf);
	bucket.entries.push(Entry {
		sequence,
		install_sequence: installation.install_sequence,
		operation_id: installation.operation_id.to_string(),
		cid: installation.cid.as_str().into(),
		data_size: installation.stored_bytes,
		total_size,
		leaf_hash: encode_hash(leaf_hash),
	});
	let (peaks, root) = append_peak(&bucket.peaks, sequence, leaf_hash)?;
	bucket.peaks = peaks.into_iter().map(encode_hash).collect();
	bucket.root = encode_hash(root);
	bucket.total_size = total_size;
	state.next_install_sequence = state
		.next_install_sequence
		.checked_add(1)
		.ok_or(ContentError::IntegrityFailed)?;
	Ok(())
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
	let root = bag_peaks(&peaks)?;
	Ok((peaks, root))
}

fn validate_state(state: &JournalState) -> Result<(), ContentError> {
	if state.version != VERSION {
		return Err(ContentError::IntegrityFailed);
	}
	let mut install_sequences = BTreeSet::new();
	let mut sources = BTreeSet::new();
	for (bucket_key, bucket) in &state.buckets {
		let bucket_id = BucketId::parse(bucket_key).map_err(|_| ContentError::IntegrityFailed)?;
		if bucket.entries.is_empty() {
			return Err(ContentError::IntegrityFailed);
		}
		let mut total_size = 0u64;
		let mut hashes = Vec::with_capacity(bucket.entries.len());
		for (index, entry) in bucket.entries.iter().enumerate() {
			if entry.sequence != index as u64
				|| !install_sequences.insert(entry.install_sequence)
				|| !sources.insert((
					bucket_id,
					OperationId::parse(&entry.operation_id)
						.map_err(|_| ContentError::IntegrityFailed)?,
				)) {
				return Err(ContentError::IntegrityFailed);
			}
			let cid = CanonicalCid::parse(&entry.cid).map_err(|_| ContentError::IntegrityFailed)?;
			total_size =
				total_size.checked_add(entry.data_size).ok_or(ContentError::IntegrityFailed)?;
			if entry.total_size != total_size {
				return Err(ContentError::IntegrityFailed);
			}
			let leaf = MmrLeafV1 {
				data_root: H256::from(cid.digest()),
				data_size: entry.data_size,
				total_size,
			};
			let hash = hash_leaf(&leaf);
			if entry.leaf_hash != encode_hash(hash) {
				return Err(ContentError::IntegrityFailed);
			}
			hashes.push(Ok(hash));
		}
		let (peaks, root) = peaks_and_root(hashes.into_iter())?;
		if bucket.total_size != total_size
			|| bucket.peaks != peaks.into_iter().map(encode_hash).collect::<Vec<_>>()
			|| bucket.root != encode_hash(root)
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	if install_sequences.len() as u64 != state.next_install_sequence
		|| install_sequences.iter().copied().ne(0..state.next_install_sequence)
	{
		return Err(ContentError::IntegrityFailed);
	}
	Ok(())
}

fn validate_prefix(
	state: &JournalState,
	installations: &[VerifiedInstallation],
) -> Result<(), ContentError> {
	if state.next_install_sequence > installations.len() as u64 {
		return Err(ContentError::IntegrityFailed);
	}
	for installation in installations.iter().take(state.next_install_sequence as usize) {
		let (bucket, entry) = entry_by_install_sequence(state, installation.install_sequence)
			.ok_or(ContentError::IntegrityFailed)?;
		if bucket != installation.bucket_id.to_string()
			|| !entry_matches_installation(entry, installation)?
		{
			return Err(ContentError::IntegrityFailed);
		}
	}
	Ok(())
}

fn entry_by_install_sequence(state: &JournalState, sequence: u64) -> Option<(&str, &Entry)> {
	state
		.buckets
		.iter()
		.flat_map(|(bucket_id, bucket)| {
			bucket.entries.iter().map(move |entry| (bucket_id.as_str(), entry))
		})
		.find(|(_, entry)| entry.install_sequence == sequence)
}

fn entry_matches_installation(
	entry: &Entry,
	installation: &VerifiedInstallation,
) -> Result<bool, ContentError> {
	Ok(entry.operation_id == installation.operation_id.to_string()
		&& entry.cid == installation.cid.as_str()
		&& entry.data_size == installation.stored_bytes
		&& CanonicalCid::parse(&entry.cid)?.digest() == installation.cid.digest())
}

fn leaf(installation: &VerifiedInstallation, total_size: u64) -> MmrLeafV1<H256> {
	MmrLeafV1 {
		data_root: H256::from(installation.cid.digest()),
		data_size: installation.stored_bytes,
		total_size,
	}
}

fn hash_leaf(leaf: &MmrLeafV1<H256>) -> H256 {
	H256::from(blake2_256(&leaf.encode()))
}

fn hash_parent(left: H256, right: H256) -> H256 {
	H256::from(blake2_256(&(left, right).encode()))
}

fn peaks_and_root<I>(hashes: I) -> Result<(Vec<H256>, H256), ContentError>
where
	I: IntoIterator<Item = Result<H256, ContentError>>,
{
	let mut slots: Vec<Option<H256>> = Vec::new();
	let mut count = 0u64;
	for hash in hashes {
		let mut current = hash?;
		count = count.checked_add(1).ok_or(ContentError::IntegrityFailed)?;
		let mut height = 0usize;
		loop {
			if height == slots.len() {
				slots.push(Some(current));
				break;
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
	}
	if count == 0 {
		return Err(ContentError::IntegrityFailed);
	}
	let peaks = slots.into_iter().rev().flatten().collect::<Vec<_>>();
	let root = bag_peaks(&peaks)?;
	Ok((peaks, root))
}

fn bag_peaks(peaks: &[H256]) -> Result<H256, ContentError> {
	peaks
		.iter()
		.rev()
		.fold(None, |right, peak| Some(right.map_or(*peak, |value| hash_parent(*peak, value))))
		.ok_or(ContentError::IntegrityFailed)
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

fn persist_state(root: &Path, state: &JournalState) -> Result<(), ContentError> {
	let bytes = serde_json::to_vec_pretty(state).map_err(io_error)?;
	let path = root.join(JOURNAL);
	let temporary = root.join(format!("{JOURNAL}.tmp-{}", std::process::id()));
	let mut file = File::create(&temporary).map_err(io_error)?;
	file.write_all(&bytes).map_err(io_error)?;
	file.sync_all().map_err(io_error)?;
	fs::rename(temporary, path).map_err(io_error)?;
	File::open(root).and_then(|directory| directory.sync_all()).map_err(io_error)
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
	use crate::storage::{StreamingDescriptor, StreamingFault};
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
			.expect("install bytes");
		assert!(root.join("streaming-v1").exists());
		(bucket_id, operation_id, cid)
	}

	#[test]
	fn runtime_scale_hashes_and_right_bagged_peaks_are_fixed() {
		let leaves = (1u8..=5)
			.map(|value| {
				Ok(hash_leaf(&MmrLeafV1 {
					data_root: H256::repeat_byte(value),
					data_size: u64::from(value),
					total_size: u64::from(value) * u64::from(value + 1) / 2,
				}))
			})
			.collect::<Vec<_>>();
		assert_eq!(
			encode_hash(leaves[0].as_ref().copied().unwrap()),
			"adcbaa3f6b801cf206aa63c8afd2607f2fb717b51803a6f095e2bd9442e4184d"
		);
		for (count, expected) in [
			(1, "adcbaa3f6b801cf206aa63c8afd2607f2fb717b51803a6f095e2bd9442e4184d"),
			(2, "6b4f9b5a21fd137869c9a8561e3e34f853a6f9097dede747b3bf84edac734b70"),
			(3, "93c8c0d7c0e8f06177602ff1b83886544fd10ac8a17cdfa05687117c36e9e5ec"),
			(4, "8887a98cd8949e50e33713cea9ea92087a49bd41afbad20466764a0b64fdfcb0"),
			(5, "f8834153b4b86a917233960176970659158448828dddb050c81627dcfd9d1d5b"),
		] {
			let (_, root) = peaks_and_root(leaves.iter().take(count).cloned()).unwrap();
			assert_eq!(encode_hash(root), expected);
		}
		let mut incremental = Vec::new();
		for (count, leaf) in leaves.iter().enumerate() {
			let (peaks, root) =
				append_peak(&incremental, count as u64, leaf.clone().unwrap()).unwrap();
			let (rebuilt, rebuilt_root) =
				peaks_and_root(leaves.iter().take(count + 1).cloned()).unwrap();
			assert_eq!((peaks.clone(), root), (rebuilt, rebuilt_root));
			incremental = peaks.into_iter().map(encode_hash).collect();
		}
		let (odd_peaks, _) = peaks_and_root(leaves.iter().take(3).cloned()).unwrap();
		assert_eq!(
			odd_peaks.into_iter().map(encode_hash).collect::<Vec<_>>(),
			vec![
				"6b4f9b5a21fd137869c9a8561e3e34f853a6f9097dede747b3bf84edac734b70",
				"74ad4fe182f780f088c5c5a5bc3e527cc672e97624dcf20ac85d639dee7ef3d9",
			]
		);
	}

	#[test]
	fn interleaved_buckets_reconcile_in_install_order_and_candidates_are_bounded() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (bucket_a, operation_a, _) = install(temp.path(), &streaming, 1, 1, b"alpha");
		let (bucket_b, _, _) = install(temp.path(), &streaming, 2, 2, b"bravo");
		let (_, _, _) = install(temp.path(), &streaming, 1, 3, b"charlie");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert_eq!(mmr.commitment_candidate(&streaming, bucket_a, 0).unwrap().leaf_count, 2);
		assert_eq!(mmr.commitment_candidate(&streaming, bucket_a, 1).unwrap().leaf_count, 1);
		assert_eq!(mmr.commitment_candidate(&streaming, bucket_b, 0).unwrap().leaf_count, 1);
		assert_eq!(
			mmr.commitment_candidate(&streaming, bucket_a, 2),
			Err(ContentError::IntegrityFailed)
		);
		assert_eq!(
			mmr.commitment_candidate(&streaming, bucket_a, 3),
			Err(ContentError::IntegrityFailed)
		);
		mmr.append_verified(&streaming, bucket_a, operation_a).expect("exact replay");
		let state = mmr.state.read().unwrap();
		assert_eq!(state.next_install_sequence, 3);
		assert_eq!(state.buckets[&bucket_a.to_string()].entries[0].sequence, 0);
		assert_eq!(state.buckets[&bucket_a.to_string()].entries[1].sequence, 1);
		assert_eq!(state.buckets[&bucket_b.to_string()].entries[0].sequence, 0);
	}

	#[test]
	fn crash_recovery_assigns_one_sequence_and_changed_source_conflicts() {
		let temp = TempDir::new().unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		streaming.inject_fault_once(StreamingFault::AfterObjectRename).unwrap();
		let bucket = BucketId::from_bytes([3; 32]);
		let operation = OperationId::from_bytes([4; 16]);
		let bytes = b"recover";
		let cid = CanonicalCid::from_digest(blake2_256(bytes));
		assert!(streaming
			.put_chunks(
				StreamingDescriptor {
					operation_id: operation,
					bucket_id: bucket,
					expected_cid: cid.as_str().into(),
					object_len: bytes.len() as u64,
				},
				[bytes.to_vec()],
			)
			.is_err());
		drop(streaming);
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let installations = streaming.verified_installations().unwrap();
		assert_eq!(installations.len(), 1);
		assert_eq!(installations[0].install_sequence, 0);
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		drop(mmr);
		let reopened = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert_eq!(reopened.state.read().unwrap().next_install_sequence, 1);

		let first = installations[0].clone();
		let mut state =
			JournalState { version: VERSION, next_install_sequence: 0, buckets: BTreeMap::new() };
		append_to_state(&mut state, &first).unwrap();
		let mut changed = first;
		changed.install_sequence = 1;
		changed.cid = CanonicalCid::from_digest([99; 32]);
		assert_eq!(append_to_state(&mut state, &changed), Err(ContentError::IdempotencyConflict));
	}

	#[test]
	fn corrupt_quarantined_and_tampered_state_fail_closed_while_legacy_is_untouched() {
		let temp = TempDir::new().unwrap();
		let legacy = temp.path().join("provider-index-v5.json");
		fs::write(&legacy, b"legacy-byte-for-byte").unwrap();
		let streaming = StreamingStore::open(temp.path()).unwrap();
		let (_, _, cid) = install(temp.path(), &streaming, 5, 5, b"durable");
		let (healthy_bucket, _, _) = install(temp.path(), &streaming, 6, 6, b"healthy");
		let mmr = BucketMmrStore::open(temp.path(), &streaming).unwrap();
		assert_eq!(fs::read(&legacy).unwrap(), b"legacy-byte-for-byte");

		let journal = temp.path().join(ROOT).join(JOURNAL);
		let original = fs::read(&journal).unwrap();
		for field in ["root", "peaks", "total_size"] {
			let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
			let bucket = value["buckets"].as_object_mut().unwrap().values_mut().next().unwrap();
			match field {
				"root" => bucket["root"] = serde_json::Value::String("00".repeat(32)),
				"peaks" => bucket["peaks"][0] = serde_json::Value::String("11".repeat(32)),
				_ => bucket["total_size"] = serde_json::Value::from(999u64),
			}
			fs::write(&journal, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
			assert!(BucketMmrStore::open(temp.path(), &streaming).is_err(), "{field}");
			fs::write(&journal, &original).unwrap();
		}

		let object = temp.path().join("streaming-v1").join("objects").join(cid.as_str());
		fs::write(&object, b"corrupt").unwrap();
		assert_eq!(mmr.commitment_candidate(&streaming, healthy_bucket, 0).unwrap().leaf_count, 1);
		assert!(BucketMmrStore::open(temp.path(), &streaming).is_err());
		assert_eq!(streaming.verify_installed(cid.as_str()), Err(ContentError::IntegrityFailed));
		fs::write(&object, b"durable").unwrap();
		assert!(mmr.commitment_candidate(&streaming, healthy_bucket, 0).is_ok());
		assert!(BucketMmrStore::open(temp.path(), &streaming).is_err());
		assert_eq!(fs::read(&legacy).unwrap(), b"legacy-byte-for-byte");
	}
}
