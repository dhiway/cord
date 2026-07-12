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
	bench_helpers::prove_vote,
	extension::{VoterAuth, VoterAuthData},
	inspect::Score,
	mock::*,
	pallet::Pallet as HonourPallet,
	*,
};
use codec::Encode;
use frame_support::{
	assert_err, assert_noop, assert_ok,
	dispatch::{GetDispatchInfo, PostDispatchInfo},
	pallet_prelude::{InvalidTransaction, TransactionValidityError},
	traits::UnixTime,
};
use frame_system::RawOrigin;
use mock::set_time;
use sp_io::hashing::blake2_256;
use sp_runtime::{
	generic::ExtensionVersion,
	traits::{DispatchTransaction, Dispatchable},
};
use verifiable::GenerateVerifiable;

type TestCrypto = crate::CryptoOf<Test>;

fn read_score(subject_id: u8) -> crate::Honour {
	<HonourPallet<Test> as Score>::read(&subject(subject_id))
}

fn make_vote(
	voter: u8,
	point: PointId,
	subject: SubjectId,
	direction: Direction,
) -> (RuntimeOrigin, VoteData) {
	let secret_key = voter_secret_key(voter);

	let subject_ctx = context::subject(&subject);
	let point_ctx = context::point(point);

	let subject_alias = TestCrypto::alias_in_context(&secret_key, &subject_ctx)
		.expect("Failed to create subject alias");
	let point_alias = TestCrypto::alias_in_context(&secret_key, &point_ctx)
		.expect("Failed to create point alias");

	let vote_data = VoteData { subject, point, direction };

	let origin =
		crate::Origin::Voter { aliases: crate::VoteAliases { subject_alias, point_alias } };

	(origin.into(), vote_data)
}

fn get_point_alias(voter: u8, point: PointId) -> PointAlias {
	TestCrypto::alias_in_context(&voter_secret_key(voter), &context::point(point))
		.expect("Failed to create point alias")
}

fn voter_secret_key(voter: u8) -> <TestCrypto as GenerateVerifiable>::Secret {
	TestCrypto::new_secret([voter; 32])
}

fn subject(n: u8) -> SubjectId {
	[n; 32]
}

/// It is allowed to re-use the same subject with the same direction.
#[test]
fn bestow_reuse_can_reuse_same_subject() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(read_score(1), 0);

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(read_score(1), 0);
	});
}

/// A user may change their vote for a subject to the opposite direction.
#[test]
fn bestow_reuse_can_reuse_same_subject_different_direction() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(read_score(1), 0);

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Dishonourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(read_score(1), -2);

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(read_score(1), 0);
	});
}

#[test]
fn double_spend_impossible() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		let (origin_1, vote_1) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin_1.clone(), vote_1.clone(), 0));

		assert_eq!(read_score(1), 0);
		assert_eq!(read_score(2), -1);

		let (origin_2, vote_2) = make_vote(VOTER, 0, subject(2), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin_2, vote_2, 0));

		assert_eq!(read_score(1), -1);
		assert_eq!(read_score(2), 0);
	});
}

#[test]
fn double_vote_impossible() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		let (origin_1, vote_1) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin_1, vote_1, 0));

		let (origin_2, vote_2) = make_vote(VOTER, 1, subject(1), Direction::Honourable);
		assert_noop!(
			HonourPallet::<Test>::bestow(origin_2, vote_2, 0),
			Error::<Test>::SubjectAlreadyVoted
		);
	});
}

#[test]
fn score_reads_correctly() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		assert_eq!(<HonourPallet::<Test> as Score>::read(&subject(1)), -1);

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));

		assert_eq!(<HonourPallet::<Test> as Score>::read(&subject(1)), 0);

		let (origin, vote) = make_vote(VOTER, 0, subject(2), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));

		assert_eq!(<HonourPallet::<Test> as Score>::read(&subject(1)), -1);
		assert_eq!(<HonourPallet::<Test> as Score>::read(&subject(2)), 0);
	});
}

#[test]
fn events_deposited() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		const VOTER: u8 = 0;

		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));

		System::assert_has_event(
			Event::<Test>::VoteCast { subject: subject(1), direction: Direction::Honourable }
				.into(),
		);
		System::assert_has_event(
			Event::<Test>::HonourChanged { subject: subject(1), old_value: -1, new_value: 0 }
				.into(),
		);

		System::reset_events();

		let (origin, vote) = make_vote(VOTER, 0, subject(2), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));

		System::assert_has_event(
			Event::<Test>::VoteReused {
				old_subject: subject(1),
				old_direction: Direction::Honourable,
				new_subject: subject(2),
				new_direction: Direction::Honourable,
			}
			.into(),
		);
		System::assert_has_event(
			Event::<Test>::HonourChanged { subject: subject(1), old_value: 0, new_value: -1 }
				.into(),
		);
		System::assert_has_event(
			Event::<Test>::HonourChanged { subject: subject(2), old_value: -1, new_value: 0 }
				.into(),
		);
	});
}

