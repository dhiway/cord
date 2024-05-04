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
use frame_system::RawOrigin;
use identifier::{IdentifierType, Ss58Identifier};
use pallet_chain_space::SpaceCodeOf;

const SEED: u32 = 0;
const MAX_PAYLOAD_BYTE_LENGTH: u32 = 15 * 1024;

pub fn generate_space_id<T: Config>(other_digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(other_digest).encode()[..], IdentifierType::Space).unwrap()
}

pub fn generate_rating_id<T: Config>(other_digest: &RatingEntryHashOf<T>) -> RatingEntryIdOf {
	Ss58Identifier::create_identifier(&(other_digest).encode()[..], IdentifierType::Rating).unwrap()
}

pub fn generate_authorization_id<T: Config>(digest: &SpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	where_clause {
		where
		<T as pallet::Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SpaceCreatorId>,
	}
	register_rating {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did1: T::SpaceCreatorId = account("did1", 0, SEED);

		let message_id = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
		let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
		let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
		let entry = RatingInputEntryOf::<T> {
			entity_id: entity_id.clone(),
			provider_id,
			total_encoded_rating: 250u64,
			count_of_txn: 7u64,
			rating_type: RatingTypeOf::Overall,
			provider_did: did1.clone(),
		};

		let raw_space = [2u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did1.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);
		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did1.encode()[..]].concat()[..],
		);
		let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as pallet::Config>::EnsureOrigin::generate_origin(caller.clone(), did1.clone());

		let entry_digest = <T as frame_system::Config>::Hashing::hash(&entry.encode()[..]);

		let id_digest =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id.encode()[..], &space_id.encode()[..], &did1.encode()[..]].concat()[..]
		);
		let identifier = generate_rating_id::<T>(&id_digest);

		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, 3u64 ).expect("Approval should not fail.");

	}: _<T::RuntimeOrigin>(origin, entry, entry_digest, message_id, authorization_id)
	verify {
		assert_last_event::<T>(Event::RatingEntryAdded { identifier, entity: entity_id, provider: did1, creator: caller}.into());
	}

	revoke_rating {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did1: T::SpaceCreatorId = account("did1", 0, SEED);

		let raw_space = [2u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did1.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);
		let message_id_add = BoundedVec::try_from([82u8; 10].to_vec()).unwrap();
		let message_id_revoke = BoundedVec::try_from([85u8; 10].to_vec()).unwrap();
		let entity_id = BoundedVec::try_from([83u8; 10].to_vec()).unwrap();
		let provider_id = BoundedVec::try_from([84u8; 10].to_vec()).unwrap();
		let entry = RatingInputEntryOf::<T> {
			entity_id: entity_id.clone(),
			provider_id,
			total_encoded_rating: 250u64,
			count_of_txn: 7u64,
			rating_type: RatingTypeOf::Overall,
			provider_did: did1.clone(),
		};
		let entry_digest = <T as frame_system::Config>::Hashing::hash(
			&[&entry.encode()[..]].concat()[..],
		);

		let id_digest_add =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id_add.encode()[..], &space_id.encode()[..], &did1.encode()[..]].concat()[..]
		);
		let identifier_add = generate_rating_id::<T>(&id_digest_add);

		let id_digest =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id_revoke.encode()[..], &space_id.encode()[..], &did1.encode()[..]].concat()[..]
		);
		let identifier_revoke = generate_rating_id::<T>(&id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did1.encode()[..]].concat()[..],
		);
		let authorization_id: AuthorizationIdOf =
			generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as pallet::Config>::EnsureOrigin::generate_origin(caller.clone(), did1.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, 3u64 ).expect("Approval should not fail.");

		let _ = Pallet::<T>::register_rating(origin.clone(), entry, entry_digest, message_id_add, authorization_id.clone());
	}: _<T::RuntimeOrigin>(origin, identifier_add, message_id_revoke, entry_digest, authorization_id)
	verify {
		assert_last_event::<T>(Event::RatingEntryRevoked { identifier: identifier_revoke, entity: entity_id, provider: did1, creator: caller}.into());
	}

	revise_rating {
		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SpaceCreatorId = account("did", 0, SEED);

		let raw_space = [2u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let message_id_add = BoundedVec::try_from([82u8; 10].to_vec()).unwrap();
		let message_id_revoke = BoundedVec::try_from([85u8; 10].to_vec()).unwrap();
		let message_id_revise = BoundedVec::try_from([86u8; 10].to_vec()).unwrap();
		let entity_id = BoundedVec::try_from([83u8; 10].to_vec()).unwrap();
		let provider_id = BoundedVec::try_from([84u8; 10].to_vec()).unwrap();
		let entry = RatingInputEntryOf::<T> {
			entity_id: entity_id.clone(),
			provider_id: provider_id.clone(),
			total_encoded_rating: 250u64,
			count_of_txn: 7u64,
			rating_type: RatingTypeOf::Overall,
			provider_did: did.clone(),
		};

		let entry_digest = <T as frame_system::Config>::Hashing::hash(
			&[&entry.encode()[..]].concat()[..],
		);

		let entry_revise = RatingInputEntryOf::<T> {
			entity_id: entity_id.clone(),
			provider_id,
			total_encoded_rating: 250u64,
			count_of_txn: 6u64,
			rating_type: RatingTypeOf::Overall,
			provider_did: did.clone(),
		};
		let entry_revise_digest = <T as frame_system::Config>::Hashing::hash(
			&[&entry.encode()[..]].concat()[..],
		);

		let id_digest_add =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id_add.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..]
		);
		let identifier_add = generate_rating_id::<T>(&id_digest_add);

		let id_digest =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id_revoke.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..]
		);
		let identifier_revoke = generate_rating_id::<T>(&id_digest);

		let id_digest_revise =  <T as frame_system::Config>::Hashing::hash(
			&[&(entry_revise_digest.clone()).encode()[..], &entity_id.encode()[..], &message_id_revise.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..]
		);
		let identifier_revise = generate_rating_id::<T>(&id_digest_revise);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
		);
		let authorization_id: AuthorizationIdOf = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as pallet::Config>::EnsureOrigin::generate_origin(caller.clone(), did.clone());
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, 10u64 ).expect("Approval should not fail.");

		let _ = Pallet::<T>::register_rating(origin.clone(), entry, entry_digest, message_id_add, authorization_id.clone());
		let _ = Pallet::<T>::revoke_rating(origin.clone(), identifier_add, message_id_revoke, entry_digest, authorization_id.clone());
	}: _<T::RuntimeOrigin>(origin, entry_revise, entry_revise_digest, message_id_revise, identifier_revoke, authorization_id)
	verify {
		assert_last_event::<T>(Event::RatingEntryRevised { identifier: identifier_revise, entity: entity_id, provider: did, creator: caller}.into());
	}

	impl_benchmark_test_suite! (Pallet, crate::mock::new_test_ext(), crate::mock::Test)
}
