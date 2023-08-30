use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf};
use sp_runtime::{traits::Hash, AccountId32};

/// Generates a stream ID from a stream digest.
///
/// This function takes a stream digest and converts it into a stream ID using
/// the SS58 encoding scheme. The resulting stream ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the stream digest.
///
/// # Returns
///
/// A `StreamIdOf` representing the generated stream ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to stream ID fails.
pub fn generate_stream_id<T: Config>(digest: &StreamHashOf<T>) -> StreamIdOf {
	Ss58Identifier::to_stream_id(&(digest).encode()[..]).unwrap()
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
fn create_stream_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);

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

		assert_ok!(Stream::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			stream_digest,
			authorization_id,
			Some(schema_id)
		));
	});
}

#[test]
fn create_stream_should_fail_if_stream_is_anchored() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);

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

		assert_ok!(Stream::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			stream_digest,
			authorization_id.clone(),
			Some(schema_id.clone())
		));

		assert_err!(
			Stream::create(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				stream_digest,
				authorization_id,
				Some(schema_id)
			),
			Error::<Test>::StreamAlreadyAnchored
		);
	});
}

#[test]
fn update_stream_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);

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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: false,
			},
		);

		let stream_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&stream_update[..]);

		assert_ok!(Stream::update(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			stream_id,
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

	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);

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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: false,
			},
		);
		assert_err!(
			Stream::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				stream_id,
				stream_digest,
				authorization_id,
			),
			Error::<Test>::StreamAlreadyAnchored
		);
	});
}

#[test]
fn update_stream_should_fail_if_stream_does_not_found() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);
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
		let stream_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&stream_update[..]);

		assert_err!(
			Stream::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				stream_id,
				update_digest,
				authorization_id,
			),
			Error::<Test>::StreamNotFound
		);
	});
}

#[test]
fn update_should_fail_if_stream_is_revoked() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);
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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: true,
			},
		);

		let stream_update = vec![12u8; 32];
		let update_digest = <Test as frame_system::Config>::Hashing::hash(&stream_update[..]);

		assert_err!(
			Stream::update(
				DoubleOrigin(author.clone(), delegate.clone()).into(),
				stream_id,
				update_digest,
				authorization_id,
			),
			Error::<Test>::RevokedStream
		);
	});
}

#[test]
fn revoke_stream_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);
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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: false,
			},
		);
		assert_ok!(Stream::revoke(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			stream_id,
			authorization_id,
		));
	});
}

#[test]
fn remove_stream_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);
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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: false,
			},
		);
		assert_ok!(Stream::remove(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			stream_id,
			authorization_id,
		));
	});
}

#[test]
fn digest_stream_should_succed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	let stream = vec![77u8; 32];
	let stream_digest = <Test as frame_system::Config>::Hashing::hash(&stream[..]);
	let stream_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let stream_id = generate_stream_id::<Test>(&stream_id_digest);
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

		<Streams<Test>>::insert(
			&stream_id,
			StreamEntryOf::<Test> {
				digest: stream_digest,
				creator: creator.clone(),
				schema: Some(schema_id.clone()),
				registry: registry_id.clone(),
				revoked: false,
			},
		);
		assert_ok!(Stream::digest(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			stream_id,
			stream_digest,
			authorization_id,
		));
	});
}
