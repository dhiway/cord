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

use crate::{mock::*, types::Recognition::*, *};
#[path = "../../../evidence_markers_v5.rs"]
mod evidence_markers_v5;
use codec::Encode;
use frame_support::{
	assert_noop, assert_ok,
	pallet_prelude::BoundedVec,
	traits::{
		fungible::{Inspect, InspectHold, Mutate, MutateHold},
		tokens::Precision,
		Get, Hooks,
	},
	weights::WeightMeter,
};
use indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
use indiv_support::traits::{
	AppendOnlyMembers, ContextualAlias, RevisedContextualAlias, RingExponent, RingMode,
	RingPosition,
};
use sp_runtime::{
	transaction_validity::InvalidTransaction,
	DispatchError::{self, Token},
	TokenError::{BelowMinimum, FundsUnavailable},
};
use verifiable::{mock::Mock, GenerateVerifiable};

// TODO: later test that offchain worker is working

// Mint some balance into the Score pot also add existential deposit if needed.
fn fund_score_pot(amount: u64) {
	if Balances::balance(&PalletScore::score_pot_id()) == 0 {
		assert_ok!(Balances::mint_into(&PalletScore::score_pot_id(), Balances::minimum_balance()));
	}
	assert_ok!(Balances::mint_into(&PalletScore::score_pot_id(), amount));
}

pub fn advance_to(b: u64) {
	while System::block_number() < b {
		System::set_block_number(System::block_number() + 1);
	}
}

// This test verifies the score of a user goes up and down as expected from the specification.
// - Month 1: ✅ +1 => 1
// - Month 2: ✅ +2 => 3
// - Month 3: ✅ +3 => 6
// - Month 4: ✅ +4 => 10
// - Month 5: ✅ +5 => 15
// - Month 6: ❌ -2 => 13
// - Month 7: ✅ +1 => 14
// - Month 8: ✅ +2 => 16
// - Month 9: ✅ +3 => 19
// - Month 10: ❌ -2 => 17
// - Month 11: ❌ -3 => 14
// - Month 12: ❌ -4 => 10
// - Month 13: ❌ -5 => 5
// - Month 14: ❌ -6 => 0
#[test]
fn update_score_with_attendance_works() {
	new_test_ext().execute_with(|| {
		let participant = 0;
		PalletScore::onboard_for_recognition(&participant).unwrap();
		let participant = AccountOrPerson::Account(participant);
		assert_eq!(
			Participants::<Test>::get(&participant),
			Some(Participant {
				score: 0,
				streak: Streak::Attended(0),
				attendance_history: AttendanceHistory::default(),
				credit: 0,
				recognition: NotRecognized,
				cashed_out: false,
				has_ever_reached_personhood: false,
				reached_personhood: false,
				last_attended_game: None,
			})
		);

		// User goes on a streak.
		let expected_scores = [1, 3, 6, 10, 15];
		for expected_score in expected_scores {
			PalletScore::start_attendance_report_session().unwrap();
			PalletScore::set_attendance(&participant, true, 0).unwrap();
			PalletScore::end_attendance_report_session().unwrap();
			People::on_poll(System::block_number(), &mut WeightMeter::new());
			assert_eq!(Participants::<Test>::get(&participant).unwrap().score, expected_score);
		}

		assert_eq!(
			Participants::<Test>::get(&participant),
			Some(Participant {
				score: 15,
				streak: Streak::Attended(5),
				attendance_history: AttendanceHistory::default(),
				credit: 0,
				recognition: NotRecognized,
				cashed_out: false,
				has_ever_reached_personhood: false,
				reached_personhood: false,
				last_attended_game: Some(0),
			})
		);

		// Miss one era
		PalletScore::start_attendance_report_session().unwrap();
		PalletScore::set_attendance(&participant, false, 0).unwrap();
		PalletScore::end_attendance_report_session().unwrap();
		People::on_poll(System::block_number(), &mut WeightMeter::new());

		// New streak started.
		let expected_scores = [15, 17, 20];
		for expected_score in expected_scores {
			PalletScore::start_attendance_report_session().unwrap();
			PalletScore::set_attendance(&participant, true, 0).unwrap();
			PalletScore::end_attendance_report_session().unwrap();
			People::on_poll(System::block_number(), &mut WeightMeter::new());
			assert_eq!(Participants::<Test>::get(&participant).unwrap().score, expected_score);
		}

		let expected_scores = [19, 17, 14, 10, 5, 0];
		// Then they miss 6 eras.
		for expected_score in expected_scores {
			PalletScore::start_attendance_report_session().unwrap();
			PalletScore::set_attendance(&participant, false, 0).unwrap();
			PalletScore::end_attendance_report_session().unwrap();
			People::on_poll(System::block_number(), &mut WeightMeter::new());
			assert_eq!(Participants::<Test>::get(&participant).unwrap().score, expected_score);
		}
	});
}

#[test]
fn insufficient_pot_amount_causes_schedule_payout_failure() {
	new_test_ext().execute_with(|| {
		// Given the Score pot has less than 100
		fund_score_pot(10);

		// When we schedule 1 round of 100
		// Then we fail due to insufficient funds being placed on hold
		assert_noop!(
			PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 100, 1, 100),
			Token(FundsUnavailable)
		);
	});
}

#[test]
fn schedule_payout_rounds_stores_correct_schedule() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let amount_per_round: u64 = 10;
		let round_count: u32 = 2;
		let round_duration: u64 = 100;

		// Given the Score pot is sufficiently funded
		fund_score_pot(amount_per_round * round_count as u64 + 81);

		// When schedule_payout_rounds is called
		assert_ok!(PalletScore::schedule_payout_rounds(
			RuntimeOrigin::root(),
			amount_per_round,
			round_count,
			round_duration,
		));

		// Then the event is emitted
		System::assert_has_event(
			Event::PayoutRoundsScheduled {
				amount: amount_per_round,
				count: round_count,
				duration: round_duration,
			}
			.into(),
		);

		// Then balance is put on hold
		let score_pot_id = PalletScore::score_pot_id();
		assert_eq!(
			Balances::balance_on_hold(&HoldReason::Payout.into(), &score_pot_id),
			amount_per_round * round_count as u64,
		);

		// And the new schedule is recorded
		let schedules = RoundSchedules::<Test>::get();
		assert_eq!(schedules.len(), 1);
		assert_eq!(schedules[0].amount_per_round, amount_per_round);
		assert_eq!(schedules[0].remaining, round_count);
	});
}

#[test]
fn remove_payout_schedule_frees_held_funds() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let amount_per_round: u64 = 10;
		let round_count: u32 = 3;
		let round_duration: u64 = 100;
		let schedule_index: u32 = 0;
		let total_held = amount_per_round * round_count as u64;

		// Given a schedule is stored
		fund_score_pot(total_held + 20);
		assert_ok!(PalletScore::schedule_payout_rounds(
			RuntimeOrigin::root(),
			amount_per_round,
			round_count,
			round_duration,
		));
		let schedules = RoundSchedules::<Test>::get();
		assert_eq!(schedules.len(), 1);
		assert_eq!(schedules[0].remaining, round_count);
		assert_eq!(
			Balances::balance_on_hold(&HoldReason::Payout.into(), &PalletScore::score_pot_id()),
			total_held,
		);

		// When we remove that schedule
		assert_ok!(PalletScore::remove_payout_schedule(RuntimeOrigin::root(), schedule_index));

		// Then the event is emitted
		System::assert_has_event(Event::PayoutScheduleRemoved { index: schedule_index }.into());

		// Then it is removed from storage
		assert_eq!(RoundSchedules::<Test>::get().len(), 0);

		// And the previously held funds are released
		assert_eq!(
			Balances::balance_on_hold(&HoldReason::Payout.into(), &PalletScore::score_pot_id()),
			0
		);
	});
}

#[test]
fn start_new_round_fails_if_no_schedule() {
	new_test_ext().execute_with(|| {
		// No schedules in storage by default
		assert_noop!(
			exec_bare_tx(Call::transition_round { round_index: CurrentRoundIndex::<Test>::get() }),
			InvalidTransaction::Future,
		);
	});
}

#[test]
fn start_new_round_fails_if_round_already_started() {
	new_test_ext().execute_with(|| {
		fund_score_pot(1000);
		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 100, 2, 10));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		assert_noop!(
			exec_bare_tx(Call::transition_round { round_index: CurrentRoundIndex::<Test>::get() }),
			InvalidTransaction::Future,
		);
	});
}

#[test]
fn start_new_round_fails_if_round_in_arg_is_not_current_round() {
	new_test_ext().execute_with(|| {
		fund_score_pot(1000);
		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 100, 2, 10));
		assert_noop!(
			exec_bare_tx(Call::transition_round {
				round_index: CurrentRoundIndex::<Test>::get() + 1
			}),
			InvalidTransaction::Future,
		);

		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
	});
}

#[test]
fn transition_round_succeeds_and_cleans_up_previous_round() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let amount_per_round: u64 = 20;
		let round_count: u32 = 2;
		let round_duration: u64 = 10;
		let participant_points: u32 = 15;

		// Start from a non-zero round to ensure logic doesn't depend on round index being 0.
		let starting_round: u32 = 2;
		CurrentRoundIndex::<Test>::put(starting_round);

		fund_score_pot(1000);
		assert_ok!(PalletScore::schedule_payout_rounds(
			RuntimeOrigin::root(),
			amount_per_round,
			round_count,
			round_duration,
		));

		// we set some pending points for round 2
		CurrentRoundPoints::<Test>::put(participant_points);
		let person_1 = [1; 32];
		PalletScore::onboard_externally_recognized(&person_1).unwrap();
		RoundsPointsForParticipant::<Test>::insert(
			starting_round,
			AccountOrPerson::Person(person_1),
			participant_points,
		);

		// Start the round 2, it includes the pending points automatically
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		System::assert_has_event(Event::RoundTransitioned { round_index: starting_round }.into());

		// The schedule is updated.
		let schedules = RoundSchedules::<Test>::get();
		assert_eq!(schedules.len(), 1);
		assert_eq!(schedules[0].remaining, 1); // decreased by 1

		advance_to(11);

		// Start the round 3, it starts the round 2 payout.
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		// The schedule is updated.
		let schedules = RoundSchedules::<Test>::get();
		assert_eq!(schedules.len(), 0); // removed

		// Round payout is started for round 2
		let round_payout =
			RoundPayouts::<Test>::get(starting_round).expect("round payout must be created");
		assert_eq!(round_payout.point_price, amount_per_round / participant_points as u64);
		assert_eq!(round_payout.remaining_balance, amount_per_round);

		assert_ok!(exec_bare_tx(Call::operate_payout_round {
			round_index: starting_round,
			limit: 10,
		}));
		System::assert_has_event(Event::PayoutRoundOperated { round_index: starting_round }.into());
		assert!(RoundPayouts::<Test>::get(starting_round).is_none());
		let participant = Participants::<Test>::get(AccountOrPerson::Person(person_1)).unwrap();
		assert_eq!(participant.credit, amount_per_round);

		let pot = PalletScore::score_pot_id();
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), amount_per_round);
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), amount_per_round);
	});
}

