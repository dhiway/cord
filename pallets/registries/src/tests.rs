use super::*;
use crate::mock::*;
use codec::Encode;
use frame_support::{assert_err, assert_ok};
use pallet_namespace::{NameSpaceCodeOf, NameSpaceIdOf};
use pallet_schema_accounts::{InputSchemaOf, SchemaHashOf};
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

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
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::SchemaAccounts)
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

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);
pub(crate) const ACCOUNT_02: AccountId = AccountId::new([3u8; 32]);

#[test]
fn add_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn add_admin_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id,
			delegate,
			namespace_authorization_id.clone(),
			authorization_id,
		));
	});
}

#[test]
fn add_admin_delegate_should_fail_if_admin_delegate_already_exists() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_admin_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn add_delegator_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegator(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id,
			delegate,
			namespace_authorization_id.clone(),
			authorization_id,
		));
	});
}

#[test]
fn add_delegator_should_fail_if_delegator_already_exists() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegator(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegator(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn add_delegate_should_fail_if_registries_is_not_created() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_registries_is_not_created() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_admin_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_delegator_should_fail_if_registries_is_not_created() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_delegator(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_delegate_should_fail_if_the_regisrty_is_revoked() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id.clone(),
				authorization_id,
			),
			Error::<Test>::RegistryRevoked
		);
	});
}

#[test]
fn add_delegate_should_fail_if_a_non_delegate_tries_to_add() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let creator1 = ACCOUNT_02;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let namespace_auth_id_digest_2 = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator1.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id_2: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest_2);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(NameSpace::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_id,
			creator1.clone(),
			namespace_authorization_id.clone(),
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator1.clone()).into(),
				registry_id,
				delegate,
				namespace_authorization_id_2.clone(),
				authorization_id,
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn add_delegate_should_fail_if_delegate_already_exists() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn creating_a_new_registries_should_succeed() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob),
		));
	});
}

#[test]
fn creating_a_duplicate_registries_should_fail() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::create(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_digest,
				namespace_authorization_id.clone(),
				None,
				Some(blob),
			),
			Error::<Test>::RegistryAlreadyAnchored
		);
	});
}

#[test]
fn revoking_a_registry_should_succeed() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn reinstating_an_revoked_a_registry_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::reinstate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn reinstating_an_non_revoked_a_registry_should_fail() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::reinstate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryNotRevoked
		);
	});
}

#[test]
fn archiving_a_registry_should_succeed() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::archive(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn restoring_an_archived_a_registry_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::archive(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::restore(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn restoring_an_non_archived_a_registry_should_fail() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::restore(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryNotArchived
		);
	});
}

#[test]
fn registry_delegation_should_fail_if_registry_delegates_limit_exceeded() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		// Create the Registries
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		// Add the maximum number of delegates to the Registries
		for delegate_count in 2..6 {
			assert_ok!(Registries::registry_delegate_addition(
				registry_id.clone(),
				AccountId::new([delegate_count; 32]),
				creator.clone(),
				Permissions::all(),
			));
		}

		// Attempt to add one more delegate, which should exceed the limit and result in the
		// expected error
		assert_err!(
			Registries::registry_delegate_addition(
				registry_id.clone(),
				AccountId::new([6u8; 32]),
				creator.clone(),
				Permissions::all(),
			),
			Error::<Test>::RegistryDelegatesLimitExceeded
		);
	});
}

#[test]
fn remove_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let delegate_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let delegate_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&delegate_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::remove_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate_authorization_id,
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn remove_delegate_should_fail_for_creator_removing_themselves() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::remove_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				authorization_id.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_registry_should_succeed() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();
	let new_digest =
		<Test as frame_system::Config>::Hashing::hash(&[3u8; 256].to_vec().encode()[..]);

	let raw_blob = [2u8; 256].to_vec();
	let initial_blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob.clone())
		.expect("Test Blob should fit into the expected input length of for the test runtime.");

	let new_raw_blob = [4u8; 256].to_vec();
	let new_blob: RegistryBlobOf<Test> = BoundedVec::try_from(new_raw_blob.clone())
		.expect("New Test Blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(initial_blob),
		));

		assert_ok!(Registries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_digest,
			Some(new_blob.clone()),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		let updated_registry =
			RegistryInfo::<Test>::get(&registry_id).expect("Registry should exist");
		assert_eq!(updated_registry.digest, new_digest);

		System::assert_last_event(
			Event::Update {
				registry_id: registry_id.clone(),
				updater: creator,
				authorization: authorization_id,
			}
			.into(),
		);
	});
}

