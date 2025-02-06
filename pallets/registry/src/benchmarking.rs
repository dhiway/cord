// This file is part of CORD – https://cord.network
//
// Copyright (C)
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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_core::H256;
use sp_std::prelude::*;

benchmarks! {
	// Benchmark for registry creation.
	create {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		// Generate dummy inputs.
		let tx_hash: H256 = H256::random();
		let doc_id: Option<Vec<u8>> = None;
		let doc_author_id: Option<CordAccountOf<T>> = None;
		let doc_node_id: Option<Vec<u8>> = None;
	}: _(origin.into(), tx_hash, doc_id, doc_author_id, doc_node_id)
	verify {
		// Ensure one registry exists.
		assert!(Registries::<T>::iter().count() == 1);
	}

	// Benchmark for adding a delegate to a registry.
	add_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		// Build a registry identifier.
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let registry_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		// Insert a registry with Active status.
		Registries::<T>::insert(&registry_id, RegistryDetails {
			creator: caller.clone(),
			tx_hash: H256::random(),
			doc_id: None,
			doc_author_id: None,
			doc_node_id: None,
			status: Status::Active,
		});
		// Ensure the caller has full permissions.
		Delegates::<T>::insert(&registry_id, &caller, Permissions::all());
		// Create a delegate.
		let delegate: T::AccountId = whitelisted_caller();
		let permissions = Permissions::all();
	}: _(origin.into(), registry_id, delegate.clone(), permissions)
	verify {
		assert!(Delegates::<T>::contains_key(&registry_id, &delegate));
	}

	// Benchmark for removing a delegate from a registry.
	remove_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let registry_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		// Setup: insert registry and add two delegates.
		Registries::<T>::insert(&registry_id, RegistryDetails {
			creator: caller.clone(),
			tx_hash: H256::random(),
			doc_id: None,
			doc_author_id: None,
			doc_node_id: None,
			status: Status::Active,
		});
		Delegates::<T>::insert(&registry_id, &caller, Permissions::all());
		let delegate: T::AccountId = whitelisted_caller();
		Delegates::<T>::insert(&registry_id, &delegate, Permissions::all());
	}: _(origin.into(), registry_id, delegate.clone())
	verify {
		assert!(!Delegates::<T>::contains_key(&registry_id, &delegate));
	}

	// Benchmark for archiving a registry.
	archive {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let registry_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		// Setup: insert an active registry.
		Registries::<T>::insert(&registry_id, RegistryDetails {
			creator: caller.clone(),
			tx_hash: H256::random(),
			doc_id: None,
			doc_author_id: None,
			doc_node_id: None,
			status: Status::Active,
		});
		Delegates::<T>::insert(&registry_id, &caller, Permissions::all());
	}: _(origin.into(), registry_id)
	verify {
		let registry = Registries::<T>::get(&registry_id).unwrap();
		assert!(registry.status == Status::Archived);
	}

	// Benchmark for restoring a registry.
	restore {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let registry_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		// Setup: insert an archived registry.
		Registries::<T>::insert(&registry_id, RegistryDetails {
			creator: caller.clone(),
			tx_hash: H256::random(),
			doc_id: None,
			doc_author_id: None,
			doc_node_id: None,
			status: Status::Archived,
		});
		Delegates::<T>::insert(&registry_id, &caller, Permissions::all());
	}: _(origin.into(), registry_id)
	verify {
		let registry = Registries::<T>::get(&registry_id).unwrap();
		assert!(registry.status == Status::Active);
	}

	// Benchmark for updating registry entry author.
	update_author {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		// Setup: create a registry.
		assert_ok!(Registry::create(
			origin.clone().into(),
			H256::random(),
			None,
			None,
			None
		));
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;
		let new_author: T::AccountId = whitelisted_caller();
	}: _(origin.into(), registry_id, new_author.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).unwrap();
		assert_eq!(registry.doc_author_id, Some(new_author));
	}

	// Benchmark for updating registry creator.
	update_creator {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
		// Setup: create a registry.
		assert_ok!(Registry::create(
			origin.clone().into(),
			H256::random(),
			None,
			None,
			None
		));
		let registry_id = Registries::<T>::iter().next().expect("Registry exists").0;
		let new_creator: T::AccountId = whitelisted_caller();
	}: _(origin.into(), registry_id, new_creator.clone())
	verify {
		let registry = Registries::<T>::get(&registry_id).unwrap();
		assert_eq!(registry.creator, new_creator);
	}
}