#[test]
fn is_vote_frozen_works() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;
		const POINT: u8 = 0;

		let point_alias = get_point_alias(VOTER, POINT);

		let now = 2;
		set_time(now);

		assert!(!HonourPallet::<Test>::is_point_frozen(&point_alias, now));

		let (origin, vote) = make_vote(VOTER, POINT, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));

		assert!(HonourPallet::<Test>::is_point_frozen(&point_alias, now));

		let now = now + PointFreezeDuration::get() - 1;
		set_time(now);

		assert!(HonourPallet::<Test>::is_point_frozen(&point_alias, now), "Still frozen");

		let now = now + 1;
		set_time(now);

		assert!(!HonourPallet::<Test>::is_point_frozen(&point_alias, now), "Not frozen");
	});
}

#[test]
fn validate_vote_works() {
	new_test_ext().execute_with(|| {
		let message = &[0, 1, 2, 3];

		let (_, vote) = make_vote(0, 0, subject(0), Direction::Honourable);
		let proof = prove_vote::<Test>(&vote, 0, message);

		assert_ok!(Pallet::<Test>::validate_vote_proof(&vote, message, &proof, 0, 0));

		let different_message = &[3, 2, 1, 0];
		assert_err!(
			Pallet::<Test>::validate_vote_proof(&vote, different_message, &proof, 0, 0),
			Error::<Test>::InvalidProof
		);
	});
}

#[test]
fn validate_vote_rejects_unknown_revision() {
	new_test_ext().execute_with(|| {
		let message = &[0, 1, 2, 3];

		let (_, vote) = make_vote(0, 0, subject(0), Direction::Honourable);
		let proof = prove_vote::<Test>(&vote, 0, message);

		// The mock ring only exposes revision 0; any other revision must be rejected.
		assert_err!(
			Pallet::<Test>::validate_vote_proof(&vote, message, &proof, 0, 1),
			Error::<Test>::InvalidProof
		);
	});
}

#[test]
fn validate_vote_rejects_unknown_ring() {
	new_test_ext().execute_with(|| {
		let message = &[0, 1, 2, 3];

		let (_, vote) = make_vote(0, 0, subject(0), Direction::Honourable);
		let proof = prove_vote::<Test>(&vote, 0, message);

		// The mock only has ring 0; any other ring index must be rejected.
		assert_err!(
			Pallet::<Test>::validate_vote_proof(&vote, message, &proof, 1, 0),
			Error::<Test>::InvalidProof
		);
	});
}

#[test]
fn tx_extension_checks_call_validity_time() {
	new_test_ext().execute_with(|| {
		set_time(CallMortality::get() + 1);

		let now = <Test as Config>::Clock::now().as_secs();
		let vote = VoteData { subject: [1; 32], point: 0, direction: Direction::Honourable };

		let call_bestow = |call_valid_from: Seconds| -> Result<(), TransactionValidityError> {
			let call = Call::bestow { vote: vote.clone(), call_valid_from };
			let call: <Test as frame_system::Config>::RuntimeCall = call.into();
			let ext_version: ExtensionVersion = 0;
			let message = (ext_version, &call).using_encoded(blake2_256);
			let proof = prove_vote::<Test>(&vote, 0, &message);

			let extension: VoterAuth<Test> =
				VoterAuth::new(Some(VoterAuthData { proof, ring_index: 0, revision: 0 }));

			let call: <Test as frame_system::Config>::RuntimeCall = call.into();
			let info = call.get_dispatch_info();
			let len = call.encoded_size();
			let post_info = PostDispatchInfo::default();

			extension
				.test_run(RawOrigin::None.into(), &call, &info, len, 0, |_| Ok(post_info))?
				.unwrap();
			Ok(())
		};

		assert_ok!(call_bestow(now));

		assert_err!(
			call_bestow(now + 1),
			TransactionValidityError::Invalid(InvalidTransaction::Future)
		);

		assert_err!(
			call_bestow(now - CallMortality::get()),
			TransactionValidityError::Invalid(InvalidTransaction::Stale)
		);
	});
}

