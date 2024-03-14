use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_ok, BoundedVec};
use frame_system::RawOrigin;
use pallet_chain_space::{SpaceCodeOf, SpaceIdOf};
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

/// Generates a space ID from a digest.
pub fn generate_space_id<T: Config>(digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

/// Generates an authorization ID from a digest.
pub fn generate_authorization_id<T: Config>(digest: &SpaceCodeOf<T>) -> AuthorizationIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Authorization)
		.unwrap()
}

/// Generates a asset ID from a digest.
pub fn generate_asset_id<T: Config>(digest: &SpaceCodeOf<T>) -> AssetIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Asset).unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn asset_create_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 5u64;

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_qty = 10;
	let asset_value = 10;
	let asset_type = AssetTypeOf::MF;

	let entry = AssetInputEntryOf::<Test> {
		asset_desc,
		asset_qty,
		asset_type,
		asset_value,
		asset_tag,
		asset_meta,
	};

	let digest = <Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		assert_ok!(Asset::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry,
			digest,
			authorization_id
		));
	});
}

#[test]
fn asset_issue_should_succeed() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 5u64;

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_qty = 10;
	let asset_value = 10;
	let asset_type = AssetTypeOf::MF;

	let entry = AssetInputEntryOf::<Test> {
		asset_desc,
		asset_qty,
		asset_type,
		asset_value,
		asset_tag,
		asset_meta,
	};

	let digest = <Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	let issue_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&digest.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);

	let asset_id: Ss58Identifier = generate_asset_id::<Test>(&issue_id_digest);

	let issue_entry = AssetIssuanceEntryOf::<Test> {
		asset_id,
		asset_owner: creator.clone(),
		asset_issuance_qty: Some(10),
	};

	let issue_entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&issue_entry.encode()[..]].concat()[..]);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		assert_ok!(Asset::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry,
			digest,
			authorization_id.clone()
		));

		assert_ok!(Asset::issue(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			issue_entry.clone(),
			issue_entry_digest,
			authorization_id
		));
	});
}
#[test]
fn trying_to_create_an_already_present_asset_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let capacity = 5u64;

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: Ss58Identifier = generate_authorization_id::<Test>(&auth_digest);

	let asset_desc = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_tag = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_meta = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let asset_qty = 10;
	let asset_value = 10;
	let asset_type = AssetTypeOf::MF;

	let entry = AssetInputEntryOf::<Test> {
		asset_desc,
		asset_qty,
		asset_type,
		asset_value,
		asset_tag,
		asset_meta,
	};

	let digest = <Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	new_test_ext().execute_with(|| {
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, capacity));

		assert_ok!(Asset::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			digest.clone(),
			authorization_id.clone()
		));
		assert_err!(
			Asset::create(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				entry,
				digest,
				authorization_id
			),
			Error::<Test>::AssetIdAlreadyExists
		)
	});
}
