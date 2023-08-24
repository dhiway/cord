// // This file is part of CORD – https://cord.network

// // Copyright (C) Dhiway Networks Pvt. Ltd.
// // SPDX-License-Identifier: GPL-3.0-or-later

// // CORD is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version.

// // CORD is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// // GNU General Public License for more details.

// // You should have received a copy of the GNU General Public License
// // along with CORD. If not, see <https://www.gnu.org/licenses/>.

// use super::*;
// use crate::mock::*;
// use codec::Encode;
// use cord_utilities::mock::{mock_origin::DoubleOrigin, SubjectId};
// use frame_support::{assert_noop, assert_ok};
// use sp_core::H256;
// use sp_runtime::{traits::Hash, AccountId32};
// use sp_std::prelude::*;



// // pub fn generate_scoring_id<T: Config>(digest: &EntryHashOf<T>) -> ScoreIdentifierOf {
// // 	Ss58Identifier::to_scoring_id(&(digest).encode()[..]).unwrap()
// // }

// pub(crate) const DID_00: SubjectId = SubjectId(AccountId32::new([1u8; 32]));
// pub(crate) const ACCOUNT_00: AccountId = AccountId::new([1u8; 32]);


// // #[test]
// // fn check_successful_schema_creation() {
// // 	let creator = DID_00;
// // 	let author = ACCOUNT_00;

// // 	let scoring: Vec<u8> = [2u8;256].to_vec(); 
// // 	let scoring_digest = <Test as frame_system::Config>::Hashing::hash(
// // 		&[&scoring.encode()[..], &creator.encode()[..]].concat()[..],
// // 	);
// // 	//EntityIdentifierOf
// // 	let e_id:EntityIdentifierOf<Test>  = ACCOUNT_00;
// // 	//RequestIdentifierOf
// // 	let request_id:RequestIdentifierOf = generate_scoring_id::<Test>(&scoring_digest);
// // 	//TransactionIdentfierOf
// // 	let t_id:TransactionIdentifierOf = generate_scoring_id::<Test>(&scoring_digest);
// // 	//CollectorIdentifierOf
// // 	let c_id:CollectorIdentifierOf<Test>= ACCOUNT_00;
// // 	//RequestorIdentifierOf
// // 	let requestor_id:RequestorIdentifierOf<Test> = ACCOUNT_00;
// // 	//ScoreTypeOf
// // 	let score_type:ScoreTypeOf = ScoreTypeOf::Overall; 
// // 	//Entity Rating
// // 	let score : ScoreOf = 12;

// // 	let journal_details = JournalDetails {
// // 		entity : e_id,
// // 		uid : request_id,
// // 		tid : t_id,
// // 		collector : c_id,
// // 		requestor : requestor_id,
// // 		score_type,
// // 		score,
// // 	};

// // 	let journal_details_digest =  <Test as frame_system::Config>::Hashing::hash(
// // 		&[&journal_details.encode()[..]].concat()[..]);

// // 	let journal_entry = JournalEntry {
// // 		entry: journal_details.clone(),
// // 		digest : journal_details_digest,
// // 		block : 1
// // 	};

// // 	let journal_entry_digest =  <Test as frame_system::Config>::Hashing::hash(
// // 		&[&journal_entry.encode()[..]].concat()[..]);


// // 	let journal_input = JournalInput {
// // 		entry: journal_details.clone(),
// // 		digest : journal_entry_digest,
// // 		signature : creator.clone()
// // 	};

// // 	new_test_ext().execute_with(|| {
// // 		// Author Transaction
// // 		assert_ok!(Scoring::entries(
// // 			DoubleOrigin(author.clone(), creator.clone()).into(),
// // 			journal_input.clone(),
// // 		));
// // 	});
// // }


