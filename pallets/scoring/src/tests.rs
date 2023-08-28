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
use frame_support::{assert_ok, BoundedVec};
use pallet_registry::{InputRegistryOf, RegistryHashOf};
use sp_runtime::{traits::Hash, AccountId32};
use sp_std::prelude::*;

pub fn generate_rating_id<T: Config>(digest: &RatingEntryHashOf<T>) -> RatingIdOf {
	Ss58Identifier::to_scoring_id(&(digest).encode()[..]).unwrap()
}

pub fn generate_registry_id<T: Config>(digest: &RegistryHashOf<T>) -> RegistryIdOf {
	Ss58Identifier::to_registry_id(&(digest).encode()[..]).unwrap()
}

pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
pub(crate) const DID_01: SubjectId = SubjectId(AccountId32::new([5u8; 32]));
pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);

#[test]
fn check_successful_rating_creation() {
	let creator = DID_00;
	let author = ACCOUNT_00;
	let delegate = DID_01;

	//EntityIdentifierOf
	let e_id: EntityIdentifierOf<Test> = ACCOUNT_00;
	//RequestIdentifierOf
	let raw_request_id = [11u8; 72].to_vec();
	let request_id: RequestIdentifierOf<Test> = BoundedVec::try_from(raw_request_id).unwrap();
	//TransactionIdentfierOf
	let raw_transaction_id = [12u8; 72].to_vec();
	let t_id: TransactionIdentifierOf<Test> = BoundedVec::try_from(raw_transaction_id).unwrap();
	//CollectorIdentifierOf
	let c_id: CollectorIdentifierOf<Test> = ACCOUNT_00;
	//RequestorIdentifierOf
	let requestor_id: RequestorIdentifierOf<Test> = ACCOUNT_00;
	//ScoreTypeOf
	let rating_type: RatingTypeOf = RatingTypeOf::Overall;
	//Entity Rating
	let rating: RatingOf = 12;

	let journal_details = RatingEntryDetails {
		entity: e_id,
		uid: request_id,
		tid: t_id,
		collector: c_id,
		requestor: requestor_id,
		rating_type,
		rating,
	};

	let journal_details_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&journal_details.encode()[..]].concat()[..],
	);

	let raw_registry = [56u8; 256].to_vec();
	let registry: InputRegistryOf<Test> = BoundedVec::try_from(raw_registry).unwrap();
	let id_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry.encode()[..], &creator.encode()[..]].concat()[..],
	);
	let registry_id: RegistryIdOf = generate_registry_id::<Test>(&id_digest);

	let journal_entry = RatingEntry {
		entry: journal_details.clone(),
		digest: journal_details_digest,
		created_at: 1,
		registry: registry_id.clone(),
		creator: creator.clone(),
	};

	let journal_entry_digest =
		<Test as frame_system::Config>::Hashing::hash(&[&journal_entry.encode()[..]].concat()[..]);

	let journal_input = RatingInput {
		entry: journal_details.clone(),
		digest: journal_entry_digest,
		creator: creator.clone(),
	};

	let auth_digest = <Test as frame_system::Config>::Hashing::hash(
		&[&registry_id.encode()[..], &delegate.encode()[..], &author.encode()[..]].concat()[..],
	);
	let authorization_id: AuthorizationIdOf =
		Ss58Identifier::to_authorization_id(&auth_digest.encode()[..]).unwrap();

	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		// Author Transaction

		assert_ok!(Registry::create(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry.clone(),
			None
		));

		assert_ok!(Registry::add_admin_delegate(
			DoubleOrigin(author.clone(), creator.clone()).into(),
			registry_id.clone(),
			delegate.clone(),
		));

		assert_ok!(Scoring::entries(
			DoubleOrigin(author.clone(), delegate.clone()).into(),
			journal_input.clone(),
			authorization_id
		));
	});
}
