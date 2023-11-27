// // This file is part of CORD – https://cord.network

// // Copyright (C) Dhiway Networks Pvt. Ltd.
// // SPDX-License-Identifier: GPL-3.0-or-later

// // CORD is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version.

// // CORD is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// // GNU General Public License for more details.

// // You should have received a copy of the GNU General Public License
// // along with CORD. If not, see <https://www.gnu.org/licenses/>.

// #![cfg(feature = "runtime-benchmarks")]

// use super::*;
// use codec::Encode;
// use cord_primitives::curi::Ss58Identifier;
// use cord_utilities::traits::GenerateBenchmarkOrigin;
// use frame_benchmarking::{account, benchmarks};
// use frame_support::{sp_runtime::traits::Hash, BoundedVec};
// use pallet_chain_space::{
// 	Authorizations, InputRegistryOf, Permissions, RegistryAuthorizationOf,
// RegistryHashOf, };
// use sp_std::{convert::TryFrom, vec::Vec};

// const SEED: u32 = 0;
// const MAX_PAYLOAD_BYTE_LENGTH: u32 = 15 * 1024;

// pub fn generate_registry_id<T: Config>(other_digest: &RegistryHashOf<T>) ->
// RegistryIdOf { 	Ss58Identifier::to_registry_id(&(other_digest).encode()[..]).
// unwrap() }

// pub fn generate_rating_id<T: Config>(other_digest: &RatingEntryHashOf<T>) ->
// RatingIdOf { 	Ss58Identifier::to_scoring_id(&(other_digest).encode()[..]).
// unwrap() }
// fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
// 	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
// }

// benchmarks! {
// 	where_clause {
// 		where
// 		<T as pallet::Config>::EnsureOrigin:
// GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId,
// T::RegistryCreatorId>, 	}
// 	add_rating {
// 		let l in 1 .. MAX_PAYLOAD_BYTE_LENGTH;

// 		let caller: T::AccountId = account("caller", 0, SEED);
// 		let did: T::RegistryCreatorId = account("did", 0, SEED);
// 		let did1: T::RegistryCreatorId = account("did1", 0, SEED);

// 		let rating: Vec<u8> = [2u8; 256].to_vec();
// 		let rating_digest = <T as frame_system::Config>::Hashing::hash(&rating[..]);

// 		//EntityIdentifierOf
// 		let e_id: EntityIdentifierOf<T> = caller.clone();
// 		//TransactionIdentfierOf
// 		let raw_transaction_id = [12u8; 72].to_vec();
// 		let t_id: TransactionIdentifierOf<T> =
// BoundedVec::try_from(raw_transaction_id).unwrap(); 		//CollectorIdentifierOf
// 		let c_id: CollectorIdentifierOf<T> = caller.clone();
// 		//ScoreTypeOf
// 		let rating_type: RatingTypeOf = RatingTypeOf::Overall;
// 		//Entity Rating
// 		let rating: RatingOf = 4;
// 		//count
// 		let count : CountOf = 4;
// 		//entry type
// 		let entry_type: RatingEntryType = RatingEntryType::Credit;

// 		let journal_details = RatingEntryDetails {
// 		entity: e_id,
// 		tid: t_id,
// 		collector: c_id,
// 		rating_type,
// 		rating,
// 		entry_type,
// 		count,
// 		};

// 	let journal_details_digest = <T as frame_system::Config>::Hashing::hash(
// 		&[&journal_details.encode()[..]].concat()[..],
// 	);

// 	let raw_registry = [56u8; 256].to_vec();
// 	let registry: InputRegistryOf<T> =
// BoundedVec::try_from(raw_registry).unwrap(); 	let id_digest = <T as
// frame_system::Config>::Hashing::hash( 		&[&registry.encode()[..],
// &did.encode()[..]].concat()[..], 	);
// 	let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

// 	let journal_entry = RatingEntry {
// 		entry: journal_details.clone(),
// 		digest: journal_details_digest,
// 		created_at: 1,
// 		registry: registry_id.clone(),
// 		creator: did.clone(),
// 	};

// 	let journal_entry_digest =
// 		<T as frame_system::Config>::Hashing::hash(&
// 			[&journal_entry.digest.encode()[..]].concat()[..]);

// 	let identifier =
// Ss58Identifier::to_scoring_id(&(journal_entry_digest.clone()).encode()[..]).
// unwrap();

// 	let journal_input = RatingInput {
// 		entry: journal_details,
// 		digest: journal_entry_digest,
// 		creator: did,
// 	};

// 	let auth_digest = <T as frame_system::Config>::Hashing::hash(
// 		&[&registry_id.encode()[..], &did1.encode()[..],
// &caller.encode()[..]].concat()[..], 	);
// 	let authorization_id: AuthorizationIdOf =
// 		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

// 		<Authorizations<T>>::insert(
// 			&authorization_id,
// 			RegistryAuthorizationOf::<T> {
// 				registry_id,
// 				delegate: did1.clone(),
// 				schema: None,
// 				permissions: Permissions::all(),
// 			},
// 		);

// 		let origin =  <T as pallet::Config>::EnsureOrigin::generate_origin(caller,
// did1.clone()); 	}: _<T::RuntimeOrigin>(origin, journal_input
// ,authorization_id) 	verify {
// 		assert_last_event::<T>(Event::JournalEntry { identifier,entity:
// journal_entry.entry.entity.clone(),author: did1}.into()); 	}
// 	impl_benchmark_test_suite! (
// 		Pallet,
// 		crate::mock::new_test_ext(),
// 		crate::mock::Test
// 	)
// }
