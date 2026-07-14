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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
extern crate alloc;
use alloc::vec;

use crate::Pallet;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

// Helper: Create a profile for an account
fn create_profile<T: Config>(account: T::AccountId) {
	let mut data = vec![];
	let key = b"pub_name".to_vec();
	let value = BoundedVec::try_from(b"test".to_vec()).expect("Value should fit");
	data.push((key.try_into().expect("Key should fit"), value));
	pallet_profile::Pallet::<T>::set_profile(RawOrigin::Signed(account.clone()).into(), data)
		.expect("Profile creation should succeed");
}

benchmarks! {
	// Benchmark for creating a registry store.
	create_store {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);

		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
	}: _(RawOrigin::Signed(caller.clone()), tx_hash, doc_id.clone(), doc_author.clone(), doc_node_id.clone())
	verify {
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;
		let registry = Registries::<T>::get(registry_id).expect("Registry should exist");
		let creator_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Creator profile ID should exist");
		let doc_author_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&doc_author)
			.expect("Doc author profile ID should exist");
		assert_eq!(registry.status, Status::Active);
		assert_eq!(registry.doc_id, Some(BoundedVec::try_from(doc_id).expect("Doc ID should fit")));
		assert_eq!(registry.doc_node_id, Some(BoundedVec::try_from(doc_node_id).expect("Doc node ID should fit")));
		assert_eq!(registry.tx_hash, tx_hash);
		assert_eq!(registry.creator, creator_profile_id);
		assert_eq!(registry.doc_author_profile_id, Some(doc_author_profile_id));
	}

	// Benchmark for adding a delegate to a registry.
	add_delegate {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);

		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author.clone(),
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;

		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");
		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());
		let delegate: T::AccountId = account("delegate", 2, 0);

		create_profile::<T>(delegate.clone());
		let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
			.expect("Delegate profile ID should exist");
		let permissions = vec![PermissionVariant::Entry, PermissionVariant::Delegate];
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone(), delegate.clone(), permissions)
	verify {
		assert!(Delegates::<T>::contains_key(&registry_id, &delegate_profile_id));
	}

	// Benchmark for removing a delegate from a registry.
	remove_delegate {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author,
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;
		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");

		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());
		let delegate: T::AccountId = account("delegate", 2, 0);

		create_profile::<T>(delegate.clone());
		let delegate_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&delegate)
			.expect("Delegate profile ID should exist");
		Delegates::<T>::insert(&registry_id, &delegate_profile_id, Permissions::all());
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone(), delegate.clone())
	verify {
		assert!(!Delegates::<T>::contains_key(registry_id, &delegate_profile_id));
	}

	// Benchmark for archiving a registry.
	archive {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);

		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author,
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;

		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");
		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).expect("Registry should exist");
		assert_eq!(registry.status, Status::Archived);
	}

	// Benchmark for restoring a registry.
	restore {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);

		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author,
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;

		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");
		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());

		Pallet::<T>::archive(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone()
		).expect("Archiving should succeed");
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).expect("Registry should exist");
		assert_eq!(registry.status, Status::Active);
	}

	// Benchmark for updating registry author.
	update_author {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);

		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author,
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;

		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");
		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());

		let new_author: T::AccountId = account("new_author", 2, 0);
		create_profile::<T>(new_author.clone());
		let new_author_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&new_author)
			.expect("New author profile ID should exist");
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone(), new_author.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).expect("Registry should exist");
		assert_eq!(registry.doc_author_profile_id, Some(new_author_profile_id));
	}

	// Benchmark for updating registry creator.
	update_creator {
		let caller: T::AccountId = account("caller", 0, 0);
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(caller.clone());
		create_profile::<T>(doc_author.clone());

		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id,
			doc_author,
			doc_node_id
		).expect("Registry creation should succeed");
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;

		let caller_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&caller)
			.expect("Caller profile ID should exist");
		Delegates::<T>::insert(&registry_id, &caller_profile_id, Permissions::all());

		let new_creator: T::AccountId = account("new_creator", 2, 0);
		create_profile::<T>(new_creator.clone());
		let new_creator_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&new_creator)
			.expect("New creator profile ID should exist");
	}: _(RawOrigin::Signed(caller.clone()), registry_id.clone(), new_creator.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).expect("Registry should exist");
		assert_eq!(registry.creator, new_creator_profile_id);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
