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

//! Unit tests for the resources pallet.

use super::{pallet::*, *};
use crate::{
	mock::*,
	types::{Credibility, FriendRequestReference},
	Error,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, dispatch::GetDispatchInfo, traits::Authorize};
use frame_system::RawOrigin as SystemOrigin;
use indiv_support::traits::AppendOnlyMembers;
use sp_core::Get;
use sp_runtime::{
	traits::DispatchTransaction,
	transaction_validity::{InvalidTransaction, TransactionSource},
	AccountId32,
};
use sp_statement_store::{get_allowance, StatementAllowance};

// --- Consumer registration tests ---

fn register_lite(idx: u64) -> AccountId32 {
	assert_ok!(Resources::register_lite_person(lite_person_origin(idx), comm_id(b"key1"),));
	id_to_account(idx)
}

#[test]
fn lite_consumer_registration_has_no_namespace_state() {
	new_test_ext().execute_with(|| {
		let account = register_lite(1);
		assert_eq!(
			Consumers::<Test>::get(&account),
			Some(crate::types::ConsumerInfo {
				identifier_key: comm_id(b"key1"),
				credibility: Credibility::Lite,
			}),
		);
		assert_noop!(
			Resources::register_lite_person(lite_person_origin(1), comm_id(b"key2")),
			Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn full_person_upgrade_only_changes_resource_credibility() {
	new_test_ext().execute_with(|| {
		set_time_sec(100);
		let account = register_lite(1);
		let proof = mock_lite_proof(account.clone());
		let origin = person_origin_for(10, 0, 0);
		let alias = id_to_alias(10);
		assert_ok!(Resources::register_person(origin, account.clone(), proof));
		assert_eq!(AccountOfAlias::<Test>::get(alias), Some(account.clone()));
		assert_eq!(
			Consumers::<Test>::get(account).unwrap().credibility,
			Credibility::Person { alias, last_update: 100, demoted: false },
		);
	});
}

mod friend_request {
	use super::*;

	#[test]
	fn friend_request_registration_rejects_non_current_period() {
		new_test_ext().execute_with(|| {
			set_time_sec(3 * SECONDS_PER_DAY + 123);

			let current_period =
				Resources::friend_request_period_from_timestamp(TestClock::now().as_secs());
			let stale_period = current_period.saturating_sub(2);
			let reference = FriendRequestReference { period: stale_period, seq: 0 };
			let origin = friend_request_origin(42);

			assert_noop!(
				Resources::set_friend_request_statement_account_for_sequence(
					origin,
					reference,
					id_to_account(99),
				),
				Error::<Test>::InvalidFriendRequestPeriod
			);
		});
	}

	#[test]
	fn friend_request_registration_accepts_previous_period_within_grace_window() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let current_period =
				Resources::friend_request_period_from_timestamp(TestClock::now().as_secs());
			let previous_period = current_period.saturating_sub(1);
			let reference = FriendRequestReference { period: previous_period, seq: 1 };
			let origin = friend_request_origin(44);

			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				origin,
				reference,
				id_to_account(101),
			));
		});
	}

	#[test]
	fn friend_request_registration_uses_period_context_and_sets_allowance() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 10);

			let now = TestClock::now().as_secs();
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 3,
			};
			let alias = id_to_alias(77);
			let origin = friend_request_origin(77);
			let stmt_account = id_to_account(99);
			let pre_allowance = get_allowance(&stmt_account);

			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				origin,
				reference,
				stmt_account.clone(),
			));

			let registration = FriendRequestRegistrationByAlias::<Test>::get(alias)
				.expect("friend request registration should exist");
			assert_eq!(registration.account_id, stmt_account);
			assert_eq!(registration.reference, reference);
			assert_eq!(FriendRequestAliasByAccount::<Test>::get(id_to_account(99)), Some(alias));

			let expected_allowance =
				pre_allowance.saturating_add(<Test as Config>::FriendRequestAllowance::get());
			assert_eq!(get_allowance(id_to_account(99)), expected_allowance);
		});
	}

	#[test]
	fn friend_request_cleanup_authorize_rejects_external_source() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let reference = FriendRequestReference { period: 0u32, seq: 2 };
			let stmt_account = id_to_account(102);
			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				friend_request_origin(55),
				reference,
				stmt_account.clone(),
			));

			set_time_sec(
				Resources::friend_request_expiration_time(reference.period).saturating_add(1),
			);
			let call = crate::Call::<Test>::clear_expired_friend_request_sequence {
				account: stmt_account,
				seq: reference.seq,
			};

			let result = call.authorize(TransactionSource::External);
			assert_eq!(result, Some(Err(InvalidTransaction::BadSigner.into())));
		});
	}

	#[test]
	fn friend_request_cleanup_authorize_returns_custom_invalidity_before_expiry() {
		new_test_ext().execute_with(|| {
			let grace_secs: u64 = <Test as Config>::FriendRequestGraceWindow::get() as u64;
			set_time_sec(SECONDS_PER_DAY + 100);

			let previous_period = 0u32;
			let reference = FriendRequestReference { period: previous_period, seq: 2 };
			let stmt_account = id_to_account(102);
			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				friend_request_origin(55),
				reference,
				stmt_account.clone(),
			));

			set_time_sec(SECONDS_PER_DAY + grace_secs + 1);
			let call = crate::Call::<Test>::clear_expired_friend_request_sequence {
				account: stmt_account,
				seq: reference.seq,
			};
			let result = call.authorize(TransactionSource::InBlock);

			assert_eq!(
				result,
				Some(Err(InvalidTransaction::Custom(
					crate::extension::CustomValidity::InvalidExpiredFriendRequestCleanup as u8
				)
				.into()))
			);
		});
	}

	#[test]
	fn offchain_worker_clears_expired_friend_request_registrations() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let reference = FriendRequestReference { period: 0, seq: 3 };
			let alias = id_to_alias(56);
			let stmt_account = id_to_account(103);
			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				friend_request_origin(56),
				reference,
				stmt_account.clone(),
			));

			assert_eq!(FriendRequestAliasByAccount::<Test>::get(&stmt_account), Some(alias));
			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(alias));

			set_time_sec(
				Resources::friend_request_expiration_time(reference.period).saturating_add(1),
			);
			advance_to_block(2);

			assert_eq!(FriendRequestAliasByAccount::<Test>::get(&stmt_account), None);
			assert!(!FriendRequestRegistrationByAlias::<Test>::contains_key(alias));
			assert_eq!(get_allowance(&stmt_account), StatementAllowance::default());
			System::assert_has_event(
				Event::<Test>::FriendRequestStmtUsageRemoved { account: id_to_account(103) }.into(),
			);
		});
	}

	#[test]
	fn friend_request_period_math_uses_full_u64_timestamp() {
		new_test_ext().execute_with(|| {
			let period_duration = u64::from(<Test as Config>::FriendRequestPeriodDuration::get());
			let now = u64::from(u32::MAX).saturating_add(period_duration);
			let expected = (now / period_duration) as u32;

			assert_eq!(Resources::friend_request_period_from_timestamp(now), expected);
		});
	}

	#[test]
	fn friend_request_context_layout_is_fixed_and_non_truncating() {
		new_test_ext().execute_with(|| {
			let reference = FriendRequestReference { period: 0x0102_0304, seq: 0xAB };
			let context = Resources::friend_request_context(reference);
			let prefix = b"FRND_REQ:";

			assert_eq!(&context[..prefix.len()], prefix);
			assert_eq!(&context[prefix.len()..prefix.len() + 4], &reference.period.to_be_bytes());
			assert_eq!(context[prefix.len() + 4], reference.seq);
			assert!(
				context[prefix.len() + 5..].iter().all(|b| *b == b' '),
				"remaining context bytes should stay as padding",
			);
		});
	}

	#[test]
	fn friend_request_registration_rejects_sequence_above_max() {
		new_test_ext().execute_with(|| {
			let now = 100u64;
			set_time_sec(now);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: <Test as Config>::FriendRequestSlotsPerPeriod::get() + 1,
			};
			let origin = friend_request_origin(7);

			assert_noop!(
				Resources::set_friend_request_statement_account_for_sequence(
					origin,
					reference,
					id_to_account(50),
				),
				Error::<Test>::InvalidFriendRequestSequence
			);
		});
	}

	#[test]
	fn friend_request_registration_rejects_signed_origin() {
		new_test_ext().execute_with(|| {
			let now = 321u64;
			set_time_sec(now);
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 0,
			};

			assert_noop!(
				Resources::set_friend_request_statement_account_for_sequence(
					RuntimeOrigin::signed(id_to_account(7)),
					reference,
					id_to_account(70),
				),
				sp_runtime::DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn friend_request_registration_rejects_duplicate_alias() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let origin = friend_request_origin(7);

			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				origin.clone(),
				reference,
				id_to_account(50),
			));
			assert_noop!(
				Resources::set_friend_request_statement_account_for_sequence(
					origin,
					reference,
					id_to_account(51),
				),
				Error::<Test>::FriendRequestRegistrationAlreadyExists
			);
		});
	}

	#[test]
	fn friend_request_registration_invalid_period_returns_custom_invalidity() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now).saturating_add(10),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);

			let secret = MockCrypto::new_secret([1u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			let commitment = MockCrypto::open((), &member, core::iter::once(member))
				.expect("commitment should open");
			// The proof message is intentionally fake — this test relies on the extension
			// rejecting the invalid period before it reaches proof verification.
			let msg = [0u8; 32];
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(proof, 0),
			));
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(99),
				},
			);

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				0,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::InvalidFriendRequestPeriod as u8
			));
		});
	}

	#[test]
	fn friend_request_registration_invalid_sequence_returns_custom_invalidity() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: <Test as Config>::FriendRequestSlotsPerPeriod::get() + 1,
			};
			let context = Resources::friend_request_context(reference);

			let secret = MockCrypto::new_secret([2u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			let commitment = MockCrypto::open((), &member, core::iter::once(member))
				.expect("commitment should open");
			let msg = [0u8; 32];
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(proof, 0),
			));
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(100),
				},
			);

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				0,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::InvalidFriendRequestSequence as u8
			));
		});
	}

	#[test]
	fn friend_request_registration_rejects_occupied_account_before_dispatch() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let stmt_account = id_to_account(79);
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 2,
			};

			let first_origin = friend_request_origin(41);
			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				first_origin,
				reference,
				stmt_account.clone(),
			));

			// Match the collection-aware extension behavior for alias/account registration
			// conflicts.
			assert_eq!(
				Resources::validate_friend_request_registration(id_to_alias(42), &stmt_account)
					.map_err(|_| {
						crate::extension::CustomValidity::FriendRequestRegistrationConflict
					}),
				Err(crate::extension::CustomValidity::FriendRequestRegistrationConflict),
			);
		});
	}

	#[test]
	fn friend_request_registration_replay_is_rejected_before_dispatch() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([21u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(People::force_recognize_personhood(RuntimeOrigin::root(), vec![member]));
			advance_to_block(3);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);
			let stmt_account = id_to_account(78);
			let extension_version = 0u8;

			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: stmt_account.clone(),
				},
			);

			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (proof, alias) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				RuntimeOrigin::from(OriginCaller::Resources(crate::Origin::FriendRequestAlias(
					alias
				))),
				reference,
				stmt_account.clone(),
			));

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(proof, 0),
			));
			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				extension_version,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code)
				)) if code
					== crate::extension::CustomValidity::FriendRequestRegistrationConflict as u8
			));
		});
	}

	#[test]
	fn friend_request_registration_uses_same_pool_tag_for_same_slot() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([22u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(People::force_recognize_personhood(RuntimeOrigin::root(), vec![member]));
			advance_to_block(3);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 4,
			};
			let context = Resources::friend_request_context(reference);
			let extension_version = 0u8;
			let ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let first_call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(81),
				},
			);
			let second_call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(82),
				},
			);
			let first_msg = sp_runtime::traits::TxBaseImplication((extension_version, &first_call))
				.using_encoded(sp_io::hashing::blake2_256);
			let second_msg =
				sp_runtime::traits::TxBaseImplication((extension_version, &second_call))
					.using_encoded(sp_io::hashing::blake2_256);
			let (first_proof, _) =
				MockCrypto::create(commitment.clone(), &secret, &context, &first_msg)
					.expect("first proof should build");
			let (second_proof, _) = MockCrypto::create(commitment, &secret, &context, &second_msg)
				.expect("second proof should build");
			let first_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(first_proof, 0),
			));
			let second_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(second_proof, 0),
			));

			let first_validity = first_tx_ext
				.validate_only(
					SystemOrigin::None.into(),
					&first_call,
					&first_call.get_dispatch_info(),
					0,
					sp_runtime::transaction_validity::TransactionSource::External,
					extension_version,
				)
				.expect("first friend request registration should validate");
			let second_validity = second_tx_ext
				.validate_only(
					SystemOrigin::None.into(),
					&second_call,
					&second_call.get_dispatch_info(),
					0,
					sp_runtime::transaction_validity::TransactionSource::External,
					extension_version,
				)
				.expect("second friend request registration should validate");

			assert_eq!(
				first_validity.0.provides, second_validity.0.provides,
				"same friend request slot should dedupe in the tx pool even if account_id changes"
			);
		});
	}

	#[test]
	fn friend_request_dispatchable_with_alias_origin_succeeds() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let now = 123u64;
			set_time_sec(now);

			let stmt_account = id_to_account(200);
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let origin = friend_request_origin(90);

			assert_ok!(Resources::set_friend_request_statement_account_for_sequence(
				origin,
				reference,
				stmt_account.clone(),
			));

			let alias = id_to_alias(90);
			assert_eq!(FriendRequestAliasByAccount::<Test>::get(&stmt_account), Some(alias));
			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(alias));

			let friend_request_allowance = <Test as Config>::FriendRequestAllowance::get();
			assert_eq!(get_allowance(&stmt_account), friend_request_allowance);

			System::assert_has_event(
				Event::<Test>::FriendRequestStmtUsageSet {
					alias,
					period: reference.period,
					seq: reference.seq,
					account: id_to_account(200),
				}
				.into(),
			);
		});
	}

	#[test]
	fn collection_based_lite_friend_request_rejects_seq_above_lite_limit() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let seq = <Test as Config>::LiteFriendRequestSlotsPerPeriod::get() + 1;
			// Ensure this seq is within the full-people limit, proving the rejection
			// is specific to the lite path.
			assert!(seq <= <Test as Config>::FriendRequestSlotsPerPeriod::get());

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq,
			};
			let context = Resources::friend_request_context(reference);

			let secret = MockCrypto::new_secret([30u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			let commitment = MockCrypto::open((), &member, core::iter::once(member))
				.expect("commitment should open");
			let msg = [0u8; 32];
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(201),
				},
			);

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				0,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::InvalidFriendRequestSequence as u8
			));
		});
	}

	#[test]
	fn collection_based_lite_friend_request_accepts_seq_at_lite_limit() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			let seq = <Test as Config>::LiteFriendRequestSlotsPerPeriod::get();
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq,
			};
			let context = Resources::friend_request_context(reference);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([30u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![member],
			));
			advance_to_block(3);

			let extension_version = 0u8;
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(201),
				},
			);
			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			assert!(
				tx_ext
					.validate_only(
						SystemOrigin::None.into(),
						&call,
						&call.get_dispatch_info(),
						0,
						sp_runtime::transaction_validity::TransactionSource::External,
						extension_version,
					)
					.is_ok(),
				"lite collection should accept the boundary seq"
			);
		});
	}

	#[test]
	fn collection_based_people_variant_accepts_seq_above_lite_limit() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			// Use a seq that is above the lite limit but within the full-people limit.
			let seq = <Test as Config>::LiteFriendRequestSlotsPerPeriod::get() + 1;
			assert!(seq <= <Test as Config>::FriendRequestSlotsPerPeriod::get());

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq,
			};
			let context = Resources::friend_request_context(reference);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([31u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(People::force_recognize_personhood(RuntimeOrigin::root(), vec![member]));
			advance_to_block(3);

			let ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let extension_version = 0u8;
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(202),
				},
			);
			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::People,
				),
			));

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				extension_version,
			);

			assert!(result.is_ok(), "people collection should accept seq above lite limit");
		});
	}

	#[test]
	fn collection_based_friend_request_shared_storage_coexistence_via_extension() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let now = 123u64;
			set_time_sec(now);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let people_secret = MockCrypto::new_secret([33u8; 32]);
			let people_member = MockCrypto::member_from_secret(&people_secret);
			assert_ok!(People::force_recognize_personhood(
				RuntimeOrigin::root(),
				vec![people_member]
			));

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let lite_secret = MockCrypto::new_secret([34u8; 32]);
			let lite_member = MockCrypto::member_from_secret(&lite_secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![lite_member],
			));
			advance_to_block(3);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);
			let extension_version = 0u8;

			let people_account = id_to_account(300);
			let people_call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: people_account.clone(),
				},
			);
			let people_msg =
				sp_runtime::traits::TxBaseImplication((extension_version, &people_call))
					.using_encoded(sp_io::hashing::blake2_256);
			let people_ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let people_commitment =
				MockCrypto::open((), &people_member, people_ring_members.into_iter())
					.expect("people commitment should open");
			let (people_proof, people_alias) =
				MockCrypto::create(people_commitment, &people_secret, &context, &people_msg)
					.expect("people proof should build");
			let people_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					people_proof,
					0,
					crate::types::MembershipCollection::People,
				),
			));
			assert_ok!(people_tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				people_call,
				&RuntimeCall::Resources(
					crate::Call::set_friend_request_statement_account_for_sequence {
						reference,
						account_id: people_account.clone(),
					},
				)
				.get_dispatch_info(),
				0,
				extension_version,
			));

			let lite_account = id_to_account(301);
			let lite_call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: lite_account.clone(),
				},
			);
			let lite_msg = sp_runtime::traits::TxBaseImplication((extension_version, &lite_call))
				.using_encoded(sp_io::hashing::blake2_256);
			let lite_ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let lite_commitment = MockCrypto::open((), &lite_member, lite_ring_members.into_iter())
				.expect("lite commitment should open");
			let (lite_proof, lite_alias) =
				MockCrypto::create(lite_commitment, &lite_secret, &context, &lite_msg)
					.expect("lite proof should build");
			let lite_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					lite_proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			assert_ok!(lite_tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				lite_call,
				&RuntimeCall::Resources(
					crate::Call::set_friend_request_statement_account_for_sequence {
						reference,
						account_id: lite_account.clone(),
					},
				)
				.get_dispatch_info(),
				0,
				extension_version,
			));

			assert_eq!(
				FriendRequestAliasByAccount::<Test>::get(&people_account),
				Some(people_alias)
			);
			assert_eq!(FriendRequestAliasByAccount::<Test>::get(&lite_account), Some(lite_alias));
			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(people_alias));
			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(lite_alias));
		});
	}

	#[test]
	fn collection_based_lite_friend_request_end_to_end_via_extension() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let now = 123u64;
			set_time_sec(now);

			// Set up a lite-people collection and add a member.
			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([40u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![member],
			));
			advance_to_block(3);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);
			let extension_version = 0u8;
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(400),
				},
			);
			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (proof, alias) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			assert_ok!(tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				call,
				&RuntimeCall::Resources(
					crate::Call::set_friend_request_statement_account_for_sequence {
						reference,
						account_id: id_to_account(400),
					},
				)
				.get_dispatch_info(),
				0,
				extension_version,
			));

			assert_eq!(FriendRequestAliasByAccount::<Test>::get(id_to_account(400)), Some(alias));
			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(alias));
			assert_eq!(
				get_allowance(id_to_account(400)),
				<Test as Config>::FriendRequestAllowance::get()
			);
		});
	}

	#[test]
	fn collection_based_wrong_collection_proof_rejected() {
		new_test_ext().execute_with(|| {
			let now = 123u64;
			set_time_sec(now);

			// Set up a people collection (full persons) with a member.
			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([41u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(People::force_recognize_personhood(RuntimeOrigin::root(), vec![member]));
			advance_to_block(3);

			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);
			let extension_version = 0u8;
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: id_to_account(401),
				},
			);
			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			// Submit with LitePeople collection, but the proof is from the People ring.
			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				sp_runtime::transaction_validity::TransactionSource::External,
				extension_version,
			);

			assert!(
				matches!(
					result,
					Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
						sp_runtime::transaction_validity::InvalidTransaction::BadProof
					))
				),
				"people proof submitted as LitePeople should be rejected"
			);
		});
	}

	#[test]
	fn collection_based_friend_request_cleanup_works_for_lite_registration_via_extension() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let now = 123u64;
			set_time_sec(now);

			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([42u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![member],
			));
			advance_to_block(3);

			let stmt_account = id_to_account(310);
			let reference = FriendRequestReference {
				period: Resources::friend_request_period_from_timestamp(now),
				seq: 1,
			};
			let context = Resources::friend_request_context(reference);
			let extension_version = 0u8;
			let call = RuntimeCall::Resources(
				crate::Call::set_friend_request_statement_account_for_sequence {
					reference,
					account_id: stmt_account.clone(),
				},
			);
			let msg = sp_runtime::traits::TxBaseImplication((extension_version, &call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (proof, alias) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");
			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			assert_ok!(tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				call,
				&RuntimeCall::Resources(
					crate::Call::set_friend_request_statement_account_for_sequence {
						reference,
						account_id: stmt_account.clone(),
					},
				)
				.get_dispatch_info(),
				0,
				extension_version,
			));

			assert!(FriendRequestRegistrationByAlias::<Test>::contains_key(alias));

			// Advance past expiration.
			set_time_sec(
				Resources::friend_request_expiration_time(reference.period).saturating_add(1),
			);
			assert_ok!(Resources::clear_expired_friend_request_sequence(
				frame_system::RawOrigin::Authorized.into(),
				stmt_account.clone(),
				reference.seq,
			));

			// Cleanup should have removed the registration.
			assert_eq!(FriendRequestAliasByAccount::<Test>::get(&stmt_account), None);
			assert!(!FriendRequestRegistrationByAlias::<Test>::contains_key(alias));
			assert_eq!(get_allowance(&stmt_account), StatementAllowance::default());
			System::assert_has_event(
				Event::<Test>::FriendRequestStmtUsageRemoved { account: id_to_account(310) }.into(),
			);
		});
	}
}

