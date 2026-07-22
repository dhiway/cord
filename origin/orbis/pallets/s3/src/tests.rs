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

use crate::{
	mock::*, BucketNameOf, BucketObjectCount, Buckets, Error, ListCursor, MetadataEntry,
	ObjectHistory, ObjectKeyCursor, ObjectKeyOf, ObjectOperations, Objects, OperationResults,
	Pallet, UserMetadata,
};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_orbis_storage_control_primitives::CommitmentState;

fn name(value: &[u8]) -> BucketNameOf<Test> {
	value.to_vec().try_into().unwrap()
}
fn key(value: &[u8]) -> ObjectKeyOf<Test> {
	value.to_vec().try_into().unwrap()
}
fn create(owner: u64, value: &[u8]) -> <Test as frame_system::Config>::Hash {
	let bounded = name(value);
	let id = Pallet::<Test>::bucket_id(&owner, &bounded);
	assert_ok!(S3::create_bucket(RuntimeOrigin::signed(owner), bounded));
	id
}
fn put(
	owner: u64,
	bucket: <Test as frame_system::Config>::Hash,
	object_key: ObjectKeyOf<Test>,
	content: [u8; 32],
	if_match: Option<[u8; 32]>,
	expected: Option<u64>,
) -> frame_support::dispatch::DispatchResult {
	S3::put_object(
		RuntimeOrigin::signed(owner),
		bucket,
		object_key,
		content,
		content,
		Default::default(),
		Default::default(),
		content,
		if_match,
		expected,
	)
}

fn metadata(value: &[u8]) -> UserMetadata {
	vec![MetadataEntry {
		key: b"purpose".to_vec().try_into().unwrap(),
		value: value.to_vec().try_into().unwrap(),
	}]
	.try_into()
	.unwrap()
}

#[allow(clippy::too_many_arguments)]
fn put_request(
	owner: u64,
	bucket: <Test as frame_system::Config>::Hash,
	object_key: ObjectKeyOf<Test>,
	content: [u8; 32],
	provider: [u8; 32],
	content_type: &[u8],
	user_metadata: UserMetadata,
	operation_id: [u8; 32],
	if_match: Option<[u8; 32]>,
	expected: Option<u64>,
) -> frame_support::dispatch::DispatchResult {
	S3::put_object(
		RuntimeOrigin::signed(owner),
		bucket,
		object_key,
		content,
		provider,
		content_type.to_vec().try_into().unwrap(),
		user_metadata,
		operation_id,
		if_match,
		expected,
	)
}

#[test]
fn bucket_and_key_limits_are_exact() {
	new_test_ext().execute_with(|| {
		for invalid in [b"ab".as_slice(), b"Upper", b"-start", b"end-", b"a.b", b"a_b"] {
			assert_noop!(
				S3::create_bucket(RuntimeOrigin::signed(1), name(invalid)),
				Error::<Test>::InvalidBucketName
			);
		}
		assert_ok!(S3::create_bucket(RuntimeOrigin::signed(1), name(&vec![b'a'; 63])));
		assert!(BucketNameOf::<Test>::try_from(vec![b'a'; 64]).is_err());
		assert_ok!(Pallet::<Test>::validate_object_key(&vec![b'a'; 1023]));
		assert_ok!(Pallet::<Test>::validate_object_key(&vec![b'a'; 1024]));
		assert_noop!(
			Pallet::<Test>::validate_object_key(&vec![b'a'; 1025]),
			Error::<Test>::InvalidObjectKey
		);
		assert_noop!(Pallet::<Test>::validate_object_key(b"a\0b"), Error::<Test>::InvalidObjectKey);
	});
}

#[test]
fn owner_controller_and_object_indexes_reject_limit_plus_one() {
	new_test_ext().execute_with(|| {
		let first = create(1, b"one");
		create(1, b"two");
		create(1, b"three");
		assert_noop!(
			S3::create_bucket(RuntimeOrigin::signed(1), name(b"four")),
			Error::<Test>::OwnerBucketIndexFull
		);
		assert_ok!(S3::set_controller(RuntimeOrigin::signed(1), first, 1, 2, true));
		assert_ok!(S3::set_controller(RuntimeOrigin::signed(1), first, 2, 3, true));
		assert_noop!(
			S3::set_controller(RuntimeOrigin::signed(1), first, 3, 4, true),
			Error::<Test>::ControllerIndexFull
		);
		for index in 0..4u8 {
			let manifest = [index.saturating_add(1); 32];
			add_canonical(manifest);
			assert_ok!(put(1, first, key(&[b'k', b'0' + index]), manifest, None, None));
		}
		let overflow = [9; 32];
		add_canonical(overflow);
		assert_noop!(
			put(1, first, key(b"overflow"), overflow, None, None),
			Error::<Test>::BucketObjectIndexFull
		);
	});
}

