// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	extension::{AsPerson, AsPersonInfo},
	mock::*,
	pallet::{AccountToPersonalId, Origin as PeopleOrigin},
	*,
};
use frame_support::{
	assert_noop, assert_ok, dispatch::Pays, pallet_prelude::Authorize, traits::Get, BoundedVec,
};
use indiv_pallet_members::RingMode;
use indiv_support::traits::RingExponent;
use sp_runtime::transaction_validity::{
	InvalidTransaction::{self, BadSigner},
	TransactionSource,
};

fn authorized() -> RuntimeOrigin {
	frame_system::RawOrigin::<u64>::Authorized.into()
}

fn generate_people_with_index(
	start: u8,
	end: u8,
) -> Vec<(PersonalId, MemberOf<Test>, SecretOf<Test>)> {
	let mut people = Vec::new();
	for i in start..=end {
		let person = PeoplePallet::reserve_new_id();
		let secret = MockCrypto::new_secret([i; 32]);
		let public = MockCrypto::member_from_secret(&secret);

		PeoplePallet::recognize_personhood(person, Some(public)).unwrap();
		people.push((person, public, secret));
	}

	people
}

fn suspended_indices_list(
	ring_index: u32,
) -> BoundedVec<u32, indiv_pallet_members::RingCapacityFromExponent<Test>> {
	let suspended_indices =
		indiv_pallet_members::PendingSuspensions::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, ring_index);
	assert!(&suspended_indices[..].windows(2).all(|pair| pair[0] < pair[1]));
	suspended_indices
}

#[test]
fn recognize_invalid_key_fails() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let id = PeoplePallet::reserve_new_id();
		assert_noop!(
			PeoplePallet::recognize_personhood(id, Some(INVALID_MEMBER)),
			indiv_pallet_members::Error::<Test>::InvalidMemberKey
		);
	});
}

#[test]
fn recognize_person_with_duplicate_key() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		// Recognize person A with a key.
		let person_a = PeoplePallet::reserve_new_id();
		let secret_a = MockCrypto::new_secret([1; 32]);
		let key_a = MockCrypto::member_from_secret(&secret_a);
		PeoplePallet::recognize_personhood(person_a, Some(key_a)).unwrap();

		// Recognize person B with the same key.
		let person_b = PeoplePallet::reserve_new_id();
		assert_noop!(
			PeoplePallet::recognize_personhood(person_b, Some(key_a)),
			Error::<Test>::KeyAlreadyInUse
		);
	});
}

#[test]
fn recognize_same_person_2_times() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let person_a = PeoplePallet::reserve_new_id();
		let secret_a = MockCrypto::new_secret([1; 32]);
		let key_a = MockCrypto::member_from_secret(&secret_a);
		assert_ok!(PeoplePallet::recognize_personhood(person_a, Some(key_a)));
		assert!(People::<Test>::get(person_a).is_some());
		assert_noop!(
			PeoplePallet::recognize_personhood(person_a, Some(key_a)),
			Error::<Test>::KeyAlreadyInUse,
		);
		assert_noop!(
			PeoplePallet::renew_id_reservation(person_a),
			Error::<Test>::PersonalIdReservationCannotRenew,
		);
	});
}

#[test]
fn recognize_person_with_duplicate_key_after_suspend() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let person_a = PeoplePallet::reserve_new_id();
		let person_b = PeoplePallet::reserve_new_id();
		let person_c = PeoplePallet::reserve_new_id();
		let secret_a = MockCrypto::new_secret([1; 32]);
		let secret_b = MockCrypto::new_secret([2; 32]);
		let secret_c = MockCrypto::new_secret([3; 32]);
		let key_a = MockCrypto::member_from_secret(&secret_a);
		let key_b = MockCrypto::member_from_secret(&secret_b);
		let key_c = MockCrypto::member_from_secret(&secret_c);
		// Recognize person A and B
		assert_ok!(PeoplePallet::recognize_personhood(person_a, Some(key_a)));
		// Onboard A so that they become part of a ring.
		Members::process_maintenance();
		// B will be part of the onboarding queue.
		assert_ok!(PeoplePallet::recognize_personhood(person_b, Some(key_b)));

		assert_eq!(
			Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &key_a).unwrap(),
			indiv_pallet_members::RingPosition::Included {
				ring_index: 0,
				ring_page: 0,
				ring_position: 0,
			}
		);

		assert!(matches!(
			Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &key_b).unwrap(),
			indiv_pallet_members::RingPosition::Onboarding { queue_page: 0, .. }
		));

		// Start suspensions.
		assert_ok!(PeoplePallet::start_people_set_mutation_session());

		// Suspend person A and B
		assert_ok!(PeoplePallet::suspend_personhood(&[person_a, person_b]));

		// End suspensions.
		assert_ok!(PeoplePallet::end_people_set_mutation_session());
		Members::process_maintenance();

		// Make sure both A and B are suspended.
		assert!(Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &key_a).unwrap().suspended());
		assert!(Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &key_b).unwrap().suspended());

		// Recognize person C with same key as A
		assert_noop!(
			PeoplePallet::recognize_personhood(person_c, Some(key_a)),
			Error::<Test>::KeyAlreadyInUse
		);

		// Recognize person C with a different key
		assert_ok!(PeoplePallet::recognize_personhood(person_c, Some(key_c)));

		// Resume personhood for A and B.
		assert_ok!(PeoplePallet::recognize_personhood(person_a, None));
		assert_ok!(PeoplePallet::recognize_personhood(person_b, None));
		// Both A and B kept their keys.
		assert_eq!(Keys::<Test>::get(key_a), Some(person_a));
		assert_eq!(Keys::<Test>::get(key_b), Some(person_b));
	});
}

#[test]
fn id_reservation_works() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		// Initially, no personal IDs are reserved or recognized.
		assert_eq!(NextPersonalId::<Test>::get(), 0);
		assert!(!ReservedPersonalId::<Test>::contains_key(0));
		assert!(People::<Test>::get(0).is_none());

		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew,
		);

		// Reserve a new ID. This should create a reservation at ID=0.
		assert_eq!(PeoplePallet::reserve_new_id(), 0);
		assert_eq!(NextPersonalId::<Test>::get(), 1);
		assert!(ReservedPersonalId::<Test>::contains_key(0));
		assert!(People::<Test>::get(0).is_none());

		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew,
		);

		// Reserve another new ID. This should create a reservation at ID=1.
		assert_eq!(PeoplePallet::reserve_new_id(), 1);
		assert_eq!(NextPersonalId::<Test>::get(), 2);
		assert!(ReservedPersonalId::<Test>::contains_key(1));
		assert!(People::<Test>::get(1).is_none());

		// Cancel the reservation for ID=0.
		assert_ok!(PeoplePallet::cancel_id_reservation(0));
		assert!(!ReservedPersonalId::<Test>::contains_key(0));
		assert!(People::<Test>::get(0).is_none());

		// Reserve a new ID again. This should create a reservation at ID=2.
		assert_eq!(PeoplePallet::reserve_new_id(), 2);
		assert_eq!(NextPersonalId::<Test>::get(), 3);
		assert!(ReservedPersonalId::<Test>::contains_key(2));
		assert!(People::<Test>::get(2).is_none());

		// Renew the reservation for ID=0.
		assert_ok!(PeoplePallet::renew_id_reservation(0));
		assert!(ReservedPersonalId::<Test>::contains_key(0));
		assert!(People::<Test>::get(0).is_none());

		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew,
		);

		// Recognize personhood for ID=0 with a dummy key.
		assert_ok!(PeoplePallet::recognize_personhood(0, Some([0; 32])));
		assert!(People::<Test>::get(0).is_some());
		assert!(!ReservedPersonalId::<Test>::contains_key(0));

		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew,
		);
	});
}

#[test]
fn force_recognize_personhood_works() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		// Force recognize an invalid key fails.
		assert_noop!(
			PeoplePallet::force_recognize_personhood(RuntimeOrigin::root(), vec![INVALID_MEMBER]),
			indiv_pallet_members::Error::<Test>::InvalidMemberKey,
		);

		// We'll create 5 new people to recognize.
		let num_people = 5;
		let mut keys = Vec::new();
		for i in 0..num_people {
			let secret = MockCrypto::new_secret([i as u8; 32]);
			let public_key = MockCrypto::member_from_secret(&secret);
			keys.push(public_key);
		}

		// Initially, no one is recognized.
		for id in 0..num_people {
			assert!(People::<Test>::get(id).is_none());
			assert!(!ReservedPersonalId::<Test>::contains_key(id));
		}
		assert_eq!(NextPersonalId::<Test>::get(), 0);

		// Using the root origin, force recognize these people.
		assert_ok!(PeoplePallet::force_recognize_personhood(RuntimeOrigin::root(), keys.clone()));

		// After recognition, each person should now exist in storage.
		for (i, key) in keys.clone().into_iter().enumerate() {
			let who = i as PersonalId;
			let record = People::<Test>::get(who).expect("Person should be recognized");
			assert_eq!(record.key, key);
			assert!(!ReservedPersonalId::<Test>::contains_key(who));
		}

		// NextPersonalId should now point to the next free ID after recognizing `num_people`.
		assert_eq!(NextPersonalId::<Test>::get(), num_people);

		// Any further IDs not used yet should be empty.
		assert!(People::<Test>::get(num_people).is_none());

		// Fails for non-root origin.
		assert_noop!(
			PeoplePallet::force_recognize_personhood(RuntimeOrigin::signed(0), keys.clone()),
			sp_runtime::DispatchError::BadOrigin
		);

		// Fails for duplicate keys.
		let another_key = {
			let secret = MockCrypto::new_secret([233; 32]);
			MockCrypto::member_from_secret(&secret)
		};
		assert_noop!(
			PeoplePallet::force_recognize_personhood(
				RuntimeOrigin::root(),
				vec![another_key, another_key]
			),
			Error::<Test>::KeyAlreadyInUse
		);
	});
}

