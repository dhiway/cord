use super::*;
use crate::mock::*;
use codec::Encode;
use frame_support::assert_ok;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

/// Generate a namespace id from a digest.
pub fn generate_namespace_id<T: Config>(digest: &NameSpaceCodeOf<T>) -> NameSpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::NameSpace).unwrap()
}

/// Generate an authorization id from a digest.
pub fn generate_authorization_id<T: Config>(digest: &NameSpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(
		&(digest).encode()[..],
		IdentifierType::NameSpaceAuthorization,
	)
	.unwrap()
}

pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);
pub(crate) const ACCOUNT_01: AccountId = AccountId::new([2u8; 32]);

//TEST FUNCTION FOR ADD DELEGATE
#[test]
fn add_delegate_should_succeed() {
	let creator = ACCOUNT_00;
	let delegate = ACCOUNT_01;

	let raw_blob = [1u8; 256].to_vec();
	let blob: NameSpaceBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);
	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			Some(blob),
		));

		assert_ok!(NameSpace::add_delegate(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_id,
			delegate.clone(),
			authorization_id,
		));
	});
}

//TEST FUNCTION FOR UPDATE
#[test]
fn update_namespace_should_succeed() {
	let creator = ACCOUNT_00;

	let raw_blob = [1u8; 256].to_vec();
	let blob: NameSpaceBlobOf<Test> = BoundedVec::try_from(raw_blob)
		.expect("Test blob should fit into the expected input length of for the test runtime.");

	let namespace = [2u8; 256].to_vec();
	let namespace_digest = <Test as frame_system::Config>::Hashing::hash(&namespace.encode()[..]);

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let namespace_id: NameSpaceIdOf = generate_namespace_id::<Test>(&id_digest);

	let auth_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&namespace_id.encode()[..], &creator.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf = generate_authorization_id::<Test>(&auth_id_digest);

	new_test_ext().execute_with(|| {
		assert_ok!(NameSpace::create(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_digest,
			Some(blob.clone()),
		));

		assert!(
			NameSpaces::<Test>::contains_key(namespace_id.clone()),
			"Namespace was not stored!"
		);

		let namespace_updated = [3u8; 256].to_vec();
		let namespace_digest_updated =
			<Test as frame_system::Config>::Hashing::hash(&namespace_updated.encode()[..]);

		assert_ok!(NameSpace::update(
			frame_system::RawOrigin::Signed(creator.clone()).into(),
			namespace_id,
			namespace_digest_updated,
			None,
			authorization_id
		));
	});
}
