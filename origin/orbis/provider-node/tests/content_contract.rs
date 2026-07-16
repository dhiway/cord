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
use chacha20poly1305::{
	aead::{Aead, Payload},
	KeyInit, XChaCha20Poly1305, XNonce,
};
use cid::{multibase::Base, multihash::Multihash, CidGeneric};
use origin_orbis_provider::{
	BeginStreaming, BucketId, CanonicalCid, ContentError, OperationId, ProgressAck,
	StreamingDescriptor, StreamingFault, StreamingStore, CHUNK_BYTES, INGRESS_WINDOW_BYTES,
	INGRESS_WINDOW_CHUNKS, MAX_RANGE_BYTES, MAX_STORED_BYTES, MAX_STREAMING_OPERATIONS,
};
use serde_json::Value;

fn operation(value: u128) -> OperationId {
	OperationId::from_bytes(value.to_be_bytes())
}

fn bucket() -> BucketId {
	BucketId::from_bytes([0x22; 32])
}

fn cid(bytes: &[u8]) -> String {
	CanonicalCid::from_digest(Blake2b::<U32>::digest(bytes).into()).to_string()
}

fn descriptor(id: u128, bytes: &[u8]) -> StreamingDescriptor {
	StreamingDescriptor {
		operation_id: operation(id),
		bucket_id: bucket(),
		expected_cid: cid(bytes),
		object_len: bytes.len() as u64,
	}
}

fn chunks(bytes: &[u8]) -> Vec<Vec<u8>> {
	bytes.chunks(CHUNK_BYTES).map(ToOwned::to_owned).collect()
}

fn push(
	store: &StreamingStore,
	bucket_id: BucketId,
	operation_id: OperationId,
	index: u16,
	bytes: &[u8],
) -> Result<ProgressAck, ContentError> {
	let permit = store
		.try_acquire_ingress(bucket_id, operation_id, index, bytes.len())?
		.expect("focused sequential test has ingress capacity");
	store.push_chunk(permit, bytes)
}

fn vectors() -> Value {
	serde_json::from_str(include_str!("../../../../docs/specs/storage-v1.vectors.json")).unwrap()
}

fn vector<'a>(vectors: &'a Value, id: &str) -> &'a Value {
	vectors["vectors"]
		.as_array()
		.unwrap()
		.iter()
		.find(|entry| entry["id"] == id)
		.unwrap_or_else(|| panic!("missing ratified storage vector {id}"))
}

fn vector_stored_bytes(entry: &Value) -> Vec<u8> {
	if let Some(envelope) = entry.get("envelope_hex").and_then(Value::as_str) {
		return hex::decode(envelope).unwrap()
	}
	let stored_len = entry["stored_len"].as_u64().unwrap() as usize;
	if entry.get("mode").and_then(Value::as_str) != Some("xchacha20poly1305-v1") {
		return vec![0xa5; stored_len]
	}
	let plaintext = vec![0xa5; entry["plaintext_len"].as_u64().unwrap() as usize];
	let key = hex::decode(entry["key_hex"].as_str().unwrap()).unwrap();
	let nonce = hex::decode(entry["nonce_hex"].as_str().unwrap()).unwrap();
	let aad = hex::decode(entry["aad_hex"].as_str().unwrap()).unwrap();
	let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
	let ciphertext = cipher
		.encrypt(XNonce::from_slice(&nonce), Payload { msg: &plaintext, aad: &aad })
		.unwrap();
	let mut envelope = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
	envelope.push(1);
	envelope.extend_from_slice(&nonce);
	envelope.extend_from_slice(&ciphertext);
	assert_eq!(envelope.len(), stored_len);
	envelope
}