#[test]
fn operate_payout_round_works() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let amount_per_round: u64 = 55;
		let round_count: u32 = 2;
		let round_duration: u64 = 1;
		let first_round: u32 = 0;
		let user_a: u64 = 2;
		let user_b: u64 = 3;
		let user_c: u64 = 5;
		let points_a: u32 = 10;
		let points_b: u32 = 10;
		let points_c: u32 = 7;
		let total_points = points_a + points_b + points_c; // 27

		fund_score_pot(200);

		// Onboard 3 participants and set some points
		PalletScore::onboard_for_recognition(&user_a).unwrap();
		RoundsPointsForParticipant::<Test>::insert(
			first_round,
			AccountOrPerson::Account(user_a),
			points_a,
		);
		PalletScore::onboard_for_recognition(&user_b).unwrap();
		RoundsPointsForParticipant::<Test>::insert(
			first_round,
			AccountOrPerson::Account(user_b),
			points_b,
		);
		PalletScore::onboard_for_recognition(&user_c).unwrap();
		RoundsPointsForParticipant::<Test>::insert(
			first_round,
			AccountOrPerson::Account(user_c),
			points_c,
		);
		CurrentRoundPoints::<Test>::put(total_points);

		// We schedule & start a round
		assert_ok!(PalletScore::schedule_payout_rounds(
			RuntimeOrigin::root(),
			amount_per_round,
			round_count,
			round_duration,
		));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		// advance & start a new round to trigger round 0 payout
		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		// The round has point_price = 55 / 27 = 2, leftover = 1

		// operate_payout_round => tries to process a chunk of participants
		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: first_round, limit: 2 }));
		System::assert_has_event(Event::PayoutRoundOperated { round_index: first_round }.into());
		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: first_round, limit: 2 }));

		// The code processes up to 4 participants, so all 3 should be converted
		assert!(RoundPayouts::<Test>::get(first_round).is_none()); // we're done

		// Check participants’ credit
		let part_a = Participants::<Test>::get(AccountOrPerson::Account(user_a)).unwrap();
		assert_eq!(part_a.credit, 20);
		let part_b = Participants::<Test>::get(AccountOrPerson::Account(user_b)).unwrap();
		assert_eq!(part_b.credit, 20);
		let part_c = Participants::<Test>::get(AccountOrPerson::Account(user_c)).unwrap();
		assert_eq!(part_c.credit, 14);

		// calling operate_payout_round again is a no-op (round finished)
		assert_noop!(
			exec_bare_tx(Call::operate_payout_round { round_index: first_round, limit: 1000 }),
			InvalidTransaction::Stale,
		);

		let pot = PalletScore::score_pot_id();
		let total_credited = part_a.credit + part_b.credit + part_c.credit; // 54
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), amount_per_round,);
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), total_credited,);
	});
}

#[test]
fn operate_payout_round_fails_if_no_round() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }),
			InvalidTransaction::Future,
		);
	});
}

#[test]
fn cash_out_works_when_unrecognized() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		// Given a participant who is not recognized or externally recognized
		let user = 10;
		PalletScore::onboard_for_recognition(&user).unwrap();
		let key = AccountOrPerson::Account(user);
		assert!(Participants::<Test>::get(&key).is_some());

		// Their `score` is zero, let's simulate they attended a few times
		PalletScore::start_attendance_report_session().unwrap();
		assert_ok!(PalletScore::set_attendance(&key, true, 0));
		PalletScore::end_attendance_report_session().unwrap();
		People::on_poll(System::block_number(), &mut WeightMeter::new());
		let s = Participants::<Test>::get(&key).unwrap();
		assert_eq!(s.score, 1);
		// The user is still not recognized (score < PersonhoodThreshold)
		assert!(!s.has_ever_reached_personhood);

		// Now they try to `cash_out`
		assert_ok!(PalletScore::cash_out(RuntimeOrigin::signed(user)));
		System::assert_has_event(Event::CashedOut { who: key.clone() }.into());
		let s = Participants::<Test>::get(&key).unwrap();
		// For a small score like 1, the reduction is 1 => new score=0
		assert_eq!(s.score, 0);
		// they gained those points
		assert_eq!(RoundsPointsForParticipant::<Test>::get(0, &key), 1);

		// Give some new bigger score by attending
		PalletScore::start_attendance_report_session().unwrap();
		assert_ok!(PalletScore::set_attendance(&key, true, 0));
		assert_ok!(PalletScore::set_attendance(&key, true, 0));
		assert_ok!(PalletScore::set_attendance(&key, true, 0));
		assert_ok!(PalletScore::set_attendance(&key, true, 0));
		PalletScore::end_attendance_report_session().unwrap();
		People::on_poll(System::block_number(), &mut WeightMeter::new());
		assert_eq!(Participants::<Test>::get(&key).unwrap().score, 10);

		// Cash out from 10 => 5
		assert_ok!(PalletScore::cash_out(RuntimeOrigin::signed(user)));
		assert_eq!(Participants::<Test>::get(&key).unwrap().score, 5);
		// The user gains 5 in the round's points
		assert_eq!(RoundsPointsForParticipant::<Test>::get(0, &key), 1 + 5);
	});
}

#[test]
fn cash_out_fails_if_externally_recognized_person() {
	new_test_ext().execute_with(|| {
		// Suppose user=1 is an external person
		let p_1 = [1; 32];
		PalletScore::onboard_externally_recognized(&p_1).unwrap();
		let key = AccountOrPerson::Person(p_1);
		let participant = Participants::<Test>::get(&key).unwrap();
		assert!(matches!(participant.recognition, ExternallyRecognized));

		// None cannot use `cash_out`
		assert_noop!(
			PalletScore::cash_out(RuntimeOrigin::none()),
			Error::<Test>::BadOriginNotSignedNotAccountParticipant
		);
		// Even if we attempt from a "person" origin=1, we fail with Recognized
		// (account 1 is considered a person, see mock configuration)
		assert_noop!(
			PalletScore::cash_out(
				indiv_pallet_people::Origin::PersonalAlias(RevisedContextualAlias {
					revision: 0,
					ring: 0,
					ca: ContextualAlias { context: crate::SCORE_CONTEXT, alias: p_1 },
				})
				.into()
			),
			Error::<Test>::BadOriginNotSignedNotAccountParticipant,
		);
	});
}

#[test]
fn cash_out_fails_if_has_been_recognized() {
	new_test_ext().execute_with(|| {
		// Test for a person that is not externally recognized but gains personhood through score.
		let user2 = 11;
		PalletScore::onboard_for_recognition(&user2).unwrap();
		let key2 = AccountOrPerson::Account(user2);
		// Accumulate attendance until the score reaches the personhood threshold.
		PalletScore::start_attendance_report_session().unwrap();
		while Participants::<Test>::get(&key2).unwrap().score < PersonhoodThreshold::<Test>::get() {
			assert_ok!(PalletScore::set_attendance(&key2, true, 0));
		}
		// Now make the person not recognized again.
		assert_ok!(PalletScore::set_attendance(&key2, false, 0));
		PalletScore::end_attendance_report_session().unwrap();
		People::on_poll(System::block_number(), &mut WeightMeter::new());

		let participant2 = Participants::<Test>::get(&key2).unwrap();
		assert!(participant2.has_ever_reached_personhood);

		// Now, cashing out should fail.
		assert_noop!(
			PalletScore::cash_out(RuntimeOrigin::signed(user2)),
			Error::<Test>::HasReachedPersonhood
		);
	});
}

#[test]
fn redeem_credit_works() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let user = 22u64;
		let user_key = AccountOrPerson::Account(user);
		let participant_points = 10u32;
		let amount_per_round = 20;
		// A participant with some points
		PalletScore::onboard_for_recognition(&user).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, &user_key, participant_points);
		CurrentRoundPoints::<Test>::put(participant_points);

		// The pot is funded
		let pot = PalletScore::score_pot_id();
		fund_score_pot(40);

		// We schedule a payout round, start a new era, and start and operate the payout round
		assert_ok!(PalletScore::schedule_payout_rounds(
			RuntimeOrigin::root(),
			amount_per_round,
			2,
			1,
		));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), amount_per_round,);
		assert_eq!(Participants::<Test>::get(&user_key).unwrap().credit, amount_per_round,);

		// Now user redeems full credit into a destination account.
		let destination = 999u64;
		let destination_balance_before = Balances::balance(&destination);
		assert_ok!(PalletScore::redeem_credit(RuntimeOrigin::signed(user), destination));
		System::assert_has_event(
			Event::CreditClaimed { who: user_key.clone(), destination, amount: amount_per_round }
				.into(),
		);

		// Assert state
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 0);
		assert_eq!(Participants::<Test>::get(&user_key).unwrap().credit, 0);
		assert_eq!(Balances::balance(&destination), destination_balance_before + 20);

		// But if the user tries again, they fail with NoReward
		assert_noop!(
			PalletScore::redeem_credit(RuntimeOrigin::signed(user), destination),
			Error::<Test>::NoReward
		);
	});
}

#[test]
fn redeem_credit_release_failure_preserves_credit() {
	new_test_ext().execute_with(|| {
		let user = 22;
		PalletScore::onboard_for_recognition(&user).unwrap();
		let user_key = AccountOrPerson::Account(user);
		Participants::<Test>::mutate(&user_key, |participant| {
			participant.as_mut().unwrap().credit = 10;
		});

		let destination = 999u64;
		assert_noop!(
			PalletScore::redeem_credit(RuntimeOrigin::signed(user), destination),
			Token(FundsUnavailable)
		);

		assert_eq!(Participants::<Test>::get(&user_key).unwrap().credit, 10);
		assert_eq!(
			Balances::balance_on_hold(&HoldReason::Credit.into(), &PalletScore::score_pot_id()),
			0
		);
		assert_eq!(Balances::balance(&destination), 0);
	});
}

#[test]
fn redeem_credit_transfer_failure_preserves_credit_and_hold() {
	new_test_ext().execute_with(|| {
		let user = 22;
		PalletScore::onboard_for_recognition(&user).unwrap();
		let user_key = AccountOrPerson::Account(user);
		Participants::<Test>::mutate(&user_key, |participant| {
			participant.as_mut().unwrap().credit = 1;
		});

		let pot = PalletScore::score_pot_id();
		fund_score_pot(10);
		assert_ok!(Balances::hold(&HoldReason::Credit.into(), &pot, 1));

		// Below existential deposit in mock runtime => transfer fails after release attempt.
		let destination = 42u64;
		assert_noop!(
			PalletScore::redeem_credit(RuntimeOrigin::signed(user), destination),
			Token(BelowMinimum)
		);

		assert_eq!(Participants::<Test>::get(&user_key).unwrap().credit, 1);
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 1);
		assert_eq!(Balances::balance(&destination), 0);
	});
}

