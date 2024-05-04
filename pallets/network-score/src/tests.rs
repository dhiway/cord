// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use super::*;
use crate::mock::*;
use codec::Encode;
use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
use frame_support::{assert_err, assert_ok, BoundedVec};
use frame_system::RawOrigin;
use pallet_chain_space::SpaceCodeOf;
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

pub fn generate_rating_id<T: Config>(digest: &RatingEntryHashOf<T>) -> RatingEntryIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Rating).unwrap()
}

pub fn generate_space_id<T: Config>(digest: &SpaceCodeOf<T>) -> SpaceIdOf {
	Ss58Identifier::create_identifier(&(digest).encode()[..], IdentifierType::Space).unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn check_successful_rating_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;

	let message_id = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id,
		provider_id,
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();

	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		// Author Transaction

		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 3u64));

		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry,
			entry_digest,
			message_id,
			authorization_id,
		));
	});
}

#[test]
fn check_duplicate_message_id() {
	let creator = DID_00.clone();
	let author = ACCOUNT_00.clone();

	let message_id = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id,
		provider_id,
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();

	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		// Author Transaction

		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 3u64));

		// Register the rating entry once
		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			entry_digest,
			message_id.clone(),
			authorization_id.clone(),
		));

		//error as if found the same message id
		assert_err!(
			Score::register_rating(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				entry,
				entry_digest,
				message_id,
				authorization_id,
			),
			Error::<Test>::MessageIdAlreadyExists
		);
	});
}

#[test]
fn revise_rating_with_entry_entity_mismatch_should_fail() {
	let creator = DID_00;
	let author = ACCOUNT_00;

	let message_id = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id: entity_id.clone(),
		provider_id,
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[
			&entry_digest.encode()[..],
			&entity_id.encode()[..],
			&message_id.encode()[..],
			&space_id.encode()[..],
			&creator.clone().encode()[..],
		]
		.concat()[..],
	);

	let identifier =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Rating);

	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		// Author Transaction

		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest,
		));

		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 3u64));

		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			entry_digest,
			message_id.clone(),
			authorization_id.clone(),
		));

		//error
		let mut mismatched_entry = entry.clone();
		mismatched_entry.entity_id = BoundedVec::try_from([80u8; 10].to_vec()).unwrap();
		let mismatched_entry_digest =
			<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);
		assert_err!(
			Score::revise_rating(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				mismatched_entry.clone(),
				mismatched_entry_digest,
				message_id.clone(),
				identifier.unwrap(),
				authorization_id.clone(),
			),
			Error::<Test>::EntityMismatch
		);
	});
}

#[test]
fn register_rating_with_existing_rating_identifier_should_fail() {
	// Define test parameters
	let creator = DID_00.clone();
	let author = ACCOUNT_00.clone();
	let message_id = BoundedVec::try_from([72u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id,
		provider_id,
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);
	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Create a space
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 3u64));

		// Register the rating entry once
		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			entry_digest,
			message_id.clone(),
			authorization_id.clone(),
		));

		// Remove message_id and provider_did from entries
		<MessageIdentifiers<Test>>::remove(message_id.clone(), creator.clone());
		// Attempt to register another rating entry with the same identifier
		assert_err!(
			Score::register_rating(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				entry.clone(),
				entry_digest,
				message_id.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RatingIdentifierAlreadyAdded
		);
	});
}

#[test]
fn revoke_rating_with_existing_rating_identifier_should_fail() {
	let creator = DID_00.clone();
	let author = ACCOUNT_00.clone();
	let message_id = BoundedVec::try_from([82u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id,
		provider_id,
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);
	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();

	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[
			&entry_digest.encode()[..],
			&entry.entity_id.encode()[..],
			&message_id.encode()[..],
			&space_id.encode()[..],
			&creator.clone().encode()[..],
		]
		.concat()[..],
	);

	let identifier =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Rating);

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 3u64));

		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			entry_digest,
			message_id.clone(),
			authorization_id.clone(),
		));

		<MessageIdentifiers<Test>>::remove(message_id.clone(), creator.clone());

		assert_err!(
			Score::revoke_rating(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				identifier.unwrap(),
				message_id.clone(),
				entry_digest,
				authorization_id.clone(),
			),
			Error::<Test>::RatingIdentifierAlreadyAdded
		);
	});
}