#[test]
fn cannot_renew_future_id() {
	TestExt::new().execute_with(|| {
		// Initially, NextPersonalId should be 0.
		assert_eq!(NextPersonalId::<Test>::get(), 0);

		// Id 0 is not reserved, can't renew.
		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew
		);

		// Id 1 is not reserved, can't renew.
		assert_noop!(
			PeoplePallet::renew_id_reservation(1),
			Error::<Test>::PersonalIdReservationCannotRenew
		);

		// Reserve a new personal ID. This will be ID 0, and NextPersonalId should now become 1.
		let first_id = PeoplePallet::reserve_new_id();
		assert_eq!(first_id, 0);
		assert_eq!(NextPersonalId::<Test>::get(), 1);

		// Id 0 is reserved, can't renew.
		assert_noop!(
			PeoplePallet::renew_id_reservation(0),
			Error::<Test>::PersonalIdReservationCannotRenew
		);

		// Id 1 is future, can't renew.
		assert_noop!(
			PeoplePallet::renew_id_reservation(1),
			Error::<Test>::PersonalIdReservationCannotRenew
		);

		// Cancel the reservation for ID=0.
		assert_ok!(PeoplePallet::cancel_id_reservation(0));

		// Id 0 is not reserved, can renew.
		assert_ok!(PeoplePallet::renew_id_reservation(0));

		// Id 1 is future, can't renew.
		assert_noop!(
			PeoplePallet::renew_id_reservation(1),
			Error::<Test>::PersonalIdReservationCannotRenew
		);
	});
}

#[test]
fn test_set_personal_id_account() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		generate_people_with_index(0, 3);

		// (In our test, we treat PersonalId as a simple u64.)
		// Verify that there are no mappings for personal id 1 and account 42.
		assert!(AccountToPersonalId::<Test>::get(42).is_none());
		assert!(People::<Test>::get(1).unwrap().account.is_none());

		// Create an origin that represents a personal identity.
		// (Recall that your pallet’s Origin enum has a variant PersonalIdentity.)
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(1));
		// Call the extrinsic to set personal id account.
		System::set_block_number(1);
		System::reset_events();
		assert_ok!(PeoplePallet::set_personal_id_account(origin, 42, 0), Pays::No.into());

		// Check that the mapping is now present.
		assert_eq!(AccountToPersonalId::<Test>::get(42), Some(1));
		assert_eq!(People::<Test>::get(1).unwrap().account, Some(42));
		assert!(System::events().iter().any(|e| matches!(
			e.event,
			RuntimeEvent::PeoplePallet(Event::PersonalIdAccountSet { who: 1, account: 42 })
		)));

		// Now update the mapping by calling the extrinsic again.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(1));
		// Here we change the account to 43.
		assert_ok!(PeoplePallet::set_personal_id_account(origin, 43, 0), Pays::Yes.into());
		// The old mapping for account 42 should be removed.
		assert!(AccountToPersonalId::<Test>::get(42).is_none());
		assert_eq!(AccountToPersonalId::<Test>::get(43), Some(1));
		assert_eq!(People::<Test>::get(1).unwrap().account, Some(43));

		// Test that a non-personal identity origin (for example, a Signed origin)
		// does not work (the call should error with BadOrigin).
		let origin = RuntimeOrigin::signed(44);
		assert_noop!(
			PeoplePallet::set_personal_id_account(origin, 44, 0),
			sp_runtime::DispatchError::BadOrigin
		);

		// Test that trying to use an account that is already in use fails.
		// First, set a mapping for personal id 2 using account 45.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(2));
		assert_ok!(PeoplePallet::set_personal_id_account(origin, 45, 0), Pays::No.into());
		// Then try to set personal id 3 to use the same account 45.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(3));
		assert_noop!(
			PeoplePallet::set_personal_id_account(origin, 45, 0),
			Error::<Test>::AccountInUse
		);
	});
}

#[test]
fn test_unset_personal_id_account() {
	TestExt::new().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		generate_people_with_index(0, 1);

		// First, set a mapping for personal id 1 to account 50.
		let id_origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(1));
		assert_ok!(
			PeoplePallet::set_personal_id_account(id_origin.clone(), 50, 0),
			Pays::No.into()
		);
		assert_eq!(AccountToPersonalId::<Test>::get(50), Some(1));

		// Now call the unset extrinsic.
		System::set_block_number(1);
		System::reset_events();
		assert_ok!(PeoplePallet::unset_personal_id_account(id_origin.clone()), Pays::Yes.into());
		// Verify that the mappings have been removed.
		assert!(AccountToPersonalId::<Test>::get(50).is_none());
		assert!(People::<Test>::get(1).unwrap().account.is_none());
		assert!(System::events().iter().any(|e| matches!(
			e.event,
			RuntimeEvent::PeoplePallet(Event::PersonalIdAccountUnset { who: 1, account: 50 })
		)));

		// Calling unset again on the same account should fail.
		assert_noop!(
			PeoplePallet::unset_personal_id_account(id_origin.clone()),
			Error::<Test>::InvalidAccount
		);
	});
}

#[test]
fn test_as_personal_identity_with_account_check_and_nonce() {
	// Use our test externalities.
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let dummy_call = frame_system::Call::<Test>::remark { remark: vec![] };
		let account: u64 = 42;

		// 0: transaction fails because there signer is wrong, no associated personal id.
		let nonce: u64 = 0;
		let tx_ext = (
			AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce))),
			frame_system::CheckNonce::<Test>::from(nonce),
		);
		assert_noop!(
			exec_tx(Some(account), tx_ext, dummy_call.clone()),
			InvalidTransaction::BadSigner
		);

		// Add a person and an associated account ---
		let personal_id = generate_people_with_index(0, 0).pop().unwrap().0;
		AccountToPersonalId::<Test>::insert(account, personal_id);
		System::inc_sufficients(&account);

		// 1: a successful transaction
		let nonce: u64 = 0;
		let tx_ext = (
			AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce))),
			frame_system::CheckNonce::<Test>::from(nonce),
		);
		assert_ok!(exec_tx(Some(account), tx_ext, dummy_call.clone()));
		assert_eq!(frame_system::Pallet::<Test>::account_nonce(account), 1);

		// 2: another successful transaction
		let nonce: u64 = 1;
		let tx_ext = (
			AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce))),
			frame_system::CheckNonce::<Test>::from(nonce),
		);
		assert_ok!(exec_tx(Some(account), tx_ext, dummy_call.clone()));
		assert_eq!(frame_system::Pallet::<Test>::account_nonce(account), 2);

		// 3: transaction fails because the nonce is wrong
		let nonce: u64 = 1;
		let tx_ext = (
			AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce))),
			frame_system::CheckNonce::<Test>::from(nonce),
		);
		assert_noop!(exec_tx(Some(account), tx_ext, dummy_call), InvalidTransaction::Stale);
		assert_eq!(frame_system::Pallet::<Test>::account_nonce(account), 2);
	});
}

mod suspensions {
	use super::*;