/// A `bestow` on a point used in the last `PointFreezeDuration` seconds is rejected by the
/// extension's freeze check, even with a fresh mortality window; it thaws after exactly
/// `PointFreezeDuration` seconds.
#[test]
fn tx_extension_rejects_frozen_point() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;
		const POINT: PointId = 0;
		set_time(0);

		let try_bestow = |subject_n: u8| -> Result<(), TransactionValidityError> {
			let now = <Test as Config>::Clock::now().as_secs();
			let vote = VoteData {
				subject: subject(subject_n),
				point: POINT,
				direction: Direction::Honourable,
			};
			let call: <Test as frame_system::Config>::RuntimeCall =
				Call::bestow { vote: vote.clone(), call_valid_from: now }.into();
			let ext_version: ExtensionVersion = 0;
			let message = (ext_version, &call).using_encoded(blake2_256);
			let proof = prove_vote::<Test>(&vote, VOTER, &message);
			let extension =
				VoterAuth::<Test>::new(Some(VoterAuthData { proof, ring_index: 0, revision: 0 }));
			let info = call.get_dispatch_info();
			let len = call.encoded_size();

			extension
				.test_run(RawOrigin::None.into(), &call, &info, len, 0, |origin| {
					call.clone().dispatch(origin)
				})?
				.expect("dispatch should succeed");
			Ok(())
		};
		let frozen = TransactionValidityError::Invalid(InvalidTransaction::Future);

		assert_ok!(try_bestow(1));

		// Past CallMortality, still inside PointFreezeDuration: `call_valid_from = now` is fresh,
		// so only the freeze check can fire.
		set_time(CallMortality::get() + 1);
		assert_err!(try_bestow(2), frozen);

		// One second before the boundary: still frozen.
		set_time(PointFreezeDuration::get() - 1);
		assert_err!(try_bestow(2), frozen);

		// At the boundary: thawed (`is_point_frozen` uses strict `>`).
		set_time(PointFreezeDuration::get());
		assert_ok!(try_bestow(2));
	});
}

/// The ring proof commits to the `subject` context — re-using a proof with a different subject
/// must fail verification.
#[test]
fn validate_vote_rejects_subject_mismatch() {
	new_test_ext().execute_with(|| {
		let message = &[0, 1, 2, 3];
		let (_, vote_a) = make_vote(0, 0, subject(1), Direction::Honourable);
		let (_, vote_b) = make_vote(0, 0, subject(2), Direction::Honourable);
		let proof = prove_vote::<Test>(&vote_a, 0, message);

		assert_err!(
			Pallet::<Test>::validate_vote_proof(&vote_b, message, &proof, 0, 0),
			Error::<Test>::InvalidProof
		);
	});
}

/// The ring proof commits to the `point` context — re-using a proof with a different point must
/// fail verification.
#[test]
fn validate_vote_rejects_point_mismatch() {
	new_test_ext().execute_with(|| {
		let message = &[0, 1, 2, 3];
		let (_, vote_a) = make_vote(0, 0, subject(1), Direction::Honourable);
		let (_, vote_b) = make_vote(0, 1, subject(1), Direction::Honourable);
		let proof = prove_vote::<Test>(&vote_a, 0, message);

		assert_err!(
			Pallet::<Test>::validate_vote_proof(&vote_b, message, &proof, 0, 0),
			Error::<Test>::InvalidProof
		);
	});
}

/// The ring proof is bound to the call via `inherited_implication`, so a proof generated with
/// `call_valid_from = X` must not be reusable with `call_valid_from = Y`.
#[test]
fn tx_extension_rejects_proof_with_tampered_call_valid_from() {
	new_test_ext().execute_with(|| {
		let now = PointFreezeDuration::get() + 1;
		set_time(now);
		let vote = VoteData { subject: subject(1), point: 0, direction: Direction::Honourable };

		// Proof is generated for `call_valid_from = now`.
		let proof_call = Call::bestow { vote: vote.clone(), call_valid_from: now };
		let proof_call: <Test as frame_system::Config>::RuntimeCall = proof_call.into();
		let ext_version: ExtensionVersion = 0;
		let message = (ext_version, &proof_call).using_encoded(blake2_256);
		let proof = prove_vote::<Test>(&vote, 0, &message);

		// Submit with a different `call_valid_from` (still inside the mortality window).
		let submit_call = Call::bestow { vote: vote.clone(), call_valid_from: now - 1 };
		let submit_call: <Test as frame_system::Config>::RuntimeCall = submit_call.into();
		let extension: VoterAuth<Test> =
			VoterAuth::new(Some(VoterAuthData { proof, ring_index: 0, revision: 0 }));
		let info = submit_call.get_dispatch_info();
		let len = submit_call.encoded_size();
		let post_info = PostDispatchInfo::default();

		assert_err!(
			extension
				.test_run(RawOrigin::None.into(), &submit_call, &info, len, 0, |_| Ok(post_info)),
			TransactionValidityError::Invalid(InvalidTransaction::BadProof)
		);
	});
}

