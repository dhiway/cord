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

use std::{fs, io::Write};

use blake2::{digest::consts::U32, Blake2b, Digest};
use cid::{multibase::Base, multihash::Multihash, CidGeneric};
use origin_orbis_provider::{
	BeginStreaming, CanonicalCid, ContentError, StreamingDescriptor, StreamingStore, CHUNK_BYTES,
	MAX_RANGE_BYTES, MAX_STORED_BYTES,
};

fn cid(bytes: &[u8]) -> String {
	CanonicalCid::from_digest(Blake2b::<U32>::digest(bytes).into()).to_string()
}

fn descriptor(operation: &str, bytes: &[u8]) -> StreamingDescriptor {
	StreamingDescriptor {
		operation_id: operation.into(),
		bucket_id: "bucket-1".into(),
		expected_cid: cid(bytes),
		object_len: bytes.len() as u64,
	}
}

fn chunks(bytes: &[u8]) -> Vec<Vec<u8>> {
	bytes.chunks(CHUNK_BYTES).map(ToOwned::to_owned).collect()
}

fn pattern_cid(length: u64, value: u8) -> String {
	let mut hash = Blake2b::<U32>::new();
	for index in 0..length.div_ceil(CHUNK_BYTES as u64) {
		let start = index * CHUNK_BYTES as u64;
		hash.update(vec![value; (length - start).min(CHUNK_BYTES as u64) as usize]);
	}
	CanonicalCid::from_digest(hash.finalize().into()).to_string()
}

fn pattern_chunks(length: u64, value: u8) -> impl Iterator<Item = Vec<u8>> {
	(0..length.div_ceil(CHUNK_BYTES as u64)).map(move |index| {
		let start = index * CHUNK_BYTES as u64;
		vec![value; (length - start).min(CHUNK_BYTES as u64) as usize]
	})
}

fn pattern_descriptor(operation: &str, length: u64, value: u8) -> StreamingDescriptor {
	StreamingDescriptor {
		operation_id: operation.into(),
		bucket_id: "bucket-1".into(),
		expected_cid: pattern_cid(length, value),
		object_len: length,
	}
}

#[test]
fn canonical_255_256_257_kib_boundaries_stream_and_verify() {
	for (index, length) in [255 * 1024, 256 * 1024, 257 * 1024].into_iter().enumerate() {
		let temp = tempfile::tempdir().unwrap();
		let store = StreamingStore::open(temp.path()).unwrap();
		let bytes = vec![index as u8 + 1; length];
		let descriptor = descriptor(&format!("boundary-{index}"), &bytes);
		let receipt = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
		assert_eq!(receipt.stored_bytes, length as u64);
		assert_eq!(receipt.chunks, if length <= CHUNK_BYTES { 1 } else { 2 });
		assert!(receipt.durably_readable);
		assert_eq!(
			store.read_chunk_verified(&receipt.cid, 0).unwrap(),
			bytes[..length.min(CHUNK_BYTES)].to_vec()
		);
		assert_eq!(store.read_range_verified(&receipt.cid, 7, 23).unwrap(), bytes[7..23]);
	}
}

#[test]
fn none_zero_max_and_plus_one_boundaries_are_exact() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let empty = Vec::new();
	let empty_receipt = store.put_chunks(descriptor("empty", &empty), Vec::new()).unwrap();
	assert_eq!(empty_receipt.chunks, 0);

	let max = pattern_descriptor("max", MAX_STORED_BYTES, 7);
	let receipt = store.put_chunks(max, pattern_chunks(MAX_STORED_BYTES, 7)).unwrap();
	assert_eq!(receipt.stored_bytes, MAX_STORED_BYTES);
	assert_eq!(receipt.chunks, 256);
	assert_eq!(
		store.read_range_verified(&receipt.cid, 0, MAX_RANGE_BYTES + 1),
		Err(ContentError::RangeInvalid)
	);

	let too_large = pattern_descriptor("too-large", MAX_STORED_BYTES + 1, 8);
	assert_eq!(store.begin(too_large).unwrap_err(), ContentError::ObjectTooLarge);
}

#[test]
fn only_cidv1_base32lower_raw_blake2b256_is_accepted() {
	let canonical = cid(b"canonical");
	assert_eq!(CanonicalCid::parse(&canonical).unwrap().as_str(), canonical);
	assert_eq!(CanonicalCid::parse(&canonical.to_uppercase()), Err(ContentError::CidInvalid));
	assert_eq!(
		CanonicalCid::parse("QmYwAPJzv5CZsnAzt8auVZRnG7hZQ3c4J4w6S2F5nY6R5f"),
		Err(ContentError::CidInvalid)
	);

	let digest: [u8; 32] = Blake2b::<U32>::digest(b"canonical").into();
	let dag = CidGeneric::<32>::new_v1(0x70, Multihash::wrap(0xb220, &digest).unwrap()).to_string();
	assert_eq!(CanonicalCid::parse(&dag), Err(ContentError::CidInvalid));
	let sha = CidGeneric::<32>::new_v1(0x55, Multihash::wrap(0x12, &digest).unwrap()).to_string();
	assert_eq!(CanonicalCid::parse(&sha), Err(ContentError::CidInvalid));
	let parsed: CidGeneric<32> = canonical.parse().unwrap();
	let base58 = parsed.to_string_of_base(Base::Base58Btc).unwrap();
	assert_eq!(CanonicalCid::parse(&base58), Err(ContentError::CidInvalid));
}

