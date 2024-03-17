use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, error::BadOrigin};
use frame_system::RawOrigin;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

pub fn generate_space_id<T: Config>(digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

pub fn generate_authorization_id<T: Config>(digest: &SpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([2u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

//TEST FUNCTION FOR ADD ADMIN DELEGATE

#[test]
fn add_delegate_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id,
			DID_01,
			authorization_id,
		));
	});
}

#[test]
fn add_admin_delegate_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id,
			DID_01,
			authorization_id,
		));
	});
}

#[test]
fn add_admin_delegate_should_fail_if_admin_delegate_already_exists() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			DID_01.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Space::add_admin_delegate(
				DoubleOrigin(author, creator).into(),
				space_id,
				DID_01,
				authorization_id,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn add_delegator_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_delegator(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id,
			DID_01,
			authorization_id,
		));
	});
}

#[test]
fn add_delegator_should_fail_if_delegator_already_exists() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_delegator(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			DID_01.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Space::add_delegator(
				DoubleOrigin(author, creator).into(),
				space_id,
				DID_01,
				authorization_id,
			),
			Error::<Test>::DelegateAlreadyAdded
		)
	});
}

#[test]
fn add_delegate_should_fail_if_space_is_not_created() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_admin_delegate_should_fail_if_space_is_not_created() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Space::add_admin_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_delegator_should_fail_if_space_is_not_created() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		//Should throw Error if registry is not created or found
		assert_err!(
			Space::add_delegator(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn add_delegate_should_fail_if_the_regisrty_is_archived() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::archive(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::ArchivedSpace
		);
	});
}

#[test]
fn add_delegate_should_fail_if_the_space_is_not_approved() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::SpaceNotApproved
		);
	});
}

#[test]
fn add_delegate_should_fail_if_a_non_delegate_tries_to_add() {
	let creator = DID_00;
	let creator1 = DID_01;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator1.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::UnauthorizedOperation
		);
	});
}

#[test]
fn add_delegate_should_fail_if_the_space_capacity_is_full() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		<Spaces<Test>>::insert(
			&space_id,
			SpaceDetailsOf::<Test> { txn_count: 3, ..<Spaces<Test>>::get(&space_id).unwrap() },
		);

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::CapacityLimitExceeded
		);
	});
}

#[test]
fn add_delegate_should_fail_if_delegate_already_exists() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		//Admin should be able to add the delegate
		assert_ok!(Space::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			DID_01.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author, creator).into(),
				space_id,
				DID_01,
				authorization_id,
			),
			Error::<Test>::DelegateAlreadyAdded
		);
	});
}

#[test]
fn creating_a_new_space_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
	});
}

#[test]
fn creating_a_duplicate_space_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_err!(
			Space::create(DoubleOrigin(author.clone(), creator.clone()).into(), space_digest,),
			Error::<Test>::SpaceAlreadyAnchored
		);
	});
}

#[test]
fn approving_a_new_space_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));
	});
}

#[test]
fn approving_a_non_exixtent_space_should_fail() {
	let creator = DID_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity),
			Error::<Test>::SpaceNotFound
		);
	});
}

#[test]
fn archiving_a_space_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 3u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));
		assert_ok!(Space::archive(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			authorization_id.clone(),
		));

		assert_err!(
			Space::add_delegate(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				SubjectId(AccountId32::new([1u8; 32])),
				authorization_id,
			),
			Error::<Test>::ArchivedSpace
		);
	});
}

#[test]
fn archiving_a_non_exixtent_space_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_err!(
			Space::archive(DoubleOrigin(author, creator).into(), space_id, authorization_id,),
			Error::<Test>::AuthorizationNotFound
		);
	});
}

#[test]
fn restoring_an_archived_a_space_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 5u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));
		assert_ok!(Space::archive(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Space::restore(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id.clone(),
			authorization_id.clone(),
		));

		assert_ok!(Space::add_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_id,
			SubjectId(AccountId32::new([1u8; 32])),
			authorization_id,
		));
	});
}

#[test]
fn restoring_an_non_archived_a_space_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 5u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_err!(
			Space::restore(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::SpaceNotArchived
		);
	});
}

