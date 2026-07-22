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

use super::*;
use alloc::{vec, vec::Vec};
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;

const SEED: u32 = 0;

pub trait BenchmarkHelper {
	fn make_publishable(manifest: Commitment, provider: Commitment);
	fn make_deletion_satisfied(manifest: Commitment);
}

fn bucket<T: Config>(owner: &T::AccountId, versioning_enabled: bool) -> T::Hash {
	let name: BucketNameOf<T> = b"benchmark-bucket".to_vec().try_into().expect("bounded name");
	let bucket = Pallet::<T>::bucket_id(owner, &name);
	let now = frame_system::Pallet::<T>::block_number();
	Buckets::<T>::insert(
		bucket,
		BucketRecord::<T> {
			name: name.clone(),
			owner: owner.clone(),
			controllers: Default::default(),
			status: BucketStatus::Active,
			versioning_enabled,
			version: 1,
			live_objects: 0,
			created_at: now,
			updated_at: now,
		},
	);
	BucketByName::<T>::insert(name, bucket);
	OwnerBuckets::<T>::try_mutate(owner, |items| items.try_push(bucket))
		.expect("bucket index has space");
	bucket
}

fn object<T: Config>(
	owner: &T::AccountId,
	bucket: T::Hash,
	key: &ObjectKeyOf<T>,
	version: u64,
	deleted: bool,
) -> ObjectRecord<T> {
	let now = frame_system::Pallet::<T>::block_number();
	ObjectRecord::<T> {
		object_id: Pallet::<T>::object_id(bucket, key),
		content_hash: Some([1; 32]),
		provider_commitment: Some([2; 32]),
		content_type: Default::default(),
		metadata: Default::default(),
		operation_id: [3; 32],
		version,
		deleted,
		updated_by: owner.clone(),
		updated_at: now,
	}
}

fn version<T: Config>(owner: &T::AccountId, i: u32) -> ObjectVersion<T> {
	ObjectVersion::<T> {
		content_hash: Some([i as u8; 32]),
		provider_commitment: Some([2; 32]),
		operation_id: [i as u8; 32],
		version: i as u64 + 1,
		deleted: false,
		updated_by: owner.clone(),
		updated_at: frame_system::Pallet::<T>::block_number(),
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn create_bucket(n: Linear<3, { T::MaxBucketNameLen::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let name: BucketNameOf<T> = vec![b'a'; n as usize].try_into().expect("bounded name");
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), name);
	}

	#[benchmark]
	fn set_controller() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let controller: T::AccountId = account("controller", 0, SEED);
		let id = bucket::<T>(&owner, false);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, controller, true);
	}

	#[benchmark]
	fn transfer_bucket() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let new_owner: T::AccountId = account("owner", 1, SEED);
		let id = bucket::<T>(&owner, false);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, new_owner);
	}

	#[benchmark]
	fn set_archived() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, false);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, true);
	}

	#[benchmark]
	fn set_versioning() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, false);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, true);
	}

	#[benchmark]
	fn put_object_create(k: Linear<1, { T::MaxObjectKeyLen::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, true);
		let key: ObjectKeyOf<T> = vec![b'k'; k as usize].try_into().expect("bounded key");
		T::BenchmarkHelper::make_publishable([9; 32], [8; 32]);
		#[block]
		{
			assert_ok!(Pallet::<T>::put_object(
				RawOrigin::Signed(owner).into(),
				id,
				key,
				[9; 32],
				[8; 32],
				Default::default(),
				Default::default(),
				[7; 32],
				None,
				None,
			));
		}
	}

	#[benchmark]
	fn put_object_update(
		k: Linear<1, { T::MaxObjectKeyLen::get() }>,
		h: Linear<0, { T::MaxObjectVersions::get() - 1 }>,
	) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, true);
		let key: ObjectKeyOf<T> = vec![b'k'; k as usize].try_into().expect("bounded key");
		let current_version = h as u64 + 1;
		Objects::<T>::insert(id, &key, object::<T>(&owner, id, &key, current_version, false));
		Buckets::<T>::mutate(id, |record| record.as_mut().unwrap().live_objects = 1);
		let history: ObjectHistoryOf<T> =
			(0..h).map(|i| version::<T>(&owner, i)).collect::<Vec<_>>().try_into().unwrap();
		ObjectHistory::<T>::insert(id, &key, history);
		T::BenchmarkHelper::make_publishable([9; 32], [8; 32]);
		#[block]
		{
			assert_ok!(Pallet::<T>::put_object(
				RawOrigin::Signed(owner).into(),
				id,
				key,
				[9; 32],
				[8; 32],
				Default::default(),
				Default::default(),
				[7; 32],
				None,
				Some(current_version),
			));
		}
	}

	#[benchmark]
	fn delete_object(
		k: Linear<1, { T::MaxObjectKeyLen::get() }>,
		h: Linear<0, { T::MaxObjectVersions::get() - 1 }>,
	) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, true);
		let key: ObjectKeyOf<T> = vec![b'k'; k as usize].try_into().expect("bounded key");
		Objects::<T>::insert(id, &key, object::<T>(&owner, id, &key, h as u64 + 1, false));
		Buckets::<T>::mutate(id, |record| record.as_mut().unwrap().live_objects = 1);
		let history: ObjectHistoryOf<T> =
			(0..h).map(|i| version::<T>(&owner, i)).collect::<Vec<_>>().try_into().unwrap();
		ObjectHistory::<T>::insert(id, &key, history);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, key, [7; 32], None, h as u64 + 1);
	}

	#[benchmark]
	fn delete_bucket() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, false);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1);
	}

	#[benchmark]
	fn prune_history(h: Linear<1, { T::MaxObjectVersions::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, true);
		let key: ObjectKeyOf<T> = b"key".to_vec().try_into().unwrap();
		let history: ObjectHistoryOf<T> = (0..h)
			.map(|i| {
				let item = version::<T>(&owner, i);
				T::BenchmarkHelper::make_deletion_satisfied(item.content_hash.unwrap());
				item
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		ObjectHistory::<T>::insert(id, &key, history);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, key, h as u64);
	}

	#[benchmark]
	fn purge_object(h: Linear<1, 65>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let id = bucket::<T>(&owner, false);
		let key: ObjectKeyOf<T> = b"key".to_vec().try_into().unwrap();
		Objects::<T>::insert(id, &key, object::<T>(&owner, id, &key, 1, true));
		BucketObjectCount::<T>::insert(id, 1);
		T::BenchmarkHelper::make_deletion_satisfied([1; 32]);
		let operation_ids: ObjectOperationIds = (0..h)
			.map(|i| {
				let mut operation_id = [0u8; 32];
				operation_id[..4].copy_from_slice(&i.to_le_bytes());
				OperationResults::<T>::insert(
					id,
					operation_id,
					OperationRecord::<T> {
						request_fingerprint: T::Hashing::hash_of(&i),
						key: key.clone(),
						content_hash: Some([1; 32]),
						deleted: true,
						expected_object_version: Some(1),
						if_match: None,
						outcome_version: 1,
					},
				);
				operation_id
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		ObjectOperations::<T>::insert(id, &key, operation_ids);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, key, 1);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