#[test]
fn payout_for_offboarded_participant_is_recycled() {
	new_test_ext().execute_with(|| {
		// 1. Fund the pot enough for this test:
		fund_score_pot(1000);

		// 2. Onboard user A and user B:
		let user_a = 11u64;
		let user_b = 12u64;
		PalletScore::onboard_for_recognition(&user_a).unwrap();
		PalletScore::onboard_for_recognition(&user_b).unwrap();

		// 3. Give them both some points in the upcoming payout round 0:
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(user_a), 10);
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(user_b), 20);
		CurrentRoundPoints::<Test>::put(30);

		// 4. Offboard user A *before* the payout: remove from `Participants`.
		PalletScore::offboard(&AccountOrPerson::Account(user_a));
		assert!(!Participants::<Test>::contains_key(AccountOrPerson::Account(user_a)));

		// 5. Schedule 1 round of 60; start and transition the round:
		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 60, 1, 10));
		//    We call `transition_round` once to plan round 0:
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		// 6. Advance enough blocks so that the round transitions to a payout round:
		advance_to(11);
		//    Call `transition_round` again, which moves round 0 to a payout state:
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		// 7. Operate the payout for round 0:
		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		// 8. Verify what happened:
		//
		// User A was offboarded, so they never got credited. Their record in
		// RoundsPointsForParticipant was skipped. User B should get all 20 * point_price. But
		// what's the point_price?
		//
		//   - total points = 30
		//   - total credit = 60
		//   - point_price = 60 / 30 = 2
		//   - user B gets 20 * 2 = 40
		//   - leftover is 20
		// That leftover (20) should be recycled at the end when the code sees no more participants.
		//
		//  => RoundPayouts::<Test>::remove(0) is expected, so round 0 no longer exists:
		assert!(RoundPayouts::<Test>::get(0).is_none());

		//  => User B ended with 40 in `credit`:
		let user_b_score = Participants::<Test>::get(AccountOrPerson::Account(user_b)).unwrap();
		assert_eq!(user_b_score.credit, 40);

		//  => User A is not in `Participants` at all (offboarded):
		assert!(!Participants::<Test>::contains_key(AccountOrPerson::Account(user_a)));

		//  => The leftover 20 is returned from the `Payout` hold back to the free balance:
		let pot = PalletScore::score_pot_id();
		//    The total 60 was held in `HoldReason::Payout`, but only 40 ended up released to
		// `Credit`.    So the leftover 20 is automatically recycled at the end:
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), 0);
		//    Likewise, 40 is now on hold under `HoldReason::Credit` for user B’s future redemption:
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 40);
	});
}

#[test]
fn participant_origin_can_cash_out_and_redeem_credit() {
	new_test_ext().execute_with(|| {
		let participant_acct = 42u64;
		frame_system::Pallet::<Test>::inc_sufficients(&participant_acct);

		// 1) Onboard account=42 as a participant.
		assert_ok!(PalletScore::onboard_for_recognition(&participant_acct));
		let key = AccountOrPerson::Account(participant_acct);

		// 2) Give them a small attendance so they have a nonzero score, e.g. 10.
		for _ in 0..4 {
			assert_ok!(PalletScore::set_attendance(&key, true, 0));
		}
		assert_eq!(Participants::<Test>::get(&key).unwrap().score, 10);

		// 3) Now call `cash_out` with participant origin.
		let nonce0 = 0;
		let call_cash_out = RuntimeCall::PalletScore(crate::Call::cash_out {});
		assert_ok!(exec_participant_score_tx(
			participant_acct,
			ScoreAsParticipantData { nonce: nonce0 },
			call_cash_out
		));

		// 4) Confirm that the user’s onchain data was updated.
		let p = Participants::<Test>::get(&key).unwrap();
		assert_eq!(p.score, 5);
		assert!(p.cashed_out); // The pallet sets `cashed_out=true` until next attendance
		assert_eq!(RoundsPointsForParticipant::<Test>::get(0, &key), 5);

		// 5) We want to produce credit for the user so that `redeem_credit` has something to claim.
		//    Let's schedule 1 round with credit=50, start it, and operate it.
		fund_score_pot(200);
		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 50, 1, 2));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		// Assert participant received credit=50.
		let p = Participants::<Test>::get(&key).unwrap();
		assert_eq!(p.credit, 50);

		// 6) This pallet-only mock omits the standard CheckNonce that owns the increment in the
		// runtime pipeline, so the explicit policy nonce remains the current account nonce (zero).
		let nonce1 = 0;
		let destination = 999u64;
		let destination_balance_before = Balances::balance(&destination);
		let call_redeem = RuntimeCall::PalletScore(crate::Call::redeem_credit { destination });
		assert_ok!(exec_participant_score_tx(
			participant_acct,
			ScoreAsParticipantData { nonce: nonce1 },
			call_redeem
		));

		// 7) The user’s full credit is transferred.
		let p = Participants::<Test>::get(&key).unwrap();
		assert_eq!(p.credit, 0);
		assert_eq!(Balances::balance(&destination), destination_balance_before + 50);
	});
}

#[test]
fn suspended_or_unknown_account_cannot_claim_participant_origin() {
	new_test_ext().execute_with(|| {
		let unknown = 41u64;
		let cash_out = RuntimeCall::PalletScore(crate::Call::cash_out {});
		assert_noop!(
			exec_participant_score_tx(
				unknown,
				ScoreAsParticipantData { nonce: 0 },
				cash_out.clone(),
			),
			InvalidTransaction::Call,
		);

		let suspended = 42u64;
		assert_ok!(PalletScore::onboard_for_recognition(&suspended));
		Participants::<Test>::mutate(AccountOrPerson::Account(suspended), |participant| {
			participant.as_mut().unwrap().recognition = Suspended(Default::default());
		});
		assert_noop!(
			exec_participant_score_tx(suspended, ScoreAsParticipantData { nonce: 0 }, cash_out,),
			InvalidTransaction::Call,
		);
	});
}

#[test]
fn suspended_and_unknown_participants_cannot_mutate_or_claim_score_state() {
	new_test_ext().execute_with(|| {
		let account = 42u64;
		assert_ok!(PalletScore::onboard_for_recognition(&account));
		let key = AccountOrPerson::Account(account);
		Participants::<Test>::mutate(&key, |participant| {
			let participant = participant.as_mut().unwrap();
			participant.score = 10;
			participant.credit = 5;
			participant.recognition = Suspended(Default::default());
		});
		let before = Participants::<Test>::get(&key).unwrap();
		let points_before = CurrentRoundPoints::<Test>::get();
		let pot_before = Balances::total_balance(&PalletScore::score_pot_id());

		assert_noop!(
			PalletScore::set_attendance(&key, true, 7),
			Error::<Test>::ParticipantSuspended
		);
		assert_noop!(
			PalletScore::cash_out(RuntimeOrigin::signed(account)),
			Error::<Test>::ParticipantSuspended
		);
		assert_noop!(
			PalletScore::redeem_credit(RuntimeOrigin::signed(account), 77),
			Error::<Test>::ParticipantSuspended
		);
		assert_eq!(Participants::<Test>::get(&key), Some(before));
		assert_eq!(CurrentRoundPoints::<Test>::get(), points_before);
		assert_eq!(Balances::total_balance(&PalletScore::score_pot_id()), pot_before);

		let unknown = AccountOrPerson::Account(404);
		assert_noop!(PalletScore::set_attendance(&unknown, true, 7), Error::<Test>::NoScore);
		assert_noop!(PalletScore::cash_out(RuntimeOrigin::signed(404)), Error::<Test>::NoScore);
		assert_noop!(
			PalletScore::redeem_credit(RuntimeOrigin::signed(404), 77),
			Error::<Test>::NoScore
		);
		assert!(!Participants::<Test>::contains_key(unknown));
	});
}

#[test]
fn payout_account_rotation_is_root_only_atomic_and_liability_safe() {
	new_test_ext().execute_with(|| {
		let old = PalletScore::score_pot_id();
		assert_eq!(old, 500);
		fund_score_pot(100);
		let old_balance = Balances::total_balance(&old);
		assert_noop!(
			PalletScore::set_payout_account(RuntimeOrigin::signed(99), 501),
			sp_runtime::DispatchError::BadOrigin
		);

		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 10, 1, 5));
		assert_noop!(
			PalletScore::set_payout_account(RuntimeOrigin::root(), 501),
			Error::<Test>::PayoutAccountLiability
		);
		assert_eq!(PalletScore::score_pot_id(), old);
		assert_ok!(PalletScore::remove_payout_schedule(RuntimeOrigin::root(), 0));

		assert_ok!(PalletScore::set_payout_account(RuntimeOrigin::root(), 501));
		assert_eq!(PalletScore::score_pot_id(), 501);
		assert_eq!(Balances::total_balance(&old), 0);
		assert_eq!(Balances::total_balance(&501), old_balance);
	});
}

#[test]
fn payout_account_rotation_watches_every_liability_and_conserves_funds() {
	new_test_ext().execute_with(|| {
		let old = PalletScore::score_pot_id();
		fund_score_pot(100);
		let assert_blocked = || {
			assert_noop!(
				PalletScore::set_payout_account(RuntimeOrigin::root(), 501),
				Error::<Test>::PayoutAccountLiability
			);
			assert_eq!(PalletScore::score_pot_id(), old);
		};

		RoundPlanning::<Test>::put(RoundInfo { finish_at: 10, credit: 1 });
		assert_blocked();
		RoundPlanning::<Test>::kill();

		RoundPayouts::<Test>::insert(
			0,
			RoundPayout { remaining_balance: 1, point_price: 1, remainder: 0, total_points: 1 },
		);
		assert_blocked();
		RoundPayouts::<Test>::remove(0);

		CurrentRoundPoints::<Test>::put(1);
		assert_blocked();
		CurrentRoundPoints::<Test>::put(0);

		assert_ok!(Balances::hold(&HoldReason::Payout.into(), &old, 1));
		assert_blocked();
		assert_ok!(Balances::release(&HoldReason::Payout.into(), &old, 1, Precision::Exact));
		assert_ok!(Balances::hold(&HoldReason::Credit.into(), &old, 1));
		assert_blocked();
		assert_ok!(Balances::release(&HoldReason::Credit.into(), &old, 1, Precision::Exact));

		let events = System::events().len();
		let old_balance = Balances::total_balance(&old);
		assert_ok!(PalletScore::set_payout_account(RuntimeOrigin::root(), old));
		assert_eq!(System::events().len(), events);
		assert_eq!(Balances::total_balance(&old), old_balance);

		assert_ok!(<Balances as frame_support::traits::ReservableCurrency<u64>>::reserve(&old, 10));
		let issuance = Balances::total_issuance();
		let before_old = Balances::total_balance(&old);
		let before_new = Balances::total_balance(&501);
		assert!(PalletScore::set_payout_account(RuntimeOrigin::root(), 501).is_err());
		assert_eq!(PalletScore::score_pot_id(), old);
		assert_eq!(Balances::total_balance(&old), before_old);
		assert_eq!(Balances::total_balance(&501), before_new);
		assert_eq!(Balances::total_issuance(), issuance);
		let _ = <Balances as frame_support::traits::ReservableCurrency<u64>>::unreserve(&old, 10);

		assert_ok!(PalletScore::set_payout_account(RuntimeOrigin::root(), 501));
		assert_eq!(Balances::total_balance(&old), 0);
		assert_eq!(Balances::total_balance(&501), before_old);
		assert_eq!(Balances::total_issuance(), issuance);
	});
}

#[test]
fn unit_test_set_attendance_to_at_least() {
	let mut streak = Streak::Attended(5);
	streak.set_attendance_to_at_least(3);
	assert_eq!(streak, Streak::Attended(5));
	streak.add_absence(1);
	assert_eq!(streak, Streak::Absent(1));
	streak.set_attendance_to_at_least(3);
	assert_eq!(streak, Streak::Attended(3));
}

