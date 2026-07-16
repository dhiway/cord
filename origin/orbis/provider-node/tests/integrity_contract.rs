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

use std::{fs, io::Cursor};

use blake2::{digest::consts::U32, Blake2b, Digest};
use origin_orbis_provider::{
	BucketId, CanonicalCid, ContentError, OperationId, StreamingDescriptor, StreamingFault,
	StreamingStore, CHUNK_BYTES,
};
use serde_json::{json, Value};

fn cid(bytes: &[u8]) -> String {
	CanonicalCid::from_digest(Blake2b::<U32>::digest(bytes).into()).to_string()
}

fn descriptor(operation: u128, bytes: &[u8]) -> StreamingDescriptor {
	StreamingDescriptor {
		operation_id: OperationId::from_bytes(operation.to_be_bytes()),
		bucket_id: BucketId::from_bytes([0x44; 32]),
		expected_cid: cid(bytes),
		object_len: bytes.len() as u64,
	}
}

fn chunks(bytes: &[u8]) -> Vec<Vec<u8>> {
	bytes.chunks(CHUNK_BYTES).map(ToOwned::to_owned).collect()
}

fn install(store: &StreamingStore, operation: u128, bytes: &[u8]) -> String {
	store.put_chunks(descriptor(operation, bytes), chunks(bytes)).unwrap().cid
}

fn object_path(root: &std::path::Path, cid: &str) -> std::path::PathBuf {
	root.join("streaming-v1/objects").join(cid)
}

fn journal(root: &std::path::Path) -> Value {
	serde_json::from_slice(&fs::read(root.join("streaming-v1/journal.json")).unwrap()).unwrap()
}

#[test]
fn corrupt_read_is_sticky_idempotent_and_releases_no_bytes_across_reopen() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![7; CHUNK_BYTES + 11];
	let store = StreamingStore::open(temp.path()).unwrap();
	let content_id = install(&store, 1, &bytes);
	let path = object_path(temp.path(), &content_id);
	let mut corrupt = bytes.clone();
	corrupt[CHUNK_BYTES] ^= 1;
	fs::write(&path, &corrupt).unwrap();

	assert_eq!(store.verify_installed(&content_id), Err(ContentError::IntegrityFailed));
	let first = store.integrity_summary().unwrap();
	assert_eq!(first.installed_objects, 1);
	assert_eq!(first.ready_objects, 0);
	assert_eq!(first.quarantined_objects, 1);
	assert!(!first.ready);
	assert_eq!(first.last_detection_sequence, 1);
	assert_eq!(store.read_chunk_verified(&content_id, 0), Err(ContentError::IntegrityFailed));
	assert_eq!(store.read_range_verified(&content_id, 0, 16), Err(ContentError::IntegrityFailed));
	assert_eq!(store.integrity_summary().unwrap(), first);
	drop(store);

	let reopened = StreamingStore::open(temp.path()).unwrap();
	assert_eq!(reopened.integrity_summary().unwrap(), first);
	assert_eq!(reopened.read_chunk_verified(&content_id, 0), Err(ContentError::IntegrityFailed));
	fs::write(&path, &bytes).unwrap();
	assert_eq!(reopened.verify_installed(&content_id), Err(ContentError::IntegrityFailed));
	assert_eq!(
		reopened.read_range_verified(&content_id, 0, 16),
		Err(ContentError::IntegrityFailed)
	);
	assert_eq!(reopened.integrity_summary().unwrap(), first);

	let state = journal(temp.path());
	let quarantine = &state["quarantine"][&content_id];
	assert_eq!(quarantine["reason"], "chunk_mismatch");
	assert_eq!(quarantine["expected_bytes"], bytes.len() as u64);
	assert_eq!(quarantine["observed_bytes"], bytes.len() as u64);
	assert_eq!(quarantine["detection_sequence"], 1);
}