	#[test]
	fn suspending_personhood_fails_if_no_session_started() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));
			// A ring with people exists
			generate_people_with_index(0, 1);
			let suspensions: &[PersonalId] = &[1];
			assert_noop!(
				PeoplePallet::suspend_personhood(suspensions),
				indiv_pallet_members::Error::<Test>::NoRemovalSession
			);
		});
	}

	#[test]
	fn suspending_personhood_fails_if_id_not_in_ring() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));
			// A ring with people exists
			generate_people_with_index(0, 9);
			Members::process_maintenance();

			// Attempt to suspend a person fails
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			let suspensions: &[PersonalId] = &[14];
			assert_noop!(PeoplePallet::suspend_personhood(suspensions), Error::<Test>::NotPerson);
		});
	}

	#[test]
	fn suspending_personhood_marks_people_as_suspended() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			// A ring with people exists
			generate_people_with_index(0, 9);
			Members::process_maintenance();

			// Attempt to suspend a person
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			let suspensions: &[PersonalId] = &[1];
			assert_ok!(PeoplePallet::suspend_personhood(suspensions));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Makes the person's record suspended
			let personal_record = People::<Test>::get(1);
			assert!(Members::member_status(
				PEOPLE_MEMBER_IDENTIFIER,
				&personal_record.unwrap().key
			)
			.unwrap()
			.suspended());

			// Pending suspensions for the ring are incremented
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![1]);
		});
	}

	#[test]
	fn suspended_people_removal_modifies_ring_data() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			// A ring exists
			generate_people_with_index(0, 9);
			Members::process_maintenance();
			let initial_root =
				indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, RI_ZERO).unwrap();

			assert_eq!(
				indiv_pallet_members::RingKeysStatus::<Test>::get(
					PEOPLE_MEMBER_IDENTIFIER,
					RI_ZERO
				),
				indiv_pallet_members::RingStatus { included: 10, total: 10, immutable_since: None }
			);

			// One person becomes suspended
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			let suspensions: &[PersonalId] = &[1];
			assert_ok!(PeoplePallet::suspend_personhood(suspensions));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Attempt to remove suspended people succeeds
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![1]);
			Members::process_maintenance();

			// Pending suspensions are cleared for the ring
			assert!(suspended_indices_list(RI_ZERO).is_empty());

			// Ring data becomes modified
			assert_eq!(
				indiv_pallet_members::RingKeysStatus::<Test>::get(
					PEOPLE_MEMBER_IDENTIFIER,
					RI_ZERO
				),
				indiv_pallet_members::RingStatus { included: 9, total: 9, immutable_since: None }
			);
			assert_eq!(
				indiv_pallet_members::RingKeys::<Test>::get((PEOPLE_MEMBER_IDENTIFIER, RI_ZERO, 0))
					.len(),
				9
			);
			assert_ne!(
				indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, RI_ZERO)
					.unwrap()
					.intermediate,
				initial_root.intermediate
			);
		});
	}

	#[test]
	fn suspending_in_multiple_sessions() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			// A ring with multiple people
			generate_people_with_index(0, 9);
			Members::process_maintenance();

			// First session: some people become suspended
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			// Suspend people 1 and 2.
			assert_ok!(PeoplePallet::suspend_personhood(&[1, 2]));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Those people are then removed
			// Initially, the indices of people 1 and 2 in the key list are 1 and 2, as they were
			// onboarded in order.
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![1, 2]);
			Members::process_maintenance();

			// Second session: some more people become suspended
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			// Suspend people 6 and 7.
			assert_ok!(PeoplePallet::suspend_personhood(&[6, 7]));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Pending suspensions are tracked correctly
			// After people 1 and 2 were removed, the indices of people 6 and 7 in the key list were
			// shifted to the left to now be 4 and 5.
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![4, 5]);

			// Those extra people are removed too
			Members::process_maintenance();

			// Final ring state is correct
			assert_eq!(
				indiv_pallet_members::RingKeys::<Test>::get((PEOPLE_MEMBER_IDENTIFIER, RI_ZERO, 0))
					.len(),
				6
			);
		});
	}

	#[test]
	fn suspending_personhood_then_resume() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			// A ring with people exists
			generate_people_with_index(0, 9);
			Members::process_maintenance();

			// Attempt to suspend a person
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			let suspensions: &[PersonalId] = &[1];
			assert_ok!(PeoplePallet::suspend_personhood(suspensions));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Makes the person's record suspended
			let personal_record = People::<Test>::get(1);
			assert!(Members::member_status(
				PEOPLE_MEMBER_IDENTIFIER,
				&personal_record.unwrap().key
			)
			.unwrap()
			.suspended());

			// Pending suspensions for the ring are incremented
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![1]);
			assert_ok!(PeoplePallet::recognize_personhood(1, None));
			// Still needs to remove a key.
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![1]);
			Members::process_maintenance();
			assert!(suspended_indices_list(RI_ZERO).is_empty());
		});
	}

	#[test]
	fn suspending_person_removes_associated_data() {
		TestExt::new().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			// A ring with multiple people
			generate_people_with_index(0, 9);
			Members::process_maintenance();

			// A person associated with an account
			let person_id = 0;
			let account_id = 42;

			let id_origin = RuntimeOrigin::from(PeopleOrigin::PersonalIdentity(person_id));
			assert_ok!(PeoplePallet::set_personal_id_account(id_origin, account_id, 0));

			// The association exists
			assert_eq!(AccountToPersonalId::<Test>::get(account_id), Some(person_id));

			// The person becomes suspended
			assert_ok!(PeoplePallet::start_people_set_mutation_session());
			assert_ok!(PeoplePallet::suspend_personhood(&[person_id]));
			assert_ok!(PeoplePallet::end_people_set_mutation_session());

			// Account to personal id is removed
			assert!(AccountToPersonalId::<Test>::get(account_id).is_none());

			// The person is removed
			assert_eq!(suspended_indices_list(RI_ZERO).into_inner(), vec![person_id as u32]);
			Members::process_maintenance();

			// Account to personal id stays removed
			assert!(AccountToPersonalId::<Test>::get(account_id).is_none());

			// Using the account for authentication fails
			let nonce = 0;
			let tx_ext = (
				AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithAccount(nonce))),
				frame_system::CheckNonce::<Test>::from(nonce),
			);
			let dummy_call = Call::<Test>::unset_alias_account {};
			assert_noop!(
				exec_tx(Some(account_id), tx_ext, dummy_call),
				InvalidTransaction::BadSigner
			);
		});
	}
}

#[test]
fn test_revision_in_tx_ext_as_alias_account() {
	new_test_ext().execute_with(|| {
		// Setup
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		let alias_account = 37;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// Use alias account successfully
		let call = frame_system::Call::remark { remark: vec![] };
		assert_ok!(exec_as_alias_tx(alias_account, call));

		// Revise the ring
		indiv_pallet_members::Root::<Test>::mutate(PEOPLE_MEMBER_IDENTIFIER, 0, |root| {
			root.as_mut().unwrap().revision = 1;
		});

		// Fail to alias account with outdated revision
		let call = frame_system::Call::remark { remark: vec![] };
		assert_noop!(exec_as_alias_tx(alias_account, call), BadSigner);
	});
}

#[test]
fn test_under_alias_revision_check() {
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		// Setup a person and its alias account
		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		let ring_info = indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0)
			.expect("Ring must exist after building");
		assert_eq!(ring_info.revision, 0);
		let alias_account: u64 = 42;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// The account can now use `under_alias` successfully
		let dummy_call = Box::new(RuntimeCall::from(frame_system::Call::remark { remark: vec![] }));
		System::set_block_number(1);
		System::reset_events();
		assert_ok!(PeoplePallet::under_alias(
			RuntimeOrigin::signed(alias_account),
			dummy_call.clone()
		));
		assert!(System::events()
			.iter()
			.any(|e| matches!(e.event, RuntimeEvent::PeoplePallet(Event::AliasDispatched { .. }))));

		// Now we change the ring to revision=1, making the stored alias outdated.
		let mut ring_info =
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0).unwrap();
		assert_eq!(ring_info.revision, 0);
		ring_info.revision = 1;
		indiv_pallet_members::Root::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, 0, ring_info);

		// Attempt `under_alias` again with the *outdated* revision=0 from storage => should fail.
		assert_noop!(
			PeoplePallet::under_alias(RuntimeOrigin::signed(alias_account), dummy_call),
			sp_runtime::DispatchError::BadOrigin,
		);
	});
}

#[test]
fn under_alias_rejects_removed_context() {
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		Members::process_maintenance();
		let alias_account: u64 = 42;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// Works while context is valid.
		let dummy_call = Box::new(RuntimeCall::from(frame_system::Call::remark { remark: vec![] }));
		assert_ok!(PeoplePallet::under_alias(
			RuntimeOrigin::signed(alias_account),
			dummy_call.clone()
		));

		// Fails after context is removed from AccountContexts.
		let dummy_call = Box::new(RuntimeCall::from(frame_system::Call::remark { remark: vec![] }));
		with_mock_context_disabled(|| {
			assert_noop!(
				PeoplePallet::under_alias(RuntimeOrigin::signed(alias_account), dummy_call),
				Error::<Test>::InvalidContext,
			);
		});
	});
}

#[test]
fn tx_ext_alias_with_account_rejects_removed_context() {
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		Members::process_maintenance();
		let alias_account: u64 = 42;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// Works while context is valid.
		let remark = RuntimeCall::from(frame_system::Call::remark { remark: vec![] });
		assert_ok!(exec_as_alias_tx(alias_account, remark));

		// Fails after context is removed from AccountContexts.
		with_mock_context_disabled(|| {
			let remark = RuntimeCall::from(frame_system::Call::remark { remark: vec![] });
			assert_eq!(
				exec_as_alias_tx(alias_account, remark),
				Err(InvalidTransaction::BadSigner.into()),
			);
		});
	});
}