/// `direction` is part of the call (not a proof context), but the proof's message commitment
/// covers the call. Tampering with `direction` after proof generation must fail verification.
#[test]
fn tx_extension_rejects_proof_with_tampered_direction() {
	new_test_ext().execute_with(|| {
		set_time(0);

		let now = 0;
		let vote_a = VoteData { subject: subject(1), point: 0, direction: Direction::Honourable };
		let vote_b = VoteData { direction: Direction::Dishonourable, ..vote_a.clone() };

		let proof_call = Call::bestow { vote: vote_a.clone(), call_valid_from: now };
		let proof_call: <Test as frame_system::Config>::RuntimeCall = proof_call.into();
		let ext_version: ExtensionVersion = 0;
		let message = (ext_version, &proof_call).using_encoded(blake2_256);
		let proof = prove_vote::<Test>(&vote_a, 0, &message);

		let submit_call = Call::bestow { vote: vote_b, call_valid_from: now };
		let submit_call: <Test as frame_system::Config>::RuntimeCall = submit_call.into();
		let extension: VoterAuth<Test> =
			VoterAuth::new(Some(VoterAuthData { proof, ring_index: 0, revision: 0 }));
		let info = submit_call.get_dispatch_info();
		let len = submit_call.encoded_size();
		let post_info = PostDispatchInfo::default();

		assert_err!(
			extension
				.test_run(RawOrigin::None.into(), &submit_call, &info, len, 0, |_| Ok(post_info)),
			TransactionValidityError::Invalid(InvalidTransaction::BadProof)
		);
	});
}

/// Aliases are derived per voter — distinct voters share neither subject_aliases nor
/// point_aliases. Two voters can therefore both bestow on the same subject using their own
/// point 0, and freezing one voter's point does not affect the other.
#[test]
fn aliases_are_per_voter() {
	new_test_ext().execute_with(|| {
		const A: u8 = 1;
		const B: u8 = 2;

		let (origin_a, vote_a) = make_vote(A, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin_a, vote_a, 0));

		// Voter B bestows on the same subject with their own point 0 — succeeds because
		// their `subject_alias` and `point_alias` differ from A's.
		let (origin_b, vote_b) = make_vote(B, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin_b, vote_b, 0));

		// Both contributions counted.
		assert_eq!(read_score(1), 1);

		// Point aliases are distinct.
		assert_ne!(get_point_alias(A, 0), get_point_alias(B, 0));
	});
}

/// `Tally` entries are reaped when a subject's score returns to the default, so that subjects
/// with no net effect do not occupy storage.
#[test]
fn tally_is_reaped_when_returning_to_default() {
	new_test_ext().execute_with(|| {
		const VOTER: u8 = 0;

		assert!(!Tally::<Test>::contains_key(subject(1)));

		// Honourable vote: subject(1) goes from default (-1) to 0; entry is stored.
		let (origin, vote) = make_vote(VOTER, 0, subject(1), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(Tally::<Test>::get(subject(1)), Some(0));

		// Redirect to subject(2): subject(1) returns to -1 and its entry is removed.
		let (origin, vote) = make_vote(VOTER, 0, subject(2), Direction::Honourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert!(!Tally::<Test>::contains_key(subject(1)));
		assert_eq!(read_score(1), SUBJECT_DEFAULT_SCORE);
		assert_eq!(Tally::<Test>::get(subject(2)), Some(0));

		// Same on the Dishonourable side: subject(3) stored at -2, reaped when redirected away.
		let (origin, vote) = make_vote(VOTER, 1, subject(3), Direction::Dishonourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert_eq!(Tally::<Test>::get(subject(3)), Some(-2));

		let (origin, vote) = make_vote(VOTER, 1, subject(4), Direction::Dishonourable);
		assert_ok!(HonourPallet::<Test>::bestow(origin, vote, 0));
		assert!(!Tally::<Test>::contains_key(subject(3)));
		assert_eq!(read_score(3), SUBJECT_DEFAULT_SCORE);
	});
}
