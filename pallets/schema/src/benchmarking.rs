use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::{Get};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	vec::Vec,
};

use cord_utilities::traits::GenerateBenchmarkOrigin;

use crate::*;
use crate::{mock::*, *};
use crate::tests::*;

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
		let schema_id: IdentifierOf = generate_schema_id::<Test>(&schema_hash);

		let origin = T::EnsureOrigin::generate_origin(caller, did.clone());

	}: _<T::Origin>(origin, schema)
	verify {
		let stored_schema_creator: T::SchemaCreatorId = Schemas::<T>::get(&schema_id).expect("Schema Identifier should be present on chain.");
		let stored_schema_hash: SchemaHashOf<T> = stored_schema_creator.digest;
		// Verify the Schema has the right owner
		assert_eq!(stored_schema_creator.creator, did);
		// Verify the Schema hash is mapped to an identifier
		assert_eq!(stored_schema_hash, schema_hash);
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::runtime::ExtBuilder::default().build_with_keystore(),
	crate::mock::runtime::Test
}