#[test]
fn bucket_deletion_requires_bounded_object_purge() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"purge-flow");
		let object_key = key(b"object");
		let manifest = [1; 32];
		let delete_operation = [2; 32];
		add_canonical(manifest);
		assert_ok!(put(1, bucket, object_key.clone(), manifest, None, None));
		assert_eq!(BucketObjectCount::<Test>::get(bucket), 1);
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(1),
			bucket,
			object_key.clone(),
			delete_operation,
			None,
			1,
		));
		let tombstone = Objects::<Test>::get(bucket, &object_key).unwrap();
		assert_eq!(tombstone.content_hash, Some(manifest));
		assert_eq!(tombstone.provider_commitment, Some(manifest));
		assert_eq!(
			OperationResults::<Test>::get(bucket, delete_operation).unwrap().content_hash,
			Some(manifest)
		);
		assert_noop!(
			S3::delete_bucket(RuntimeOrigin::signed(1), bucket, 3),
			Error::<Test>::BucketNotEmpty
		);

		set_deletion_requirements(manifest, &[1, 2, 3]);
		set_drive_referenced(manifest, true);
		assert_noop!(
			S3::purge_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2),
			Error::<Test>::ActiveDriveReference
		);
		set_drive_referenced(manifest, false);
		assert_noop!(
			S3::purge_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2),
			Error::<Test>::DeletionEvidencePending
		);

		// Replica acknowledgements without the primary must fail closed.
		set_deletion_acknowledgements(manifest, &[2, 3]);
		assert_noop!(
			S3::purge_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2),
			Error::<Test>::DeletionEvidencePending
		);
		// The primary and one replica are still insufficient when any canonical replica is missing.
		set_deletion_acknowledgements(manifest, &[1, 2]);
		assert_noop!(
			S3::purge_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2),
			Error::<Test>::DeletionEvidencePending
		);

		// Failed purge attempts must not erase the evidence or idempotency authority.
		assert_eq!(Objects::<Test>::get(bucket, &object_key).unwrap().content_hash, Some(manifest));
		assert!(OperationResults::<Test>::contains_key(bucket, delete_operation));
		assert_eq!(ObjectOperations::<Test>::get(bucket, &object_key), vec![delete_operation]);
		assert_eq!(BucketObjectCount::<Test>::get(bucket), 1);
		assert_noop!(
			S3::delete_bucket(RuntimeOrigin::signed(1), bucket, 3),
			Error::<Test>::BucketNotEmpty
		);

		set_deletion_acknowledgements(manifest, &[1, 2, 3]);
		assert_ok!(S3::purge_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2,));
		assert!(!Objects::<Test>::contains_key(bucket, &object_key));
		assert!(!OperationResults::<Test>::contains_key(bucket, delete_operation));
		assert_eq!(BucketObjectCount::<Test>::get(bucket), 0);
		assert_ok!(S3::delete_bucket(RuntimeOrigin::signed(1), bucket, 3));
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().status, crate::BucketStatus::Deleted);
	});
}

#[test]
fn put_requires_publishable_manifest_provider_and_exact_conditions() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"objects");
		let object_key = key(b"private/path/object");
		let first = [1; 32];
		assert_noop!(
			put(1, bucket, object_key.clone(), first, None, None),
			Error::<Test>::ManifestMissing
		);
		set_state(first, CommitmentState::Tombstoned, first);
		assert_noop!(
			put(1, bucket, object_key.clone(), first, None, None),
			Error::<Test>::ManifestTombstoned
		);
		set_state(first, CommitmentState::Publishable, [9; 32]);
		assert_noop!(
			put(1, bucket, object_key.clone(), first, None, None),
			Error::<Test>::ProviderCommitmentInvalid
		);
		add_canonical(first);
		assert_ok!(put(1, bucket, object_key.clone(), first, None, None));
		let events = System::events().len();
		assert_ok!(put(1, bucket, object_key.clone(), first, None, None));
		assert_eq!(Objects::<Test>::get(bucket, &object_key).unwrap().version, 1);
		assert_eq!(System::events().len(), events);
		assert_noop!(
			put(1, bucket, key(b"changed-key"), first, None, None),
			Error::<Test>::OperationIdConflict
		);
		let second = [2; 32];
		add_canonical(second);
		assert_noop!(
			put(1, bucket, object_key.clone(), second, Some([8; 32]), Some(1)),
			Error::<Test>::PreconditionFailed
		);
		assert_noop!(
			put(1, bucket, object_key.clone(), second, Some(first), Some(9)),
			Error::<Test>::ObjectVersionMismatch
		);
		assert_ok!(put(1, bucket, object_key.clone(), second, Some(first), Some(1)));
		assert_eq!(Objects::<Test>::get(bucket, object_key).unwrap().version, 2);
	});
}