fn run_golden_contract() {
	let source = vectors();
	assert_eq!(source["version"], 1);
	assert_eq!(source["cid_profile"]["multicodec"], 85);
	assert_eq!(source["cid_profile"]["multihash"], 45600);
	let ids = [
		"none-empty",
		"none-stored-255k",
		"none-stored-256k",
		"none-stored-257k",
		"none-64m",
		"encrypted-empty",
		"encrypted-hello",
		"encrypted-stored-255k",
		"encrypted-stored-256k",
		"encrypted-stored-257k",
		"encrypted-max-plaintext-67108823",
	];
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	for (index, id) in ids.into_iter().enumerate() {
		let entry = vector(&source, id);
		let bytes = vector_stored_bytes(entry);
		assert_eq!(bytes.len() as u64, entry["stored_len"].as_u64().unwrap(), "{id}");
		assert_eq!(cid(&bytes), entry["cid"].as_str().unwrap(), "{id}");
		let receipt =
			store.put_chunks(descriptor(index as u128 + 1, &bytes), chunks(&bytes)).unwrap();
		assert_eq!(receipt.cid, entry["cid"].as_str().unwrap(), "{id}");
		assert_eq!(receipt.stored_bytes, bytes.len() as u64, "{id}");
		assert_eq!(
			receipt.chunks as u64,
			entry
				.get("chunk_count")
				.and_then(Value::as_u64)
				.unwrap_or_else(|| bytes.len().div_ceil(CHUNK_BYTES) as u64),
			"{id}"
		);
		assert!(receipt.locally_installed);
		store.verify_installed(&receipt.cid).unwrap();
	}
	let too_large = vector(&source, "none-64m-plus-1");
	let impossible = StreamingDescriptor {
		operation_id: operation(100),
		bucket_id: bucket(),
		expected_cid: cid(b"not consumed"),
		object_len: too_large["plaintext_len"].as_u64().unwrap(),
	};
	assert_eq!(store.begin(impossible), Err(ContentError::ObjectTooLarge));
	let encrypted_plus_one = vector(&source, "encrypted-plaintext-plus-1");
	assert_eq!(encrypted_plus_one["stored_len"].as_u64().unwrap(), MAX_STORED_BYTES + 1);
	assert_eq!(encrypted_plus_one["expected_error"], "STORAGE_OBJECT_TOO_LARGE");
	// Plaintext admission and encryption are host-owned. The provider sees only the resulting stored
	// envelope length and must independently reject the same stored-byte plus-one boundary.
	let stored_plus_one = StreamingDescriptor {
		operation_id: operation(101),
		bucket_id: bucket(),
		expected_cid: cid(b"not consumed"),
		object_len: encrypted_plus_one["stored_len"].as_u64().unwrap(),
	};
	assert_eq!(store.begin(stored_plus_one), Err(ContentError::ObjectTooLarge));
}

/// Canonical AC2 entry point. This test must execute nonzero ratified vector mappings.
#[test]
fn golden_contract() {
	run_golden_contract();
}

#[test]
fn identifiers_and_cid_schema_are_exact_and_canonical() {
	let operation_id = operation(7);
	let bucket_id = bucket();
	assert_eq!(OperationId::parse(&operation_id.to_string()).unwrap(), operation_id);
	assert_eq!(BucketId::parse(&bucket_id.to_string()).unwrap(), bucket_id);
	assert_eq!(OperationId::parse("00"), Err(ContentError::SchemaInvalid));
	assert_eq!(
		BucketId::parse(&BucketId::from_bytes([0xab; 32]).to_string().to_uppercase()),
		Err(ContentError::SchemaInvalid)
	);
	assert_eq!(serde_json::to_string(&operation_id).unwrap(), format!("\"{operation_id}\""));

	let canonical = cid(b"canonical");
	assert_eq!(CanonicalCid::parse(&canonical).unwrap().as_str(), canonical);
	assert_eq!(CanonicalCid::parse(&canonical.to_uppercase()), Err(ContentError::SchemaInvalid));
	assert_eq!(
		CanonicalCid::parse("QmYwAPJzv5CZsnAzt8auVZRnG7hZQ3c4J4w6S2F5nY6R5f"),
		Err(ContentError::SchemaInvalid)
	);
	let digest: [u8; 32] = Blake2b::<U32>::digest(b"canonical").into();
	let dag = CidGeneric::<32>::new_v1(0x70, Multihash::wrap(0xb220, &digest).unwrap()).to_string();
	assert_eq!(CanonicalCid::parse(&dag), Err(ContentError::SchemaInvalid));
	let sha = CidGeneric::<32>::new_v1(0x55, Multihash::wrap(0x12, &digest).unwrap()).to_string();
	assert_eq!(CanonicalCid::parse(&sha), Err(ContentError::SchemaInvalid));
	let parsed: CidGeneric<32> = canonical.parse().unwrap();
	let base58 = parsed.to_string_of_base(Base::Base58Btc).unwrap();
	assert_eq!(CanonicalCid::parse(&base58), Err(ContentError::SchemaInvalid));
}

