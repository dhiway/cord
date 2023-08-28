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
use frame_benchmarking::v1::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{sp_runtime::traits::Hash, traits::Get, BoundedVec};
use sp_std::{
	convert::{TryFrom, TryInto},
	vec::Vec,
};

use cord_utilities::traits::GenerateBenchmarkOrigin;

const SEED: u32 = 0;

benchmarks! {
	where_clause {
		where
		T::EnsureOrigin: GenerateBenchmarkOrigin<T::RuntimeOrigin, T::AccountId, T::SchemaCreatorId>,
	}
	create {
		let l in 1 .. T::MaxEncodedSchemaLength::get();

		let caller = account("caller", 0, SEED);
		let did: T::SchemaCreatorId = account("did", 0, SEED);

		let raw_schema: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let schema = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of the test runtime.");
		let digest = <T as frame_system::Config>::Hashing::hash(&schema[..]);
		let id_digest = <T as frame_system::Config>::Hashing::hash(
			&[&schema.encode()[..], &did.encode()[..]].concat()[..],
		);
		let schema_id = Ss58Identifier::to_schema_id(&(id_digest).encode()[..]).unwrap();

		let origin = T::EnsureOrigin::generate_origin(caller, did.clone());
	}: _<T::RuntimeOrigin>(origin, schema)
	verify {
		let stored_schema = Schemas::<T>::get(&schema_id).expect("Schema Identifier should be present on chain.");
		// Verify the Schema has the right owner
		assert_eq!(stored_schema.creator, did);
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::new_test_ext(),
	crate::mock::Test
}