#[test]
fn put_operation_id_fingerprints_every_semantic_input() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"put-replay");
		assert_ok!(S3::set_controller(RuntimeOrigin::signed(1), bucket, 1, 2, true));
		let object_key = key(b"object");
		let content = [1; 32];
		let provider = [1; 32];
		let operation_id = [0xA1; 32];
		add_canonical(content);

		assert_ok!(put_request(
			1,
			bucket,
			object_key.clone(),
			content,
			provider,
			b"text/plain",
			metadata(b"original"),
			operation_id,
			None,
			None,
		));
		let event_count = System::events().len();
		assert_ok!(put_request(
			1,
			bucket,
			object_key.clone(),
			content,
			provider,
			b"text/plain",
			metadata(b"original"),
			operation_id,
			None,
			None,
		));
		assert_eq!(System::events().len(), event_count);

		for changed in [
			put_request(
				1,
				bucket,
				object_key.clone(),
				content,
				[2; 32],
				b"text/plain",
				metadata(b"original"),
				operation_id,
				None,
				None,
			),
			put_request(
				1,
				bucket,
				object_key.clone(),
				content,
				provider,
				b"application/json",
				metadata(b"original"),
				operation_id,
				None,
				None,
			),
			put_request(
				1,
				bucket,
				object_key.clone(),
				content,
				provider,
				b"text/plain",
				metadata(b"changed"),
				operation_id,
				None,
				None,
			),
			put_request(
				1,
				bucket,
				object_key.clone(),
				content,
				[2; 32],
				b"application/json",
				metadata(b"changed"),
				operation_id,
				Some([9; 32]),
				Some(7),
			),
		] {
			assert_noop!(changed, Error::<Test>::OperationIdConflict);
		}
		assert_noop!(
			put_request(
				2,
				bucket,
				object_key,
				content,
				provider,
				b"text/plain",
				metadata(b"original"),
				operation_id,
				None,
				None,
			),
			Error::<Test>::OperationIdConflict
		);
	});
}

#[test]
fn delete_operation_id_fingerprints_actor_key_conditions_and_version() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"delete-replay");
		assert_ok!(S3::set_controller(RuntimeOrigin::signed(1), bucket, 1, 2, true));
		let object_key = key(b"object");
		let content = [3; 32];
		let operation_id = [0xD1; 32];
		add_canonical(content);
		assert_ok!(put(1, bucket, object_key.clone(), content, None, None));
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(1),
			bucket,
			object_key.clone(),
			operation_id,
			Some(content),
			1,
		));
		let event_count = System::events().len();
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(1),
			bucket,
			object_key.clone(),
			operation_id,
			Some(content),
			1,
		));
		assert_eq!(System::events().len(), event_count);

		assert_noop!(
			S3::delete_object(
				RuntimeOrigin::signed(1),
				bucket,
				key(b"other"),
				operation_id,
				Some(content),
				1,
			),
			Error::<Test>::OperationIdConflict
		);
		assert_noop!(
			S3::delete_object(
				RuntimeOrigin::signed(1),
				bucket,
				object_key.clone(),
				operation_id,
				None,
				1,
			),
			Error::<Test>::OperationIdConflict
		);
		assert_noop!(
			S3::delete_object(
				RuntimeOrigin::signed(1),
				bucket,
				object_key.clone(),
				operation_id,
				Some(content),
				2,
			),
			Error::<Test>::OperationIdConflict
		);
		assert_noop!(
			S3::delete_object(
				RuntimeOrigin::signed(2),
				bucket,
				object_key,
				operation_id,
				Some(content),
				1,
			),
			Error::<Test>::OperationIdConflict
		);
	});
}

#[test]
fn list_is_raw_byte_ordered_snapshot_bound_and_tombstones_are_hidden() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"listing");
		for (key_bytes, value) in [(b"z".as_slice(), [1; 32]), (b"A", [2; 32]), (b"a", [3; 32])] {
			add_canonical(value);
			assert_ok!(put(1, bucket, key(key_bytes), value, None, None));
		}
		let page = Pallet::<Test>::list_objects(bucket, None, None, 2).unwrap();
		assert_eq!(page.objects, vec![key(b"A"), key(b"a")]);
		let cursor = page.next_cursor.unwrap();
		let content = [4; 32];
		add_canonical(content);
		assert_ok!(put(1, bucket, key(b"b"), content, None, None));
		assert_noop!(
			Pallet::<Test>::list_objects(bucket, None, Some(cursor), 2),
			Error::<Test>::CursorStale
		);
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(1),
			bucket,
			key(b"A"),
			[7; 32],
			Some([2; 32]),
			1
		));
		let events = System::events().len();
		assert_ok!(S3::delete_object(
			RuntimeOrigin::signed(1),
			bucket,
			key(b"A"),
			[7; 32],
			Some([2; 32]),
			1
		));
		assert_eq!(System::events().len(), events);
		let all = Pallet::<Test>::list_objects(bucket, None, None, 100).unwrap();
		assert!(!all.objects.contains(&key(b"A")));
	});
}

