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


use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Get;
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	vec::Vec,
};

use cord_utilities::traits::GenerateBenchmarkOrigin;
use crate::{cord_utilities::mock::*,tests::*, *};

const SEED: u32 = 0;
const MAX_SCHEMA_SIZE: u32 = 5 * 1024 * 1024;

benchmarks! {
	where_clause {
		where
		T::EnsureOrigin: GenerateBenchmarkOrigin<T::Origin, T::AccountId, T::SchemaCreatorId>,
	}

	create {
		let l in 1 .. MAX_SCHEMA_SIZE;

		let caller = account("caller", 0, SEED);
		let did: T::SchemaCreatorId = account("did", 0, SEED);

		let schema: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let schema_hash = <T as frame_system::Config>::Hashing::hash(&schema[..]);
		let id_digest = <Test as frame_system::Config>::Hashing::hash(
			&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
		);
		let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

		let origin = T::EnsureOrigin::generate_origin(caller, did.clone());

	}: _<T::Origin>(origin, schema)
	verify {
		let stored_schema_creator: T::SchemaCreatorId = Schemas::<T>::get(&schema_id).expect("Schema Identifier should be present on chain.");
		// Verify the Schema has the right owner
		assert_eq!(stored_schema_creator.creator, did);
		// Verify the Schema hash is mapped to an identifier
		assert_eq!(stored_schema_creator.digest, schema_hash);
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::new_test_ext(),
    crate::mock::Test,
}