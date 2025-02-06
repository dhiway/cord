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
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_core::H256;
use sp_std::prelude::*;

benchmarks! {
	create {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());
	}: _(origin.into())
	verify {
		assert!(Collections::<T>::iter().count() == 1);
	}

	add_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());

		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());

		let delegate: T::AccountId = whitelisted_caller();
		let permissions = Permissions::all();
	}: _(origin.into(), collection_id, delegate.clone(), permissions)
	verify {
		assert!(Delegates::<T>::contains_key(&collection_id, &delegate));
	}

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
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());

		let delegate: T::AccountId = whitelisted_caller();
		Delegates::<T>::insert(&collection_id, &delegate, Permissions::all());
	}: _(origin.into(), collection_id, delegate.clone())
	verify {
		assert!(!Delegates::<T>::contains_key(&collection_id, &delegate));
	}

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
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());
	}: _(origin.into(), collection_id)
	verify {
		let collection = Collections::<T>::get(&collection_id).unwrap();
		assert!(collection.status == Status::Archived);
	}

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
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());
		Collections::<T>::mutate(&collection_id, |maybe| {
			if let Some(ref mut collection) = maybe {
				collection.status = Status::Archived;
			}
		});
	}: _(origin.into(), collection_id)
	verify {
		let collection = Collections::<T>::get(&collection_id).unwrap();
		assert!(collection.status == Status::Active);
	}

	add_registry {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());

		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());

		let registry_id: RegistryIdentifierOf = H256::random().as_fixed_bytes().to_vec().try_into()
			.map_err(|_| "Registry id generation failed")?;
	}: _(origin.into(), collection_id, registry_id.clone())
	verify {
		assert!(CollectionRegistries::<T>::contains_key(&collection_id, &registry_id));
	}

	remove_registry {
		let caller: T::AccountId = whitelisted_caller();
		let origin = RawOrigin::Signed(caller.clone());

		let pallet_name = <Pallet<T> as PalletInfoAccess>::name();
		let previous_block_hash = <frame_system::Pallet<T>>::block_hash(
			<frame_system::Pallet<T>>::block_number().saturating_sub(One::one())
		);
		let mut input = previous_block_hash.encode();
		input.extend_from_slice(&caller.encode());
		let digest = T::Hashing::hash(&input);
		let collection_id = <cord_uri::Pallet<T> as Identifier>::build(&digest.encode(), pallet_name)
			.map_err(|_| "Identifier build failed")?;
		Collections::<T>::insert(&collection_id, CollectionDetails { creator: caller.clone(), status: Status::Active });
		Delegates::<T>::insert(&collection_id, &caller, Permissions::all());

		let registry_id: RegistryIdentifierOf = H256::random().as_fixed_bytes().to_vec().try_into()
			.map_err(|_| "Registry id generation failed")?;
		CollectionRegistries::<T>::insert(&collection_id, &registry_id, ());
	}: _(origin.into(), collection_id, registry_id.clone())
	verify {
		assert!(!CollectionRegistries::<T>::contains_key(&collection_id, &registry_id));
	}
}