// We simulate:
// 1) Initially => `NotRecognized(None)`, has not reached personhood (score < threshold)
// 2) Attendance until `score >= PersonhoodThreshold` => still `NotRecognized(None)` (because user
//    hasn't yet called `register`), but `has_ever_reached_personhood = true`
// 3) Some absences => drop below threshold => still `NotRecognized(None)`
// 4) More attendance => score >= threshold again => still `NotRecognized(None)`
// 5) User calls `register(Some(key))` => transitions to `Recognized(id)`
// 6) User absences => drops below threshold => transitions to `NotRecognized(Some(id))`
// 7) More attendance => remains `NotRecognized(Some(id))` (score >= threshold, but must
//    re-register)
// 8) User calls `register(None)` => transitions to `Recognized(id)` again.
#[test]
fn recognition_flow() {
	new_test_ext().execute_with(|| {
		// Create the people collection first
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let user = 99u64;
		let user_key = AccountOrPerson::Account(user);

		//
		// 1) Initially onboarded => NotRecognized(None), no prior personhood
		//
		assert_ok!(PalletScore::onboard_for_recognition(&user));
		let participant = Participants::<Test>::get(&user_key).expect("Just onboarded");
		assert_eq!(participant.recognition, Recognition::NotRecognized);
		assert!(!participant.has_ever_reached_personhood, "should not have reached personhood yet");
		assert_eq!(participant.score, 0);

		//
		// 2) Attendance until `score >= PersonhoodThreshold` => still NotRecognized but
		//    has_ever_reached_personhood = true
		//
		let personhood_threshold = PersonhoodThreshold::<Test>::get();
		while Participants::<Test>::get(&user_key).unwrap().score < personhood_threshold {
			assert_ok!(PalletScore::start_attendance_report_session());
			let p = PalletScore::set_attendance(&user_key, true, 0).expect("Should succeed");
			assert_ok!(PalletScore::end_attendance_report_session());
			People::on_poll(System::block_number(), &mut WeightMeter::new());
			assert!(p.score <= personhood_threshold);
		}
		let participant = Participants::<Test>::get(&user_key).unwrap();

		// eligible for personhood, but not yet registered as person
		assert!(participant.reached_personhood);
		assert!(
			participant.has_ever_reached_personhood,
			"score has reached threshold at least once"
		);
		assert_eq!(participant.recognition, Recognition::NotRecognized);

		assert_eq!(participant.score, personhood_threshold);

		//
		// 3) Some absence => drop below threshold => still NotRecognized, but
		//    `has_ever_reached_personhood` remains true
		//
		while Participants::<Test>::get(&user_key).unwrap().score >= personhood_threshold {
			assert_ok!(PalletScore::start_attendance_report_session());
			PalletScore::set_attendance(&user_key, false, 0).expect("Should succeed");
			assert_ok!(PalletScore::end_attendance_report_session());
			People::on_poll(System::block_number(), &mut WeightMeter::new());
		}
		let participant = Participants::<Test>::get(&user_key).unwrap();
		assert_eq!(participant.score, 20);
		assert_eq!(participant.recognition, Recognition::NotRecognized);
		assert!(participant.has_ever_reached_personhood);

		//
		// 4) More attendance => go above threshold again => still NotRecognized(None)
		//
		while Participants::<Test>::get(&user_key).unwrap().score < personhood_threshold {
			assert_ok!(PalletScore::start_attendance_report_session());
			PalletScore::set_attendance(&user_key, true, 0).expect("Should succeed");
			assert_ok!(PalletScore::end_attendance_report_session());
			People::on_poll(System::block_number(), &mut WeightMeter::new());
		}
		let participant = Participants::<Test>::get(&user_key).unwrap();
		assert_eq!(participant.score, personhood_threshold);
		assert_eq!(participant.recognition, Recognition::NotRecognized);
		assert!(participant.has_ever_reached_personhood);

		//
		// 5.0) User calls `register(Some(key))` with wrong proof => fails
		//
		let (key_for_person, sk) = mock_key(1234);
		let wrong_proof = {
			let mut m = b"wrong message".to_vec();
			m.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &m[..]).unwrap()
		};
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), Some((key_for_person, wrong_proof))),
			Error::<Test>::InvalidProofOfOwnership
		);

		//
		// 5.1) User calls `register(Some(key))` with correct proof => transitions to Recognized(id)
		//
		let proof = {
			let mut m = b"pop register using".to_vec();
			m.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &m[..]).unwrap()
		};
		assert_ok!(PalletScore::register(
			RuntimeOrigin::signed(user),
			Some((key_for_person, proof))
		));
		let participant = Participants::<Test>::get(&user_key).unwrap();
		let personal_id = match participant.recognition {
			Recognition::Recognized(id) => id,
			_ => panic!("Expected participant to be Recognized now"),
		};

		// Check that our mock People pallet sees them as recognized
		let record = indiv_pallet_people::People::<Test>::get(personal_id).unwrap();
		assert!(matches!(
			Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &record.key).unwrap(),
			RingPosition::Onboarding { queue_page: 0, .. },
		));

		//
		// 6) Some absence => dropping below threshold => transitions to Suspended(id)
		//
		while Participants::<Test>::get(&user_key).unwrap().score >= personhood_threshold {
			assert_ok!(PalletScore::start_attendance_report_session());
			PalletScore::set_attendance(&user_key, false, 0).expect("Should succeed");
			assert_ok!(PalletScore::end_attendance_report_session());
			People::on_poll(System::block_number(), &mut WeightMeter::new());
		}
		let participant = Participants::<Test>::get(&user_key).unwrap();
		match participant.recognition {
			Recognition::Suspended(same_id) => assert_eq!(same_id, personal_id),
			_ => panic!("Expected participant to be suspended => Suspended(id)"),
		}
		// Also check People pallet => Suspended
		let record = indiv_pallet_people::People::<Test>::get(personal_id).unwrap();
		assert_eq!(
			Members::member_status(PEOPLE_MEMBER_IDENTIFIER, &record.key).unwrap(),
			RingPosition::Suspended,
		);

		//
		// 7) Suspension is a hard Score boundary: attendance cannot rebuild score or mutate state.
		let before = Participants::<Test>::get(&user_key).unwrap();
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_noop!(
			PalletScore::set_attendance(&user_key, true, 0),
			Error::<Test>::ParticipantSuspended
		);
		assert_ok!(PalletScore::end_attendance_report_session());
		assert_eq!(Participants::<Test>::get(&user_key), Some(before));
	});
}

#[test]
fn round_planning_is_cleared_after_last_schedule() {
	new_test_ext().execute_with(|| {
		// Fund the Score pot enough for 2 rounds of 10 each.
		fund_score_pot(100);

		// Schedule 2 rounds with a short duration so we can advance quickly.
		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 10, 2, 3));

		// Initially there is no planning.
		assert!(RoundPlanning::<Test>::get().is_none());

		// 1) First transition: plan round 0.
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		let plan0 = RoundPlanning::<Test>::get().expect("round 0 should be planned");

		// 2) Advance to the end of round 0 and transition:
		//    - moves round 0 to payout
		//    - plans round 1 (remaining schedule goes from 1 -> 0 and is removed)
		advance_to(plan0.finish_at);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));
		let plan1 = RoundPlanning::<Test>::get().expect("round 1 should be planned");
		assert_eq!(RoundSchedules::<Test>::get().len(), 0, "all schedules should be consumed");

		// 3) Advance to the end of round 1 and transition:
		//    - moves round 1 to payout
		//    - no schedules remain, so no new planning should be created
		advance_to(plan1.finish_at);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		// Regression check: planning must be cleared after the last scheduled round.
		assert!(RoundPlanning::<Test>::get().is_none(), "no lingering round planning expected");
	});
}

#[test]
fn no_points_minted_for_persons_on_attendance() {
	// Attending while recognised as a person must not mint payout points.
	new_test_ext().execute_with(|| {
		let person_alias = [42u8; 32];
		assert_ok!(PalletScore::onboard_externally_recognized(&person_alias));
		let who = AccountOrPerson::Person(person_alias);

		assert_ok!(PalletScore::start_attendance_report_session());
		assert_ok!(PalletScore::set_attendance(&who, true, 0));
		assert_ok!(PalletScore::end_attendance_report_session());
		People::on_poll(System::block_number(), &mut WeightMeter::new());

		assert_eq!(RoundsPointsForParticipant::<Test>::get(0, &who), 0);
		assert_eq!(CurrentRoundPoints::<Test>::get(), 0);
	});
}

#[test]
fn remainder_distribution_works_even() {
	new_test_ext().execute_with(|| {
		fund_score_pot(500);

		let person_1 = 100u64;
		let person_2 = 101u64;

		// Set up a scenario that will produce a remainder of 10:
		// Credit: 100, Total points: 30
		// point_price = 100 / 30 = 3 (integer division)
		// remainder = 100 - (3 * 30) = 10
		// This remainder should be distributed proportionally

		// Onboard 2 participants with 15 points each
		PalletScore::onboard_for_recognition(&person_1).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_1), 15);
		PalletScore::onboard_for_recognition(&person_2).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_2), 15);
		CurrentRoundPoints::<Test>::put(30);

		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), 100, 1, 1));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		let round_payout = RoundPayouts::<Test>::get(0).expect("round payout must be created");
		assert_eq!(round_payout.point_price, 3); // 100 / 30 = 3 (integer division)
		assert_eq!(round_payout.total_points, 30);
		assert_eq!(round_payout.remainder, 10);

		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		let part1 = Participants::<Test>::get(AccountOrPerson::Account(person_1)).unwrap();
		let part2 = Participants::<Test>::get(AccountOrPerson::Account(person_2)).unwrap();

		// Each participant should get:
		// base_reward = 3 * 15 = 45
		// remainder_portion = (10 * 15) / 30 = 5
		// So each gets: 45 + 5 = 50
		// Total: 50 + 50 = 100 (entire pot distributed!)
		assert_eq!(part1.credit, 50);
		assert_eq!(part2.credit, 50);

		// Total distributed: 100 (the entire pot is distributed with no remainder lost)
		let total_distributed = part1.credit + part2.credit;
		assert_eq!(total_distributed, 100, "entire pot should be distributed");

		assert!(RoundPayouts::<Test>::get(0).is_none());

		let pot = PalletScore::score_pot_id();
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), 0);
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 100);

		// Initial: 500 + ED, Held for payout: 100, Distributed: 100
		// After payout: Payout hold releases 100 to Credit hold
		// Free balance = 500 + existential deposit - 100 (in Credit) = 400 + ED
		let free_balance = Balances::balance(&pot);
		assert_eq!(free_balance, 400 + Balances::minimum_balance());
	});
}

#[test]
fn remainder_distribution_works_odd_equal_participants() {
	new_test_ext().execute_with(|| {
		fund_score_pot(500);

		let person_1 = 102u64;
		let person_2 = 103u64;

		let total_credit = 101;

		// Set up a scenario that will produce a remainder of 11:
		// Credit: 101, Total points: 30
		// point_price = 101 / 30 = 3 (integer division)
		// remainder = 101 - (3 * 30) = 11
		// This remainder should be distributed proportionally

		PalletScore::onboard_for_recognition(&person_1).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_1), 15);
		PalletScore::onboard_for_recognition(&person_2).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_2), 15);
		CurrentRoundPoints::<Test>::put(30);

		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), total_credit, 1, 1));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		let round_payout = RoundPayouts::<Test>::get(0).expect("round payout must be created");
		assert_eq!(round_payout.point_price, 3);
		assert_eq!(round_payout.total_points, 30);
		assert_eq!(round_payout.remainder, 11);

		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		let part1 = Participants::<Test>::get(AccountOrPerson::Account(person_1)).unwrap();
		let part2 = Participants::<Test>::get(AccountOrPerson::Account(person_2)).unwrap();

		// Each participant should get:
		// base_reward = 3 * 15 = 45
		// remainder_portion = (11 * 15) / 30 = 5.5 = 5
		// So each gets: 45 + 5 = 50
		// Total: 50 + 50 = 100 (not entire pot distributed!)
		assert_eq!(part1.credit, 50);
		assert_eq!(part2.credit, 50);

		let total_distributed = part1.credit + part2.credit;
		let dust = total_credit - total_distributed;
		assert_eq!(dust, 1, "1 credit can not be distributed");
		assert_eq!(total_distributed, 100, "pot minus dust, should be distributed");

		assert!(RoundPayouts::<Test>::get(0).is_none());

		let pot = PalletScore::score_pot_id();
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), 0); // Dust released
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 100); // Distributed

		// Verify dust (1) was recycled into free balance
		// Initial: 500 + ED, Held for payout: 101, Distributed: 100, Dust: 1
		// After payout: Payout hold releases 100 to Credit hold, 1 to free
		// Free balance = 500 + existential deposit - 100 (in Credit) = 400 + ED
		let free_balance = Balances::balance(&pot);
		assert_eq!(free_balance, 400 + Balances::minimum_balance());
	})
}