#[test]
fn add_delegate_should_fail_if_registry_delegates_limit_exceeded() {
	let creator = ACCOUNT_00;
	let delegate_1 = ACCOUNT_01;
	let delegate_2 = ACCOUNT_02;
	let delegate_3: AccountId = AccountId::new([4u8; 32]);
	let delegate_4: AccountId = AccountId::new([5u8; 32]);
	let delegate_5: AccountId = AccountId::new([6u8; 32]);

	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		// Create the Registries
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		// Add maximum delegates allowed in a registry
		let delegates = vec![delegate_1, delegate_2, delegate_3, delegate_4];
		for delegate in delegates {
			assert_ok!(Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			));
		}

		// Attempt to add one more delegate, which should exceed the limit and result in the
		// expected error
		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate_5.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryDelegatesLimitExceeded
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_registry_delegates_limit_exceeded() {
	let creator = ACCOUNT_00;
	let delegate_1 = ACCOUNT_01;
	let delegate_2 = ACCOUNT_02;
	let delegate_3: AccountId = AccountId::new([4u8; 32]);
	let delegate_4: AccountId = AccountId::new([5u8; 32]);
	let delegate_5: AccountId = AccountId::new([6u8; 32]);

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		// Create the Registries
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		// Add maximum delegates allowed in a registry
		let delegates = vec![delegate_1, delegate_2, delegate_3, delegate_4];
		for delegate in delegates {
			assert_ok!(Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			));
		}

		// Attempt to add an admin delegate, which should exceed the limit and result in the
		// expected error
		assert_err!(
			Registries::add_admin_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate_5.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryDelegatesLimitExceeded
		);
	});
}

#[test]
fn add_delegator_should_fail_if_registry_delegates_limit_exceeded() {
	let creator = ACCOUNT_00;
	let delegate_1 = ACCOUNT_01;
	let delegate_2 = ACCOUNT_02;
	let delegate_3: AccountId = AccountId::new([4u8; 32]);
	let delegate_4: AccountId = AccountId::new([5u8; 32]);
	let delegate_5: AccountId = AccountId::new([6u8; 32]);

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		// Create the Registries
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		// Add maximum delegates allowed in a registry
		let delegates = vec![delegate_1, delegate_2, delegate_3, delegate_4];
		for delegate in delegates {
			assert_ok!(Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			));
		}

		// Attempt to add a delegator, which should exceed the limit and result in the
		// expected error
		assert_err!(
			Registries::add_delegator(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate_5.clone(),
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryDelegatesLimitExceeded
		);
	});
}

