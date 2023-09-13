#![allow(unused)]

use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf, SchemaIdOf};
use pallet_schema::{InputSchemaOf, SchemaHashOf};
use sp_runtime::{traits::Hash, AccountId32};

/// Generates a unique ID from a unique digest.
///
/// This function takes a unique digest and converts it into a unique ID using
/// the SS58 encoding scheme. The resulting unique ID is returned.
///
/// # Arguments
///
/// * `digest` - A reference to the unique digest.
///
/// # Returns
///
/// A `UniqueIdOf` representing the generated unique ID.
///
/// # Panics
///
/// This function will panic if the conversion from digest to unique ID fails.
pub(crate) fn generate_unique_id<T: Config>(digest: &UniqueHashOf<T>) -> UniqueIdOf {
	Ss58Identifier::to_unique_id(&(digest).encode()[..]).unwrap()
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
pub(crate) fn generate_schema_id<T: Config>(digest: &SchemaHashOf<T>) -> SchemaIdOf {
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
pub(crate) fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(digest).encode()[..]).unwrap()
}

// did, acount,
// origin: subject (did) + creator(accountid) == author

// unq txn Boundedvec
// acl = access control list

// Option<AuthorizationIdOf> = check in registry pallet.
//

/*
Input parameters:
1. create:
```
origin: OriginFor<T>,
unique_txn: InputUniqueOf<T>,
authorization: Option<AuthorizationIdOf>,
```

2. update:
```
origin: OriginFor<T>,
unique_id: UniqueIdOf,
unique_txn: InputUniqueOf<T>,
authorization: Option<AuthorizationIdOf>,
```

3. revoke:
```
origin: OriginFor<T>,
unique_txn: InputUniqueOf<T>,
authorization: AuthorizationIdOf,
```

4. remove:
```
origin: OriginFor<T>,
unique_id: UniqueIdOf,
authorization: Option<AuthorizationIdOf>,
```

---

OriginFor<T> == DoubleOrigin(author.clone(), delegate.clone()).into()

InputUniqueOf<T> ==
let raw_unique = vec![77u8; 32];
let unique: InputUniqueOf<Test> = BoundedVec::try_from(raw_unique).expect("Test Unique should fit into the expected input length of for the test runtime.");

UniqueIdOf ==
let unique_digest = <Test as frame_system::Config>::Hashing::hash(&unique[..]);
let unique_id: UniqueIdOf = generate_unique_id::<Test>(&unique_digest);

AuthorizationIdOf == Not clear
```
/// Authorization Identifier
pub type AuthorizationIdOf = Ss58Identifier;
```

Maybe this, but not working. Getting registry mismatch error:
```
let auth_digest = <Test as frame_system::Config>::Hashing::hash(&[&registry_id.encode()[..], &delegate.encode()[..], &creator.encode()[..]].concat()[..],);

let authorization_id: Ss58Identifier =
Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();
```
*/

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([5u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn create_unique_without_authorization() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_unique = vec![77u8; 32];
	let unique: InputUniqueOf<Test> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of for the test runtime.");
	let unique_digest = <Test as frame_system::Config>::Hashing::hash(&unique[..]);
	let unique_id: UniqueIdOf = generate_unique_id::<Test>(&unique_digest);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let schema_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_digest);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let registry_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_digest);

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

		assert_ok!(Unique::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			unique,
			None
		));
	})
}

// TODO
#[test]
fn create_unique_with_authorization() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_unique = vec![77u8; 32];
	let unique: InputUniqueOf<Test> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of for the test runtime.");
	let unique_digest = <Test as frame_system::Config>::Hashing::hash(&unique[..]);
	let unique_id: UniqueIdOf = generate_unique_id::<Test>(&unique_digest);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let schema_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_digest);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let registry_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_digest);

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

		// 		<Registries<T>>::insert(
		// 	&identifier,
		// 	RegistryEntryOf::<T> {
		// 		details: tx_registry,
		// 		digest,
		// 		schema: tx_schema,
		// 		creator: creator.clone(),
		// 		archive: true,
		// 	},
		// );

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id,
			delegate.clone(),
		));

		System::set_block_number(1);

		// <Authorizations<T>>::insert(
		// 	&authorization_id,
		// 	RegistryAuthorizationOf::<T> {
		// 		registry_id: registry_id.clone(),
		// 		delegate: delegate.clone(),
		// 		schema: registry_details.schema,
		// 		permissions: Permissions::all(),
		// 	},
		// );

		assert_ok!(Unique::create(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			unique,
			Some(authorization_id)
		));
	})
}

#[test]
fn update_unique_without_authorization() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	let raw_unique = vec![77u8; 32];
	let unique: InputUniqueOf<Test> = BoundedVec::try_from(raw_unique)
		.expect("Test Unique should fit into the expected input length of for the test runtime.");
	let unique_digest = <Test as frame_system::Config>::Hashing::hash(&unique[..]);
	let unique_id: UniqueIdOf = generate_unique_id::<Test>(&unique_digest);

	let raw_schema = [11u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");

	let schema_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id: SchemaIdOf = generate_schema_id::<Test>(&schema_digest);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let registry_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&registry_digest);

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

		<UniqueDigestEntries<Test>>::insert(&unique, &unique_id);

		<UniqueIdentifiers<Test>>::insert(
			&unique_id,
			UniqueEntryOf::<Test> {
				digest: unique.clone(),
				creator: creator.clone(),
				registry: Some(Some(registry_id.clone())),
				revoked: false,
			},
		);

		let new_raw_unique = vec![12u8; 32];
		let new_unique: InputUniqueOf<Test> = BoundedVec::try_from(new_raw_unique).expect(
			"Test Unique should fit into the expected input length of for the test runtime.",
		);
		let updated_unique_digest = <Test as frame_system::Config>::Hashing::hash(&new_unique[..]);

		assert_ok!(Unique::update(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			unique_id,
			new_unique,
			None
		));
	})
}