#[test]
fn remainder_distribution_works_odd_unequal_participants() {
	new_test_ext().execute_with(|| {
		fund_score_pot(500);

		let person_1 = 104u64;
		let person_2 = 105u64;
		let person_3 = 106u64;

		let total_credit = 101;

		// Set up a scenario that will produce a remainder of 11:
		// Credit: 101, Total points: 30
		// point_price = 101 / 30 = 3 (integer division)
		// remainder = 101 - (3 * 30) = 11
		// This remainder should be distributed proportionally

		PalletScore::onboard_for_recognition(&person_1).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_1), 9);
		PalletScore::onboard_for_recognition(&person_2).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_2), 10);
		PalletScore::onboard_for_recognition(&person_3).unwrap();
		RoundsPointsForParticipant::<Test>::insert(0, AccountOrPerson::Account(person_3), 11);
		CurrentRoundPoints::<Test>::put(30);

		assert_ok!(PalletScore::schedule_payout_rounds(RuntimeOrigin::root(), total_credit, 1, 1));
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		advance_to(2);
		assert_ok!(exec_bare_tx(Call::transition_round {
			round_index: CurrentRoundIndex::<Test>::get()
		}));

		let round_payout = RoundPayouts::<Test>::get(0).expect("round payout must be created");
		assert_eq!(round_payout.point_price, 3);
		assert_eq!(round_payout.total_points, 30);
		assert_eq!(round_payout.remainder, 11);

		assert_ok!(exec_bare_tx(Call::operate_payout_round { round_index: 0, limit: 1000 }));

		let part1 = Participants::<Test>::get(AccountOrPerson::Account(person_1)).unwrap();
		let part2 = Participants::<Test>::get(AccountOrPerson::Account(person_2)).unwrap();
		let part3 = Participants::<Test>::get(AccountOrPerson::Account(person_3)).unwrap();

		// participants should get:
		// *** person_1 ***
		// * base_reward = 3 * 9 = 27
		// * remainder_portion = (11 * 9) / 30 = 3.3 = 3
		// * total: 27 + 3 = 30
		//
		// *** person_2 ***
		// * base_reward = 3 * 10 = 30
		// * remainder_portion = (11 * 10) / 30 = 3.67 = 3
		// * total: 30 + 3 = 33
		//
		// *** person_3 ***
		// * base_reward = 3 * 11 = 33
		// * remainder_portion = (11 * 11) / 30 = 4.03 = 4
		// * total: 33 + 4 = 37
		//
		// total distributed: 30 + 33 + 37 = 100

		assert_eq!(part1.credit, 30);
		assert_eq!(part2.credit, 33);
		assert_eq!(part3.credit, 37);

		let total_distributed = part1.credit + part2.credit + part3.credit;
		let dust = total_credit - total_distributed;
		assert_eq!(dust, 1, "1 dust credit can not be distributed");
		assert_eq!(total_distributed, 100, "pot minus dust, should be distributed");

		assert!(RoundPayouts::<Test>::get(0).is_none());

		let pot = PalletScore::score_pot_id();
		assert_eq!(Balances::balance_on_hold(&HoldReason::Payout.into(), &pot), 0);
		assert_eq!(Balances::balance_on_hold(&HoldReason::Credit.into(), &pot), 100);

		let free_balance = Balances::balance(&pot);
		assert_eq!(free_balance, 400 + Balances::minimum_balance());
	})
}

#[test]
fn register_works_for_suspended_participant() {
	new_test_ext().execute_with(|| {
		// Create the people collection first
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		advance_to(1);

		let user = 11u64;
		let who = AccountOrPerson::Account(user);

		// Get to Recognized state.
		PalletScore::onboard_for_recognition(&user).unwrap();
		let (key, sk) = mock_key(user);
		let proof = {
			let mut msg = b"pop register using".to_vec();
			msg.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &msg[..]).unwrap()
		};
		Participants::<Test>::mutate(&who, |p| {
			p.as_mut().unwrap().score = 21;
			p.as_mut().unwrap().reached_personhood = true;
		});
		assert_ok!(PalletScore::register(RuntimeOrigin::signed(user), Some((key, proof))));
		let participant = Participants::<Test>::get(&who).unwrap();
		let id = match participant.recognition {
			Recognized(id) => id,
			_ => panic!("expected participant to be Recognized after registration"),
		};

		// Manually suspend them and lower score.
		assert_ok!(PalletScore::start_attendance_report_session());
		People::suspend_personhood(&[id]).unwrap();
		assert_ok!(PalletScore::end_attendance_report_session());
		People::on_poll(System::block_number(), &mut WeightMeter::new());
		Participants::<Test>::mutate(&who, |p| {
			p.as_mut().unwrap().recognition = Suspended(id);
			p.as_mut().unwrap().score = 10;
			p.as_mut().unwrap().reached_personhood = false;
		});

		// Should fail, because score is too low and reached_personhood is false.
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), None),
			Error::<Test>::HasNotReachedPersonhood
		);

		// Get score back up.
		Participants::<Test>::mutate(&who, |p| {
			p.as_mut().unwrap().score = 21;
		});

		// Should succeed now.
		assert_ok!(PalletScore::register(RuntimeOrigin::signed(user), None));
		let participant = Participants::<Test>::get(&who).unwrap();
		assert_eq!(participant.recognition, Recognized(id));
		assert!(participant.reached_personhood);
		assert!(participant.has_ever_reached_personhood);
		System::assert_last_event(
			Event::PersonhoodRecognized { who: who.clone(), resumed: true }.into(),
		);

		// Should fail, because they are already recognized.
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), None),
			Error::<Test>::Recognized
		);
	});
}

#[test]
fn register_works_for_not_recognized_participant() {
	new_test_ext().execute_with(|| {
		// Create the people collection first
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		advance_to(1);

		let user = 12u64;
		let who = AccountOrPerson::Account(user);

		// Should fail, because score is 0 and reached_personhood is false.
		PalletScore::onboard_for_recognition(&user).unwrap();
		let participant = Participants::<Test>::get(&who).unwrap();
		assert_eq!(participant.recognition, NotRecognized);
		assert!(!participant.reached_personhood);
		let (key, sk) = mock_key(user);
		let proof = {
			let mut msg = b"pop register using".to_vec();
			msg.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &msg[..]).unwrap()
		};
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), Some((key, proof))),
			Error::<Test>::HasNotReachedPersonhood
		);

		// Set reached_personhood to true.
		Participants::<Test>::mutate(&who, |p| {
			p.as_mut().unwrap().reached_personhood = true;
		});

		// Should fail, because key is missing.
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), None),
			Error::<Test>::KeyMustBeProvided
		);

		// Should fail, because proof is invalid.
		let wrong_proof = {
			let mut msg = b"wrong message".to_vec();
			msg.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &msg[..]).unwrap()
		};
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), Some((key, wrong_proof))),
			Error::<Test>::InvalidProofOfOwnership
		);

		// Should succeed now.
		assert_ok!(PalletScore::register(RuntimeOrigin::signed(user), Some((key, proof))));
		let participant = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(participant.recognition, Recognized(_)));
		assert!(participant.reached_personhood);
		assert!(participant.has_ever_reached_personhood);
		System::assert_last_event(
			Event::PersonhoodRecognized { who: who.clone(), resumed: false }.into(),
		);

		// Should fail, because they are already recognized.
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), None),
			Error::<Test>::Recognized
		);
	});
}

// Helper: onboard a user, attend until recognised, then register as a person.
// Returns the AccountOrPerson key and PersonalId.
fn setup_recognised_person(user: u64) -> (AccountOrPerson<u64>, u64) {
	assert_ok!(Members::create_collection(
		0,
		PEOPLE_MEMBER_IDENTIFIER,
		1,
		RingMode::Flexible,
		RingExponent::R2e9,
		None,
	));

	assert_ok!(PalletScore::onboard_for_recognition(&user));
	let who = AccountOrPerson::Account(user);

	// Attend until score reaches personhood threshold.
	let personhood_threshold = PersonhoodThreshold::<Test>::get();
	while Participants::<Test>::get(&who).unwrap().score < personhood_threshold {
		assert_ok!(PalletScore::start_attendance_report_session());
		PalletScore::set_attendance(&who, true, 0).expect("attendance ok");
		assert_ok!(PalletScore::end_attendance_report_session());
		People::on_poll(System::block_number(), &mut WeightMeter::new());
	}

	// Register as a person.
	let (key, sk) = mock_key(user);
	let proof = {
		let mut msg = b"pop register using".to_vec();
		msg.extend_from_slice(&user.encode()[..]);
		Mock::sign(&sk, &msg[..]).unwrap()
	};
	assert_ok!(PalletScore::register(RuntimeOrigin::signed(user), Some((key, proof))));

	let participant = Participants::<Test>::get(&who).unwrap();
	let personal_id = match participant.recognition {
		Recognized(id) => id,
		_ => panic!("Expected Recognized"),
	};

	(who, personal_id)
}

// Helper: run one attendance session.
fn attend(who: &AccountOrPerson<u64>, attended: bool) {
	assert_ok!(PalletScore::start_attendance_report_session());
	PalletScore::set_attendance(who, attended, 0).expect("attendance ok");
	assert_ok!(PalletScore::end_attendance_report_session());
	People::on_poll(System::block_number(), &mut WeightMeter::new());
}

// Helper: override active member count and re-run threshold update to get a specific
// grace period. Must be called inside execute_with after new_test_ext.
fn set_active_members(count: u32) {
	indiv_pallet_members::ActiveMembers::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, count);
	// Trigger an attendance session to update both thresholds.
	assert_ok!(PalletScore::start_attendance_report_session());
	assert_ok!(PalletScore::end_attendance_report_session());
}

/// With few active people (N = 100, grace ratio = 5/6), a recognised person can
/// miss 5 out of 6 games. A 6th miss within the window triggers suspension.
#[test]
fn grace_ratio_suspension() {
	new_test_ext().execute_with(|| {
		set_active_members(100); // grace ratio = (5, 6), personhood threshold = 1
		let (who, personal_id) = setup_recognised_person(99);
		let (allowed_misses, window) = AbsenceGraceRatio::<Test>::get();
		assert_eq!((allowed_misses, window), (5, 6));

		// Miss 5 times in a window of 6. Within allowance.
		for i in 1..=5 {
			attend(&who, false);
			let p = Participants::<Test>::get(&who).unwrap();
			assert!(
				matches!(p.recognition, Recognized(_)),
				"should remain Recognized after {i} misses (allowed_misses=5)"
			);
			assert!(p.reached_personhood);
		}

		// 6th miss. 6 misses in window of 6. Exceeds allowance → suspended.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Suspended(id) if id == personal_id),
			"should be Suspended after 6 misses in window of 6"
		);
		assert!(!p.reached_personhood);
	});
}