#[test]
fn tx_ext_alias_with_account_revised_rejects_removed_context() {
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		Members::process_maintenance();
		let alias_account: u64 = 42;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// Bump ring revision so revision update is needed.
		let mut ring_info =
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0).unwrap();
		ring_info.revision = 1;
		indiv_pallet_members::Root::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, 0, ring_info);

		// Fails when context is removed — even with a valid proof.
		with_mock_context_disabled(|| {
			let remark = RuntimeCall::from(frame_system::Call::remark { remark: vec![] });
			assert_eq!(
				exec_as_alias_with_updated_revision_tx(alias_account, &pk, &sk, remark),
				Err(InvalidTransaction::BadSigner.into()),
			);
		});
	});
}

#[test]
fn test_unset_alias_account() {
	new_test_ext().execute_with(|| {
		use crate::pallet::Origin as PeopleOrigin;
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		// Create a person and build the ring.
		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();

		// Set up an alias account.
		let alias_account: u64 = 55;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		// Verify the alias-to-account mapping exists.
		let rev_ca = AccountToAlias::<Test>::get(alias_account).expect("alias should be set");
		assert_eq!(AliasToAccount::<Test>::get(&rev_ca.ca), Some(alias_account));

		// Unset the alias account.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca.clone()));
		System::set_block_number(1);
		System::reset_events();
		assert_ok!(PeoplePallet::unset_alias_account(origin));

		// Verify the mappings have been removed.
		assert!(AliasToAccount::<Test>::get(&rev_ca.ca).is_none());
		assert!(AccountToAlias::<Test>::get(alias_account).is_none());

		// Verify event was emitted.
		assert!(System::events().iter().any(|e| matches!(
			e.event,
			RuntimeEvent::PeoplePallet(Event::AliasAccountUnset { .. })
		)));

		// Calling unset again should fail.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca));
		assert_noop!(PeoplePallet::unset_alias_account(origin), Error::<Test>::InvalidAccount);
	});
}

#[test]
fn resetting_alias_account_for_new_revision_is_refunded() {
	new_test_ext().execute_with(|| {
		use crate::pallet::Origin as PeopleOrigin;
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		// Create a single person.
		let (_, _key, _secret) = generate_people_with_index(0, 0).pop().unwrap();

		// Build the ring with the single key we just inserted. This sets the ring revision to 0.
		Members::process_maintenance();
		let ring_info = indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0)
			.expect("Ring must exist");
		assert_eq!(ring_info.revision, 0);

		// Set an alias with revision=0 for an account for the first time.
		// We expect `Pays::No` because no alias was previously set.
		let account: u64 = 42;
		let ca = ContextualAlias { alias: [1u8; 32], context: MOCK_CONTEXT };
		let rev_ca = RevisedContextualAlias { revision: 0, ring: 0, ca: ca.clone() };
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca.clone()));
		System::set_block_number(1);
		System::reset_events();
		let result = PeoplePallet::set_alias_account(origin, account, 0);
		assert_eq!(result.unwrap(), frame_support::pallet_prelude::Pays::No.into());
		assert!(System::events()
			.iter()
			.any(|e| matches!(e.event, RuntimeEvent::PeoplePallet(Event::AliasAccountSet { .. }))));

		// Fail attempt to set the same alias again with the *same* revision=0 for the same account.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca.clone()));
		assert_noop!(
			PeoplePallet::set_alias_account(origin, account, 0),
			Error::<Test>::AliasAccountAlreadySet
		);

		// Attempt to set the same alias again with the *same* revision=0 for a different account.
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca.clone()));
		let account2: u64 = 43;
		let result = PeoplePallet::set_alias_account(origin, account2, 0);
		assert_eq!(result.unwrap(), frame_support::pallet_prelude::Pays::Yes.into());

		// Set the ring revision to 1.
		indiv_pallet_members::Root::<Test>::mutate(PEOPLE_MEMBER_IDENTIFIER, 0, |root| {
			root.as_mut().unwrap().revision = 1
		});

		// Now set the alias account again, but *with the newer revision=1*.
		// We expect `Pays::No` because the revision of the alias <-> Account is needed.
		let rev_ca_new = RevisedContextualAlias { revision: 1, ring: 0, ca: ca.clone() };
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca_new.clone()));
		let result = PeoplePallet::set_alias_account(origin, account, 0);
		assert_eq!(result.unwrap(), frame_support::pallet_prelude::Pays::No.into());

		// Move to a different ring.
		let ring_info =
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0).unwrap();
		indiv_pallet_members::Root::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, 1, ring_info);

		// Now set the alias account again, but *with the different ring=1*.
		// We expect `Pays::No` because the revision of the alias <-> Account is needed.
		let rev_ca_new = RevisedContextualAlias { revision: 1, ring: 1, ca };
		let origin = RuntimeOrigin::from(PeopleOrigin::PersonalAlias(rev_ca_new.clone()));
		let result = PeoplePallet::set_alias_account(origin, account, 0);
		assert_eq!(result.unwrap(), frame_support::pallet_prelude::Pays::No.into());
	});
}

// The below tests demonstrate replay protection for identity-based transactions in this pallet.
// Unlike standard extrinsics, replay protection here is NOT enforced solely by account nonce.
// Instead, the extrinsic (set_personal_id_account) uses a time tolerance window: the same
// transaction (with same nonce, call, and signature) can be replayed and executed **multiple**
// times within a valid window of blocks (account_setup_time_tolerance) before becoming stale.
//
// Setup: Alice is registered as a person and prepares a signed transaction to set her personal id
// account. The signature is bound to the call and nonce.
//
// Transaction 1: Executed for the first time, succeeds.
// Immediate replay (same block): Fails due to Substrate's "stale" check (same nonce already
// processed). Transaction 2: Prepared with future validity, fails until the block advances.
//
// After advancing the block (within the tolerance window):
// - Transaction 2 becomes valid and succeeds.
// - Transaction 1 can also be replayed and will succeed again, because it is still within the
//   allowed time window. This is intentional and differs from classic nonce-based replay
//   protection.
//
// Once the tolerance window expires:
// - Any replay of transaction 1 becomes invalid and is rejected as stale.
//
// Key takeaway: For this extrinsic, replay protection is enforced by a combination of nonce and a
// time-based window, allowing "mortal" transactions to be relayed and replayed for a short period.
// This means that classic nonce replay protection does not apply in the same way. If you are
// expecting strict nonce-based replay protection, be aware of this difference!
#[test]
fn replay_protection_for_identity() {
	new_test_ext().execute_with(|| {
		const EXTENSION_VERSION: u8 = 0;
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		// Setup alice as a member.
		let alice_sec = MockCrypto::new_secret([1u8; 32]);
		let alice_pub = MockCrypto::member_from_secret(&alice_sec);
		let alice_index = PeoplePallet::reserve_new_id();
		PeoplePallet::recognize_personhood(alice_index, Some(alice_pub)).unwrap();
		let generate_setup_account_tx_ext_for_call = |call: RuntimeCall| {
			let other_tx_ext = (frame_system::CheckNonce::<Test>::from(0),);
			// Here we simply ignore implicit as they are null.
			let msg = (&EXTENSION_VERSION, &call, &other_tx_ext)
				.using_encoded(sp_io::hashing::blake2_256);
			let signature = MockCrypto::sign(&alice_sec, &msg).unwrap();
			(
				AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalIdentityWithProof(
					signature,
					alice_index,
				))),
				other_tx_ext.0,
			)
		};
		// Transaction 1: Alice sets its personal id account to 10.
		let call = RuntimeCall::PeoplePallet(crate::Call::set_personal_id_account {
			account: 10,
			call_valid_at: System::block_number(),
		});
		let tx_ext = generate_setup_account_tx_ext_for_call(call.clone());
		assert_ok!(exec_tx(None, tx_ext.clone(), call.clone()));
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(10));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), Some(alice_index));
		// Somebody tries to replay the transaction, it must fail, replay is protected.
		assert_noop!(exec_tx(None, tx_ext.clone(), call.clone()), InvalidTransaction::Stale);
		// Transaction 2: Alice sets its personal id account to 11, with call valid only in the
		// future.
		let call_2 = RuntimeCall::PeoplePallet(crate::Call::set_personal_id_account {
			account: 11,
			call_valid_at: System::block_number() + 1,
		});
		let tx_ext_2 = generate_setup_account_tx_ext_for_call(call_2.clone());
		// Transaction 2 is not valid yet.
		assert_noop!(exec_tx(None, tx_ext_2.clone(), call_2.clone()), InvalidTransaction::Future);
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(10));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), Some(alice_index));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(11), None);
		// Advance some time. Transaction 1 is still valid, transaction 2 becomes valid.
		mock::advance_to(System::block_number() + PeoplePallet::account_setup_time_tolerance());
		// Transaction 2 is now valid.
		assert_ok!(exec_tx(None, tx_ext_2.clone(), call_2.clone()));
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(11));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(11), Some(alice_index));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), None);
		// Somebody replays the transaction 1, it must succeed. It is within time tolerance.
		assert_ok!(exec_tx(None, tx_ext.clone(), call.clone()));
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(10));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), Some(alice_index));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(11), None);
		// Replay the transaction 2.
		assert_ok!(exec_tx(None, tx_ext_2.clone(), call_2.clone()));
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(11));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(11), Some(alice_index));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), None);
		// Advance some time, Now time tolerance is exceeded for transaction 1.
		mock::advance_to(System::block_number() + 1);
		// Somebody replays the first transaction, it is invalid.
		assert_noop!(exec_tx(None, tx_ext, call), InvalidTransaction::Stale);
		assert_eq!(crate::People::<Test>::get(alice_index).unwrap().account, Some(11));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(11), Some(alice_index));
		assert_eq!(crate::AccountToPersonalId::<Test>::get(10), None);
	});
}

