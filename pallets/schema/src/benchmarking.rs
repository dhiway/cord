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
use frame_support::{sp_runtime::traits::Hash, traits::Get, BoundedVec};
use frame_system::RawOrigin;
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Schema).unwrap()
}

/// Generates a space ID from a digest.
pub fn generate_space_id<T: Config>(digest: &SchemaHashOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

/// Generates an authorization ID from a digest.
pub fn generate_authorization_id<T: Config>(digest: &SchemaHashOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

benchmarks! {
	where_clause {
		where
		<T as Config>::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SchemaCreatorId>,
		// T::ChainSpaceOrigin: EnsureOrigin<T::RuntimeOrigin>,
	}
	create {
		let l in 1 .. T::MaxEncodedSchemaLength::get();

		let caller: T::AccountId = account("caller", 0, SEED);
		let did: T::SchemaCreatorId = account("did", 0, SEED);
		let did1: T::SpaceCreatorId = account("did", 0, SEED);
		let capacity = 3u64;

		let raw_space = [2u8; 256].to_vec();
		let space_digest = <T as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
		let space_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_digest.encode()[..], &did.encode()[..]].concat()[..],
		);
		let space_id: SpaceIdOf = generate_space_id::<T>(&space_id_digest);

		let raw_schema: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let schema = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of the test runtime.");
		let digest = <T as frame_system::Config>::Hashing::hash(&schema[..]);
		let schema_id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&schema.encode()[..], &space_id.encode()[..], &did.encode()[..]].concat()[..],
		);
		let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

		let auth_digest = <T as frame_system::Config>::Hashing::hash(
			&[&space_id.encode()[..], &did.encode()[..]].concat()[..],
		);

		let authorization_id: Ss58Identifier = generate_authorization_id::<T>(&auth_digest);

		let origin =  <T as Config>::EnsureOrigin::generate_origin(caller, did);
		let chain_space_origin = RawOrigin::Root.into();

		pallet_chain_space::Pallet::<T>::create(origin.clone(), space_digest )?;
		pallet_chain_space::Pallet::<T>::approve(chain_space_origin, space_id, capacity ).expect("Approval should not fail.");

	}: _<T::RuntimeOrigin>(origin, schema, authorization_id)
	verify {
		assert_last_event::<T>(Event::<T>::Created { identifier: schema_id, creator: did1 }.into());
	}
	impl_benchmark_test_suite! (
		Pallet,
		crate::mock::new_test_ext(),
		crate::mock::Test
	)
}