#[test]
fn registry_id_should_be_updated_on_namespace_chainstorage_on_create() {
	let creator = ACCOUNT_00;

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let registry = [2u8; 256].to_vec();

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let registry_2 = [3u8; 256].to_vec();

	let raw_blob_2 = [3u8; 256].to_vec();
	let blob_2: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob_2)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let registry_digest_2 = <Test as frame_system::Config>::Hashing::hash(&registry_2.encode()[..]);

	let id_digest_2 = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest_2.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id_2: RegistryIdOf = generate_registry_id::<Test>(&id_digest_2);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		// Create Registry 1
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id.clone()),
			Some(blob)
		));

		// Create Registry 2
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest_2,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob_2)
		));

		// Verify if the newly created registry-id is added as a list in the Namespace Chain
		// Storage.
		let name_space_details = pallet_namespace::NameSpaces::<Test>::get(namespace_id.clone())
			.ok_or(pallet_namespace::pallet::Error::<Test>::NameSpaceNotFound)
			.unwrap();
		assert!(
			name_space_details
				.registry_ids
				.clone()
				.unwrap_or_default()
				.contains(&registry_id),
			"Registry ID 1 not found in the Namespace Chain Storage."
		);
		assert!(
			name_space_details.registry_ids.unwrap_or_default().contains(&registry_id_2),
			"Registry ID 2 not found in the Namespace Chain Storage."
		);

		// Verify if the newly created registry-id 1 is present in the Registry Chain Storage.
		let registry_info = RegistryInfo::<Test>::get(&registry_id)
			.ok_or(Error::<Test>::RegistryNotFound)
			.unwrap();
		assert_eq!(
			registry_info.namespace_id, namespace_id,
			"Namespace ID not found in the Registry 1 Chain Storage."
		);

		// Verify if the newly created registry-id 2 is present in the Registry Chain Storage.
		let registry_info = RegistryInfo::<Test>::get(&registry_id_2)
			.ok_or(Error::<Test>::RegistryNotFound)
			.unwrap();
		assert_eq!(
			registry_info.namespace_id, namespace_id,
			"Namespace ID not found in the Registry 2 Chain Storage."
		);
	});
}

#[test]
fn remove_delegate_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let delegate_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let delegate_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&delegate_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::remove_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate_authorization_id,
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn remove_delegate_should_fail_if_remove_authorization_is_not_found_as_delegate_not_added() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let delegate_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let delegate_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&delegate_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::remove_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate_authorization_id,
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn remove_delegate_should_fail_if_remove_authorization_is_not_found_as_non_existant_auth_id_entered(
) {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::remove_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				non_existent_authorization_id,
				namespace_authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn revoke_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::revoke(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn reinstate_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::reinstate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn update_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let new_digest =
		<Test as frame_system::Config>::Hashing::hash(&[3u8; 256].to_vec().encode()[..]);

	let raw_blob = [2u8; 256].to_vec();
	let initial_blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob.clone())
		.expect("Test Blob should fit into the expected input length for the test runtime.");

	let new_raw_blob = [4u8; 256].to_vec();
	let new_blob: RegistryBlobOf<Test> = BoundedVec::try_from(new_raw_blob.clone())
		.expect("New Test Blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(initial_blob),
		));

		assert_err!(
			Registries::update(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				new_digest,
				Some(new_blob.clone()),
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn archive_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::archive(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn restore_should_fail_if_admin_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let registry = [2u8; 256].to_vec();

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let namespace_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_authorization_id: NamespaceAuthorizationIdOf =
		generate_namespace_authorization_id::<Test>(&namespace_auth_id_digest);

	let raw_blob = [2u8; 256].to_vec();
	let blob: RegistryBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length for the test runtime.");

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&auth_id_digest);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			None,
		));

		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_digest,
			namespace_authorization_id.clone(),
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::archive(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			namespace_authorization_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::restore(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				namespace_authorization_id.clone(),
				non_existent_authorization_id.clone(),
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_origin(&non_existent_authorization_id, &delegate),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_reinstate_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_reinstate_origin(
				&non_existent_authorization_id,
				&delegate
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_restore_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_restore_origin(
				&non_existent_authorization_id,
				&delegate
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_admin_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_admin_origin(
				&non_existent_authorization_id,
				&delegate
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_delegator_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_delegator_origin(
				&non_existent_authorization_id,
				&delegate
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn ensure_authorization_admin_remove_origin_should_fail_if_authorization_is_not_found() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let non_existent_auth_id_digest =
		<Test as frame_system::Config>::Hashing::hash(&registry_id.encode()[..]);

	let non_existent_authorization_id: RegistryAuthorizationIdOf =
		generate_authorization_id::<Test>(&non_existent_auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Registries::ensure_authorization_admin_remove_origin(
				&non_existent_authorization_id,
				&delegate
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}