#[test]
fn version_history_accepts_64_and_rejects_65_without_state_or_event() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"history");
		let object_key = key(b"object");
		assert_ok!(S3::set_versioning(RuntimeOrigin::signed(1), bucket, 1, true));
		let first = [1; 32];
		add_canonical(first);
		assert_ok!(put(1, bucket, object_key.clone(), first, None, None));
		let mut current = first;
		for version in 1..=64u64 {
			let next = [(version as u8).wrapping_add(1); 32];
			add_canonical(next);
			assert_ok!(put(1, bucket, object_key.clone(), next, Some(current), Some(version)));
			current = next;
		}
		assert_eq!(ObjectHistory::<Test>::get(bucket, &object_key).len(), 64);
		let before_events = System::events().len();
		let overflow = [99; 32];
		add_canonical(overflow);
		assert_noop!(
			put(1, bucket, object_key.clone(), overflow, Some(current), Some(65)),
			Error::<Test>::ObjectHistoryFull
		);
		assert_eq!(Objects::<Test>::get(bucket, object_key).unwrap().version, 65);
		assert_eq!(System::events().len(), before_events);
	});
}

#[test]
fn history_pruning_requires_no_drive_reference_and_deletion_evidence() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"pruning");
		let object_key = key(b"object");
		assert_ok!(S3::set_versioning(RuntimeOrigin::signed(1), bucket, 1, true));
		let first = [1; 32];
		let second = [2; 32];
		add_canonical(first);
		add_canonical(second);
		assert_ok!(put(1, bucket, object_key.clone(), first, None, None));
		assert_ok!(put(1, bucket, object_key.clone(), second, Some(first), Some(1)));
		set_drive_referenced(first, true);
		assert_noop!(
			S3::prune_history(RuntimeOrigin::signed(1), bucket, object_key.clone(), 1),
			Error::<Test>::ActiveDriveReference
		);
		set_drive_referenced(first, false);
		assert_noop!(
			S3::prune_history(RuntimeOrigin::signed(1), bucket, object_key.clone(), 1),
			Error::<Test>::DeletionEvidencePending
		);
		set_deletion_evidence(first);
		assert_ok!(S3::prune_history(RuntimeOrigin::signed(1), bucket, object_key.clone(), 1));
		assert!(ObjectHistory::<Test>::get(bucket, object_key).is_empty());
	});
}

#[test]
fn metadata_and_cursor_bounds_are_deterministic() {
	new_test_ext().execute_with(|| {
		let entry = |key: &[u8]| MetadataEntry {
			key: key.to_vec().try_into().unwrap(),
			value: BoundedVec::default(),
		};
		let sorted: UserMetadata = vec![entry(b"A"), entry(b"a")].try_into().unwrap();
		let reverse: UserMetadata = vec![entry(b"b"), entry(b"a")].try_into().unwrap();
		assert_ok!(Pallet::<Test>::validate_metadata(&sorted));
		assert_noop!(
			Pallet::<Test>::validate_metadata(&reverse),
			Error::<Test>::MetadataOrderInvalid
		);
		let bucket = create(1, b"cursor");
		assert_noop!(
			Pallet::<Test>::list_objects(bucket, None, None, 101),
			Error::<Test>::PageLimitInvalid
		);
		let stale = ListCursor { snapshot_version: 0, last_key: ObjectKeyCursor::default() };
		assert_noop!(
			Pallet::<Test>::list_objects(bucket, None, Some(stale), 100),
			Error::<Test>::CursorStale
		);
	});
}

#[test]
fn weights_are_monotonic_at_contract_boundaries() {
	use crate::WeightInfo;
	assert!(<() as WeightInfo>::create_bucket(63).all_gte(<() as WeightInfo>::create_bucket(62)));
	assert!(Pallet::<Test>::put_object_weight(1024, 64)
		.all_gte(Pallet::<Test>::put_object_weight(1023, 64)));
	let create = <() as WeightInfo>::put_object_create(1024);
	let update = <() as WeightInfo>::put_object_update(1024, 64);
	let dispatch = Pallet::<Test>::put_object_weight(1024, 64);
	assert!(dispatch.all_gte(create));
	assert!(dispatch.all_gte(update));
	assert!(<() as WeightInfo>::delete_object(1024, 64)
		.all_gte(<() as WeightInfo>::delete_object(1023, 64)));
	assert!(<() as WeightInfo>::prune_history(64).all_gte(<() as WeightInfo>::prune_history(63)));
	assert!(<() as WeightInfo>::purge_object(65).all_gte(<() as WeightInfo>::purge_object(64)));
}