#[test]
fn replay_protection_for_alias() {
	new_test_ext().execute_with(|| {
		const EXTENSION_VERSION: u8 = 0;
		// Setup Alice as a member.
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let alice_sec = MockCrypto::new_secret([1u8; 32]);
		let alice_pub = MockCrypto::member_from_secret(&alice_sec);
		let alice_index = PeoplePallet::reserve_new_id();
		PeoplePallet::recognize_personhood(alice_index, Some(alice_pub)).unwrap();
		Members::process_maintenance();
		let generate_alias_tx_ext_for_call = |call: RuntimeCall| {
			let other_tx_ext = (frame_system::CheckNonce::<Test>::from(0),);
			// The message is the hash over the extension version, call, and other extensions.
			let msg = (&EXTENSION_VERSION, &call, &other_tx_ext)
				.using_encoded(sp_io::hashing::blake2_256);
			// Open a commitment (using Alice's public key and public data)
			let commitment = MockCrypto::open((), &alice_pub, Some(alice_pub).into_iter()).unwrap();
			// Create a VRF proof and compute the alias output from the call message.
			let (proof, alias_value) =
				MockCrypto::create(commitment, &alice_sec, &MOCK_CONTEXT, &msg).unwrap();
			let alias = ContextualAlias { context: MOCK_CONTEXT, alias: alias_value };
			let tx_ext = (
				AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalAliasWithProof(
					proof,
					0,
					MOCK_CONTEXT,
				))),
				other_tx_ext.0,
			);
			(tx_ext, alias)
		};
		// --- Transaction 1: set alias account to 10 ---
		// Use the current block number as the valid time.
		let call1 = RuntimeCall::PeoplePallet(crate::Call::set_alias_account {
			account: 10,
			call_valid_at: System::block_number(),
		});
		let (tx_ext1, alias) = generate_alias_tx_ext_for_call(call1.clone());
		let rev_alias = RevisedContextualAlias { revision: 0, ring: 0, ca: alias.clone() };
		// Execute transaction 1. It should succeed.
		assert_ok!(exec_tx(None, tx_ext1.clone(), call1.clone()));
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(10));
		assert_eq!(crate::AccountToAlias::<Test>::get(10), Some(rev_alias.clone()));
		// Replay transaction 1 immediately: it must fail (replay protected).
		assert_noop!(exec_tx(None, tx_ext1.clone(), call1.clone()), InvalidTransaction::Stale);
		// --- Transaction 2: set alias account to 11 ---
		// Set its valid time to the future: current block number plus the allowed tolerance + 1.
		let call2 = RuntimeCall::PeoplePallet(crate::Call::set_alias_account {
			account: 11,
			call_valid_at: System::block_number() + 1,
		});
		let (tx_ext2, _) = generate_alias_tx_ext_for_call(call2.clone());
		// Transaction 2 is too early: it should be rejected as "Future".
		assert_noop!(exec_tx(None, tx_ext2.clone(), call2.clone()), InvalidTransaction::Future);
		// The mapping still reflects transaction 1.
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(10));
		assert_eq!(crate::AccountToAlias::<Test>::get(10), Some(rev_alias.clone()));
		assert_eq!(crate::AccountToAlias::<Test>::get(11), None);
		// Advance time by the allowed tolerance. Now transaction 2 becomes valid.
		mock::advance_to(System::block_number() + PeoplePallet::account_setup_time_tolerance());
		// Execute transaction 2. It now succeeds.
		assert_ok!(exec_tx(None, tx_ext2.clone(), call2.clone()));
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(11));
		assert_eq!(crate::AccountToAlias::<Test>::get(11), Some(rev_alias.clone()));
		assert_eq!(crate::AccountToAlias::<Test>::get(10), None);
		// --- Replaying old transactions within tolerance ---
		// Replay transaction 1. Within the tolerance window its replay is allowed.
		assert_ok!(exec_tx(None, tx_ext1.clone(), call1.clone()));
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(10));
		assert_eq!(crate::AccountToAlias::<Test>::get(10), Some(rev_alias.clone()));
		assert_eq!(crate::AccountToAlias::<Test>::get(11), None);
		// Replay transaction 2 to set it back to 11.
		assert_ok!(exec_tx(None, tx_ext2.clone(), call2.clone()));
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(11));
		assert_eq!(crate::AccountToAlias::<Test>::get(11), Some(rev_alias.clone()));
		assert_eq!(crate::AccountToAlias::<Test>::get(10), None);
		// --- Advance time beyond tolerance ---
		// After advancing time a bit more, the time tolerance for transaction 1 is exceeded.
		mock::advance_to(System::block_number() + 1);
		// Now replaying transaction 1 must be rejected as stale.
		assert_noop!(exec_tx(None, tx_ext1, call1), InvalidTransaction::Stale);
		assert_eq!(crate::AliasToAccount::<Test>::get(&alias), Some(11));
		assert_eq!(crate::AccountToAlias::<Test>::get(11), Some(rev_alias));
	});
}