mod long_term_storage {
	use super::*;
	use crate::types::MembershipCollection;
	use indiv_support::utils::BigEndianU32;

	#[test]
	fn claim_extension_accepts_real_proof_and_rejects_invalid_or_expired_intents() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);
			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([71u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(People::force_recognize_personhood(
				RuntimeOrigin::root(),
				vec![member.clone()],
			));
			advance_to_block(3);
			set_time_sec(3 * day_secs + 100);
			let revision = Members::ring_revision(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0)
				.expect("recognized ring has a revision");
			let ring_members =
				Members::ring_members(indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let extension_version = 0u8;
			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());

			let claim_account = id_to_account(700);
			let call = RuntimeCall::Resources(crate::Call::claim_long_term_storage {
				period,
				counter: 0,
				account_id: claim_account.clone(),
			});
			let inherited = sp_runtime::traits::TxBaseImplication((extension_version, &call));
			let context = Resources::long_term_storage_context(period, 0);
			let (_, alias) = MockCrypto::create(commitment.clone(), &secret, &context, &[0u8; 32])
				.expect("alias preimage should build");
			let msg = (
				crate::extension::DIRECT_LONG_TERM_STORAGE_DOMAIN,
				&claim_account,
				&claim_account,
				alias,
				&MembershipCollection::People,
				0u32,
				revision,
				context,
				&call,
				inherited,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, proof_alias) =
				MockCrypto::create(commitment.clone(), &secret, &context, &msg)
					.expect("proof should build");
			assert_eq!(proof_alias, alias);
			let binding = indiv_support::traits::RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias { context, alias },
			};
			indiv_pallet_people::AccountToAlias::<Test>::insert(&claim_account, &binding);
			indiv_pallet_people::AliasToAccount::<Test>::insert(&binding.ca, &claim_account);
			let extension = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::ClaimLongTermStorage(
					proof,
					0,
					revision,
					MembershipCollection::People,
				),
			));
			for rejected_origin in
				[SystemOrigin::None.into(), SystemOrigin::Signed(id_to_account(799)).into()]
			{
				assert!(matches!(
					extension.clone().validate_only(
						rejected_origin,
						&call,
						&call.get_dispatch_info(),
						call.encoded_size(),
						TransactionSource::External,
						extension_version,
					),
					Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
						InvalidTransaction::BadSigner
					))
				));
			}
			assert_ok!(extension.dispatch_transaction(
				SystemOrigin::Signed(claim_account).into(),
				call.clone(),
				&call.get_dispatch_info(),
				call.encoded_size(),
				extension_version,
			));
			assert_eq!(StorageClaims::<Test>::count(), 1);

			let invalid_call = RuntimeCall::Resources(crate::Call::claim_long_term_storage {
				period,
				counter: 1,
				account_id: id_to_account(701),
			});
			let wrong_call = RuntimeCall::Resources(crate::Call::claim_long_term_storage {
				period,
				counter: 1,
				account_id: id_to_account(702),
			});
			let invalid_account = id_to_account(701);
			let wrong_inherited =
				sp_runtime::traits::TxBaseImplication((extension_version, &wrong_call));
			let invalid_context = Resources::long_term_storage_context(period, 1);
			let (_, invalid_alias) =
				MockCrypto::create(commitment.clone(), &secret, &invalid_context, &[1u8; 32])
					.expect("invalid alias preimage should build");
			let invalid_binding = indiv_support::traits::RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias {
					context: invalid_context,
					alias: invalid_alias,
				},
			};
			indiv_pallet_people::AccountToAlias::<Test>::insert(&invalid_account, &invalid_binding);
			indiv_pallet_people::AliasToAccount::<Test>::insert(
				&invalid_binding.ca,
				&invalid_account,
			);
			let wrong_msg = (
				crate::extension::DIRECT_LONG_TERM_STORAGE_DOMAIN,
				&id_to_account(702),
				&id_to_account(702),
				invalid_alias,
				&MembershipCollection::People,
				0u32,
				revision,
				invalid_context,
				&wrong_call,
				wrong_inherited,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (invalid_proof, _) =
				MockCrypto::create(commitment.clone(), &secret, &invalid_context, &wrong_msg)
					.expect("invalid-vector proof should build");
			let invalid_extension = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::ClaimLongTermStorage(
					invalid_proof,
					0,
					revision,
					MembershipCollection::People,
				),
			));
			assert!(matches!(
				invalid_extension.validate_only(
					SystemOrigin::Signed(invalid_account).into(),
					&invalid_call,
					&invalid_call.get_dispatch_info(),
					invalid_call.encoded_size(),
					TransactionSource::External,
					extension_version,
				),
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					InvalidTransaction::BadProof
				))
			));

			let stale_call = RuntimeCall::Resources(crate::Call::claim_long_term_storage {
				period: 0,
				counter: 2,
				account_id: id_to_account(703),
			});
			let stale_account = id_to_account(703);
			let stale_inherited =
				sp_runtime::traits::TxBaseImplication((extension_version, &stale_call));
			let stale_context = Resources::long_term_storage_context(0, 2);
			let stale_msg = (
				crate::extension::DIRECT_LONG_TERM_STORAGE_DOMAIN,
				&stale_account,
				&stale_account,
				id_to_alias(703),
				&MembershipCollection::People,
				0u32,
				revision,
				stale_context,
				&stale_call,
				stale_inherited,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (stale_proof, _) =
				MockCrypto::create(commitment, &secret, &stale_context, &stale_msg)
					.expect("stale-vector proof should build");
			let stale_extension = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::ClaimLongTermStorage(
					stale_proof,
					0,
					revision,
					MembershipCollection::People,
				),
			));
			assert!(matches!(
				stale_extension.validate_only(
					SystemOrigin::Signed(stale_account).into(),
					&stale_call,
					&stale_call.get_dispatch_info(),
					stale_call.encoded_size(),
					TransactionSource::External,
					extension_version,
				),
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					InvalidTransaction::Custom(code)
				)) if code == crate::extension::CustomValidity::InvalidLongTermStoragePeriod as u8
			));
		});
	}

	#[test]
	fn claim_for_people_succeeds() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let counter = 0u8;
			let target_account = id_to_account(99);
			let origin = lts_people_origin(7);

			assert_ok!(Resources::claim_long_term_storage(
				origin,
				period,
				counter,
				target_account.clone(),
			));

			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7)
			));

			System::assert_has_event(
				Event::<Test>::LongTermStorageReserved {
					reservation_id: 0,
					alias: id_to_alias(7),
					period,
					counter,
					account: target_account,
					collection: MembershipCollection::People,
				}
				.into(),
			);
		});
	}

	#[test]
	fn claim_for_lite_people_succeeds() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let counter = 0u8;
			let target_account = id_to_account(99);
			let origin = lts_lite_people_origin(7);

			assert_ok!(Resources::claim_long_term_storage(
				origin,
				period,
				counter,
				target_account.clone(),
			));

			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7)
			));

			System::assert_has_event(
				Event::<Test>::LongTermStorageReserved {
					reservation_id: 0,
					alias: id_to_alias(7),
					period,
					counter,
					account: target_account,
					collection: MembershipCollection::LitePeople,
				}
				.into(),
			);
		});
	}

	#[test]
	fn double_claim_same_period_and_counter_rejected() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let counter = 0u8;
			let target_account = id_to_account(99);

			// First claim succeeds.
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period,
				counter,
				target_account.clone(),
			));

			// The alias is now marked as spent — the extension would reject a second claim.
			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7)
			));
		});
	}

	#[test]
	fn different_counter_same_period_succeeds() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());

			// Alias for counter 0 is different from alias for counter 1 (different context),
			// so we use different alias_ids to simulate this.
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period,
				0,
				id_to_account(99),
			));

			// Different alias (simulating a different counter producing a different alias).
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(8),
				period,
				1,
				id_to_account(99),
			));

			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7)
			));
			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(8)
			));
		});
	}

	#[test]
	fn claim_in_new_period_succeeds_after_previous() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period_3 =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period_3,
				0,
				id_to_account(99),
			));

			// Advance to next period.
			set_time_sec(4 * day_secs + 100);
			let period_4 =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			assert_ne!(period_3, period_4);

			// Same alias_id is fine in a different period because the real alias would be
			// different (different context due to different period). We simulate this with
			// a fresh origin.
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period_4,
				0,
				id_to_account(99),
			));
		});
	}

	#[test]
	fn invalid_period_rejected() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let stale_period = 0u32;
			assert!(!Resources::is_accepted_long_term_storage_period(stale_period));
		});
	}

	#[test]
	fn previous_period_accepted_within_grace_window() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			let grace_secs: u64 = <Test as Config>::LongTermStorageGraceWindow::get() as u64;

			// Set time to just after period boundary but within grace.
			set_time_sec(4 * day_secs + grace_secs / 2);

			let previous_period = 3u32;
			assert!(Resources::is_accepted_long_term_storage_period(previous_period));
		});
	}

	#[test]
	fn cleanup_expired_period_authorize_succeeds() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;

			let period = 3u32;
			SpentLongTermStorageAliases::<Test>::insert(
				BigEndianU32::from(period),
				id_to_alias(1),
				(),
			);
			SpentLongTermStorageAliases::<Test>::insert(
				BigEndianU32::from(period),
				id_to_alias(2),
				(),
			);

			// Advance time past period end + grace window.
			let grace_secs: u64 = <Test as Config>::LongTermStorageGraceWindow::get() as u64;
			set_time_sec(4 * day_secs + grace_secs + 1);

			// Verify authorization succeeds.
			let call =
				crate::Call::<Test>::clear_expired_long_term_storage_aliases { period, limit: 100 };
			let result = call.authorize(TransactionSource::External);
			assert!(result.is_some());
			assert!(result.unwrap().is_ok());
		});
	}

	#[test]
	fn cleanup_authorize_fails_when_period_not_expired() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let current_period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());

			let call = crate::Call::<Test>::clear_expired_long_term_storage_aliases {
				period: current_period,
				limit: 100,
			};
			let result = call.authorize(TransactionSource::External);
			assert_eq!(
				result,
				Some(Err(InvalidTransaction::Custom(
					crate::extension::CustomValidity::LongTermStoragePeriodNotExpired as u8
				)
				.into()))
			);
		});
	}

	#[test]
	fn cleanup_current_period_fails() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let current_period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			assert!(Resources::validate_clear_long_term_storage_period(current_period).is_err());
		});
	}

	#[test]
	fn cleanup_recent_period_within_grace_fails() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			let grace_secs: u64 = <Test as Config>::LongTermStorageGraceWindow::get() as u64;

			// We're in period 4, within grace window of period 3's end.
			set_time_sec(4 * day_secs + grace_secs / 2);

			let previous_period = 3u32;
			assert!(Resources::validate_clear_long_term_storage_period(previous_period).is_err());
		});
	}

	#[test]
	fn context_construction_is_deterministic() {
		let ctx1 = Resources::long_term_storage_context(5, 3);
		let ctx2 = Resources::long_term_storage_context(5, 3);
		assert_eq!(ctx1, ctx2);

		// Different period produces different context.
		let ctx3 = Resources::long_term_storage_context(6, 3);
		assert_ne!(ctx1, ctx3);

		// Different counter produces different context.
		let ctx4 = Resources::long_term_storage_context(5, 4);
		assert_ne!(ctx1, ctx4);
	}

	#[test]
	fn bad_origin_rejected() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());

			// Using a friend request origin should fail.
			assert_noop!(
				Resources::claim_long_term_storage(
					friend_request_origin(7),
					period,
					0,
					id_to_account(99),
				),
				sp_runtime::DispatchError::BadOrigin
			);

			// Using a signed origin should fail.
			assert_noop!(
				Resources::claim_long_term_storage(
					RuntimeOrigin::signed(id_to_account(1)),
					period,
					0,
					id_to_account(99),
				),
				sp_runtime::DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn claim_rolls_back_alias_and_reservation_on_allocation_failure() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let counter = 0u8;
			let target_account = id_to_account(99);

			crate::mock::BULLETIN_STORAGE_SHOULD_FAIL.with(|f| f.set(true));

			assert_noop!(
				Resources::claim_long_term_storage(
					lts_people_origin(7),
					period,
					counter,
					target_account.clone(),
				),
				Error::<Test>::ReservationBackendFailed
			);

			assert!(!SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7)
			));
			assert_eq!(NextStorageReservationId::<Test>::get(), 0);
			assert!(StorageClaims::<Test>::get(0).is_none());

			crate::mock::BULLETIN_STORAGE_SHOULD_FAIL.with(|f| f.set(false));
		});
	}

	#[test]
	fn cleanup_authorize_rejects_when_nothing_to_clear() {
		new_test_ext().execute_with(|| {
			let day_secs: u64 = 24 * 60 * 60;
			let grace_secs: u64 = <Test as Config>::LongTermStorageGraceWindow::get() as u64;

			// Advance past period end + grace so the period-expiry check passes.
			set_time_sec(4 * day_secs + grace_secs + 1);

			// No entries inserted for the expired period.
			let period = 3u32;
			let call =
				crate::Call::<Test>::clear_expired_long_term_storage_aliases { period, limit: 100 };
			let result = call.authorize(TransactionSource::External);
			assert_eq!(
				result,
				Some(Err(InvalidTransaction::Custom(
					crate::extension::CustomValidity::NothingToClearForLongTermStoragePeriod as u8
				)
				.into()))
			);
		});
	}

	#[test]
	fn offchain_worker_submits_cleanup_for_stale_period() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			let day_secs: u64 = 24 * 60 * 60;
			set_time_sec(3 * day_secs + 100);

			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let counter = 0u8;
			let target_account = id_to_account(99);

			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period,
				counter,
				target_account.clone(),
			));

			// Run the OCW while the entry is still in its current period — it should NOT
			// submit a cleanup tx because nothing is yet clearable.
			advance_to_block(2);
			assert!(SpentLongTermStorageAliases::<Test>::contains_key(
				BigEndianU32::from(period),
				id_to_alias(7),
			));

			// Advance past period end + grace so the OCW triggers cleanup.
			let grace_secs: u64 = <Test as Config>::LongTermStorageGraceWindow::get() as u64;
			let period_duration: u64 =
				<Test as Config>::LongTermStoragePeriodDuration::get() as u64;
			set_time_sec((period as u64 + 1) * period_duration + grace_secs + 1);
			advance_to_block(3);

			// The OCW should have submitted a cleanup tx that drained the period.
			assert!(SpentLongTermStorageAliases::<Test>::iter_key_prefix(BigEndianU32::from(
				period
			))
			.next()
			.is_none(),);
			let cleared_emitted = System::events().into_iter().any(|record| {
				matches!(
					record.event,
					RuntimeEvent::Resources(Event::LongTermStorageAliasesCleared { period: p, .. })
						if p == period
				)
			});
			assert!(
				cleared_emitted,
				"expected a LongTermStorageAliasesCleared event for the period"
			);
		});
	}

	#[test]
	fn reservation_is_auditable_and_owner_can_cancel() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(3 * 24 * 60 * 60 + 100);
			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			let owner = id_to_account(99);
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(7),
				period,
				0,
				owner.clone(),
			));
			let claim = StorageClaims::<Test>::get(0).unwrap();
			assert_eq!(claim.owner, owner);
			assert_eq!(StorageReservationByPurpose::<Test>::get(&claim.purpose), Some(0));
			assert_eq!(NextStorageReservationId::<Test>::get(), 1);
			assert_noop!(
				Resources::cancel_long_term_storage_reservation(
					RuntimeOrigin::signed(id_to_account(100)),
					0,
				),
				Error::<Test>::NotReservationOwner
			);
			assert_ok!(Resources::cancel_long_term_storage_reservation(
				RuntimeOrigin::signed(owner.clone()),
				0,
			));
			System::assert_last_event(
				Event::<Test>::LongTermStorageReservationCancelled {
					reservation_id: 0,
					account: owner,
				}
				.into(),
			);
			// Audit rows remain until Bulletin prunes the matching tombstone.
			assert!(StorageClaims::<Test>::contains_key(0));
		});
	}

	#[test]
	fn expiry_is_bounded_and_claim_cleanup_is_infallible() {
		use indiv_support::traits::ResourceClaimLifecycle;

		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(3 * 24 * 60 * 60 + 100);
			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			assert_ok!(Resources::claim_long_term_storage(
				lts_lite_people_origin(9),
				period,
				0,
				id_to_account(99),
			));
			assert_noop!(
				Resources::expire_long_term_storage_reservations(
					RuntimeOrigin::signed(id_to_account(1)),
					51,
				),
				Error::<Test>::CleanupLimitExceeded
			);
			System::set_block_number(101);
			assert_ok!(Resources::expire_long_term_storage_reservations(
				RuntimeOrigin::signed(id_to_account(1)),
				1,
			));
			System::assert_last_event(
				Event::<Test>::LongTermStorageReservationExpired { reservation_id: 0 }.into(),
			);
			let outcome = <Resources as ResourceClaimLifecycle<
				ReservationId,
				ReservationPurpose,
			>>::prune_claim(0);
			assert!(outcome.removed);
			assert!(outcome.purpose.is_some());
			assert!(!StorageClaims::<Test>::contains_key(0));
			assert!(!<Resources as ResourceClaimLifecycle<
				ReservationId,
				ReservationPurpose,
			>>::prune_claim(0)
			.removed);
		});
	}

	#[test]
	fn claim_cleanup_reports_reverse_mapping_mismatch_without_failing() {
		use indiv_support::traits::ResourceClaimLifecycle;

		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(3 * 24 * 60 * 60 + 100);
			let period =
				Resources::long_term_storage_period_from_timestamp(TestClock::now().as_secs());
			assert_ok!(Resources::claim_long_term_storage(
				lts_people_origin(9),
				period,
				0,
				id_to_account(99),
			));
			let purpose = StorageClaims::<Test>::get(0).unwrap().purpose;
			StorageReservationByPurpose::<Test>::insert(&purpose, 99);
			let outcome = <Resources as ResourceClaimLifecycle<
				ReservationId,
				ReservationPurpose,
			>>::prune_claim(0);
			assert!(!outcome.removed);
			assert_eq!(outcome.purpose, Some(purpose.clone()));
			assert_eq!(StorageReservationByPurpose::<Test>::get(purpose), Some(99));
		});
	}
}