#[test]
fn progress_replay_and_ingress_window_are_enforceable() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![3; CHUNK_BYTES * 4];
	let descriptor = descriptor(20, &bytes);
	let BeginStreaming::Receiving(initial) = store.begin(descriptor.clone()).unwrap() else {
		panic!()
	};
	assert_eq!(initial.persisted_bytes, 0);
	assert_eq!(initial.available_window_chunks, INGRESS_WINDOW_CHUNKS);
	assert_eq!(initial.available_window_bytes, INGRESS_WINDOW_BYTES);
	let permits: Vec<_> = (0..INGRESS_WINDOW_CHUNKS)
		.map(|index| {
			store
				.try_acquire_ingress(bucket(), operation(20), index as u16, CHUNK_BYTES)
				.unwrap()
				.unwrap()
		})
		.collect();
	assert!(store
		.try_acquire_ingress(bucket(), operation(20), 0, CHUNK_BYTES)
		.unwrap()
		.is_none());
	drop(permits);

	let ack = push(&store, bucket(), operation(20), 0, &bytes[..CHUNK_BYTES]).unwrap();
	assert_eq!(ack.next_chunk, 1);
	assert_eq!(ack.persisted_chunks, 1);
	assert_eq!(ack.persisted_bytes, CHUNK_BYTES as u64);
	assert_eq!(ack.available_window_chunks, INGRESS_WINDOW_CHUNKS);
	assert_eq!(ack.available_window_bytes, INGRESS_WINDOW_BYTES);
	let receipt = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
	assert_eq!(receipt.chunks, 4);
	let replay = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
	assert_eq!(receipt, replay);

	let changed_descriptor =
		StreamingDescriptor { object_len: descriptor.object_len - 1, ..descriptor };
	assert_eq!(store.begin(changed_descriptor), Err(ContentError::IdempotencyConflict));
}