/// Attending pushes misses out of the rolling window, preventing suspension.
#[test]
fn attendance_within_window_prevents_suspension() {
	new_test_ext().execute_with(|| {
		set_active_members(100); // grace ratio = (5, 6), personhood threshold = 1
		let (who, _personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (5, 6));

		// Miss 5 times — within allowance.
		for _ in 0..5 {
			attend(&who, false);
		}
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)), "still Recognized after 5 misses");

		// Attend once — window still has 5 misses out of 6, but within allowance.
		attend(&who, true);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)), "still Recognized after recovery");

		// Keep missing — the single attendance stays in the window, capping misses at 5.
		for _ in 0..5 {
			attend(&who, false);
			let p = Participants::<Test>::get(&who).unwrap();
			assert!(
				matches!(p.recognition, Recognized(_)),
				"still Recognized — attendance keeps misses within allowance"
			);
		}

		// One more miss pushes the attendance out of the 6-game window.
		// Window now has 6 misses in 6 games, exceeding the allowance of 5.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Suspended(_)), "suspended after exceeding allowed misses");
	});
}

/// Externally recognised persons are exempt from absence score penalties and should not
/// be suspended by the grace period mechanism.
#[test]
fn externally_recognized_unaffected_by_grace_period() {
	new_test_ext().execute_with(|| {
		let person_alias = [42u8; 32];
		assert_ok!(PalletScore::onboard_externally_recognized(&person_alias));
		let who = AccountOrPerson::Person(person_alias);
		let personhood_threshold = PersonhoodThreshold::<Test>::get();

		// Miss many games (more than any possible grace period).
		for _ in 0..10 {
			attend(&who, false);
		}

		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, ExternallyRecognized),
			"externally recognised should never be suspended"
		);
		assert_eq!(p.score, personhood_threshold, "score should be unchanged");
		assert!(p.reached_personhood);
	});
}

/// The grace ratio scales with active people count. Verify the default calculation.
#[test]
fn grace_ratio_scales_with_active_count() {
	new_test_ext().execute_with(|| {
		// (allowed_misses, window)
		assert_eq!(PalletScore::calculate_absence_grace_ratio(0), (5, 6));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(5_000), (5, 6));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(5_001), (4, 5));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(10_000), (4, 5));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(10_001), (3, 4));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(20_000), (3, 4));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(20_001), (2, 3));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(35_000), (2, 3));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(35_001), (1, 2));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(50_000), (1, 2));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(50_001), (1, 6));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(100_000), (1, 6));
	});
}

/// The AbsenceGraceRatio storage is updated when start_attendance_report_session is called.
#[test]
fn grace_ratio_updated_on_session_start() {
	new_test_ext().execute_with(|| {
		// Default mock has 100k active members => ratio = (1, 6).
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 6));

		// After starting a session with 100k members, ratio stays (1, 6).
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 6));
		assert_ok!(PalletScore::end_attendance_report_session());

		// Lower active count to 100 => ratio becomes (5, 6).
		indiv_pallet_members::ActiveMembers::<Test>::insert(PEOPLE_MEMBER_IDENTIFIER, 100);
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (5, 6));
		assert_ok!(PalletScore::end_attendance_report_session());
	});
}

/// With ratio (1, 6) at 100k active people, one miss is tolerated. Suspension
/// happens on the second miss.
#[test]
fn large_network_second_miss_in_window_suspends() {
	new_test_ext().execute_with(|| {
		// Default: 100k active members => grace ratio = (1, 6).
		let (who, personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 6));

		// First miss: 1 miss in 6, within allowance.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)), "1 miss in 6 is tolerated");

		// Attend a few then miss again: 2 misses in window of 6.
		attend(&who, true);
		attend(&who, true);
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Suspended(id) if id == personal_id),
			"2 misses in window of 6 should trigger suspension"
		);
	});
}

/// 20,001..=35,000 active users (grace ratio 2/3, threshold = 10):
/// Miss twice => still recognised. Miss 3 times within 3 games => suspend personhood.
/// Attend to rebuild score, then register(None) to resume personhood.
#[test]
fn grace_ratio_window_three_full_cycle() {
	new_test_ext().execute_with(|| {
		set_active_members(25_000);
		let (who, personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (2, 3));
		assert_eq!(PersonhoodThreshold::<Test>::get(), 10);

		// --- Suspension path ---

		// First two misses: 2 misses in 3, within allowance.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)));
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)));
		assert!(p.reached_personhood);

		// Third miss: 3 misses in window of 3, exceeds allowance → suspended.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Suspended(id) if id == personal_id));
		assert!(!p.reached_personhood);
		// Score: started at 10, lost 1 then 2 then 3 => 4.
		assert_eq!(p.score, 4);

		// --- Terminal suspension boundary ---
		let before = Participants::<Test>::get(&who).unwrap();
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_noop!(
			PalletScore::set_attendance(&who, true, 0),
			Error::<Test>::ParticipantSuspended
		);
		assert_ok!(PalletScore::end_attendance_report_session());
		assert_eq!(Participants::<Test>::get(&who), Some(before));
	});
}

/// 35,001..=50,000 active users (ratio 1/2, threshold = 15):
/// Miss once => tolerated. Miss twice within 2 => suspended.
/// Attend to rebuild score, then register(None) to resume personhood.
#[test]
fn grace_ratio_window_two_full_cycle() {
	new_test_ext().execute_with(|| {
		set_active_members(40_000);
		let (who, personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 2));
		assert_eq!(PersonhoodThreshold::<Test>::get(), 15);

		// --- Suspension path ---

		// First miss: 1/2, tolerated.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)));
		assert!(p.reached_personhood);
		// Score: started at 15, lost 1 => 14.
		assert_eq!(p.score, 14);

		// Second miss: 2 misses in 2 → suspended.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Suspended(id) if id == personal_id));
		assert!(!p.reached_personhood);
		// Score: 14 - 2 => 12.
		assert_eq!(p.score, 12);

		// --- Terminal suspension boundary ---
		let before = Participants::<Test>::get(&who).unwrap();
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_noop!(
			PalletScore::set_attendance(&who, true, 0),
			Error::<Test>::ParticipantSuspended
		);
		assert_ok!(PalletScore::end_attendance_report_session());
		assert_eq!(Participants::<Test>::get(&who), Some(before));
	});
}

/// 50,001+ active users (grace ratio 1/6, threshold = 21):
/// Miss once => tolerated. Miss twice in 6 => suspended.
/// Attend to rebuild, register(None) to resume.
/// Then verify that continued attendance does not re-suspend.
#[test]
fn grace_ratio_large_network_full_cycle() {
	new_test_ext().execute_with(|| {
		set_active_members(50_001);
		let (who, personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 6));
		assert_eq!(PersonhoodThreshold::<Test>::get(), 21);

		// --- Suspension path ---

		// First miss: 1/6, tolerated.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(_)));
		// Score: started at 21 (capped), lost 1 => 20.
		assert_eq!(p.score, 20);

		// Second miss: 2 misses in 6 → suspended.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Suspended(id) if id == personal_id));
		assert!(!p.reached_personhood);
		// Score: 20 - 2 => 18.
		assert_eq!(p.score, 18);

		// --- Terminal suspension boundary ---
		let before = Participants::<Test>::get(&who).unwrap();
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_noop!(
			PalletScore::set_attendance(&who, true, 0),
			Error::<Test>::ParticipantSuspended
		);
		assert_ok!(PalletScore::end_attendance_report_session());
		assert_eq!(Participants::<Test>::get(&who), Some(before));
	});
}

/// With 50k+ active people, attending does not cause suspension, only missing
/// does.
#[test]
fn attending_does_not_suspend() {
	new_test_ext().execute_with(|| {
		set_active_members(50_001); // grace ratio = (1, 6), personhood threshold = 21
		let (who, _personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 6));

		// Attend several games — should remain Recognized.
		for _ in 0..8 {
			attend(&who, true);
			let p = Participants::<Test>::get(&who).unwrap();
			assert!(matches!(p.recognition, Recognized(_)));
			assert!(p.reached_personhood);
		}
	});
}

/// Regression: a NotRecognized participant who reached personhood at low population
/// (threshold=1) should lose `reached_personhood` when their score drops below
/// threshold. Without this, they can call `register()` later when the threshold is
/// higher (e.g. 21), bypassing it via the stale `reached_personhood` flag.
#[test]
fn not_recognized_loses_reached_personhood_on_score_drop() {
	new_test_ext().execute_with(|| {
		// Low population: grace ratio = (1, 2), personhood threshold = 1
		set_active_members(100);
		assert_eq!(PersonhoodThreshold::<Test>::get(), 1);

		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));

		let user = 99u64;
		let who = AccountOrPerson::Account(user);
		assert_ok!(PalletScore::onboard_for_recognition(&user));

		// Attend once — score reaches threshold (1). reached_personhood = true.
		attend(&who, true);
		let p = Participants::<Test>::get(&who).unwrap();
		assert_eq!(p.score, 1);
		assert!(p.reached_personhood);
		assert_eq!(p.recognition, Recognition::NotRecognized);

		// Do NOT register. Miss one game, so score drops below threshold.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert_eq!(p.score, 0);
		assert!(!p.reached_personhood, "should lose reached_personhood when score drops");
		assert_eq!(p.recognition, Recognition::NotRecognized);

		// Demonstrate the exploit: grow the network so threshold rises to 21.
		set_active_members(50_001);
		assert_eq!(PersonhoodThreshold::<Test>::get(), 21);

		// The participant can still register despite score=0 and threshold=21,
		// because reached_personhood is still true.
		let (key, sk) = mock_key(user);
		let proof = {
			let mut msg = b"pop register using".to_vec();
			msg.extend_from_slice(&user.encode()[..]);
			Mock::sign(&sk, &msg[..]).unwrap()
		};
		assert_noop!(
			PalletScore::register(RuntimeOrigin::signed(user), Some((key, proof))),
			Error::<Test>::HasNotReachedPersonhood
		);
	});
}

/// Setting a schedule with a window > 8 should fail because AttendanceHistory
/// only tracks the last 8 games.
#[test]
fn set_absence_grace_schedule_rejects_window_too_large() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 9,
			allowed_misses: 1,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::WindowTooLarge
		);

		// Verify storage was not modified — still the default.
		assert_eq!(AbsenceGraceSchedule::<Test>::get(), DefaultAbsenceGraceTiers::<Test>::get());
	});
}

/// A valid schedule with window <= 8 should be accepted.
#[test]
fn set_absence_grace_schedule_works() {
	new_test_ext().execute_with(|| {
		advance_to(1);
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![
			AbsenceGraceTier { population_size_threshold: 10_000, window: 3, allowed_misses: 1 },
			AbsenceGraceTier { population_size_threshold: u32::MAX, window: 6, allowed_misses: 1 },
		])
		.unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(
			RuntimeOrigin::root(),
			schedule.clone()
		));
		assert_eq!(AbsenceGraceSchedule::<Test>::get(), schedule);
		System::assert_last_event(Event::AbsenceGraceScheduleSet.into());
	});
}