#[test]
fn updating_space_capacity_by_root_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 5u64;
	let new_capacty = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::update_transaction_capacity(
			RawOrigin::Root.into(),
			space_id.clone(),
			new_capacty
		));
	});
}

#[test]
fn updating_space_capacity_by_non_root_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 5u64;
	let new_capacty = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));
		assert_err!(
			Space::update_transaction_capacity(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
				new_capacty,
			),
			BadOrigin
		);
	});
}

#[test]
fn reducing_space_capacity_by_root_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let new_capacty = 5u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::update_transaction_capacity(
			RawOrigin::Root.into(),
			space_id.clone(),
			new_capacty
		));
	});
}

#[test]
fn reducing_space_capacity_by_root_below_usage_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let new_capacty = 5u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		<Spaces<Test>>::insert(
			&space_id,
			SpaceDetailsOf::<Test> { txn_count: 7, ..<Spaces<Test>>::get(&space_id).unwrap() },
		);

		assert_err!(
			Space::update_transaction_capacity(
				RawOrigin::Root.into(),
				space_id.clone(),
				new_capacty
			),
			Error::<Test>::CapacityLessThanUsage
		);
	});
}

#[test]
fn resetting_space_usage_by_root_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		<Spaces<Test>>::insert(
			&space_id,
			SpaceDetailsOf::<Test> { txn_count: 7, ..<Spaces<Test>>::get(&space_id).unwrap() },
		);

		assert_ok!(Space::reset_transaction_count(RawOrigin::Root.into(), space_id.clone()));
	});
}

#[test]
fn resetting_space_usage_by_non_root_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		<Spaces<Test>>::insert(
			&space_id,
			SpaceDetailsOf::<Test> { txn_count: 7, ..<Spaces<Test>>::get(&space_id).unwrap() },
		);

		assert_err!(
			Space::reset_transaction_count(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				space_id,
			),
			BadOrigin
		);
	});
}

#[test]
fn revoking_approval_of_a_space_by_root_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::approval_revoke(RawOrigin::Root.into(), space_id.clone()));
	});
}

#[test]
fn revoking_approval_of_a_space_by_non_root_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_err!(
			Space::approval_revoke(DoubleOrigin(author.clone(), creator.clone()).into(), space_id,),
			BadOrigin
		);
	});
}

#[test]
fn restoring_approval_of_a_space_by_root_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::approval_revoke(RawOrigin::Root.into(), space_id.clone()));
		assert_ok!(Space::approval_restore(RawOrigin::Root.into(), space_id.clone()));
	});
}

#[test]
fn restoring_approval_of_a_non_revoked_space_by_root_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_err!(
			Space::approval_restore(RawOrigin::Root.into(), space_id.clone()),
			Error::<Test>::SpaceAlreadyApproved
		);
	});
}

#[test]
fn restoring_approval_of_a_space_by_non_root_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let capacity = 10u64;
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id.clone(), capacity));

		assert_ok!(Space::approval_revoke(RawOrigin::Root.into(), space_id.clone()));

		assert_err!(
			Space::approval_restore(DoubleOrigin(author.clone(), creator.clone()).into(), space_id,),
			BadOrigin
		);
	});
}

#[test]
fn add_delegate_should_fail_if_space_delegates_limit_exceeded() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&space.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let space_id: SpaceIdOf = generate_space_id::<Test>(&id_digest);

	new_test_ext().execute_with(|| {
		// Create the space
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		// Add the maximum number of delegates to the space
		for delegate_count in 0..4 {
			assert_ok!(Space::space_delegate_addition(
				space_id.clone(),
				SubjectId(AccountId32::new([delegate_count; 32])),
				creator.clone(),
				Permissions::all(),
			));
		}

		// Attempt to add one more delegate, which should exceed the limit and result in the
		// expected error
		assert_err!(
			Space::space_delegate_addition(
				space_id.clone(),
				SubjectId(AccountId32::new([4u8; 32])),
				creator.clone(),
				Permissions::all(),
			),
			Error::<Test>::SpaceDelegatesLimitExceeded
		);
	});
}