mod stmt_store_allowance {
	use super::*;
	use indiv_support::utils::BigEndianU32;

	#[test]
	fn claim_succeeds_and_grants_allowance() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(SECONDS_PER_DAY + 100);

			let now = TestClock::now().as_secs();
			let period = Resources::stmt_store_period_from_timestamp(now);
			let seq = 0u32;
			let alias = id_to_alias(50);
			let origin = stmt_store_slot_origin(50);
			let target = id_to_account(500);

			let accounts_allowance = <Test as Config>::AccountsApiAllowance::get();

			assert_eq!(get_allowance(&target), StatementAllowance::default());

			assert_ok!(
				Resources::set_statement_store_account(origin, period, seq, target.clone(),)
			);

			assert_eq!(get_allowance(&target), accounts_allowance);
			let entry = StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), alias)
				.expect("allowance entry should exist");
			assert_eq!(entry.account_id, target);
			assert_eq!(entry.seq, seq);
			assert_eq!(
				StmtStoreAllowanceByAccount::<Test>::get(
					&target,
					(BigEndianU32::from(period), seq, alias),
				),
				Some(()),
			);

			System::assert_has_event(
				Event::<Test>::StmtStoreAllowanceSet { alias, period, seq, account: target }.into(),
			);
		});
	}

	#[test]
	fn extension_rejects_replacement_during_cooldown() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let now = TestClock::now().as_secs();
			let period = Resources::stmt_store_period_from_timestamp(now);

			// Set up a lite-people ring with one member.
			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([51u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![member],
			));
			advance_to_block(3);
			set_time_sec(SECONDS_PER_DAY + 100);

			let context = Resources::stmt_store_slot_context(period, 0);
			let extension_version = 0u8;

			// First claim: dispatch through the extension.
			let first_call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period,
				seq: 0,
				target_account: id_to_account(501),
			});
			let first_msg = sp_runtime::traits::TxBaseImplication((extension_version, &first_call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (first_proof, _) =
				MockCrypto::create(commitment.clone(), &secret, &context, &first_msg)
					.expect("first proof should build");
			let first_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					first_proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			assert_ok!(first_tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				first_call.clone(),
				&first_call.get_dispatch_info(),
				0,
				extension_version,
			));

			// Second claim with a different target account: extension should reject.
			let second_call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period,
				seq: 0,
				target_account: id_to_account(502),
			});
			let second_msg =
				sp_runtime::traits::TxBaseImplication((extension_version, &second_call))
					.using_encoded(sp_io::hashing::blake2_256);
			let (second_proof, _) = MockCrypto::create(commitment, &secret, &context, &second_msg)
				.expect("second proof should build");
			let second_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					second_proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			let result = second_tx_ext.validate_only(
				SystemOrigin::None.into(),
				&second_call,
				&second_call.get_dispatch_info(),
				0,
				TransactionSource::External,
				extension_version,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::StmtStoreReplacementTooEarly as u8
			));
		});
	}

	#[test]
	fn extension_accepts_replacement_after_cooldown() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let now = TestClock::now().as_secs();
			let period = Resources::stmt_store_period_from_timestamp(now);

			// Set up a lite-people ring with one member.
			assert_ok!(Members::create_collection(
				0,
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				1,
				indiv_pallet_members::RingMode::Flexible,
				indiv_support::traits::RingExponent::R2e9,
				None,
			));
			let secret = MockCrypto::new_secret([55u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			assert_ok!(Members::add_members(
				indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				vec![member],
			));
			advance_to_block(3);
			set_time_sec(SECONDS_PER_DAY + 100);

			let context = Resources::stmt_store_slot_context(period, 0);
			let extension_version = 0u8;

			// First claim through the extension.
			let first_call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period,
				seq: 0,
				target_account: id_to_account(550),
			});
			let first_msg = sp_runtime::traits::TxBaseImplication((extension_version, &first_call))
				.using_encoded(sp_io::hashing::blake2_256);
			let ring_members =
				Members::ring_members(indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
			let commitment = MockCrypto::open((), &member, ring_members.into_iter())
				.expect("commitment should open");
			let (first_proof, _) =
				MockCrypto::create(commitment.clone(), &secret, &context, &first_msg)
					.expect("first proof should build");
			let first_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					first_proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			assert_ok!(first_tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				first_call.clone(),
				&first_call.get_dispatch_info(),
				0,
				extension_version,
			));

			// Advance past the replacement cooldown.
			let cooldown =
				<<Test as Config>::StmtStoreReplacementCooldown as Get<u32>>::get() as u64;
			advance_time_sec(cooldown + 1);

			// Replacement claim should now succeed through the extension.
			let second_call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period,
				seq: 0,
				target_account: id_to_account(551),
			});
			let second_msg =
				sp_runtime::traits::TxBaseImplication((extension_version, &second_call))
					.using_encoded(sp_io::hashing::blake2_256);
			let (second_proof, _) = MockCrypto::create(commitment, &secret, &context, &second_msg)
				.expect("second proof should build");
			let second_tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					second_proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));

			assert_ok!(second_tx_ext.dispatch_transaction(
				SystemOrigin::None.into(),
				second_call.clone(),
				&second_call.get_dispatch_info(),
				0,
				extension_version,
			));

			// Old account allowance is gone, new account has it.
			let accounts_allowance = <Test as Config>::AccountsApiAllowance::get();
			assert_eq!(get_allowance(id_to_account(550)), StatementAllowance::default());
			assert_eq!(get_allowance(id_to_account(551)), accounts_allowance);
		});
	}

	#[test]
	fn extension_rejects_invalid_period() {
		new_test_ext().execute_with(|| {
			set_time_sec(3 * SECONDS_PER_DAY + 100);

			let stale_period =
				Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs())
					.checked_sub(2)
					.expect("must be able to go this far back");
			let context = Resources::stmt_store_slot_context(stale_period, 0);

			// Build a fake proof — the extension rejects the period before proof verification.
			let secret = MockCrypto::new_secret([52u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			let commitment = MockCrypto::open((), &member, core::iter::once(member))
				.expect("commitment should open");
			let msg = [0u8; 32];
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			let call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period: stale_period,
				seq: 0,
				target_account: id_to_account(503),
			});

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				TransactionSource::External,
				0,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::InvalidStmtStorePeriod as u8
			));
		});
	}

	#[test]
	fn extension_rejects_previous_period_for_claiming() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let previous_period =
				Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs())
					.checked_sub(1)
					.expect("must be able to go this far back");
			let context = Resources::stmt_store_slot_context(previous_period, 1);

			let secret = MockCrypto::new_secret([53u8; 32]);
			let member = MockCrypto::member_from_secret(&secret);
			let commitment = MockCrypto::open((), &member, core::iter::once(member))
				.expect("commitment should open");
			let msg = [0u8; 32];
			let (proof, _) = MockCrypto::create(commitment, &secret, &context, &msg)
				.expect("proof should build");

			let tx_ext = crate::extension::AsResources::<Test>::new(Some(
				crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
					proof,
					0,
					crate::types::MembershipCollection::LitePeople,
				),
			));
			let call = RuntimeCall::Resources(crate::Call::set_statement_store_account {
				period: previous_period,
				seq: 1,
				target_account: id_to_account(504),
			});

			let result = tx_ext.validate_only(
				SystemOrigin::None.into(),
				&call,
				&call.get_dispatch_info(),
				0,
				TransactionSource::External,
				0,
			);

			assert!(matches!(
				result,
				Err(sp_runtime::transaction_validity::TransactionValidityError::Invalid(
					sp_runtime::transaction_validity::InvalidTransaction::Custom(code),
				)) if code == crate::extension::CustomValidity::InvalidStmtStorePeriod as u8
			));
		});
	}

	#[test]
	fn cleanup_blocked_during_grace_window() {
		new_test_ext().execute_with(|| {
			let grace_secs: u64 =
				<<Test as Config>::StmtStoreGraceWindow as Get<u32>>::get() as u64;
			set_time_sec(SECONDS_PER_DAY + 1);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(54),
				period,
				0,
				id_to_account(505),
			));

			// Advance to the next period but still within the grace window.
			// Period `period` ends at `(period + 1) * SECONDS_PER_DAY`. Clearable after
			// that + grace_secs.
			let period_end = (period as u64 + 1) * SECONDS_PER_DAY;
			set_time_sec(period_end + grace_secs);

			let first_alias = id_to_alias(54);
			let call = crate::Call::<Test>::clear_expired_stmt_store_allowances {
				period,
				first_entry: first_alias,
			};
			let result = call.authorize(TransactionSource::InBlock);
			assert_eq!(
				result,
				Some(Err(InvalidTransaction::Custom(
					crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup as u8
				)
				.into()))
			);

			// Advance past the grace window — now cleanup should work.
			set_time_sec(period_end + grace_secs + 1);

			assert_ok!(Resources::clear_expired_stmt_store_allowances(
				frame_system::RawOrigin::Authorized.into(),
				period,
				first_alias,
			));
		});
	}

	#[test]
	fn rejects_bad_origin() {
		new_test_ext().execute_with(|| {
			set_time_sec(100);
			let period = Resources::stmt_store_period_from_timestamp(100);

			assert_noop!(
				Resources::set_statement_store_account(
					RuntimeOrigin::signed(id_to_account(1)),
					period,
					0,
					id_to_account(600),
				),
				sp_runtime::DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn cleanup_removes_allowances_and_emits_event() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(SECONDS_PER_DAY + 100);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());
			let target_a = id_to_account(600);
			let target_b = id_to_account(601);
			let alias_a = id_to_alias(60);
			let alias_b = id_to_alias(61);

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(60),
				period,
				0,
				target_a.clone(),
			));
			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(61),
				period,
				1,
				target_b.clone(),
			));

			let accounts_allowance = <Test as Config>::AccountsApiAllowance::get();
			assert_eq!(get_allowance(&target_a), accounts_allowance);
			assert_eq!(get_allowance(&target_b), accounts_allowance);

			// Advance past the period end + grace window so cleanup is allowed.
			let grace_secs: u64 =
				<<Test as Config>::StmtStoreGraceWindow as Get<u32>>::get() as u64;
			let period_end = (period as u64 + 1) * SECONDS_PER_DAY;
			set_time_sec(period_end + grace_secs + 1);

			// The first_entry must match the first alias in the period's iteration order.
			let first_alias =
				StatementStoreAllowances::<Test>::iter_prefix(BigEndianU32::from(period))
					.next()
					.expect("entries exist")
					.0;

			assert_ok!(Resources::clear_expired_stmt_store_allowances(
				frame_system::RawOrigin::Authorized.into(),
				period,
				first_alias,
			));

			assert_eq!(
				StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), alias_a),
				None
			);
			assert_eq!(
				StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), alias_b),
				None
			);
			// Reverse lookups should also be cleared.
			assert!(StmtStoreAllowanceByAccount::<Test>::iter_prefix(&target_a).next().is_none());
			assert!(StmtStoreAllowanceByAccount::<Test>::iter_prefix(&target_b).next().is_none());
			assert_eq!(get_allowance(&target_a), StatementAllowance::default());
			assert_eq!(get_allowance(&target_b), StatementAllowance::default());

			System::assert_has_event(
				Event::<Test>::StmtStoreAllowancesCleared {
					period,
					first_key: first_alias,
					count: 2,
				}
				.into(),
			);
		});
	}

	#[test]
	fn cleanup_authorize_rejects_current_period() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());

			let call = crate::Call::<Test>::clear_expired_stmt_store_allowances {
				period,
				first_entry: id_to_alias(0),
			};
			let result = call.authorize(TransactionSource::InBlock);

			assert_eq!(
				result,
				Some(Err(InvalidTransaction::Custom(
					crate::extension::CustomValidity::InvalidExpiredStmtStoreCleanup as u8
				)
				.into()))
			);
		});
	}

	#[test]
	fn cleanup_authorize_rejects_external_source() {
		new_test_ext().execute_with(|| {
			let grace_secs: u64 =
				<<Test as Config>::StmtStoreGraceWindow as Get<u32>>::get() as u64;
			set_time_sec(SECONDS_PER_DAY + 100);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(80),
				period,
				0,
				id_to_account(800),
			));

			// Advance past grace — would be clearable if source were local.
			let period_end = (period as u64 + 1) * SECONDS_PER_DAY;
			set_time_sec(period_end + grace_secs + 1);

			let call = crate::Call::<Test>::clear_expired_stmt_store_allowances {
				period,
				first_entry: id_to_alias(80),
			};

			// External source is rejected with BadSigner.
			let result = call.authorize(TransactionSource::External);
			assert_eq!(result, Some(Err(InvalidTransaction::BadSigner.into())));

			// Local source should succeed.
			let result = call.authorize(TransactionSource::Local);
			assert!(result.unwrap().is_ok());
		});
	}

	#[test]
	fn offchain_worker_submits_cleanup_for_stale_period() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());
			let target = id_to_account(700);
			let accounts_allowance = <Test as Config>::AccountsApiAllowance::get();

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(70),
				period,
				0,
				target.clone(),
			));

			// Run the OCW while the entry is still in its current period — it should NOT
			// submit a cleanup tx because nothing is yet clearable.
			advance_to_block(2);
			assert_eq!(
				StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), id_to_alias(70)),
				Some(crate::types::StmtStoreAllowanceEntry {
					account_id: target.clone(),
					seq: 0,
					since: TestClock::now().as_secs(),
				}),
			);
			assert_eq!(get_allowance(&target), accounts_allowance);

			// Advance past the period end + grace window so the offchain worker triggers cleanup.
			let grace_secs: u64 =
				<<Test as Config>::StmtStoreGraceWindow as Get<u32>>::get() as u64;
			let period_end = (period as u64 + 1) * SECONDS_PER_DAY;
			set_time_sec(period_end + grace_secs + 1);
			advance_to_block(3);

			// The offchain worker should have submitted a cleanup tx which was applied.
			assert_eq!(
				StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), id_to_alias(70)),
				None,
			);
			assert_eq!(get_allowance(&target), StatementAllowance::default());
		});
	}

	#[test]
	fn context_layout_is_fixed_and_non_truncating() {
		new_test_ext().execute_with(|| {
			let period = 0x0102_0304u32;
			let seq = 0x05060708u32;
			let context = Resources::stmt_store_slot_context(period, seq);
			let prefix = b"SSS_SLOT:";

			assert_eq!(&context[..prefix.len()], prefix);
			assert_eq!(&context[prefix.len()..prefix.len() + 4], &period.to_be_bytes());
			assert_eq!(&context[prefix.len() + 4..prefix.len() + 8], &seq.to_be_bytes());
			assert!(
				context[prefix.len() + 8..].iter().all(|b| *b == b' '),
				"remaining context bytes should stay as padding",
			);
		});
	}

	#[test]
	fn period_from_timestamp_uses_days() {
		new_test_ext().execute_with(|| {
			assert_eq!(Resources::stmt_store_period_from_timestamp(0), 0);
			assert_eq!(Resources::stmt_store_period_from_timestamp(86_399), 0);
			assert_eq!(Resources::stmt_store_period_from_timestamp(86_400), 1);
			assert_eq!(Resources::stmt_store_period_from_timestamp(86_401), 1);
			assert_eq!(Resources::stmt_store_period_from_timestamp(2 * 86_400), 2);
		});
	}

	#[test]
	fn replacement_within_cooldown_is_rejected() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);
			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(90),
				period,
				0,
				id_to_account(900),
			));

			// Same alias trying to replace before cooldown elapses.
			let cooldown =
				<<Test as Config>::StmtStoreReplacementCooldown as Get<u32>>::get() as u64;
			advance_time_sec(cooldown - 1);

			assert_noop!(
				Resources::set_statement_store_account(
					stmt_store_slot_origin(90),
					period,
					0,
					id_to_account(901),
				),
				Error::<Test>::StmtStoreReplacementTooEarly
			);
		});
	}

	#[test]
	fn replacement_after_cooldown_succeeds_and_swaps_allowance() {
		new_test_ext().execute_with(|| {
			System::set_block_number(1);
			set_time_sec(SECONDS_PER_DAY + 100);
			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());
			let alias = id_to_alias(91);

			let target_a = id_to_account(910);
			let target_b = id_to_account(911);

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(91),
				period,
				0,
				target_a.clone(),
			));

			let accounts_allowance = <Test as Config>::AccountsApiAllowance::get();
			assert_eq!(get_allowance(&target_a), accounts_allowance);

			// Advance past the cooldown.
			let cooldown =
				<<Test as Config>::StmtStoreReplacementCooldown as Get<u32>>::get() as u64;
			advance_time_sec(cooldown + 1);

			assert_ok!(Resources::set_statement_store_account(
				stmt_store_slot_origin(91),
				period,
				1,
				target_b.clone(),
			));

			// Old account's allowance is revoked, new account's is granted.
			assert_eq!(get_allowance(&target_a), StatementAllowance::default());
			assert_eq!(get_allowance(&target_b), accounts_allowance);

			// Reverse lookup for the old account/seq is gone; new entry exists.
			assert_eq!(
				StmtStoreAllowanceByAccount::<Test>::get(
					&target_a,
					(BigEndianU32::from(period), 0u32, alias),
				),
				None,
			);
			assert_eq!(
				StmtStoreAllowanceByAccount::<Test>::get(
					&target_b,
					(BigEndianU32::from(period), 1u32, alias),
				),
				Some(()),
			);

			// Stored entry now points at target_b with seq 1.
			let entry =
				StatementStoreAllowances::<Test>::get(BigEndianU32::from(period), alias).unwrap();
			assert_eq!(entry.account_id, target_b);
			assert_eq!(entry.seq, 1);
		});
	}

	#[test]
	fn clearing_respects_cleanup_limit() {
		new_test_ext().execute_with(|| {
			set_time_sec(SECONDS_PER_DAY + 100);

			let period = Resources::stmt_store_period_from_timestamp(TestClock::now().as_secs());
			let limit = <<Test as Config>::StmtStoreCleanupLimit as Get<u32>>::get();
			let total = limit + 1;

			// Insert more entries than the cleanup limit.
			for i in 0..total {
				assert_ok!(Resources::set_statement_store_account(
					stmt_store_slot_origin(100 + i as u64),
					period,
					0,
					id_to_account(1000 + i as u64),
				));
			}

			let period_key = BigEndianU32::from(period);
			assert_eq!(
				StatementStoreAllowances::<Test>::iter_prefix(period_key).count(),
				total as usize,
			);

			// Advance past grace window.
			let grace_secs = <<Test as Config>::StmtStoreGraceWindow as Get<u32>>::get() as u64;
			let period_end = (period as u64 + 1) * SECONDS_PER_DAY;
			set_time_sec(period_end + grace_secs + 1);

			let first_alias =
				StatementStoreAllowances::<Test>::iter_keys().next().expect("entries exist").1;

			let last_alias =
				StatementStoreAllowances::<Test>::iter_keys().last().expect("must exist").1;

			// First clear should remove exactly `limit` entries.
			assert_ok!(Resources::clear_expired_stmt_store_allowances(
				frame_system::RawOrigin::Authorized.into(),
				period,
				first_alias,
			));

			let aliases_left: Vec<_> = StatementStoreAllowances::<Test>::iter_prefix(period_key)
				.map(|(alias, _)| alias)
				.collect();
			assert_eq!(aliases_left.len(), (total - limit) as usize);
			assert_eq!(aliases_left, vec![last_alias]);
		});
	}
}