#[test]
fn contiguous_missing_length_and_cid_failures_commit_no_object() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![3; CHUNK_BYTES + 1];
	let descriptor = descriptor("ordering", &bytes);
	assert!(matches!(store.begin(descriptor.clone()).unwrap(), BeginStreaming::Receiving { .. }));
	assert_eq!(
		store.push_chunk("bucket-1", "ordering", 1, &[3]),
		Err(ContentError::ChunkOutOfOrder)
	);
	assert_eq!(
		store.push_chunk("bucket-1", "ordering", 0, &[3]),
		Err(ContentError::ChunkSizeInvalid)
	);
	store.push_chunk("bucket-1", "ordering", 0, &bytes[..CHUNK_BYTES]).unwrap();
	store.push_chunk("bucket-1", "ordering", 0, &bytes[..CHUNK_BYTES]).unwrap();
	let mut changed_chunk = bytes[..CHUNK_BYTES].to_vec();
	changed_chunk[0] ^= 1;
	assert_eq!(
		store.push_chunk("bucket-1", "ordering", 0, &changed_chunk),
		Err(ContentError::IdempotencyConflict)
	);
	assert_eq!(store.finalize("bucket-1", "ordering"), Err(ContentError::ChunkMissing));

	let wrong = StreamingDescriptor {
		operation_id: "wrong-cid".into(),
		bucket_id: "bucket-1".into(),
		expected_cid: cid(b"other"),
		object_len: 4,
	};
	store.begin(wrong).unwrap();
	store.push_chunk("bucket-1", "wrong-cid", 0, b"data").unwrap();
	assert_eq!(store.finalize("bucket-1", "wrong-cid"), Err(ContentError::CidMismatch));
}

#[test]
fn exact_replay_returns_same_receipt_and_changed_replay_conflicts() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![4; CHUNK_BYTES + 9];
	let descriptor = descriptor("replay", &bytes);
	let first = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
	let replay = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
	assert_eq!(first, replay);
	let mut changed = bytes.clone();
	changed[CHUNK_BYTES] ^= 1;
	assert_eq!(
		store.put_chunks(descriptor.clone(), chunks(&changed)),
		Err(ContentError::IdempotencyConflict)
	);
	let mut changed_descriptor = descriptor;
	changed_descriptor.object_len -= 1;
	assert_eq!(store.begin(changed_descriptor), Err(ContentError::IdempotencyConflict));
}

#[test]
fn receiving_and_finalizing_states_recover_and_orphans_are_removed() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![5; CHUNK_BYTES + 5];
	let descriptor = descriptor("recover", &bytes);
	{
		let store = StreamingStore::open(temp.path()).unwrap();
		store.begin(descriptor.clone()).unwrap();
		store.push_chunk("bucket-1", "recover", 0, &bytes[..CHUNK_BYTES]).unwrap();
	}
	let staging = temp.path().join("streaming-v1/staging");
	let part = fs::read_dir(&staging).unwrap().next().unwrap().unwrap().path();
	OpenOptionsExt::append(&part, b"torn");
	fs::write(staging.join("orphan.part"), b"orphan").unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	assert!(!staging.join("orphan.part").exists());
	assert!(matches!(
		store.begin(descriptor.clone()).unwrap(),
		BeginStreaming::Receiving { next_chunk: 1, .. }
	));
	store.push_chunk("bucket-1", "recover", 1, &bytes[CHUNK_BYTES..]).unwrap();

	let journal = temp.path().join("streaming-v1/journal.json");
	let mut value: serde_json::Value =
		serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
	let operation = value["operations"].as_object_mut().unwrap().values_mut().next().unwrap();
	operation["phase"] = serde_json::Value::String("finalizing".into());
	fs::write(&journal, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
	drop(store);
	let recovered = StreamingStore::open(temp.path()).unwrap();
	assert_eq!(recovered.read_range_verified(&descriptor.expected_cid, 0, 5).unwrap(), &bytes[..5]);
}

struct OpenOptionsExt;
impl OpenOptionsExt {
	fn append(path: &std::path::Path, bytes: &[u8]) {
		let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
		file.write_all(bytes).unwrap();
		file.sync_all().unwrap();
	}
}

#[test]
fn corruption_releases_no_full_chunk_or_range_bytes() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![6; CHUNK_BYTES + 3];
	let receipt = store.put_chunks(descriptor("corrupt", &bytes), chunks(&bytes)).unwrap();
	let object = temp.path().join("streaming-v1/objects").join(&receipt.cid);
	let mut corrupted = fs::read(&object).unwrap();
	corrupted[10] ^= 1;
	fs::write(object, corrupted).unwrap();
	assert!(store.open_verified(&receipt.cid).is_err());
	assert!(store.read_chunk_verified(&receipt.cid, 0).is_err());
	assert!(store.read_range_verified(&receipt.cid, 0, 16).is_err());
}