#[test]
fn frozen_content_failures_and_verified_reads_release_no_unverified_bytes() {
	assert_eq!(ContentError::SchemaInvalid.to_string(), "WIRE_SCHEMA_INVALID");
	assert_eq!(ContentError::ChunkTooLarge.to_string(), "STORAGE_CHUNK_TOO_LARGE");
	assert_eq!(ContentError::LengthMismatch.to_string(), "STORAGE_LENGTH_MISMATCH");
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![4; CHUNK_BYTES + 9];
	let descriptor = descriptor(30, &bytes);
	store.begin(descriptor.clone()).unwrap();
	assert_eq!(
		push(&store, bucket(), operation(30), 0, &vec![4; CHUNK_BYTES + 1]),
		Err(ContentError::ChunkTooLarge)
	);
	assert_eq!(push(&store, bucket(), operation(30), 0, &[4]), Err(ContentError::LengthMismatch));
	assert_eq!(
		push(&store, bucket(), operation(30), 1, &[4; 9]),
		Err(ContentError::ChunkOutOfOrder)
	);
	push(&store, bucket(), operation(30), 0, &bytes[..CHUNK_BYTES]).unwrap();
	assert_eq!(store.finalize(bucket(), operation(30)), Err(ContentError::ChunkMissing));
	push(&store, bucket(), operation(30), 1, &bytes[CHUNK_BYTES..]).unwrap();
	let receipt = store.finalize(bucket(), operation(30)).unwrap();
	assert_eq!(
		store.read_range_verified(&receipt.cid, 0, MAX_RANGE_BYTES + 1),
		Err(ContentError::RangeInvalid)
	);
	assert_eq!(store.read_chunk_verified(&receipt.cid, 0).unwrap(), bytes[..CHUNK_BYTES]);
	assert_eq!(
		store
			.read_range_verified(&receipt.cid, CHUNK_BYTES as u64 - 2, CHUNK_BYTES as u64 + 2)
			.unwrap(),
		bytes[CHUNK_BYTES - 2..CHUNK_BYTES + 2]
	);

	let object = temp.path().join("streaming-v1/objects").join(&receipt.cid);
	let mut corrupted = fs::read(&object).unwrap();
	corrupted[10] ^= 1;
	fs::write(object, corrupted).unwrap();
	assert_eq!(store.verify_installed(&receipt.cid), Err(ContentError::IntegrityFailed));
	assert_eq!(store.read_chunk_verified(&receipt.cid, 0), Err(ContentError::IntegrityFailed));
	assert_eq!(store.read_range_verified(&receipt.cid, 0, 16), Err(ContentError::IntegrityFailed));
}

#[test]
fn journal_is_bounded_while_exact_replay_survives_the_bound() {
	assert_eq!(MAX_STREAMING_OPERATIONS, 8_192);
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open_with_operation_limit(temp.path(), 2).unwrap();
	let one = descriptor(40, b"one");
	let two = descriptor(41, b"two");
	store.begin(one.clone()).unwrap();
	store.begin(two).unwrap();
	assert!(matches!(store.begin(one), Ok(BeginStreaming::Receiving(_))));
	assert_eq!(store.begin(descriptor(42, b"three")), Err(ContentError::ProviderRecoveryTableFull));
}

#[test]
fn recovery_fault_boundaries_and_unowned_files_are_closed() {
	// Durable staging before journal becomes a removable orphan.
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	store.inject_fault_once(StreamingFault::AfterStagingSync).unwrap();
	assert!(store.begin(descriptor(50, b"stage")).is_err());
	drop(store);
	StreamingStore::open(temp.path()).unwrap();
	assert_eq!(fs::read_dir(temp.path().join("streaming-v1/staging")).unwrap().count(), 0);

	// Durable chunk before journal is truncated and may be replayed from zero.
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![5; CHUNK_BYTES + 3];
	let recovering_descriptor = descriptor(51, &bytes);
	let store = StreamingStore::open(temp.path()).unwrap();
	store.begin(recovering_descriptor.clone()).unwrap();
	store.inject_fault_once(StreamingFault::AfterChunkSync).unwrap();
	assert!(push(&store, bucket(), operation(51), 0, &bytes[..CHUNK_BYTES]).is_err());
	drop(store);
	let reopened = StreamingStore::open(temp.path()).unwrap();
	let BeginStreaming::Receiving(ack) = reopened.begin(recovering_descriptor.clone()).unwrap()
	else {
		panic!()
	};
	assert_eq!(ack.persisted_bytes, 0);
	let receipt = reopened.put_chunks(recovering_descriptor, chunks(&bytes)).unwrap();
	reopened.verify_installed(&receipt.cid).unwrap();

	for (id, fault) in
		[(52, StreamingFault::AfterFinalizingJournal), (53, StreamingFault::AfterObjectRename)]
	{
		let temp = tempfile::tempdir().unwrap();
		let bytes = vec![id as u8; CHUNK_BYTES + 1];
		let descriptor = descriptor(id, &bytes);
		let store = StreamingStore::open(temp.path()).unwrap();
		store.begin(descriptor.clone()).unwrap();
		for (index, chunk) in chunks(&bytes).iter().enumerate() {
			push(&store, bucket(), operation(id), index as u16, chunk).unwrap();
		}
		store.inject_fault_once(fault).unwrap();
		assert!(store.finalize(bucket(), operation(id)).is_err());
		drop(store);
		let recovered = StreamingStore::open(temp.path()).unwrap();
		recovered.verify_installed(&descriptor.expected_cid).unwrap();
	}

	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let receipt = store.put_chunks(descriptor(54, b"owned"), vec![b"owned".to_vec()]).unwrap();
	drop(store);
	let objects = temp.path().join("streaming-v1/objects");
	let orphan = objects.join("unowned");
	let mut file = fs::File::create(&orphan).unwrap();
	file.write_all(b"orphan").unwrap();
	file.sync_all().unwrap();
	let reopened = StreamingStore::open(temp.path()).unwrap();
	assert!(!orphan.exists());
	reopened.verify_installed(&receipt.cid).unwrap();
}