#[test]
fn dispatch_tx_as_alias_while_updating_revision() {
	new_test_ext().execute_with(|| {
		let chunks: BoundedVec<
			indiv_pallet_chunks_manager::UncheckedChunk<Test>,
			<Test as indiv_pallet_chunks_manager::Config>::PageSize,
		> = [(); 1024]
			.into_iter()
			.map(indiv_pallet_chunks_manager::UncheckedChunk)
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		let ring_building_batch_size: u32 =
			<Test as indiv_pallet_members::Config>::RingBuildingMemberLimit::get();
		indiv_pallet_chunks_manager::Chunks::<Test>::insert(RingExponent::R2e9, 0, chunks);
		// R2e9 has capacity 255, so we need 510 people for two full rings
		let ring_capacity = RingExponent::R2e9.ring_capacity();
		let total_people = ring_capacity * 2;
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			2,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		// Create enough people to fill two rings (inline generation with u32 indices)
		let mut people: Vec<(PersonalId, MemberOf<Test>, SecretOf<Test>)> = Vec::new();
		for i in 0..total_people {
			let person = PeoplePallet::reserve_new_id();
			// Use u32 bytes for entropy to support indices > 255
			let mut entropy = [0u8; 32];
			entropy[..4].copy_from_slice(&i.to_le_bytes());
			let secret = MockCrypto::new_secret(entropy);
			let public = MockCrypto::member_from_secret(&secret);
			PeoplePallet::recognize_personhood(person, Some(public)).unwrap();
			people.push((person, public, secret));
		}

		// Build the rings with the keys we just inserted.
		// Run multiple iterations to ensure all people are onboarded and rings are built.
		for _ in 0..10 {
			Members::process_maintenance();
		}
		let ring0_info = indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0)
			.expect("Ring 0 must exist");
		let ring1_info = indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 1)
			.expect("Ring 1 must exist");
		let initial_revision = ring0_info.revision;
		// Both rings should have the same initial revision
		assert_eq!(ring0_info.revision, ring1_info.revision);

		// Set the aliases for all people
		for index in 0..(total_people as u64) {
			let alias_account = 100 + index;
			setup_alias_account(
				&people[index as usize].1,
				&people[index as usize].2,
				MOCK_CONTEXT,
				alias_account,
			);
		}

		// Suspend and remove more than half of the people in both rings
		// For merge to work, each ring must be below 50% capacity (127 members for R2e9).
		// We need to suspend at least 128 people from each ring (255 - 127 = 128).
		// Suspend PersonalIds 2-130 from ring 0 (keep PersonalId 1 for alias test)
		// Suspend PersonalIds 257-385 from ring 1 (keep PersonalId 256 for alias test)
		assert_ok!(PeoplePallet::start_people_set_mutation_session());
		let suspensions = (2..=130).chain(257..=385).collect::<Vec<_>>();
		assert_ok!(PeoplePallet::suspend_personhood(&suspensions));
		// Check that the suspended positions are stored
		let ring0_suspensions = suspended_indices_list(RI_ZERO).into_inner();
		let ring1_suspensions = suspended_indices_list(1).into_inner();
		assert_eq!(ring0_suspensions.len(), 129); // 129 people suspended from ring 0
		assert_eq!(ring1_suspensions.len(), 129); // 129 people suspended from ring 1
		assert_ok!(PeoplePallet::end_people_set_mutation_session());

		// Remove suspended keys and rebuild rings.
		for _ in 0..10 {
			Members::process_maintenance();
		}

		// Check the revision of ring 0.
		let members_ring0_built_after_suspensions =
			RingExponent::R2e9.ring_capacity() - ring0_suspensions.len() as u32;
		let number_of_builds_required = members_ring0_built_after_suspensions /
			ring_building_batch_size +
			if !members_ring0_built_after_suspensions.is_multiple_of(ring_building_batch_size) {
				1
			} else {
				0
			}
		+ 1 // The revision for the empty ring root
			;
		let expected_revision_after_suspension_ring0 = initial_revision + number_of_builds_required;
		assert_eq!(
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0)
				.expect("Ring 0 must exist")
				.revision,
			expected_revision_after_suspension_ring0
		);

		// Check the revision of ring 1.
		let members_ring1_built_after_suspensions =
			RingExponent::R2e9.ring_capacity() - ring1_suspensions.len() as u32;
		let number_of_builds_required = members_ring1_built_after_suspensions /
			ring_building_batch_size +
			if !members_ring1_built_after_suspensions.is_multiple_of(ring_building_batch_size) {
				1
			} else {
				0
			}
		+ 1 // The revision for the empty ring root
		;
		let expected_revision_after_suspension_ring1 = initial_revision + number_of_builds_required;
		assert_eq!(
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 1)
				.expect("Ring 1 must exist")
				.revision,
			expected_revision_after_suspension_ring1
		);

		// The current ring has a higher index than the ones being merged
		indiv_pallet_members::CurrentRingIndex::<Test>::set(PEOPLE_MEMBER_IDENTIFIER, 14);

		assert_ok!(Members::merge_rings(
			RuntimeOrigin::signed(1234567),
			*PEOPLE_MEMBER_IDENTIFIER,
			0,
			1
		));

		// Build the new ring.
		for _ in 0..10 {
			Members::process_maintenance();
		}

		// Person from ring 0 (index 0, account 100) and person from ring 1 (index 255, account 355)
		let ring1_person_idx = ring_capacity as usize;
		let ring1_account = 100 + ring_capacity as u64;

		// Insert some mock nonce for the second account.
		frame_system::Account::<Test>::mutate(ring1_account, |info| info.nonce = 42);

		let dummy_call = frame_system::Call::<Test>::remark { remark: vec![] };
		// Update the alias and run a tx for a person from the original first ring
		assert_ok!(exec_as_alias_with_updated_revision_tx(
			100,
			&people[0].1,
			&people[0].2,
			dummy_call.clone()
		));
		// Update the alias and run a tx for a person from the original second ring
		assert_ok!(exec_as_alias_with_updated_revision_tx(
			ring1_account,
			&people[ring1_person_idx].1,
			&people[ring1_person_idx].2,
			dummy_call.clone()
		));

		println!("\n------------ Finished updating revised aliases ----------\n");

		// Ensure both alias revisions were updated.
		// The revision should have incremented by the number of builds required after the merge
		// with the other ring.
		let new_members_from_ring1_count =
			RingExponent::R2e9.ring_capacity() - ring1_suspensions.len() as u32;
		let number_of_builds_required = new_members_from_ring1_count / ring_building_batch_size +
			if !new_members_from_ring1_count.is_multiple_of(ring_building_batch_size) {
				1
			} else {
				0
			};

		let expected_revision =
			expected_revision_after_suspension_ring0 + number_of_builds_required;
		assert_eq!(
			AccountToAlias::<Test>::get(100),
			Some(RevisedContextualAlias {
				revision: expected_revision,
				ring: 0,
				ca: ContextualAlias {
					alias: CryptoOf::<Test>::alias_in_context(&people[0].2, &MOCK_CONTEXT).unwrap(),
					context: MOCK_CONTEXT
				}
			})
		);
		assert_eq!(
			AccountToAlias::<Test>::get(ring1_account),
			Some(RevisedContextualAlias {
				revision: expected_revision,
				ring: 0,
				ca: ContextualAlias {
					alias: CryptoOf::<Test>::alias_in_context(
						&people[ring1_person_idx].2,
						&MOCK_CONTEXT
					)
					.unwrap(),
					context: MOCK_CONTEXT
				}
			})
		);
		assert_eq!(frame_system::Account::<Test>::get(100).nonce, 1);
		assert_eq!(frame_system::Account::<Test>::get(ring1_account).nonce, 43);
	});
}

#[test]
fn set_alias_account_fails_for_invalid_context_in_extension() {
	new_test_ext().execute_with(|| {
		const EXTENSION_VERSION: u8 = 0;
		// Setup Alice as a member.
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let alice_sec = MockCrypto::new_secret([1u8; 32]);
		let alice_pub = MockCrypto::member_from_secret(&alice_sec);
		let alice_index = PeoplePallet::reserve_new_id();
		PeoplePallet::recognize_personhood(alice_index, Some(alice_pub)).unwrap();
		Members::process_maintenance();

		// Use a context that is NOT in AccountContexts (MOCK_CONTEXT is the only valid one).
		let invalid_context: indiv_support::traits::Context = *b"pop:invalid.context/notallowed  ";

		// Create the call and proof with the invalid context.
		let call = RuntimeCall::PeoplePallet(crate::Call::set_alias_account {
			account: 10,
			call_valid_at: System::block_number(),
		});
		let other_tx_ext = (frame_system::CheckNonce::<Test>::from(0),);
		let msg =
			(&EXTENSION_VERSION, &call, &other_tx_ext).using_encoded(sp_io::hashing::blake2_256);
		let commitment = MockCrypto::open((), &alice_pub, Some(alice_pub).into_iter()).unwrap();
		let (proof, _alias) =
			MockCrypto::create(commitment, &alice_sec, &invalid_context, &msg).unwrap();
		let tx_ext = (
			AsPerson::<Test>::new(Some(AsPersonInfo::AsPersonalAliasWithProof(
				proof,
				0,
				invalid_context,
			))),
			other_tx_ext.0,
		);

		// The transaction should fail with InvalidTransaction::Call because the context is not
		// in AccountContexts.
		assert_noop!(exec_tx(None, tx_ext, call), InvalidTransaction::Call);

		// Verify that no alias was set.
		assert!(crate::AccountToAlias::<Test>::get(10).is_none());
	});
}

mod create_people_collection {
	use super::*;

	#[test]
	fn create_people_collection_works() {
		TestExt::new().execute_with(|| {
			// Initially, collection is not created
			assert!(!PeopleCollectionCreated::<Test>::get());

			// Create the collection
			System::set_block_number(1);
			System::reset_events();
			assert_ok!(PeoplePallet::create_people_collection(
				frame_system::Origin::<Test>::Authorized.into()
			));

			// Collection is now created
			assert!(PeopleCollectionCreated::<Test>::get());

			// Verify the collection exists in the members pallet
			assert!(Members::ring_status(PEOPLE_MEMBER_IDENTIFIER, 0).is_some());

			// Verify event was emitted
			assert!(System::events()
				.iter()
				.any(|e| matches!(e.event, RuntimeEvent::PeoplePallet(Event::CollectionCreated))));
		});
	}

	#[test]
	fn create_people_collection_fails_if_already_exists() {
		TestExt::new().execute_with(|| {
			// Create the collection first
			assert_ok!(PeoplePallet::create_people_collection(
				frame_system::Origin::<Test>::Authorized.into()
			));
			assert!(PeopleCollectionCreated::<Test>::get());

			// Try to create it again - should fail via the authorize check
			let result =
				PeoplePallet::authorize_create_people_collection(TransactionSource::InBlock);
			assert!(result.is_err());
		});
	}
}

mod stale_alias_cleanup {
	use super::*;

