use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_schema::InputSchemaOf;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(digest).encode()[..]).unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([2u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

//TEST FUNCTION FOR ADD ADMIN DELEGATE

#[test]
fn add_admin_delegate_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//Admin should be able to add the delegate
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id,
			DID_01,
		));
	});
}

#[test]
fn add_admin_delegate_should_fail_if_registry_is_not_created() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Registry::add_admin_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				registry_id.clone(),
				SubjectId(AccountId32::new([1u8; 32])),
			),
			Error::<Test>::RegistryNotFound
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_the_regisrty_is_archived() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		<Registries<Test>>::insert(
			&registry_id,
			RegistryEntryOf::<Test> {
				archive: true,
				..<Registries<Test>>::get(&registry_id).unwrap()
			},
		);

		//Admin should be able to add the delegate
		assert_err!(
			Registry::add_admin_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				registry_id,
				DID_01,
			),
			Error::<Test>::ArchivedRegistry
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_creator_is_not_a_authority() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//Checking whether registry creator and creator are different
		assert_ne!(<Registries<Test>>::get(&registry_id).unwrap().creator, DID_01);

		assert_err!(
			Registry::is_an_authority(&registry_id, DID_01),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_delegate_is_already_added() {
	// env_logger::init();
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	//Schema creation from schema pallet
	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id_of: Ss58Identifier =
		Ss58Identifier::to_schema_id(&schema_id_digest.encode()[..]).unwrap();

	let new_block_number: BlockNumberFor<Test> = 1;

	new_test_ext().execute_with(|| {
		//adding schema
		System::set_block_number(new_block_number);
		//creating regisrty
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id_of)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			DID_00,
		));

		//When Trying to add same delegate again it says delegate already added
		assert_err!(
			Registry::add_admin_delegate(
				DoubleOrigin(author.clone(), DID_00.clone()).into(),
				registry_id.clone(),
				DID_00,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn add_admin_delegate_should_max_authorities() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//Adding the delegate limit that is 3 Max Authorities
		for a in 5..8u8 {
			assert_ok!(Registry::add_admin_delegate(
				DoubleOrigin(author.clone(), DID_00.clone()).into(),
				registry_id.clone(),
				SubjectId(AccountId32::new([a; 32])),
			));
		}
		let did_08 = SubjectId(AccountId32::new([8u8; 32]));

		//Should throw Error When Max authorities reached
		assert_err!(
			Registry::add_admin_delegate(
				DoubleOrigin(author.clone(), DID_00.clone()).into(),
				registry_id.clone(),
				did_08,
			),
			Error::<Test>::RegistryAuthoritiesLimitExceeded
		);
	});
}

#[test]

fn add_admin_delegate_should_update_commit() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		assert_ok!(Registry::update_commit(
			&registry_id,
			id_digest,
			creator.clone(),
			RegistryCommitActionOf::Authorization
		));

		//Check wheter that event has been emitted
		assert_eq!(
			registry_events_since_last_call(),
			vec![Event::Create { registry: registry_id, creator }]
		);
	});
}

//TEST FUNCTIONS FOR ADD_DELEGATE
#[test]
fn add_delegate_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//should be able to add the delegate
		assert_ok!(Registry::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id,
			DID_01,
		));
	});
}

#[test]
fn add_delegate_should_fail_if_registry_is_not_created() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Registry::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				registry_id.clone(),
				SubjectId(AccountId32::new([1u8; 32])),
			),
			Error::<Test>::RegistryNotFound
		);
	});
}

#[test]
fn add_delegate_should_fail_if_the_regisrty_is_archived() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		<Registries<Test>>::insert(
			&registry_id,
			RegistryEntryOf::<Test> {
				archive: true,
				..<Registries<Test>>::get(&registry_id).unwrap()
			},
		);

		//Admin should be able to add the delegate
		assert_err!(
			Registry::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				registry_id,
				DID_01,
			),
			Error::<Test>::ArchivedRegistry
		);
	});
}

#[test]
fn add_delegate_should_fail_if_creator_is_not_a_authority() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//Checking whether registry creator and creator are different
		assert_ne!(<Registries<Test>>::get(&registry_id).unwrap().creator, DID_01);

		assert_err!(
			Registry::is_an_authority(&registry_id, DID_01),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn add_delegate_should_fail_if_delegate_is_already_added() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	//Schema creation from schema pallet
	let raw_schema = [2u8; 256].to_vec();
	let schema: InputSchemaOf<Test> = BoundedVec::try_from(raw_schema)
		.expect("Test Schema should fit into the expected input length of for the test runtime.");
	let schema_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&schema.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let schema_id_of: Ss58Identifier =
		Ss58Identifier::to_schema_id(&schema_id_digest.encode()[..]).unwrap();

	let new_block_number: BlockNumberFor<Test> = 1;

	new_test_ext().execute_with(|| {
		//adding schema
		System::set_block_number(new_block_number);
		//creating regisrty
		assert_ok!(Schema::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			schema.clone()
		));

		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			Some(schema_id_of)
		));

		//Admin should be able to add the delegate
		assert_ok!(Registry::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			DID_00,
		));

		//When Trying to add same delegate again it says delegate already added
		assert_err!(
			Registry::add_delegate(
				DoubleOrigin(author.clone(), DID_00.clone()).into(),
				registry_id.clone(),
				DID_00,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]

fn add_delegate_should_update_commit() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		assert_ok!(Registry::update_commit(
			&registry_id,
			id_digest,
			creator.clone(),
			RegistryCommitActionOf::Authorization
		));

		//Check wheter that event has been emitted
		assert_eq!(
			registry_events_since_last_call(),
			vec![Event::Create { registry: registry_id, creator }]
		);
	});
}

//TEST CASES FOR REMOVE DELEGATE FUNCTION

#[test]
fn remove_delegate_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;
	let raw_registry = [2u8; 256].to_vec();
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
		System::set_block_number(1);

		//creating regisrty
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//Admin should be able to add the delegate
		assert_ok!(Registry::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			DID_01,
		));

		//removing the registry should succedd
		assert_ok!(Registry::remove_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			authorization_id,
		));
	});
}
// TODO - remove delegate should fail if it is not perfomed by an authority

#[test]
fn create_registry_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();

	new_test_ext().execute_with(|| {
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));
	});
}

// TODO registry create with non-existent schema id
// TODO registry create failure - wrong input

#[test]
fn update_registry_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry,
			None
		));

		let update_registry = [5u8; 256].to_vec();
		let utx_registry: InputRegistryOf<Test> = BoundedVec::try_from(update_registry).unwrap();

		assert_ok!(Registry::update(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			utx_registry,
			registry_id
		));
	});
}

// TODO update failure test case

#[test]
fn archive_registry_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		//restoring a regisrty
		assert_ok!(Registry::archive(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone()
		));
	});
}

// TODO archive a non-existent registry - should fail
// TODO archive a registry by an admin who is not the creator - should succeed
// TODO archive registry by a delegate - should fail

#[test]
fn restore_registry_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let raw_registry = [2u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		//Creating a registry
		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		assert_ok!(Registry::archive(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone()
		));

		//Updating a regisrty
		assert_ok!(Registry::restore(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id
		));
	});
}
//TODO add test cases to check error conditions