#[test]
fn revise_rating_with_existing_rating_identifier_should_fail() {
	let creator = DID_00.clone();
	let author = ACCOUNT_00.clone();
	let message_id = BoundedVec::try_from([82u8; 10].to_vec()).unwrap();
	let message_id_revise: BoundedVec<u8, MaxEncodedValueLength> =
		BoundedVec::try_from([75u8; 10].to_vec()).unwrap();
	let message_id_revoke = BoundedVec::try_from([84u8; 10].to_vec()).unwrap();
	let entity_id = BoundedVec::try_from([73u8; 10].to_vec()).unwrap();
	let provider_id = BoundedVec::try_from([74u8; 10].to_vec()).unwrap();
	let entry = RatingInputEntryOf::<Test> {
		entity_id: entity_id.clone(),
		provider_id: provider_id.clone(),
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry.encode()[..]].concat()[..]);

	let entry_revise = RatingInputEntryOf::<Test> {
		entity_id: entity_id.clone(),
		provider_id: provider_id.clone(),
		total_encoded_rating: 250u64,
		count_of_txn: 7u64,
		rating_type: RatingTypeOf::Overall,
		provider_did: creator.clone(),
	};
	let entry_revise_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&entry_revise.encode()[..]].concat()[..]);

	let raw_space = [2u8; 256].to_vec();
	let space_digest = <Test as frame_system::Config>::Hashing::hash(&raw_space.encode()[..]);
	let space_id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_digest.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let space_id: SpaceIdOf = generate_space_id::<Test>(&space_id_digest);
	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&space_id.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::create_identifier(&auth_digest.encode()[..], IdentifierType::Authorization)
			.unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[
			&entry_digest.encode()[..],
			&entry.entity_id.encode()[..],
			&message_id.encode()[..],
			&space_id.encode()[..],
			&creator.clone().encode()[..],
		]
		.concat()[..],
	);

	let identifier =
		Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Rating)
			.unwrap();
	let id_digest_revise = <Test as frame_system::Config>::Hashing::hash(
		&[
			&entry_revise_digest.encode()[..],
			&entry_revise.entity_id.encode()[..],
			&message_id_revoke.encode()[..],
			&space_id.encode()[..],
			&creator.clone().encode()[..],
		]
		.concat()[..],
	);
	let identifier_add =
		Ss58Identifier::create_identifier(&(id_digest_revise).encode()[..], IdentifierType::Rating)
			.unwrap();
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Create a space
		assert_ok!(Space::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			space_digest
		));
		assert_ok!(Space::approve(RawOrigin::Root.into(), space_id, 5u64));

		// Register the rating entry once
		assert_ok!(Score::register_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry.clone(),
			entry_digest,
			message_id.clone(),
			authorization_id.clone(),
		));

		// Revoke the rating to create a debit entry
		assert_ok!(Score::revoke_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			identifier.clone(),
			message_id_revoke.clone(),
			entry_digest,
			authorization_id.clone()
		));

		// Revise rating to create a new rating for the entity
		assert_ok!(Score::revise_rating(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			entry_revise.clone(),
			entry_revise_digest,
			message_id_revise.clone(),
			identifier_add.clone(),
			authorization_id.clone(),
		));

		// // Remove the messgae_id from list to reach `RatingIdentifierAlreadyAdded` block.
		<MessageIdentifiers<Test>>::remove(message_id_revise.clone(), creator.clone());

		// // Error when revising rating again with same rating identifier
		assert_err!(
			Score::revise_rating(
				DoubleOrigin(author.clone(), creator.clone()).into(),
				entry_revise.clone(),
				entry_revise_digest,
				message_id_revise.clone(),
				identifier_add.clone(),
				authorization_id.clone(),
			),
			Error::<Test>::RatingIdentifierAlreadyAdded
		);
	})
}