	/// Helper: create a collection, register a person, build the ring, and set up an alias
	/// account. Returns (public_key, secret_key, alias_account).
	fn setup_person_with_alias() -> (
		<MockCrypto as verifiable::GenerateVerifiable>::Member,
		<MockCrypto as verifiable::GenerateVerifiable>::Secret,
		u64,
	) {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		let alias_account: u64 = 100;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);
		(pk, sk, alias_account)
	}

	/// Ring revision mismatch alone is not grounds for removal — the user
	/// can re-register after a ring rebuild.
	#[test]
	fn rejects_ring_revision_mismatch() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();

			// Bump the ring revision so the stored mapping becomes stale.
			indiv_pallet_members::Root::<Test>::mutate(PEOPLE_MEMBER_IDENTIFIER, 0, |root| {
				root.as_mut().unwrap().revision += 1;
			});

			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let aliases = BoundedVec::truncate_from(vec![rev_ca.ca.clone()]);

			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::AliasNotStale.into()),
			);

			// Mapping is still present.
			assert!(AliasToAccount::<Test>::get(&rev_ca.ca).is_some());
			assert!(AccountToAlias::<Test>::get(alias_account).is_some());
		});
	}

	/// A deleted ring makes the alias stale — authorize accepts it and dispatch cleans it up.
	#[test]
	fn accepts_deleted_ring() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();

			// Remove the ring root entirely so ring_revision returns None.
			indiv_pallet_members::Root::<Test>::remove(PEOPLE_MEMBER_IDENTIFIER, 0);

			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let aliases = BoundedVec::truncate_from(vec![rev_ca.ca.clone()]);

			assert_ok!(PeoplePallet::authorize_clean_up_stale_aliases(
				TransactionSource::Local,
				&aliases
			));

			assert_ok!(PeoplePallet::clean_up_stale_aliases(authorized(), aliases));

			// Mapping was removed.
			assert!(AliasToAccount::<Test>::get(&rev_ca.ca).is_none());
			assert!(AccountToAlias::<Test>::get(alias_account).is_none());
		});
	}

	#[test]
	fn succeeds_when_context_no_longer_allowed() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();
			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let ca = rev_ca.ca.clone();

			with_mock_context_disabled(|| {
				assert_ok!(PeoplePallet::clean_up_stale_aliases(
					authorized(),
					BoundedVec::truncate_from(vec![ca.clone()]),
				));

				assert!(AliasToAccount::<Test>::get(&ca).is_none());
				assert!(AccountToAlias::<Test>::get(alias_account).is_none());
			});
		});
	}

	#[test]
	fn decrements_sufficients_on_cleanup() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();

			let sufficients_before = frame_system::Account::<Test>::get(alias_account).sufficients;
			assert!(sufficients_before > 0);

			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			with_mock_context_disabled(|| {
				assert_ok!(PeoplePallet::clean_up_stale_aliases(
					authorized(),
					BoundedVec::truncate_from(vec![rev_ca.ca]),
				));
			});

			let sufficients_after = frame_system::Account::<Test>::get(alias_account).sufficients;
			assert_eq!(sufficients_after, sufficients_before - 1);
		});
	}

	/// Authorization rejects cleanup while the dynamic context is still in
	/// `AccountContexts` (i.e. before the event ID is removed from
	/// `ActiveEventIds`). Once the context is removed, cleanup succeeds.
	#[test]
	fn fails_when_dynamic_context_still_active() {
		new_test_ext().execute_with(|| {
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));

			let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
			Members::process_maintenance();
			Members::process_maintenance();

			// Simulate an airdrop-like dynamic context.
			let dynamic_context: Context = *b"pop:polkadot.network/airdrop\x01\0\0\0";
			add_extra_context(dynamic_context);

			let alias_account: u64 = 42;
			setup_alias_account(&pk, &sk, dynamic_context, alias_account);

			// Context is still active — authorize should fail (alias is not stale).
			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let aliases = BoundedVec::truncate_from(vec![rev_ca.ca.clone()]);
			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::AliasNotStale.into()),
			);

			// Now remove the dynamic context (simulating ActiveEventIds removal).
			remove_extra_context(&dynamic_context);

			// Cleanup succeeds now that the context is gone.
			assert_ok!(PeoplePallet::clean_up_stale_aliases(
				authorized(),
				BoundedVec::truncate_from(vec![rev_ca.ca]),
			));
		});
	}

	#[test]
	fn fails_when_mappings_are_inconsistent() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();
			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let ca = rev_ca.ca.clone();

			// Make the mappings inconsistent: point AccountToAlias to a different alias.
			let wrong_ca = ContextualAlias { alias: [99u8; 32], context: MOCK_CONTEXT };
			let wrong_rev_ca = RevisedContextualAlias { ca: wrong_ca, ..rev_ca };
			AccountToAlias::<Test>::insert(alias_account, &wrong_rev_ca);

			let aliases = BoundedVec::truncate_from(vec![ca]);
			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::AliasMismatch.into()),
			);
		});
	}

	#[test]
	fn double_cleanup_fails_on_second_call() {
		new_test_ext().execute_with(|| {
			let (_pk, _sk, alias_account) = setup_person_with_alias();
			let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
			let ca = rev_ca.ca.clone();

			with_mock_context_disabled(|| {
				assert_ok!(PeoplePallet::clean_up_stale_aliases(
					authorized(),
					BoundedVec::truncate_from(vec![ca.clone()]),
				));

				// Second call fails — mapping already removed.
				assert_noop!(
					PeoplePallet::clean_up_stale_aliases(
						authorized(),
						BoundedVec::truncate_from(vec![ca]),
					),
					crate::Error::<Test>::NoStaleAliases
				);
			});
		});
	}
}

mod clean_up_stale_aliases {
	use super::*;
	use crate::pallet::{CustomInvalidity, MAX_BULK_CLEANUP};

	#[test]
	fn succeeds_for_batch_within_limit() {
		new_test_ext().execute_with(|| {
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";
			let count = MAX_BULK_CLEANUP;

			let mut cas = Vec::new();
			for i in 0..count {
				let mut alias = [0u8; 32];
				alias[..4].copy_from_slice(&i.to_le_bytes());
				let alias_account: u64 = 500 + i as u64;
				let rev_ca = RevisedContextualAlias {
					revision: 0,
					ring: 0,
					ca: ContextualAlias { alias, context: invalid_context },
				};
				AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
				AccountToAlias::<Test>::insert(alias_account, &rev_ca);
				frame_system::Pallet::<Test>::inc_sufficients(&alias_account);
				cas.push(rev_ca.ca);
			}

			let aliases = BoundedVec::try_from(cas).unwrap();
			assert_eq!(aliases.len(), MAX_BULK_CLEANUP as usize);
			assert_ok!(PeoplePallet::clean_up_stale_aliases(authorized(), aliases));
		});
	}

	#[test]
	fn skips_failures_and_cleans_rest() {
		new_test_ext().execute_with(|| {
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";

			// One stale alias that will succeed.
			let alias_account: u64 = 500;
			let rev_ca = RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1u8; 32], context: invalid_context },
			};
			AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
			AccountToAlias::<Test>::insert(alias_account, &rev_ca);
			frame_system::Pallet::<Test>::inc_sufficients(&alias_account);

			// One bogus alias that has no mapping — dispatch skips it.
			let bogus = ContextualAlias { alias: [99u8; 32], context: MOCK_CONTEXT };

			let aliases = BoundedVec::try_from(vec![bogus, rev_ca.ca.clone()]).unwrap();
			assert_ok!(PeoplePallet::clean_up_stale_aliases(authorized(), aliases));
		});
	}

	#[test]
	fn fails_when_nothing_cleaned() {
		new_test_ext().execute_with(|| {
			// No mapping exists for this alias.
			let ca = ContextualAlias { alias: [0u8; 32], context: MOCK_CONTEXT };
			let aliases = BoundedVec::try_from(vec![ca]).unwrap();
			assert_noop!(
				PeoplePallet::clean_up_stale_aliases(authorized(), aliases,),
				crate::Error::<Test>::NoStaleAliases
			);
		});
	}

	/// Authorize rejects a batch that mixes stale and non-stale aliases.
	/// Staleness is enforced at the authorize layer; the dispatchable trusts authorize.
	#[test]
	fn authorize_rejects_mixed_stale_and_non_stale() {
		new_test_ext().execute_with(|| {
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";

			// Build a real ring so non-stale aliases have a matching revision.
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));
			let people = generate_people_with_index(0, 1);
			Members::process_maintenance();

			// Set up a valid (non-stale) alias through the normal flow.
			let (_, pk, sk) = &people[0];
			let valid_account: u64 = 501;
			setup_alias_account(pk, sk, MOCK_CONTEXT, valid_account);
			let valid_rev_ca = AccountToAlias::<Test>::get(valid_account).unwrap();

			// Manually insert a stale alias (removed context).
			let stale_account: u64 = 500;
			let stale_rev_ca = RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias: [1u8; 32], context: invalid_context },
			};
			AliasToAccount::<Test>::insert(&stale_rev_ca.ca, stale_account);
			AccountToAlias::<Test>::insert(stale_account, &stale_rev_ca);
			frame_system::Pallet::<Test>::inc_sufficients(&stale_account);

			let aliases =
				BoundedVec::try_from(vec![valid_rev_ca.ca.clone(), stale_rev_ca.ca.clone()])
					.unwrap();
			let call = crate::Call::<Test>::clean_up_stale_aliases { aliases };
			assert_eq!(
				call.authorize(TransactionSource::Local)
					.expect("Call must give some authorization"),
				Err(CustomInvalidity::AliasNotStale.into()),
			);
		});
	}

	#[test]
	fn fails_when_all_aliases_exist_but_none_are_stale() {
		new_test_ext().execute_with(|| {
			// Set up two valid (non-stale) aliases via the helper.
			assert_ok!(Members::create_collection(
				0,
				PEOPLE_MEMBER_IDENTIFIER,
				1,
				RingMode::Flexible,
				RingExponent::R2e9,
				None,
			));
			let people = generate_people_with_index(0, 1);
			Members::process_maintenance();

			let mut cas = Vec::new();
			for (i, (_, pk, sk)) in people.iter().enumerate() {
				let alias_account = 500 + i as u64;
				setup_alias_account(pk, sk, MOCK_CONTEXT, alias_account);
				let rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
				cas.push(rev_ca.ca);
			}

			// All aliases exist and are valid (not stale) — staleness is checked in authorize.
			let aliases = BoundedVec::try_from(cas).unwrap();
			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::AliasNotStale.into()),
			);
		});
	}

	#[test]
	fn authorize_rejects_empty_aliases() {
		new_test_ext().execute_with(|| {
			let aliases = BoundedVec::try_from(vec![]).unwrap();
			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::EmptyAliases.into()),
			);
		});
	}

	#[test]
	fn authorize_rejects_missing_account() {
		new_test_ext().execute_with(|| {
			let ca = ContextualAlias { alias: [0u8; 32], context: MOCK_CONTEXT };
			let aliases = BoundedVec::try_from(vec![ca]).unwrap();
			assert_eq!(
				PeoplePallet::authorize_clean_up_stale_aliases(TransactionSource::Local, &aliases),
				Err(CustomInvalidity::InvalidAccount.into()),
			);
		});
	}
}

