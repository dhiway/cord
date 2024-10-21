use super::*;
use crate::mock::*;
use codec::Encode;
use frame_support::{assert_err, assert_ok};
use pallet_schema_accounts::{InputSchemaOf, SchemaHashOf};
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Registries).unwrap()
}

pub fn generate_authorization_id<T: Config>(digest: &RegistryHashOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::RegistryAuthorization)
		.unwrap()
}

pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::SchemaAccounts)
		.unwrap()
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);
pub(crate) const ACCOUNT_02: AccountId = AccountId::new([3u8; 32]);

#[test]
fn add_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn add_admin_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id,
			delegate,
			authorization_id,
		));
	});
}

#[test]
fn add_admin_delegate_should_fail_if_admin_delegate_already_exists() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registries::add_admin_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_admin_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegator(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id,
			delegate,
			authorization_id,
		));
	});
}

#[test]
fn add_delegator_should_fail_if_delegator_already_exists() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegator(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegator(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_admin_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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
	let registry = [2u8; 256].to_vec();

	let registry_digest = <Test as frame_system::Config>::Hashing::hash(&registry.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Registries::add_delegator(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id,
				delegate,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator1.clone()).into(),
				registry_id,
				delegate,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob)
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::add_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				delegate.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn creating_a_new_registries_should_succeed() {
	let creator = ACCOUNT_00;
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
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob),
		));
	});
}

#[test]
fn creating_a_duplicate_registries_should_fail() {
	let creator = ACCOUNT_00;
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
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::create(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				registry_digest,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn reinstating_an_revoked_a_registry_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::revoke(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::reinstate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn reinstating_an_non_revoked_a_registry_should_fail() {
	let creator = ACCOUNT_00;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::reinstate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryNotRevoked
		);
	});
}

#[test]
fn archiving_a_registry_should_succeed() {
	let creator = ACCOUNT_00;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::archive(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn restoring_an_archived_a_registry_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::archive(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::restore(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));
	});
}

#[test]
fn restoring_an_non_archived_a_registry_should_fail() {
	let creator = ACCOUNT_00;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_err!(
			Registries::restore(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RegistryNotArchived
		);
	});
}

#[test]
fn add_delegate_should_fail_if_registry_delegates_limit_exceeded() {
	let creator = ACCOUNT_00;
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
		// Create the Registries
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let delegate_auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let delegate_authorization_id: AuthorizationIdOf =
		generate_authorization_id::<Test>(&delegate_auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Registries::remove_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate_authorization_id,
			authorization_id.clone(),
		));
	});
}

#[test]
fn remove_delegate_should_fail_for_creator_removing_themselves() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(blob.clone()),
		));

		assert_ok!(Registries::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Registries::remove_delegate(
				frame_system::RawOrigin::Signed(creator.clone()).into(),
				registry_id.clone(),
				authorization_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn update_registry_should_succeed() {
	let creator = ACCOUNT_00;
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

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let _digest: SchemaHashOf<Test> = <Test as frame_system::Config>::Hashing::hash(&schema[..]);
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(&schema.encode()[..]);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Registries::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			registry_digest,
			Some(schema_id),
			Some(initial_blob),
		));

		assert_ok!(Registries::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			registry_id.clone(),
			new_digest,
			Some(new_blob.clone()),
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
