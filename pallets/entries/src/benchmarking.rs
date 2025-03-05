#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;
use identifier::{IdentifierType, Ss58Identifier};
use pallet_namespace::{NameSpaceCodeOf, NameSpaceIdOf};
use pallet_registries::{RegistryBlobOf, RegistryHashOf, RegistryIdOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf, SchemaIdOf};
use serde_json::json;
use sp_std::prelude::*;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Registries).unwrap()
}

pub fn generate_authorization_id<T: Config>(
	digest: &RegistryHashOf<T>,
) -> RegistryAuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::RegistryAuthorization)
		.unwrap()
}

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Schema).unwrap()
}

pub fn generate_namespace_id<T: Config>(digest: &NameSpaceCodeOf<T>) -> NameSpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::NameSpace).unwrap()
}

pub fn generate_namespace_authorization_id<T: Config>(
	digest: &NameSpaceCodeOf<T>,
) -> NamespaceAuthorizationIdOf {
	Ss58Identifier::create_identifier(
		&(digest).encode()[..],
		IdentifierType::NameSpaceAuthorization,
	)
	.unwrap()
}

pub fn generate_registry_entry_id<T: Config>(digest: &RegistryEntryHashOf<T>) -> RegistryEntryIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Entries).unwrap()
}

const SEED: u32 = 0;

benchmarks! {
		where_clause {
			where
				T: pallet_namespace::Config,
				T: pallet_registries::Config,
				T: pallet_schema::Config,
				T: frame_system::Config,
		}


		create {
			let creator: T::AccountId = account("creator", 0, SEED);

			let namespace = [1u8; 256].to_vec();
			let namespace_digest = <T as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
			);
			let namespace_id: NameSpaceIdOf = generate_namespace_id::<T>(&id_digest);

			let namespace_auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
			);
			let namespace_authorization_id: NamespaceAuthorizationIdOf = generate_namespace_authorization_id::<T>(&namespace_auth_id_digest);

			let registry = [2u8; 256].to_vec();

			let raw_blob = [2u8; 256].to_vec();
			let blob: RegistryBlobOf<T> = BoundedVec::try_from(raw_blob)
				.expect("Test blob should fit into the expected input length for the test runtime.");

			let registry_digest = <T as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let registry_id: RegistryIdOf = generate_registry_id::<T>(&id_digest);

			let auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let authorization_id: RegistryAuthorizationIdOf = generate_authorization_id::<T>(&auth_id_digest);

			let raw_schema = [2u8; 256].to_vec();
			let schema: InputSchemaOf<T> = BoundedVec::try_from(raw_schema)
				.expect("Test schema should fit into the expected input length for the test runtime.");
			let schema_id_digest = <T as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
			let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

			let registry_entry_json_object = json!({
				"name": "Alice",
				"age": 25,
				"email": "alice@dhiway.com",
				"isActive": true,
				"address": {
					"street": "Koramangala",
					"city": "Bengaluru",
					"zipcode": "560001"
				},
				"phoneNumbers": [
					"+91-234787324",
					"+91-283746823"
				]
			});

			let registry_entry_json_string =
				serde_json::to_string(&registry_entry_json_object).expect("Failed to serialize JSON");

			let registry_entry_raw_bytes = registry_entry_json_string.as_bytes().to_vec();

			let registry_entry_blob: RegistryEntryBlobOf<T> =
				BoundedVec::try_from(registry_entry_raw_bytes.clone()).expect(
					"Test Blob should fit into the expected input length of BLOB for the test runtime.",
				);

			let registry_entry_digest: RegistryHashOf<T> =
				<T as frame_system::Config>::Hashing::hash(&registry_entry_raw_bytes.encode()[..]);

			let registry_entry_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&registry_entry_digest.encode()[..],
					&registry_id.encode()[..],
					&creator.encode()[..],
				]
				.concat()[..],
			);

			let registry_entry_id: RegistryEntryIdOf =
				generate_registry_entry_id::<T>(&registry_entry_id_digest);

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None
			)?;

			pallet_registries::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
			RawOrigin::Signed(creator.clone()).into(),
			registry_entry_id.clone(),
			authorization_id.clone(),
			registry_entry_digest,
			None
		)

		verify {
			assert_last_event::<T>(
				Event::RegistryEntryCreated {
					creator: creator,
					registry_id: registry_id,
					registry_entry_id: registry_entry_id,
				}
				.into()
			);
		}

		impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
