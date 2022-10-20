use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{
	sp_runtime::traits::Hash,
	traits::{Currency, Get},
};
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	vec::Vec,
};

use cord_utilities::traits::GenerateBenchmarkOrigin;

use crate::*;

const SEED: u32 = 0;
const MAX_SCHEMA_SIZE: u32 = 5 * 1024 * 1024;

benchmarks! {
	where_clause {
		where
		<<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance: TryFrom<usize>,
		<<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance as TryFrom<usize>>::Error: Debug,
		T::EnsureOrigin: GenerateBenchmarkOrigin<T::Origin, T::AccountId, T::SchemaCreatorId>,
	}

	create {
		let l in 1 .. MAX_SCHEMA_SIZE;

		let caller = account("caller", 0, SEED);
		let did: T::SchemaCreatorId = account("did", 0, SEED);

		let schema: Vec<u8> = (0u8..u8::MAX).cycle().take(l.try_into().unwrap()).collect();
		let schema_hash = <T as frame_system::Config>::Hashing::hash(&schema[..]);
		let schema_id: IdentifierOf = generate_schema_id::<Test>(&schema_hash);

		let initial_balance = <T as Config>::Fee::get() * schema.len().try_into().unwrap() + <T as Config>::Currency::minimum_balance();
		<T as Config>::Currency::make_free_balance_be(&caller, initial_balance);
		let origin = T::EnsureOrigin::generate_origin(caller, did.clone());

	}: _<T::Origin>(origin, schema)
	verify {
		let stored_schema_creator: T::SchemaCreatorId = Schemas::<T>::get(&schema_id).expect("Schema Identifier should be present on chain.");
		let stored_schema_identifier: T::IdentifierOf = Schema::schema_hashes(&schema_hash)
				.expect("Schema Hash should be present on chain.");
		// Verify the Schema has the right owner
		assert_eq!(stored_schema_creator, did);
		// Verify the Schema hash is mapped to an identifier
		assert_eq!(stored_schema_identifier, schema_id);
	}
}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::runtime::ExtBuilder::default().build_with_keystore(),
	crate::mock::runtime::Test
}
