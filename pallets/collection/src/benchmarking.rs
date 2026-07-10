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
use alloc::{vec, vec::Vec};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use pallet_doken::Doken;

fn setup_profile<T: Config>(account: &T::AccountId) -> Result<ProfileIdOf, &'static str> {
	if let Ok(profile_id) = pallet_profile::Pallet::<T>::get_profile_id(account) {
		return Ok(profile_id)
	}

	pallet_profile::Pallet::<T>::set_profile(RawOrigin::Signed(account.clone()).into(), Vec::new())
		.map_err(|_| "Profile setup failed")?;
	pallet_profile::Pallet::<T>::get_profile_id(account).map_err(|_| "Profile lookup failed")
}

fn build_identifier<T: Config>(
	seed: &[u8],
	pallet_name: &'static str,
) -> Result<Ss58Identifier, &'static str> {
	let digest = T::Hashing::hash(seed);
	<pallet_doken::Pallet<T> as Doken<T>>::build(&(digest).encode()[..], pallet_name)
		.map_err(|_| "Identifier build failed")
}

fn setup_collection<T: Config>(
	caller: &T::AccountId,
	seed: &[u8],
) -> Result<(CollectionIdentifierOf, ProfileIdOf), &'static str> {
	let caller_profile_id = setup_profile::<T>(caller)?;
	let collection_id = build_identifier::<T>(
		seed,
		<Pallet<T> as frame_support::traits::PalletInfoAccess>::name(),
	)?;

	Collections::<T>::insert(
		&collection_id,
		CollectionDetails { creator: caller_profile_id.clone(), status: Status::Active },
	);
	Delegates::<T>::insert(&collection_id, &caller_profile_id, Permissions::all());

	Ok((collection_id, caller_profile_id))
}

fn build_registry_id<T: Config>(seed: &[u8]) -> Result<RegistryIdentifierOf, &'static str> {
	build_identifier::<T>(seed, "Registry")
}

benchmarks! {
	create {
		let caller: T::AccountId = whitelisted_caller();
		setup_profile::<T>(&caller)?;
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_eq!(Collections::<T>::iter().count(), 1);
	}

	add_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let delegate: T::AccountId = account("delegate", 0, 0);
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-add-delegate")?;
		let delegate_profile_id = setup_profile::<T>(&delegate)?;
		let permissions = vec![PermissionVariant::Entry, PermissionVariant::Delegate, PermissionVariant::Admin];
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone(), delegate.clone(), permissions)
	verify {
		assert!(Delegates::<T>::contains_key(&collection_id, &delegate_profile_id));
	}

	remove_delegate {
		let caller: T::AccountId = whitelisted_caller();
		let delegate: T::AccountId = account("delegate", 0, 0);
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-remove-delegate")?;
		let delegate_profile_id = setup_profile::<T>(&delegate)?;
		Delegates::<T>::insert(&collection_id, &delegate_profile_id, Permissions::all());
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone(), delegate.clone())
	verify {
		assert!(!Delegates::<T>::contains_key(&collection_id, &delegate_profile_id));
	}

	archive {
		let caller: T::AccountId = whitelisted_caller();
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-archive")?;
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone())
	verify {
		let collection = Collections::<T>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Archived);
	}

	restore {
		let caller: T::AccountId = whitelisted_caller();
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-restore")?;
		Collections::<T>::mutate(&collection_id, |maybe| {
			if let Some(ref mut collection) = maybe {
				collection.status = Status::Archived;
			}
		});
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone())
	verify {
		let collection = Collections::<T>::get(&collection_id).unwrap();
		assert_eq!(collection.status, Status::Active);
	}

	add_registry {
		let caller: T::AccountId = whitelisted_caller();
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-add-registry")?;
		let registry_id = build_registry_id::<T>(b"collection-registry-add")?;
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone(), registry_id.clone())
	verify {
		assert!(CollectionRegistries::<T>::contains_key(&collection_id, &registry_id));
	}

	remove_registry {
		let caller: T::AccountId = whitelisted_caller();
		let (collection_id, _) = setup_collection::<T>(&caller, b"collection-remove-registry")?;
		let registry_id = build_registry_id::<T>(b"collection-registry-remove")?;
		CollectionRegistries::<T>::insert(&collection_id, &registry_id, ());
	}: _(RawOrigin::Signed(caller.clone()), collection_id.clone(), registry_id.clone())
	verify {
		assert!(!CollectionRegistries::<T>::contains_key(&collection_id, &registry_id));
	}
}
