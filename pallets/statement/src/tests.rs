use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf};
use sp_runtime::{traits::Hash, AccountId32};

/// Generates a statement ID from a statement digest.
///
/// This function takes a statement digest and converts it into a statement ID
/// using the SS58 encoding scheme. The resulting statement ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the statement digest.
///
/// # Returns
///
/// A `StatementIdOf` representing the generated statement ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to statement ID
/// fails.
pub fn generate_statement_id<T: Config>(digest: &StatementDigestOf<T>) -> StatementIdOf {
	Ss58Identifier::to_statement_id(&(digest).encode()[..]).unwrap()
}

/// Generates a schema ID from a schema digest.
///
/// This function takes a schema digest and converts it into a schema ID using
/// the SS58 encoding scheme. The resulting schema ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the schema digest.
///
/// # Returns
///
/// A `SchemaIdOf` representing the generated schema ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to schema ID fails.
pub fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
	Ss58Identifier::to_schema_id(&(digest).encode()[..]).unwrap()
}

/// Generates a registry ID from a registry digest.
///
/// This function takes a registry digest and converts it into a registry ID
/// using the SS58 encoding scheme. The resulting registry ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the registry digest.
///
/// # Returns
///
/// A `RegistryIdOf` representing the generated registry ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to registry ID fails.
pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(digest).encode()[..]).unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([5u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn create_statement_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id,
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id,
			Some(schema_id)
		));
	});
}

#[test]
fn create_statement_batch_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id,
			delegate.clone(),
		));

		assert_ok!(Statement::create_batch(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			vec![statement_digest],
			authorization_id,
			Some(schema_id)
		));
	});
}

#[test]
fn create_statement_should_fail_if_statement_is_anchored() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_err!(
			Statement::create(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				statement_digest,
				authorization_id,
				Some(schema_id)
			),
			Error::<Test>::StatementAlreadyAnchored
		);
	});
}

#[test]
fn update_statement_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		let statement_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&statement_update[..]);

		assert_ok!(Statement::update(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id,
			update_digest,
			authorization_id,
		));
	});
}

#[test]
fn update_should_fail_if_digest_is_same() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_err!(
			Statement::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				statement_id,
				statement_digest,
				authorization_id,
			),
			Error::<Test>::StatementAlreadyAnchored
		);
	});
}

#[test]
fn update_statement_should_fail_if_statement_does_not_found() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);
	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));
		let statement_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&statement_update[..]);

		assert_err!(
			Statement::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				statement_id,
				update_digest,
				authorization_id,
			),
			Error::<Test>::StatementNotFound
		);
	});
}

#[test]
fn update_should_fail_if_statement_is_revoked() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);
	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Statement::revoke(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id.clone(),
			authorization_id.clone(),
		));

		let statement_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&statement_update[..]);

		assert_err!(
			Statement::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				statement_id,
				update_digest,
				authorization_id,
			),
			Error::<Test>::StatementRevoked
		);
	});
}

#[test]
fn revoke_statement_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);
	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Statement::revoke(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id,
			authorization_id,
		));
	});
}

#[test]
fn restore_statement_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);
	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Statement::revoke(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Statement::restore(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id,
			authorization_id,
		));
	});
}

#[test]
fn remove_statement_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let statement = vec![77u8; 32];
	let statement_digest = <Test as frame_system::Config>::Hashing::hash(&statement[..]);
	let statement_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&statement_digest.encode()[..], &registry_id.encode()[..], &delegate.encode()[..]]
			.concat()[..],
	);
	let statement_id = generate_statement_id::<Test>(&statement_id_digest);
	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
	new_test_ext().execute_with(|| {
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id.clone())
		));
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Statement::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_ok!(Statement::remove(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			statement_id,
			authorization_id,
		));
	});
}
