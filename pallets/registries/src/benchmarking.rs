#![cfg(feature = "runtime-benchmarks")]

use super::*;
use codec::Encode;
use frame_benchmarking::{account, benchmarks};
use frame_support::sp_runtime::traits::Hash;
use frame_system::RawOrigin;
use identifier::{IdentifierType, Ss58Identifier};
use pallet_namespace::{NameSpaceCodeOf, NameSpaceIdOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf};
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
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Schema)
		.unwrap()
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

const SEED: u32 = 0;

benchmarks! {
		where_clause {
			where
				T: pallet_namespace::Config,
				T: pallet_schema::Config,
				T: frame_system::Config,
		}


		add_delegate {
			let creator: T::AccountId = account("creator", 0, SEED);
			let delegate: T::AccountId = account("delegate", 0, SEED);

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

			let delegate_auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let delegate_authorization_id: RegistryAuthorizationIdOf = generate_authorization_id::<T>(&delegate_auth_id_digest);

			let raw_schema = [2u8; 256].to_vec();
			let schema: InputSchemaOf<T> = BoundedVec::try_from(raw_schema)
				.expect("Test schema should fit into the expected input length for the test runtime.");
			let schema_id_digest = <T as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
			let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
			RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Authorization {
					registry_id: registry_id,
					authorization: delegate_authorization_id,
					delegate: delegate,
				}
				.into()
			);
		}


		add_admin_delegate {
			let creator: T::AccountId = account("creator", 0, SEED);
			let delegate: T::AccountId = account("delegate", 0, SEED);

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

			let delegate_auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let delegate_authorization_id: RegistryAuthorizationIdOf = generate_authorization_id::<T>(&delegate_auth_id_digest);

			let raw_schema = [2u8; 256].to_vec();
			let schema: InputSchemaOf<T> = BoundedVec::try_from(raw_schema)
				.expect("Test schema should fit into the expected input length for the test runtime.");
			let schema_id_digest = <T as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
			let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

			let origin = RawOrigin::Signed(creator.clone()).into();

		}: _<T::RuntimeOrigin>(
			origin,
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Authorization {
					registry_id: registry_id,
					authorization: delegate_authorization_id,
					delegate: delegate,
				}
				.into()
			);
		}


		add_delegator {
			let creator: T::AccountId = account("creator", 0, SEED);
			let delegate: T::AccountId = account("delegate", 0, SEED);

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

			let delegate_auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let delegate_authorization_id: RegistryAuthorizationIdOf = generate_authorization_id::<T>(&delegate_auth_id_digest);

			let raw_schema = [2u8; 256].to_vec();
			let schema: InputSchemaOf<T> = BoundedVec::try_from(raw_schema)
				.expect("Test schema should fit into the expected input length for the test runtime.");
			let schema_id_digest = <T as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
			let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
			RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Authorization {
					registry_id: registry_id,
					authorization: delegate_authorization_id,
					delegate: delegate,
				}
				.into()
			);
		}


		remove_delegate {
			let creator: T::AccountId = account("creator", 0, SEED);
			let delegate: T::AccountId = account("delegate", 0, SEED);

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

			let delegate_auth_id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let delegate_authorization_id: RegistryAuthorizationIdOf = generate_authorization_id::<T>(&delegate_auth_id_digest);

			let raw_schema = [2u8; 256].to_vec();
			let schema: InputSchemaOf<T> = BoundedVec::try_from(raw_schema)
				.expect("Test schema should fit into the expected input length for the test runtime.");
			let schema_id_digest = <T as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
			let schema_id: SchemaIdOf = generate_schema_id::<T>(&schema_id_digest);

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

			Pallet::<T>::add_delegate(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
			)?;

		}: _<T::RuntimeOrigin>(
			RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate_authorization_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Deauthorization {
					registry_id: registry_id,
					authorization: delegate_authorization_id
				}
				.into()
			);
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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob)
		)
		verify {
			assert_last_event::<T>(
				Event::Create {
					registry_id: registry_id,
					creator: creator,
					authorization: authorization_id,
				}
				.into(),
			);
		}


		update {
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

			let new_digest =
				<T as frame_system::Config>::Hashing::hash(&[3u8; 256].to_vec().encode()[..]);

			let new_raw_blob = [4u8; 256].to_vec();
			let new_blob: RegistryBlobOf<T> = BoundedVec::try_from(new_raw_blob.clone())
				.expect("New Test Blob should fit into the expected input length of for the test runtime.");

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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				new_digest,
				Some(new_blob.clone()),
				namespace_authorization_id.clone(),
				authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Update {
					registry_id: registry_id,
					updater: creator,
					authorization: authorization_id
				}
				.into(),
			);
		}


		revoke {
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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Revoke {
					registry_id: registry_id,
					authority: creator,
				}
				.into(),
			);
		}


		reinstate {
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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

			Pallet::<T>::revoke(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Reinstate {
					registry_id: registry_id,
					authority: creator,
				}
				.into(),
			);
		}


		archive {
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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Archive {
					registry_id: registry_id,
					authority: creator,
				}
				.into(),
			);
		}


		restore {
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

			pallet_namespace::Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				namespace_digest,
				None,
			)?;

			Pallet::<T>::create(
				RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				Some(schema_id.clone()),
				Some(blob),
			)?;

			Pallet::<T>::archive(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			)?;

		}: _<T::RuntimeOrigin>(
				RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone()
		)
		verify {
			assert_last_event::<T>(
				Event::Restore {
					registry_id: registry_id,
					authority: creator,
				}
				.into(),
			);
		}

		impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
