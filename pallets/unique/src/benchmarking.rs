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
use codec::Encode;
use cord_utilities::traits::GenerateBenchmarkOrigin;
use frame_benchmarking::{account, benchmarks};
use frame_support::{sp_runtime::traits::Hash, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};

const SEED: u32 = 0;
const MAX_PAYLOAD_BYTE_LENGTH: u32 = 5 * 1024;

pub fn generate_registry_id<T: Config>(other_digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(other_digest).encode()[..]).unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause {
		where
		<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::RegistryCreatorId>,
		}
	create {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let raw_unique: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let unique_txn : InputUniqueOf::<T> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of the test runtime.");

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);

		let authorization_id: Option<T> = None;

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&unique_txn.encode()[..], &did.encode()[..]].concat()[..],
		);

		let identifier = Ss58Identifier::to_unique_id(&(id_digest).encode()[..]).expect("Invalid");


	}: _<T::RuntimeOrigin>(origin, unique_txn.clone(), None)
	verify {
		assert_last_event::<T>(Event::Create { identifier, digest: unique_txn, author: did }.into());
	}

	revoke {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let raw_unique: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let unique_txn: InputUniqueOf::<T> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of the test runtime.");

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);
		let did1: T::RegistryCreatorId = account("did1", 0, SEED);

		let raw_registry = [56u8; 256].to_vec();
		let registry: InputRegistryOf<T> = BoundedVec::try_from(raw_registry).unwrap();

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
			);

		let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &did1.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&unique_txn.encode()[..], &did.encode()[..]].concat()[..],
		);

		let unique_id = Ss58Identifier::to_unique_id(&(id_digest).encode()[..]).expect("Invalid");

		<UniqueDigestEntries<T>>::insert(&unique_txn, &unique_id);

		<UniqueIdentifiers<T>>::insert(
			&unique_id,
			UniqueEntryOf::<T> {
				digest: unique_txn.clone(),
				creator: did.clone(),
				registry: Some(Some(registry_id)),
				revoked: false,
			},
		);

	}: _<T::RuntimeOrigin>(origin, unique_txn, authorization_id)
	verify {
		assert_last_event::<T>(Event::Revoke { identifier: unique_id, author: did }.into());
	}

	update {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let raw_unique: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let unique_txn: InputUniqueOf::<T> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of the test runtime.");

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);
		let did1: T::RegistryCreatorId = account("did1", 0, SEED);

		let raw_registry = [56u8; 256].to_vec();
		let registry: InputRegistryOf<T> = BoundedVec::try_from(raw_registry).unwrap();

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
			);

		let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&unique_txn.encode()[..], &did.encode()[..]].concat()[..],
		);

		let unique_id = Ss58Identifier::to_unique_id(&(id_digest).encode()[..]).expect("Invalid");

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &did1.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

		<UniqueDigestEntries<T>>::insert(&unique_txn, &unique_id);

		<UniqueIdentifiers<T>>::insert(
			&unique_id,
			UniqueEntryOf::<T> {
				digest: unique_txn,
				creator: did.clone(),
				registry: Some(Some(registry_id)),
				revoked: false,
			},
		);

		let new_raw_unique: Vec<u8> = (12u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let new_unique_txn: InputUniqueOf::<T> = BoundedVec::try_from(new_raw_unique)
		.expect("Unique should fit into the expected input length of the test runtime.");
		let updated_unique_digest = <T as frame_system::Config>::Hashing::hash(
			&[&new_unique_txn.encode()[..], &did.encode()[..]].concat()[..],
		);

	}: _<T::RuntimeOrigin>(origin, unique_id.clone(), new_unique_txn.clone(), Some(authorization_id))
	verify {
		assert_last_event::<T>(Event::Update { identifier: unique_id,
			digest: new_unique_txn,
			author: did }.into());
	}

	remove {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let raw_unique: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let unique_txn: InputUniqueOf::<T>  = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of the test runtime.");

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::RegistryCreatorId = account("did", 0, SEED);

		let raw_registry = [56u8; 256].to_vec();
		let registry: InputRegistryOf<T> = BoundedVec::try_from(raw_registry).unwrap();

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&registry.encode()[..], &did.encode()[..]].concat()[..],
			);

		let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

		let authorization_id: Option<AuthorizationIdOf> = None;

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did.clone());

		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&unique_txn.encode()[..], &did.encode()[..]].concat()[..],
		);

		let unique_id = Ss58Identifier::to_unique_id(&(id_digest).encode()[..]).expect("Invalid");

		<UniqueDigestEntries<T>>::insert(&unique_txn, &unique_id);

		<UniqueIdentifiers<T>>::insert(
			&unique_id,
			UniqueEntryOf::<T> {
				digest: unique_txn,
				creator: did.clone(),
				registry: Some(Some(registry_id)),
				revoked: false,
			},
		);

	}: _<T::RuntimeOrigin>(origin, unique_id.clone(), None)
	verify {
		assert_last_event::<T>(Event::Remove { identifier: unique_id, author: did }.into());
	}

	impl_benchmark_test_suite! (
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	)

}