/// After a ring rebuild the alias revision no longer matches, but the alias is
/// still usable via `AsPersonalAliasWithAccountRevised` and must NOT be cleaned
/// up as stale.
#[test]
fn old_revision_alias_is_not_stale_and_can_be_revised() {
	new_test_ext().execute_with(|| {
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let (_, pk, sk) = generate_people_with_index(0, 0).pop().unwrap();
		Members::process_maintenance();
		Members::process_maintenance();

		let alias_account: u64 = 42;
		setup_alias_account(&pk, &sk, MOCK_CONTEXT, alias_account);

		let old_rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
		assert_eq!(old_rev_ca.revision, 0);

		// Bump ring revision to simulate a ring rebuild.
		let mut ring_info =
			indiv_pallet_members::Root::<Test>::get(PEOPLE_MEMBER_IDENTIFIER, 0).unwrap();
		ring_info.revision += 1;
		indiv_pallet_members::Root::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, 0, ring_info);

		// The alias has an outdated revision but must NOT be considered stale.
		// Staleness is enforced at the authorize layer; the dispatchable trusts authorize.
		let aliases = BoundedVec::try_from(vec![old_rev_ca.ca.clone()]).unwrap();
		let call = crate::Call::<Test>::clean_up_stale_aliases { aliases };
		assert_eq!(
			call.authorize(TransactionSource::Local)
				.expect("Call must give some authorization"),
			Err(CustomInvalidity::AliasNotStale.into()),
		);

		// The alias can still be used via AsPersonalAliasWithAccountRevised.
		let remark = RuntimeCall::from(frame_system::Call::remark { remark: vec![] });
		assert_ok!(exec_as_alias_with_updated_revision_tx(alias_account, &pk, &sk, remark));

		// After revision update, the alias revision matches the current ring revision.
		let updated_rev_ca = AccountToAlias::<Test>::get(alias_account).unwrap();
		assert_eq!(updated_rev_ca.revision, 1);
	});
}

mod offchain_worker {
	use super::*;
	use codec::Decode;
	use frame_support::traits::OffchainWorker;
	use parking_lot::RwLock;
	use sp_core::offchain::{
		testing::{PoolState, TestOffchainExt, TestTransactionPoolExt},
		OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
	};
	use std::sync::Arc;

	/// Create a test externalities with offchain worker and transaction pool extensions.
	fn new_test_ext_with_ocw() -> (sp_io::TestExternalities, Arc<RwLock<PoolState>>) {
		let mut ext = new_test_ext();
		let (offchain, _state) = TestOffchainExt::new();
		let (pool, state) = TestTransactionPoolExt::new();
		ext.register_extension(OffchainDbExt::new(offchain.clone()));
		ext.register_extension(OffchainWorkerExt::new(offchain));
		ext.register_extension(TransactionPoolExt::new(pool));
		(ext, state)
	}

	#[test]
	fn submits_cleanup_only_at_interval() {
		let (mut ext, state) = new_test_ext_with_ocw();
		ext.execute_with(|| {
			// Directly insert a mapping with an invalid context (not in AccountContexts).
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";
			let alias: Alias = [42u8; 32];
			let alias_account: u64 = 300;
			let rev_ca = RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias, context: invalid_context },
			};
			AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
			AccountToAlias::<Test>::insert(alias_account, &rev_ca);
			frame_system::Pallet::<Test>::inc_sufficients(&alias_account);

			let interval: u64 = <Test as crate::Config>::StaleAliasCleanupInterval::get();

			// OCW should NOT run before the interval.
			for block in 1..interval {
				System::set_block_number(block);
				PeoplePallet::offchain_worker(block);
				assert_eq!(state.read().transactions.len(), 0);
			}

			// At the interval, OCW should submit a cleanup transaction.
			System::set_block_number(interval);
			PeoplePallet::offchain_worker(interval);
			assert_eq!(state.read().transactions.len(), 1);

			// Verify the transaction is a clean_up_stale_aliases call (authorized extrinsic).
			let tx = state.write().transactions.pop().unwrap();
			let ex: Extrinsic = Decode::decode(&mut &*tx).unwrap();
			match ex.function {
				RuntimeCall::PeoplePallet(crate::Call::clean_up_stale_aliases { aliases }) => {
					assert_eq!(aliases.len(), 1);
					assert_eq!(aliases[0].context, invalid_context);
					assert_eq!(aliases[0].alias, alias);
				},
				e => panic!("unexpected call: {e:?}"),
			}
		});
	}

	#[test]
	fn bulk_cleanup_limit_is_honored() {
		use crate::pallet::MAX_BULK_CLEANUP;

		let (mut ext, state) = new_test_ext_with_ocw();
		ext.execute_with(|| {
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";
			let count = (MAX_BULK_CLEANUP + 5) as u64;

			// Insert more stale aliases than the limit allows.
			for i in 0..count {
				let alias: Alias = {
					let mut a = [0u8; 32];
					a[..8].copy_from_slice(&i.to_le_bytes());
					a
				};
				let alias_account: u64 = 500 + i;
				let rev_ca = RevisedContextualAlias {
					revision: 0,
					ring: 0,
					ca: ContextualAlias { alias, context: invalid_context },
				};
				AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
				AccountToAlias::<Test>::insert(alias_account, &rev_ca);
				frame_system::Pallet::<Test>::inc_sufficients(&alias_account);
			}

			let interval: u64 = <Test as crate::Config>::StaleAliasCleanupInterval::get();
			System::set_block_number(interval);
			PeoplePallet::offchain_worker(interval);

			// Exactly one bulk transaction should be submitted.
			assert_eq!(state.read().transactions.len(), 1);

			let tx = state.write().transactions.pop().unwrap();
			let ex: Extrinsic = Decode::decode(&mut &*tx).unwrap();
			match ex.function {
				RuntimeCall::PeoplePallet(crate::Call::clean_up_stale_aliases { aliases }) => {
					// The batch must be capped at MAX_BULK_CLEANUP, not the full count.
					assert_eq!(aliases.len(), MAX_BULK_CLEANUP as usize);
				},
				e => panic!("unexpected call: {e:?}"),
			}
		});
	}

	#[test]
	fn zero_interval_disables_offchain_worker() {
		use crate::mock::StaleAliasCleanupInterval;

		let (mut ext, state) = new_test_ext_with_ocw();
		ext.execute_with(|| {
			// Insert a stale alias that would normally trigger cleanup.
			let invalid_context: Context = *b"pop:invalid.removed/stale       ";
			let alias: Alias = [77u8; 32];
			let alias_account: u64 = 400;
			let rev_ca = RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias, context: invalid_context },
			};
			AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
			AccountToAlias::<Test>::insert(alias_account, &rev_ca);
			frame_system::Pallet::<Test>::inc_sufficients(&alias_account);

			// Set interval to zero — offchain worker should be disabled entirely.
			StaleAliasCleanupInterval::set(&0);

			// Must not panic (previously would divide by zero) and must not submit.
			for block in 0..10u64 {
				System::set_block_number(block);
				PeoplePallet::offchain_worker(block);
			}
			assert_eq!(state.read().transactions.len(), 0);

			// Restore default interval for other tests.
			StaleAliasCleanupInterval::set(&5);
		});
	}

	#[test]
	fn does_not_submit_for_valid_contexts() {
		let (mut ext, state) = new_test_ext_with_ocw();
		ext.execute_with(|| {
			// Directly insert a mapping with MOCK_CONTEXT (which is in AccountContexts).
			let alias: Alias = [99u8; 32];
			let alias_account: u64 = 100;
			let rev_ca = RevisedContextualAlias {
				revision: 0,
				ring: 0,
				ca: ContextualAlias { alias, context: MOCK_CONTEXT },
			};
			AliasToAccount::<Test>::insert(&rev_ca.ca, alias_account);
			AccountToAlias::<Test>::insert(alias_account, &rev_ca);

			let interval: u64 = <Test as crate::Config>::StaleAliasCleanupInterval::get();
			System::set_block_number(interval);
			PeoplePallet::offchain_worker(interval);

			// No transactions should be submitted — context is still valid.
			assert_eq!(state.read().transactions.len(), 0);
		});
	}
}