#[test]
fn audit_detects_missing_and_unread_corruption_before_ready_summary() {
	let temp = tempfile::tempdir().unwrap();
	let first_bytes = vec![1; 32];
	let second_bytes = vec![2; CHUNK_BYTES + 1];
	let store = StreamingStore::open(temp.path()).unwrap();
	let missing = install(&store, 2, &first_bytes);
	let corrupt = install(&store, 3, &second_bytes);
	fs::remove_file(object_path(temp.path(), &missing)).unwrap();
	let mut changed = second_bytes.clone();
	changed[9] ^= 1;
	fs::write(object_path(temp.path(), &corrupt), changed).unwrap();
	let audited = store.integrity_summary().unwrap();
	assert_eq!(audited.installed_objects, 2);
	assert_eq!(audited.ready_objects, 0);
	assert_eq!(audited.quarantined_objects, 2);
	assert!(!audited.ready);
	assert_eq!(audited.last_detection_sequence, 2);
	assert_eq!(store.integrity_summary().unwrap(), audited);
	let state = journal(temp.path());
	assert_eq!(state["quarantine"][&missing]["reason"], "missing");
	assert_eq!(state["quarantine"][&missing]["observed_bytes"], Value::Null);
	assert_eq!(state["quarantine"][&corrupt]["reason"], "chunk_mismatch");
}

#[test]
fn invalid_repair_changes_nothing_and_exact_repair_is_the_only_ready_transition() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![3; CHUNK_BYTES + 5];
	let store = StreamingStore::open(temp.path()).unwrap();
	let content_id = install(&store, 4, &bytes);
	let path = object_path(temp.path(), &content_id);
	let mut corrupt = bytes.clone();
	corrupt[2] ^= 1;
	fs::write(&path, &corrupt).unwrap();
	assert_eq!(store.verify_installed(&content_id), Err(ContentError::IntegrityFailed));
	let quarantined = store.integrity_summary().unwrap();

	assert_eq!(
		store.install_verified_repair(&content_id, &bytes[..bytes.len() - 1]),
		Err(ContentError::LengthMismatch)
	);
	assert_eq!(fs::read(&path).unwrap(), corrupt);
	assert_eq!(store.integrity_summary().unwrap(), quarantined);
	let wrong = vec![9; bytes.len()];
	assert_eq!(
		store.install_verified_repair_reader(&content_id, Cursor::new(wrong)),
		Err(ContentError::CidMismatch)
	);
	assert_eq!(fs::read(&path).unwrap(), corrupt);
	assert_eq!(store.integrity_summary().unwrap(), quarantined);

	store
		.install_verified_repair_reader(&content_id, Cursor::new(bytes.clone()))
		.unwrap();
	let ready = store.integrity_summary().unwrap();
	assert!(ready.ready);
	assert_eq!(ready.ready_objects, 1);
	assert_eq!(ready.quarantined_objects, 0);
	assert_eq!(ready.last_detection_sequence, quarantined.last_detection_sequence);
	assert_eq!(store.read_range_verified(&content_id, 0, 16).unwrap(), bytes[..16]);
	drop(store);
	let reopened = StreamingStore::open(temp.path()).unwrap();
	assert!(reopened.integrity_summary().unwrap().ready);
	reopened.verify_installed(&content_id).unwrap();
}

#[test]
fn repair_rename_crash_remains_quarantined_until_an_exact_retry_clears_it() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![5; CHUNK_BYTES + 7];
	let store = StreamingStore::open(temp.path()).unwrap();
	let content_id = install(&store, 5, &bytes);
	let mut corrupt = bytes.clone();
	corrupt[0] ^= 1;
	fs::write(object_path(temp.path(), &content_id), corrupt).unwrap();
	assert_eq!(store.verify_installed(&content_id), Err(ContentError::IntegrityFailed));
	store.inject_fault_once(StreamingFault::AfterRepairRename).unwrap();
	assert!(matches!(store.install_verified_repair(&content_id, &bytes), Err(ContentError::Io(_))));
	assert_eq!(fs::read(object_path(temp.path(), &content_id)).unwrap(), bytes);
	assert!(!store.integrity_summary().unwrap().ready);
	drop(store);

	let reopened = StreamingStore::open(temp.path()).unwrap();
	assert!(!reopened.integrity_summary().unwrap().ready);
	assert_eq!(reopened.verify_installed(&content_id), Err(ContentError::IntegrityFailed));
	reopened.install_verified_repair(&content_id, &bytes).unwrap();
	assert!(reopened.integrity_summary().unwrap().ready);
	reopened.verify_installed(&content_id).unwrap();
}

