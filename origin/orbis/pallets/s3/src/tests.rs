// This file is part of CORD – https://cord.network
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
	mock::*, BucketByName, BucketNameOf, BucketObjectKeys, BucketStatus, Buckets, Error,
	ObjectHistory, ObjectKeyOf, Objects, OwnerBuckets, Pallet,
};
use frame_support::{assert_noop, assert_ok, traits::StorageVersion};

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

#[test]
fn genesis_is_empty_at_storage_version_one_and_names_are_strict() {
	new_test_ext().execute_with(|| {
		assert_eq!(StorageVersion::get::<S3>(), StorageVersion::new(1));
		assert_eq!(Buckets::<Test>::iter().count(), 0);
		for invalid in [b"".as_slice(), b"Upper", b"-start", b"end-", b"a..b", b"a.-b", b"a_b"] {
			assert_noop!(
				S3::create_bucket(RuntimeOrigin::signed(1), name(invalid)),
				if invalid.is_empty() {
					Error::<Test>::EmptyBucketName
				} else {
					Error::<Test>::InvalidBucketName
				}
			);
		}
		let bucket = create(1, b"valid.bucket-1");
		assert_eq!(BucketByName::<Test>::get(name(b"valid.bucket-1")), Some(bucket));
		assert_eq!(OwnerBuckets::<Test>::get(1).as_slice(), &[bucket]);
		assert_noop!(
			S3::create_bucket(RuntimeOrigin::signed(2), name(b"valid.bucket-1")),
			Error::<Test>::BucketNameTaken
		);
	});
}

#[test]
fn puts_use_only_canonical_hashes_and_exact_object_versions() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"objects");
		let object_key = key(b"private/path/object");
		let first = [1u8; 32];
		let second = [2u8; 32];
		assert_noop!(
			S3::put_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), first, None),
			Error::<Test>::ContentNotFound
		);
		add_canonical(first);
		add_canonical(second);
		assert_ok!(S3::set_versioning(RuntimeOrigin::signed(1), bucket, 1, true));
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(1),
			bucket,
			object_key.clone(),
			first,
			None,
		));
		let current = Objects::<Test>::get(bucket, &object_key).unwrap();
		assert_eq!(current.version, 1);
		assert_eq!(current.content_hash, Some(first));
		assert_noop!(
			S3::put_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), second, None,),
			Error::<Test>::ObjectAlreadyExists
		);
		assert_noop!(
			S3::put_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), second, Some(9),),
			Error::<Test>::ObjectVersionMismatch
		);
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(1),
			bucket,
			object_key.clone(),
			second,
			Some(1),
		));
		assert_eq!(Objects::<Test>::get(bucket, &object_key).unwrap().version, 2);
		assert_eq!(ObjectHistory::<Test>::get(bucket, &object_key).len(), 1);
		assert_noop!(
			S3::delete_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 1),
			Error::<Test>::ObjectVersionMismatch
		);
		assert_ok!(S3::delete_object(RuntimeOrigin::signed(1), bucket, object_key.clone(), 2));
		let deleted = Objects::<Test>::get(bucket, &object_key).unwrap();
		assert!(deleted.deleted);
		assert_eq!(deleted.version, 3);
		assert_eq!(deleted.content_hash, None);
		assert_eq!(ObjectHistory::<Test>::get(bucket, object_key).len(), 2);
	});
}

#[test]
fn controllers_archive_transfer_and_delete_are_bounded_and_fail_closed() {
	new_test_ext().execute_with(|| {
		let bucket = create(1, b"lifecycle");
		let object_key = key(b"key");
		let content = [7u8; 32];
		add_canonical(content);
		assert_ok!(S3::set_controller(RuntimeOrigin::signed(1), bucket, 1, 2, true));
		assert_ok!(S3::put_object(
			RuntimeOrigin::signed(2),
			bucket,
			object_key.clone(),
			content,
			None,
		));
		assert_ok!(S3::set_archived(RuntimeOrigin::signed(1), bucket, 2, true));
		assert_noop!(
			S3::put_object(RuntimeOrigin::signed(2), bucket, key(b"other"), content, None,),
			Error::<Test>::BucketArchived
		);
		assert_noop!(
			S3::delete_bucket(RuntimeOrigin::signed(1), bucket, 3),
			Error::<Test>::BucketNotEmpty
		);
		assert_ok!(S3::set_archived(RuntimeOrigin::signed(1), bucket, 3, false));
		assert_ok!(S3::delete_object(RuntimeOrigin::signed(2), bucket, object_key.clone(), 1));
		assert_ok!(S3::transfer_bucket(RuntimeOrigin::signed(1), bucket, 4, 3));
		assert!(OwnerBuckets::<Test>::get(1).is_empty());
		assert_eq!(OwnerBuckets::<Test>::get(3).as_slice(), &[bucket]);
		assert!(Buckets::<Test>::get(bucket).unwrap().controllers.is_empty());
		assert_ok!(S3::delete_bucket(RuntimeOrigin::signed(3), bucket, 5));
		assert_eq!(Buckets::<Test>::get(bucket).unwrap().status, BucketStatus::Deleted);
		assert!(BucketObjectKeys::<Test>::get(bucket).is_empty());
		assert!(!Objects::<Test>::contains_key(bucket, object_key));
		assert!(OwnerBuckets::<Test>::get(3).is_empty());
		assert_noop!(
			S3::create_bucket(RuntimeOrigin::signed(4), name(b"lifecycle")),
			Error::<Test>::BucketNameTaken
		);
	});
}
