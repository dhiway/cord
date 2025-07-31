// This file is part of CORD – https://cord.network
//
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
//
// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use super::*;
extern crate alloc;
use alloc::vec;

use cord_primitives::Ss58Identifier;
use frame_benchmarking::{account, benchmarks};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;
use pallet_profile::{DataKeyOf, DataValueOf};
use pallet_registry::{PermissionVariant, RegistryIdentifierOf};
use sp_runtime::traits::Hash;

/// Helper: Create a profile for an account
fn create_profile<T: Config>(account: T::AccountId) {
	let mut data = vec![];
	let key = b"pub_key".to_vec();
	let value = vec![1u8; 100]; // Reasonable value size, matching pallet_profile
	let key_bounded: DataKeyOf<T> = key.try_into().expect("Key should fit");
	let value_bounded: DataValueOf<T> = value.try_into().expect("Value should fit");
	data.push((key_bounded, value_bounded));
	pallet_profile::Pallet::<T>::set_profile(RawOrigin::Signed(account).into(), data)
		.expect("Profile creation should succeed");
}

benchmarks! {
	create {
		// Whitelisted caller (creator)
		let caller: T::AccountId = account("caller", 0, 0);
		// Create a profile for the caller
		create_profile::<T>(caller.clone());
		// Create a registry
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		pallet_registry::Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id.clone(),
			doc_author.clone(),
			doc_node_id.clone(),
		)?;
		let registry_id = pallet_registry::Registries::<T>::iter()
			.next()
			.expect("Registry exists")
			.0;
		// Generate a transaction hash
		let entry_tx_hash = T::Hashing::hash(b"test_entry");
		// Create a maximum-size blob
		let blob: BoundedVec<u8, T::MaxRegistryEntryBlobSize> =
			BoundedVec::try_from(vec![0u8; T::MaxRegistryEntryBlobSize::get() as usize])
				.expect("Blob size within bounds");
	}: _(RawOrigin::Signed(caller), registry_id.clone(), entry_tx_hash, Some(blob))
	verify {
		// Verify the registry entry was created
		let entry_id = HashToIdentifier::<T>::get(entry_tx_hash, registry_id).expect("Entry ID exists");
		assert!(RegistryEntries::<T>::contains_key(&entry_id));
	}

	update {
		// Whitelisted caller (creator)
		let caller: T::AccountId = account("caller", 0, 0);
		// Create a profile for the caller
		create_profile::<T>(caller.clone());
		// Create a registry
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		pallet_registry::Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id.clone(),
			doc_author.clone(),
			doc_node_id.clone(),
		)?;
		let registry_id = pallet_registry::Registries::<T>::iter()
			.next()
			.expect("Registry exists")
			.0;
		// Create a registry entry
		let entry_tx_hash = T::Hashing::hash(b"test_entry");
		let blob: BoundedVec<u8, T::MaxRegistryEntryBlobSize> =
			BoundedVec::try_from(vec![0u8; T::MaxRegistryEntryBlobSize::get() as usize])
				.expect("Blob size within bounds");
		Pallet::<T>::create(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			entry_tx_hash,
			Some(blob.clone()),
		)?;
		let entry_id = HashToIdentifier::<T>::get(entry_tx_hash, &registry_id).expect("Entry ID exists");
		// New transaction hash for update
		let new_tx_hash = T::Hashing::hash(b"updated_entry");
	}: _(RawOrigin::Signed(caller), registry_id, entry_id.clone(), new_tx_hash, Some(blob))
	verify {
		// Verify the entry was updated
		let entry = RegistryEntries::<T>::get(&entry_id).expect("Entry exists");
		assert_eq!(entry.tx_hash, new_tx_hash);
	}

	revoke {
		// Whitelisted caller (creator)
		let caller: T::AccountId = account("caller", 0, 0);
		// Create a profile for the caller
		create_profile::<T>(caller.clone());
		// Create a registry
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		pallet_registry::Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id.clone(),
			doc_author.clone(),
			doc_node_id.clone(),
		)?;
		let registry_id = pallet_registry::Registries::<T>::iter()
			.next()
			.expect("Registry exists")
			.0;
		// Create a registry entry
		let entry_tx_hash = T::Hashing::hash(b"test_entry");
		Pallet::<T>::create(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			entry_tx_hash,
			None,
		)?;
		let entry_id = HashToIdentifier::<T>::get(entry_tx_hash, &registry_id).expect("Entry ID exists");
	}: _(RawOrigin::Signed(caller), registry_id, entry_id.clone())
	verify {
		// Verify the entry was revoked
		let entry = RegistryEntries::<T>::get(&entry_id).expect("Entry exists");
		assert!(entry.revoked);
	}

	reinstate {
		// Whitelisted caller (creator)
		let caller: T::AccountId = account("caller", 0, 0);
		// Create a profile for the caller
		create_profile::<T>(caller.clone());
		// Create a registry
		let doc_author: T::AccountId = account("doc_author", 1, 0);
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		pallet_registry::Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id.clone(),
			doc_author.clone(),
			doc_node_id.clone(),
		)?;
		let registry_id = pallet_registry::Registries::<T>::iter()
			.next()
			.expect("Registry exists")
			.0;
		// Create a registry entry
		let entry_tx_hash = T::Hashing::hash(b"test_entry");
		Pallet::<T>::create(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			entry_tx_hash,
			None,
		)?;
		let entry_id = HashToIdentifier::<T>::get(entry_tx_hash, &registry_id).expect("Entry ID exists");
		// Revoke the entry first
		Pallet::<T>::revoke(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			entry_id.clone(),
		)?;
	}: _(RawOrigin::Signed(caller), registry_id, entry_id.clone())
	verify {
		// Verify the entry was reinstated
		let entry = RegistryEntries::<T>::get(&entry_id).expect("Entry exists");
		assert!(!entry.revoked);
	}

	update_ownership {
		// Whitelisted caller (creator)
		let caller: T::AccountId = account("caller", 0, 0);
		// New owner account
		let new_owner: T::AccountId = account("new_owner", 1, 0);
		// Create profiles for both
		create_profile::<T>(caller.clone());
		create_profile::<T>(new_owner.clone());
		// Create a registry
		let doc_author: T::AccountId = account("doc_author", 2, 0);
		create_profile::<T>(doc_author.clone());
		let tx_hash = T::Hash::default();
		let doc_id = b"doc_id".to_vec();
		let doc_node_id = b"doc_node_id".to_vec();
		pallet_registry::Pallet::<T>::create_store(
			RawOrigin::Signed(caller.clone()).into(),
			tx_hash,
			doc_id.clone(),
			doc_author.clone(),
			doc_node_id.clone(),
		)?;
		let registry_id = pallet_registry::Registries::<T>::iter()
			.next()
			.expect("Registry exists")
			.0;
		// Grant new owner ENTRY permission
		let new_owner_profile_id = pallet_profile::Pallet::<T>::get_profile_id(&new_owner)
			.expect("New owner profile ID should exist");
		pallet_registry::Pallet::<T>::add_delegate(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			new_owner.clone(),
			vec![PermissionVariant::Entry],
		)?;
		// Create a registry entry
		let entry_tx_hash = T::Hashing::hash(b"test_entry");
		Pallet::<T>::create(
			RawOrigin::Signed(caller.clone()).into(),
			registry_id.clone(),
			entry_tx_hash,
			None,
		)?;
		let entry_id = HashToIdentifier::<T>::get(entry_tx_hash, &registry_id).expect("Entry ID exists");
	}: _(RawOrigin::Signed(caller), registry_id, entry_id.clone(), new_owner)
	verify {
		// Verify the entry ownership was updated
		let entry = RegistryEntries::<T>::get(&entry_id).expect("Entry exists");
		assert_eq!(entry.creator, new_owner_profile_id);
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	);
}