#[test]
fn quarantine_is_bounded_by_distinct_installed_objects_and_orphans_are_pruned() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = b"shared canonical object";
	let store = StreamingStore::open(temp.path()).unwrap();
	let content_id = install(&store, 6, bytes);
	assert_eq!(install(&store, 7, bytes), content_id);
	assert_eq!(store.integrity_summary().unwrap().installed_objects, 1);
	drop(store);

	let journal_path = temp.path().join("streaming-v1/journal.json");
	let mut state = journal(temp.path());
	let orphan_cid = cid(b"not installed");
	state["quarantine"][&orphan_cid] = json!({
		"reason": "missing",
		"expected_bytes": 0,
		"observed_bytes": null,
		"detection_sequence": 99
	});
	state["detection_sequence"] = Value::from(99u64);
	fs::write(&journal_path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
	let reopened = StreamingStore::open(temp.path()).unwrap();
	let summary = reopened.integrity_summary().unwrap();
	assert_eq!(summary.installed_objects, 1);
	assert_eq!(summary.quarantined_objects, 0);
	assert_eq!(journal(temp.path())["quarantine"].as_object().unwrap().len(), 0);

	let mut corrupt = bytes.to_vec();
	corrupt[0] ^= 1;
	fs::write(object_path(temp.path(), &content_id), corrupt).unwrap();
	let audited = reopened.integrity_summary().unwrap();
	assert_eq!(audited.installed_objects, 1);
	assert_eq!(audited.quarantined_objects, 1);
	assert_eq!(journal(temp.path())["quarantine"].as_object().unwrap().len(), 1);
}

#[test]
fn quarantined_receipt_fingerprint_is_reconstructed_and_tampering_never_escapes_repair() {
	let temp = tempfile::tempdir().unwrap();
	let bytes = vec![8; CHUNK_BYTES + 3];
	let descriptor = descriptor(8, &bytes);
	let store = StreamingStore::open(temp.path()).unwrap();
	let original = store.put_chunks(descriptor.clone(), chunks(&bytes)).unwrap();
	let mut corrupt = bytes.clone();
	corrupt[1] ^= 1;
	fs::write(object_path(temp.path(), &original.cid), corrupt).unwrap();
	assert_eq!(store.verify_installed(&original.cid), Err(ContentError::IntegrityFailed));

	let journal_path = temp.path().join("streaming-v1/journal.json");
	let mut tampered = journal(temp.path());
	let operation = tampered["operations"].as_object_mut().unwrap().values_mut().next().unwrap();
	operation["receipt"]["fingerprint"] = Value::String(format!("0x{}", "00".repeat(32)));
	fs::write(&journal_path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
	store.install_verified_repair(&original.cid, &bytes).unwrap();
	let replay = store.begin(descriptor).unwrap();
	assert_eq!(replay, origin_orbis_provider::BeginStreaming::Installed(original.clone()));
	drop(store);
	let reopened = StreamingStore::open(temp.path()).unwrap();
	reopened.verify_installed(&original.cid).unwrap();

	let mut quarantined_again = journal(temp.path());
	let operation = quarantined_again["operations"]
		.as_object_mut()
		.unwrap()
		.values_mut()
		.next()
		.unwrap();
	operation["receipt"]["fingerprint"] = Value::String(format!("0x{}", "ff".repeat(32)));
	quarantined_again["quarantine"][&original.cid] = json!({
		"reason": "chunk_mismatch",
		"expected_bytes": bytes.len() as u64,
		"observed_bytes": bytes.len() as u64,
		"detection_sequence": 1
	});
	quarantined_again["detection_sequence"] = Value::from(1u64);
	fs::write(&journal_path, serde_json::to_vec_pretty(&quarantined_again).unwrap()).unwrap();
	drop(reopened);
	assert!(matches!(StreamingStore::open(temp.path()), Err(ContentError::IntegrityFailed)));
}