/// The custom schedule is used by calculate_absence_grace_ratio.
#[test]
fn custom_schedule_overrides_defaults() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 4,
			allowed_misses: 2,
		}])
		.unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule));

		// Any active count should now yield (2, 4) instead of the hardcoded defaults.
		assert_eq!(PalletScore::calculate_absence_grace_ratio(100), (2, 4));
		assert_eq!(PalletScore::calculate_absence_grace_ratio(100_000), (2, 4));
	});
}

/// Tiers submitted out of order are rejected.
#[test]
fn set_absence_grace_schedule_rejects_unsorted_tiers() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![
			AbsenceGraceTier { population_size_threshold: 50_000, window: 6, allowed_misses: 1 },
			AbsenceGraceTier { population_size_threshold: 10_000, window: 3, allowed_misses: 1 },
		])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::AbsenceScheduleNotSorted,
		);
	});
}

/// A tier with (allowed_misses = 0, window = 0) is accepted: no grace.
#[test]
fn set_absence_grace_schedule_accepts_zero_zero() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 0,
			allowed_misses: 0,
		}])
		.unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(
			RuntimeOrigin::root(),
			schedule.clone()
		));
		assert_eq!(AbsenceGraceSchedule::<Test>::get(), schedule);
	});
}

/// A tier with window = 0 but non-zero allowed_misses is rejected.
#[test]
fn set_absence_grace_schedule_rejects_nonzero_misses_with_zero_window() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 0,
			allowed_misses: 1,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::AllowedMissesTooLarge,
		);
	});
}

/// A grace ratio of (0, 0) means no grace: any single absence immediately suspends.
#[test]
fn grace_ratio_zero_zero_suspends_immediately() {
	new_test_ext().execute_with(|| {
		set_active_members(100);
		let (who, _personal_id) = setup_recognised_person(99);

		// Set a schedule where every tier is (0, 0) — no grace at any population size.
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 0,
			allowed_misses: 0,
		}])
		.unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule));

		// A single absence should immediately suspend.
		attend(&who, false);

		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Suspended(_)),
			"suspended immediately — (0, 0) ratio gives no grace"
		);
	});
}

/// A tier where allowed_misses == window is rejected (must be strictly less).
#[test]
fn set_absence_grace_schedule_rejects_misses_equal_to_window() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 5,
			allowed_misses: 5,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::AllowedMissesTooLarge,
		);
	});
}

/// A tier where allowed_misses > window (both non-zero) is rejected.
#[test]
fn set_absence_grace_schedule_rejects_misses_greater_than_window() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 3,
			allowed_misses: 4,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::AllowedMissesTooLarge,
		);
	});
}

/// An empty schedule disables the grace period entirely.
#[test]
fn empty_schedule_disables_grace_period() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![]).unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), schedule));

		// Empty schedule means no grace — any absence suspends immediately.
		assert_eq!(PalletScore::calculate_absence_grace_ratio(100), (0, 0));
	});
}

/// An unset schedule reads back as `DefaultAbsenceGraceTiers` and yields a
/// non-disabling grace ratio.
#[test]
fn unset_schedule_falls_back_to_defaults() {
	new_test_ext().execute_with(|| {
		// Storage default equals DefaultAbsenceGraceTiers when never set.
		assert_eq!(AbsenceGraceSchedule::<Test>::get(), DefaultAbsenceGraceTiers::<Test>::get());
		let ratio = PalletScore::calculate_absence_grace_ratio(100);
		assert_ne!(ratio, (0, 0), "default schedule should not disable grace");
	});
}

/// A non-privileged origin cannot set the absence grace schedule.
#[test]
fn set_absence_grace_schedule_rejects_unprivileged_origin() {
	new_test_ext().execute_with(|| {
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 6,
			allowed_misses: 1,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::signed(1), schedule),
			DispatchError::BadOrigin,
		);
	});
}

/// A mid-session schedule change does not affect attendance reported in that
/// same session, because the ratio is cached at session start. The new rules
/// only apply from the next session onward.
#[test]
fn mid_session_schedule_tightening_does_not_affect_current_session() {
	new_test_ext().execute_with(|| {
		set_active_members(100); // default grace ratio = (5, 6)
		let (who, _personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (5, 6));
		// Miss 5 times — at the limit of (5, 6).
		for _ in 0..5 {
			attend(&who, false);
		}
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Recognized(_)),
			"still recognized after 5 misses under (5,6)"
		);
		// Start a new session (caches ratio), then tighten the schedule mid-session.
		assert_ok!(PalletScore::start_attendance_report_session());
		let strict_schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 0,
			allowed_misses: 0,
		}])
		.unwrap();
		assert_ok!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), strict_schedule,)
		);
		// The cached ratio is still (5, 6) from the session start.
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (5, 6));
		// Attend in the current session — still evaluated under old (5, 6) ratio.
		// 5 misses + 1 attend = safe.
		PalletScore::set_attendance(&who, true, 0).expect("attendance ok");
		assert_ok!(PalletScore::end_attendance_report_session());
		People::on_poll(System::block_number(), &mut WeightMeter::new());
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Recognized(_)),
			"still recognized: mid-session change did not affect current session"
		);
		// Next session picks up the strict (0, 0) ratio. Any absence now suspends.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Suspended(_)),
			"suspended: strict schedule took effect in the next session"
		);
	});
}

/// Relaxing the schedule between sessions saves a participant who would have
/// been suspended under the old rules.
#[test]
fn schedule_relaxation_saves_participant_next_session() {
	new_test_ext().execute_with(|| {
		// Start with a strict custom schedule: (1, 2) — only 1 miss per 2 games allowed.
		let strict_schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 2,
			allowed_misses: 1,
		}])
		.unwrap();
		assert_ok!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::root(), strict_schedule,)
		);
		set_active_members(100); // caches (1, 2) into AbsenceGraceRatio
		let (who, _personal_id) = setup_recognised_person(99);
		assert_eq!(AbsenceGraceRatio::<Test>::get(), (1, 2));
		// Miss once — 1 miss in 2, within (1, 2) allowance.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Recognized(_)),
			"still recognized after 1 miss under (1,2)"
		);
		// Governance relaxes the schedule to (5, 6) before the next session.
		let relaxed_schedule: AbsenceGraceTiers = BoundedVec::try_from(vec![AbsenceGraceTier {
			population_size_threshold: u32::MAX,
			window: 6,
			allowed_misses: 5,
		}])
		.unwrap();
		assert_ok!(PalletScore::set_absence_grace_schedule(
			RuntimeOrigin::root(),
			relaxed_schedule,
		));
		// Under the old (1, 2), a second consecutive miss would suspend (2 misses in 2).
		// But the next session picks up the relaxed (5, 6) ratio — 2 misses in 6 is safe.
		attend(&who, false);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Recognized(_)),
			"still recognized: relaxed (5,6) ratio saved the participant"
		);
	});
}

// TODO: add a test for the full cycle: onboard, attend, cash_out, redeem_credit.
// TODO: integration test that test the whole process can be completed for free with
// invitation.

// =====================================================================
// set_personhood_threshold_schedule
// =====================================================================

/// Helper: build the canonical default curve (matches DEFAULT_PERSONHOOD_THRESHOLDS).
fn default_personhood_schedule() -> PersonhoodThresholdTiers {
	BoundedVec::try_from(vec![
		PersonhoodThresholdTier { population_size_threshold: 5_000, score_threshold: 1 },
		PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 3 },
		PersonhoodThresholdTier { population_size_threshold: 20_000, score_threshold: 6 },
		PersonhoodThresholdTier { population_size_threshold: 35_000, score_threshold: 10 },
		PersonhoodThresholdTier { population_size_threshold: 50_000, score_threshold: 15 },
		PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 21 },
	])
	.unwrap()
}

/// Helper: a flat single-tier schedule with the given threshold.
fn flat_personhood_schedule(score_threshold: u32) -> PersonhoodThresholdTiers {
	BoundedVec::try_from(vec![PersonhoodThresholdTier {
		population_size_threshold: u32::MAX,
		score_threshold,
	}])
	.unwrap()
}

/// Happy path: a valid schedule is stored and emits the expected event.
#[test]
fn set_personhood_threshold_schedule_works() {
	new_test_ext().execute_with(|| {
		let schedule = default_personhood_schedule();
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			schedule.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), schedule);
	});
}

/// A non-privileged origin cannot set the schedule.
#[test]
fn set_personhood_threshold_schedule_rejects_unprivileged_origin() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(
				RuntimeOrigin::signed(1),
				default_personhood_schedule(),
			),
			DispatchError::BadOrigin,
		);
		// Storage was not modified — still the default.
		assert_eq!(
			PersonhoodThresholdSchedule::<Test>::get(),
			DefaultPersonhoodThresholdTiers::<Test>::get(),
		);
	});
}

/// An empty schedule is rejected — the curve must always resolve a value.
#[test]
fn set_personhood_threshold_schedule_rejects_empty() {
	new_test_ext().execute_with(|| {
		let empty: PersonhoodThresholdTiers = BoundedVec::try_from(vec![]).unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), empty),
			Error::<Test>::PersonhoodScheduleEmpty,
		);
	});
}

/// A tier with `score_threshold = 0` is rejected (would mean everyone is a person).
#[test]
fn set_personhood_threshold_schedule_rejects_zero_score_threshold() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers =
			BoundedVec::try_from(vec![PersonhoodThresholdTier {
				population_size_threshold: u32::MAX,
				score_threshold: 0,
			}])
			.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::PersonhoodScoreThresholdZero,
		);
	});
}

/// A tier with `score_threshold > MAX_PERSONHOOD_THRESHOLD` (= 21) is rejected.
#[test]
fn set_personhood_threshold_schedule_rejects_oversized_score_threshold() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers =
			BoundedVec::try_from(vec![PersonhoodThresholdTier {
				population_size_threshold: u32::MAX,
				score_threshold: 22,
			}])
			.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::PersonhoodScoreThresholdTooLarge,
		);
	});
}

/// `score_threshold == MAX_PERSONHOOD_THRESHOLD` (boundary) is accepted.
#[test]
fn set_personhood_threshold_schedule_accepts_max_score_threshold() {
	new_test_ext().execute_with(|| {
		let schedule = flat_personhood_schedule(21);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			schedule.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), schedule);
	});
}

/// Tiers submitted out of order by population are rejected.
#[test]
fn set_personhood_threshold_schedule_rejects_unsorted_population() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 50_000, score_threshold: 5 },
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 10 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 15 },
		])
		.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::PersonhoodScheduleNotSorted,
		);
	});
}

/// Score thresholds that decrease across tiers are rejected (a larger
/// population must not have a lower bar).
#[test]
fn set_personhood_threshold_schedule_rejects_non_monotonic_score() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 10 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 5 },
		])
		.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::PersonhoodScheduleNotMonotonic,
		);
	});
}

/// A schedule whose last tier is below `u32::MAX` is rejected — the curve
/// must cover all populations.
#[test]
fn set_personhood_threshold_schedule_rejects_non_total() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers =
			BoundedVec::try_from(vec![PersonhoodThresholdTier {
				population_size_threshold: 50_000,
				score_threshold: 21,
			}])
			.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule),
			Error::<Test>::PersonhoodScheduleNotTotal,
		);
	});
}

/// A single-tier schedule that covers `u32::MAX` is the minimum legal config.
#[test]
fn set_personhood_threshold_schedule_accepts_single_tier_total() {
	new_test_ext().execute_with(|| {
		let schedule = flat_personhood_schedule(7);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			schedule.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), schedule);
	});
}