#[test]
fn ordered_chunk_hashes_are_durable_and_unsigned_receipt_is_local_only() {
	let temp = tempfile::tempdir().unwrap();
	let store = StreamingStore::open(temp.path()).unwrap();
	let bytes = vec![9; CHUNK_BYTES + 1];
	let receipt = store.put_chunks(descriptor(60, &bytes), chunks(&bytes)).unwrap();
	let receipt_json = serde_json::to_value(&receipt).unwrap();
	assert_eq!(receipt_json["locally_installed"], true);
	assert!(receipt_json.get("durably_readable").is_none());
	let journal: Value =
		serde_json::from_slice(&fs::read(temp.path().join("streaming-v1/journal.json")).unwrap())
			.unwrap();
	let record = journal["operations"].as_object().unwrap().values().next().unwrap();
	let hashes = record["chunks"].as_array().unwrap();
	assert_eq!(hashes.len(), 2);
	assert_eq!(hashes[0]["length"], CHUNK_BYTES as u64);
	assert_eq!(hashes[1]["length"], 1);
	assert_ne!(hashes[0]["hash"], hashes[1]["hash"]);
}

#[test]
fn hostile_journal_key_and_every_receipt_field_fail_closed_on_reopen() {
	for mutation in 0..8 {
		let temp = tempfile::tempdir().unwrap();
		let store = StreamingStore::open(temp.path()).unwrap();
		store
			.put_chunks(descriptor(70, b"journal-integrity"), vec![b"journal-integrity".to_vec()])
			.unwrap();
		drop(store);
		let journal = temp.path().join("streaming-v1/journal.json");
		let mut state: Value = serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
		let operations = state["operations"].as_object_mut().unwrap();
		if mutation == 0 {
			let (key, record) = operations
				.iter()
				.next()
				.map(|(key, value)| (key.clone(), value.clone()))
				.unwrap();
			operations.remove(&key);
			operations.insert("00".repeat(32), record);
		} else {
			let receipt =
				operations.values_mut().next().unwrap()["receipt"].as_object_mut().unwrap();
			match mutation {
				1 => receipt.insert("operation_id".into(), Value::String("ff".repeat(16))),
				2 => receipt.insert("bucket_id".into(), Value::String("ee".repeat(32))),
				3 => receipt.insert("cid".into(), Value::String(cid(b"other"))),
				4 => receipt.insert("stored_bytes".into(), Value::from(1_000u64)),
				5 => receipt.insert("chunks".into(), Value::from(2u64)),
				6 => receipt
					.insert("fingerprint".into(), Value::String(format!("0x{}", "00".repeat(32)))),
				7 => receipt.insert("locally_installed".into(), Value::Bool(false)),
				_ => unreachable!(),
			};
		}
		fs::write(&journal, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
		assert!(matches!(StreamingStore::open(temp.path()), Err(ContentError::IntegrityFailed)));
	}
}