/// A flat curve (same `score_threshold` across all tiers) is accepted —
/// non-decreasing means equal is allowed.
#[test]
fn set_personhood_threshold_schedule_accepts_flat_curve() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 1_000, score_threshold: 5 },
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 5 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 5 },
		])
		.unwrap();
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			schedule.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), schedule);
	});
}

/// Tiers with duplicate `population_size_threshold` are accepted (matches
/// the absence-grace precedent — first match wins).
#[test]
fn set_personhood_threshold_schedule_accepts_duplicate_population() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 3 },
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 6 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 10 },
		])
		.unwrap();
		assert_ok!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule,)
		);
		// First matching tier wins for active_count <= 10_000.
		assert_eq!(PalletScore::calculate_personhood_threshold(10_000), 3);
		// And the second tier is unreachable but stored.
		assert_eq!(PalletScore::calculate_personhood_threshold(10_001), 10);
	});
}

/// A tier with `population_size_threshold = 0` is allowed — it matches only
/// when `active_count == 0`.
#[test]
fn set_personhood_threshold_schedule_accepts_zero_population_threshold() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 0, score_threshold: 1 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 21 },
		])
		.unwrap();
		assert_ok!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule,)
		);
		assert_eq!(PalletScore::calculate_personhood_threshold(0), 1);
		assert_eq!(PalletScore::calculate_personhood_threshold(1), 21);
	});
}

/// A failed validation must NOT modify storage even after a previous valid set.
#[test]
fn set_personhood_threshold_schedule_failed_validation_preserves_previous() {
	new_test_ext().execute_with(|| {
		let valid = flat_personhood_schedule(7);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			valid.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), valid.clone());

		// Submit invalid (zero score).
		let bad: PersonhoodThresholdTiers = BoundedVec::try_from(vec![PersonhoodThresholdTier {
			population_size_threshold: u32::MAX,
			score_threshold: 0,
		}])
		.unwrap();
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), bad),
			Error::<Test>::PersonhoodScoreThresholdZero,
		);

		// Previous schedule is intact.
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), valid);
	});
}

/// A second valid set replaces (not appends).
#[test]
fn set_personhood_threshold_schedule_replaces_previous() {
	new_test_ext().execute_with(|| {
		let first = flat_personhood_schedule(5);
		let second = flat_personhood_schedule(15);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), first,));
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			second.clone(),
		));
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), second);
	});
}

/// `calculate_personhood_threshold` reads back the default tiers from
/// `DefaultPersonhoodThresholdTiers` when the schedule has never been
/// explicitly set.
#[test]
fn unset_personhood_schedule_falls_back_to_defaults() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			PersonhoodThresholdSchedule::<Test>::get(),
			DefaultPersonhoodThresholdTiers::<Test>::get(),
		);

		// Match DefaultPersonhoodThresholdTiers exactly across boundaries.
		assert_eq!(PalletScore::calculate_personhood_threshold(0), 1);
		assert_eq!(PalletScore::calculate_personhood_threshold(5_000), 1);
		assert_eq!(PalletScore::calculate_personhood_threshold(5_001), 3);
		assert_eq!(PalletScore::calculate_personhood_threshold(10_000), 3);
		assert_eq!(PalletScore::calculate_personhood_threshold(10_001), 6);
		assert_eq!(PalletScore::calculate_personhood_threshold(20_000), 6);
		assert_eq!(PalletScore::calculate_personhood_threshold(20_001), 10);
		assert_eq!(PalletScore::calculate_personhood_threshold(35_000), 10);
		assert_eq!(PalletScore::calculate_personhood_threshold(35_001), 15);
		assert_eq!(PalletScore::calculate_personhood_threshold(50_000), 15);
		assert_eq!(PalletScore::calculate_personhood_threshold(50_001), 21);
		assert_eq!(PalletScore::calculate_personhood_threshold(100_000), 21);
	});
}

/// A custom schedule overrides the defaults for `calculate_personhood_threshold`.
#[test]
fn custom_personhood_schedule_overrides_defaults() {
	new_test_ext().execute_with(|| {
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(vec![
			PersonhoodThresholdTier { population_size_threshold: 100, score_threshold: 2 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 4 },
		])
		.unwrap();
		assert_ok!(
			PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), schedule,)
		);

		// Boundaries: <= 100 yields 2, above yields 4.
		assert_eq!(PalletScore::calculate_personhood_threshold(0), 2);
		assert_eq!(PalletScore::calculate_personhood_threshold(100), 2);
		assert_eq!(PalletScore::calculate_personhood_threshold(101), 4);
		assert_eq!(PalletScore::calculate_personhood_threshold(u32::MAX), 4);
	});
}

/// Setting a schedule does NOT update the cached `PersonhoodThreshold`
/// immediately. It updates at the next session boundary via
/// `update_thresholds()`.
#[test]
fn personhood_schedule_change_is_lazy_until_next_session() {
	new_test_ext().execute_with(|| {
		set_active_members(100); // default curve at 100 active people => threshold = 1
		assert_eq!(PersonhoodThreshold::<Test>::get(), 1);

		// Change the schedule mid-session.
		let strict = flat_personhood_schedule(15);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(
			RuntimeOrigin::root(),
			strict.clone(),
		));
		// Storage is updated immediately…
		assert_eq!(PersonhoodThresholdSchedule::<Test>::get(), strict);
		// …but the cached PersonhoodThreshold is unchanged until the next session.
		assert_eq!(PersonhoodThreshold::<Test>::get(), 1);

		// Next session start refreshes the cached threshold.
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_eq!(PersonhoodThreshold::<Test>::get(), 15);
		assert_ok!(PalletScore::end_attendance_report_session());
	});
}

/// Raising the threshold does NOT retroactively suspend an already-recognized
/// participant whose score is now below the new bar.
#[test]
fn personhood_schedule_raise_does_not_retroactively_suspend() {
	new_test_ext().execute_with(|| {
		// Few active people => default threshold = 1. Easy to reach personhood.
		set_active_members(100);
		assert_eq!(PersonhoodThreshold::<Test>::get(), 1);
		let (who, personal_id) = setup_recognised_person(99);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(matches!(p.recognition, Recognized(id) if id == personal_id));
		let original_score = p.score;
		assert!(original_score < 21, "score should be modest under default curve at N=100");

		// Governance raises the bar to 21, far above the participant's score.
		let strict = flat_personhood_schedule(21);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), strict,));
		// Trigger a session boundary to refresh the cached threshold.
		assert_ok!(PalletScore::start_attendance_report_session());
		assert_eq!(PersonhoodThreshold::<Test>::get(), 21);
		PalletScore::set_attendance(&who, true, 0).expect("attendance ok");
		assert_ok!(PalletScore::end_attendance_report_session());
		People::on_poll(System::block_number(), &mut WeightMeter::new());

		// Recognition retained — non-retroactivity guarantee.
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			matches!(p.recognition, Recognized(_)),
			"raising the bar must not retroactively suspend already-recognized participants"
		);
		assert!(p.reached_personhood, "reached_personhood must remain true");
	});
}

/// Lowering the threshold lets a `NotRecognized` participant cross the bar
/// at the next session — the new curve takes effect prospectively.
#[test]
fn personhood_schedule_lower_recognizes_eligible_at_next_session() {
	new_test_ext().execute_with(|| {
		// Set a strict curve (threshold = 10) before onboarding.
		let strict = flat_personhood_schedule(10);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), strict,));
		set_active_members(100); // refreshes cached threshold to 10
		assert_eq!(PersonhoodThreshold::<Test>::get(), 10);

		// Onboard and attend a few sessions — score grows but stays below 10.
		let participant = 7u64;
		assert_ok!(Members::create_collection(
			0,
			PEOPLE_MEMBER_IDENTIFIER,
			1,
			RingMode::Flexible,
			RingExponent::R2e9,
			None,
		));
		assert_ok!(PalletScore::onboard_for_recognition(&participant));
		let who = AccountOrPerson::Account(participant);
		// Attend twice: scores progress 1, 3.
		attend(&who, true);
		attend(&who, true);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(p.score < 10, "expected score below the strict bar of 10, got {}", p.score);
		assert!(!p.reached_personhood, "should not have crossed the bar yet");

		// Governance lowers the threshold.
		let relaxed = flat_personhood_schedule(1);
		assert_ok!(PalletScore::set_personhood_threshold_schedule(RuntimeOrigin::root(), relaxed,));

		// Next attendance session refreshes the cached threshold AND evaluates
		// the participant against the new bar.
		attend(&who, true);
		assert_eq!(PersonhoodThreshold::<Test>::get(), 1);
		let p = Participants::<Test>::get(&who).unwrap();
		assert!(
			p.reached_personhood,
			"score crossed the lowered bar — should have reached personhood"
		);
	});
}

/// Default curve is preserved exactly when `PersonhoodThresholdSchedule` has
/// never been explicitly set. Regression guard against accidental changes to
/// `DefaultPersonhoodThresholdTiers`.
#[test]
fn default_personhood_curve_matches_calculate_at_session_start() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			PersonhoodThresholdSchedule::<Test>::get(),
			DefaultPersonhoodThresholdTiers::<Test>::get(),
		);
		// Cycle through every default tier boundary.
		for (active, expected) in [
			(0u32, 1u32),
			(5_000, 1),
			(5_001, 3),
			(10_000, 3),
			(10_001, 6),
			(20_000, 6),
			(20_001, 10),
			(35_000, 10),
			(35_001, 15),
			(50_000, 15),
			(50_001, 21),
		] {
			set_active_members(active);
			assert_eq!(
				PersonhoodThreshold::<Test>::get(),
				expected,
				"default curve at active={active} expected {expected}",
			);
		}
	});
}

#[test]
fn named_manager_rotates_under_root_and_unauthorized_accounts_fail() {
	new_test_ext().execute_with(|| {
		let schedule = DefaultAbsenceGraceTiers::<Test>::get();
		assert_eq!(ManagerAccount::<Test>::get(), Some(99));
		assert_ok!(PalletScore::set_absence_grace_schedule(
			RuntimeOrigin::signed(99),
			schedule.clone(),
		));
		assert_noop!(
			PalletScore::set_manager_account(RuntimeOrigin::signed(1), Some(1)),
			sp_runtime::DispatchError::BadOrigin,
		);
		assert_ok!(PalletScore::set_manager_account(RuntimeOrigin::root(), Some(1)));
		assert_eq!(ManagerAccount::<Test>::get(), Some(1));
		assert_noop!(
			PalletScore::set_absence_grace_schedule(RuntimeOrigin::signed(99), schedule.clone()),
			sp_runtime::DispatchError::BadOrigin,
		);
		assert_ok!(PalletScore::set_absence_grace_schedule(RuntimeOrigin::signed(1), schedule));
		assert_ok!(PalletScore::set_manager_account(RuntimeOrigin::root(), None));
		assert_noop!(
			PalletScore::set_personhood_threshold_schedule(
				RuntimeOrigin::signed(1),
				DefaultPersonhoodThresholdTiers::<Test>::get(),
			),
			sp_runtime::DispatchError::BadOrigin,
		);
	});
}

#[test]
fn slice2_v5_payout_evidence() {
	payout_account_rotation_watches_every_liability_and_conserves_funds();
	evidence_markers_v5::emit_evidence_marker_v5("slice2-payout");
}
