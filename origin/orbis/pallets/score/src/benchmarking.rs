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

//! Score pallet benchmarks

use super::*;
use crate::{types::Recognition, Pallet as Score};
use alloc::vec::Vec;
use frame_benchmarking::v2::{benchmarks, *};
use frame_support::{
	dispatch::{DispatchInfo, PostDispatchInfo},
	traits::{fungible::InspectHold, UnfilteredDispatchable},
	BoundedVec,
};
// TODO: remove once the score pallet migrates to `#[pallet::authorize]` (deadline April 2027).
// See https://github.com/paritytech/polkadot-sdk/issues/2415.
#[allow(deprecated)]
use frame_support::pallet_prelude::ValidateUnsigned;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin as SystemOrigin};
use sp_core::Get;
use sp_runtime::traits::{AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable, Zero};

const BENCHMARK_MAX_OPERATE: u32 = 10_000;

/// Benchmark Helper
pub trait BenchmarkHelper<T: Config> {
	fn create_member(seed: u64) -> MemberOf<T>;
	fn setup_currency();
}

#[benchmarks(
	where T: Config,
	<T as frame_system::Config>::RuntimeCall:
		Dispatchable<
			Info = DispatchInfo,
			PostInfo = PostDispatchInfo,
			RuntimeOrigin: AsTransactionAuthorizedOrigin,
		>
		+ From<Call<T>>,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn schedule_payout_rounds() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::setup_currency();
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let amount: BalanceOf<T> = 10u32.into();
		let count: u32 = 5;
		let duration: BlockNumberFor<T> = 10u32.into();
		let ed = T::Currency::minimum_balance();

		// Pre-fund the Score pot
		let pot = Score::<T>::score_pot_id();
		T::Currency::mint_into(&pot, amount * count.into() + ed)?;

		// Pre-fill `RoundSchedules` so it already contains
		// `MaxPayoutRoundSchedules - 1` entries
		let max = T::MaxPayoutRoundSchedules::get();
		if max > 1 {
			let dummy = PayoutSchedule {
				remaining: 1,
				amount_per_round: 1u32.into(),
				duration: 10u32.into(),
			};
			let vec = alloc::vec![dummy; (max - 1) as usize];
			let bounded: BoundedVec<_, <T as Config>::MaxPayoutRoundSchedules> =
				vec.try_into().unwrap();
			RoundSchedules::<T>::put(bounded);
		}

		#[extrinsic_call]
		_(SystemOrigin::Root, amount, count, duration);

		// The vector is now at full capacity
		assert_eq!(RoundSchedules::<T>::get().len() as u32, max);

		// The funds are held under the correct `HoldReason`
		let held = T::Currency::balance_on_hold(&HoldReason::Payout.into(), &pot);
		assert_eq!(held, amount * count.into());

		frame_system::Pallet::<T>::assert_last_event(
			Event::PayoutRoundsScheduled { amount, count, duration }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn remove_payout_schedule() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::setup_currency();
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let max = T::MaxPayoutRoundSchedules::get();
		let amount = 10u32.into();
		let count = 5;
		let duration = 10u32.into();
		let ed = T::Currency::minimum_balance();
		let removed_index: u32 = 0;

		// Fund the Score pot with enough free balance for *all* schedules.
		let pot = Score::<T>::score_pot_id();
		T::Currency::mint_into(&pot, amount * (max * count).into() + ed)?;

		// Schedule all payout rounds.
		for _ in 0..max {
			Score::<T>::schedule_payout_rounds(SystemOrigin::Root.into(), amount, count, duration)?;
		}
		// Sanity-check: vector is full.
		assert_eq!(RoundSchedules::<T>::get().len() as u32, max);

		#[extrinsic_call]
		_(SystemOrigin::Root, removed_index); // remove first element, forces full shift

		assert_eq!(RoundSchedules::<T>::get().len() as u32, max - 1);

		let expected_held = amount * ((max - 1) * count).into();
		let still_held = T::Currency::balance_on_hold(&HoldReason::Payout.into(), &pot);
		assert_eq!(still_held, expected_held);

		frame_system::Pallet::<T>::assert_last_event(
			Event::PayoutScheduleRemoved { index: removed_index }.into(),
		);

		Ok(())
	}

	// Includes the cost of `ValidateUnsigned::pre_dispatch`
	#[benchmark]
	fn transition_round() -> Result<(), BenchmarkError> {
		// Put the chain a few blocks ahead so the current round is expired.
		frame_system::Pallet::<T>::set_block_number(10u32.into());

		// Existing planned round is finished.
		let finished_credit: BalanceOf<T> = 100u32.into();
		RoundPlanning::<T>::put(RoundInfo {
			finish_at: 5u32.into(), // < current block, so round is finished
			credit: finished_credit,
		});
		// Non-zero points so point_price calculation is exercised.
		CurrentRoundPoints::<T>::put(10);

		// `RoundSchedules` to capacity; first entry will be removed.
		let max = T::MaxPayoutRoundSchedules::get();
		let sched_amount: BalanceOf<T> = 50u32.into();
		let duration: BlockNumberFor<T> = 20u32.into();

		let bounded: BoundedVec<_, <T as Config>::MaxPayoutRoundSchedules> = (0..max)
			.map(|i| PayoutSchedule {
				remaining: if i == 0 { 1 } else { 2 },
				amount_per_round: sched_amount,
				duration,
			})
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		RoundSchedules::<T>::put(bounded);
		let current_round_index = CurrentRoundIndex::<T>::get();

		let call = Call::<T>::transition_round { round_index: current_round_index };

		#[block]
		{
			#[allow(deprecated)]
			<Pallet<T> as ValidateUnsigned>::pre_dispatch(&call)
				.expect("pre-dispatch must succeed");
			call.dispatch_bypass_filter(SystemOrigin::None.into())?;
		}

		// A new payout entry for previous round (index 0)
		let payout = RoundPayouts::<T>::get(0).expect("payout must be created");
		assert_eq!(payout.remaining_balance, finished_credit);
		assert_eq!(payout.point_price, finished_credit / 10u32.into());

		// Current round index advanced and points reset
		assert_eq!(CurrentRoundIndex::<T>::get(), 1);
		assert_eq!(CurrentRoundPoints::<T>::get(), 0);

		// A fresh planning was stored for the new round
		let plan = RoundPlanning::<T>::get().expect("new planning must exist");
		assert_eq!(plan.credit, sched_amount);

		// `RoundSchedules` shrank by one (first element removed & vector shifted)
		assert_eq!(RoundSchedules::<T>::get().len() as u32, max - 1);

		frame_system::Pallet::<T>::assert_has_event(
			Event::RoundTransitioned { round_index: current_round_index }.into(),
		);

		Ok(())
	}

	// Includes the cost of `ValidateUnsigned::pre_dispatch`
	#[benchmark]
	fn operate_payout_round(l: Linear<1, { BENCHMARK_MAX_OPERATE }>) -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::setup_currency();
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let limit = l;
		let ed = T::Currency::minimum_balance();
		let round_index = 0;
		let points_per_user = 10; // each entry has 10 points
		let point_price = ed; // to ensure transfer is successful
		let pot = Score::<T>::score_pot_id();

		// Create `limit + 1` participants and their point records.
		for i in 0..=limit {
			// note: +1 so one entry remains
			let user: T::AccountId = account("user", i, 0);
			Score::<T>::onboard_for_recognition(&user)?;
			RoundsPointsForParticipant::<T>::insert(
				round_index,
				AccountOrPerson::Account(user),
				points_per_user,
			);
		}

		// Fund the pot.
		let total_points = (limit + 1) * points_per_user;
		let total_balance: BalanceOf<T> = point_price * total_points.into();
		T::Currency::mint_into(&pot, total_balance + ed)?;
		T::Currency::hold(&HoldReason::Payout.into(), &pot, total_balance)?;

		// Build the RoundPayout entry.
		RoundPayouts::<T>::insert(
			round_index,
			RoundPayout {
				remaining_balance: total_balance,
				point_price,
				// Remainder is 0: total_balance = point_price × total_points by construction,
				// so total_balance % total_points = 0. q.e.d.
				remainder: 0u32.into(),
				total_points,
			},
		);

		let call = Call::<T>::operate_payout_round { round_index, limit };

		#[block]
		{
			#[allow(deprecated)]
			<Pallet<T> as ValidateUnsigned>::pre_dispatch(&call)
				.expect("pre-dispatch must succeed");
			call.dispatch_bypass_filter(SystemOrigin::None.into())?;
		}

		// Expected credited amount this call:
		let credited: BalanceOf<T> = point_price * (limit * points_per_user).into();
		let round_after = RoundPayouts::<T>::get(round_index).expect("round must continue");
		assert_eq!(round_after.remaining_balance, total_balance.saturating_sub(credited));

		// Exactly one entry is left in the points map.
		assert_eq!(RoundsPointsForParticipant::<T>::iter_prefix(round_index).count() as u32, 1);

		// Payout/Credit holds were updated.
		let payout_hold = T::Currency::balance_on_hold(&HoldReason::Payout.into(), &pot);
		let credit_hold = T::Currency::balance_on_hold(&HoldReason::Credit.into(), &pot);
		assert_eq!(payout_hold, round_after.remaining_balance);
		assert_eq!(credit_hold, credited);

		frame_system::Pallet::<T>::assert_has_event(
			Event::PayoutRoundOperated { round_index }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn cash_out() -> Result<(), BenchmarkError> {
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let caller: T::AccountId = whitelisted_caller();
		Score::<T>::onboard_for_recognition(&caller)?;

		let key = AccountOrPerson::Account(caller.clone());
		Participants::<T>::mutate(&key, |maybe_p| {
			let mut p = maybe_p.take().expect("participant must exist");
			p.score = 10;
			p.streak = Streak::Attended(3); // Just to test the reset.
			p.cashed_out = false;
			*maybe_p = Some(p);
		});

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller.clone()));

		let after = Participants::<T>::get(&key).expect("entry must exist");
		assert_eq!(after.score, 5, "score reduced by half (rounded up)");
		assert!(after.cashed_out, "cash-out flag set");
		assert_eq!(after.streak, Streak::Attended(0), "attendance streak reset");

		assert_eq!(RoundsPointsForParticipant::<T>::get(CurrentRoundIndex::<T>::get(), &key), 5);
		assert_eq!(CurrentRoundPoints::<T>::get(), 5);

		frame_system::Pallet::<T>::assert_last_event(Event::CashedOut { who: key }.into());

		Ok(())
	}

	#[benchmark]
	fn redeem_credit() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::setup_currency();
		let caller: T::AccountId = whitelisted_caller();
		let destination: T::AccountId = account("destination", 0, 0);
		Score::<T>::onboard_for_recognition(&caller)?;
		let key = AccountOrPerson::Account(caller.clone());
		let credit_needed: BalanceOf<T> = 10u32.into();
		Participants::<T>::mutate(&key, |maybe_p| {
			let mut p = maybe_p.take().expect("participant must exist");
			p.credit = credit_needed;
			*maybe_p = Some(p);
		});
		let pot = Score::<T>::score_pot_id();
		let ed = T::Currency::minimum_balance();
		T::Currency::mint_into(&destination, ed)?;
		T::Currency::mint_into(&pot, credit_needed + ed)?;
		T::Currency::hold(&HoldReason::Credit.into(), &pot, credit_needed)?;

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller.clone()), destination);

		let after = Participants::<T>::get(&key).expect("entry exists");
		assert!(after.credit.is_zero(), "all credit must be consumed");

		let remaining_hold = T::Currency::balance_on_hold(&HoldReason::Credit.into(), &pot);
		assert!(remaining_hold.is_zero(), "credit hold released after payout transfer");

		Ok(())
	}

	#[benchmark]
	fn register() -> Result<(), BenchmarkError> {
		// Set up the member collection required for personhood operations
		T::People::initialize_people_collection();

		let caller: T::AccountId = whitelisted_caller();
		Score::<T>::onboard_for_recognition(&caller)?;

		let participant_key = AccountOrPerson::Account(caller.clone());
		Participants::<T>::mutate(&participant_key, |maybe_p| {
			let mut p = maybe_p.take().expect("participant must exist");
			p.score = PersonhoodThreshold::<T>::get();
			p.reached_personhood = true;
			*maybe_p = Some(p);
		});

		let sk = T::Crypto::new_secret([12; 32]);
		let pk = T::Crypto::member_from_secret(&sk);
		let proof_of_ownership = {
			let mut msg = b"pop register using".to_vec();
			msg.extend_from_slice(&caller.encode());
			T::Crypto::sign(&sk, &msg[..]).unwrap()
		};

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller.clone()), Some((pk, proof_of_ownership)));

		let after = Participants::<T>::get(&participant_key).expect("entry must exist");
		assert!(
			matches!(after.recognition, Recognition::Recognized(_)),
			"participant must now be Recognized"
		);

		Ok(())
	}

	#[benchmark]
	fn set_absence_grace_schedule() -> Result<(), BenchmarkError> {
		let bound = AbsenceGraceTiers::bound() as u32;
		let mut tiers = Vec::with_capacity(bound as usize);
		for i in 0..bound {
			tiers.push(AbsenceGraceTier {
				population_size_threshold: (i + 1) * 1_000,
				window: (i + 1) as u8,
				allowed_misses: i as u8,
			});
		}
		let schedule: AbsenceGraceTiers = BoundedVec::try_from(tiers).unwrap();

		#[extrinsic_call]
		_(SystemOrigin::Root, schedule.clone());

		assert_eq!(AbsenceGraceSchedule::<T>::get(), schedule);

		Ok(())
	}

	#[benchmark]
	fn set_personhood_threshold_schedule() -> Result<(), BenchmarkError> {
		// Maximum-length schedule — worst case for validation cost.
		let schedule: PersonhoodThresholdTiers = BoundedVec::try_from(alloc::vec![
			PersonhoodThresholdTier { population_size_threshold: 1_000, score_threshold: 1 },
			PersonhoodThresholdTier { population_size_threshold: 5_000, score_threshold: 2 },
			PersonhoodThresholdTier { population_size_threshold: 10_000, score_threshold: 4 },
			PersonhoodThresholdTier { population_size_threshold: 20_000, score_threshold: 6 },
			PersonhoodThresholdTier { population_size_threshold: 28_000, score_threshold: 8 },
			PersonhoodThresholdTier { population_size_threshold: 35_000, score_threshold: 11 },
			PersonhoodThresholdTier { population_size_threshold: 42_000, score_threshold: 12 },
			PersonhoodThresholdTier { population_size_threshold: 50_000, score_threshold: 15 },
			PersonhoodThresholdTier { population_size_threshold: 58_000, score_threshold: 16 },
			PersonhoodThresholdTier { population_size_threshold: 65_000, score_threshold: 17 },
			PersonhoodThresholdTier { population_size_threshold: 72_000, score_threshold: 18 },
			PersonhoodThresholdTier { population_size_threshold: 80_000, score_threshold: 18 },
			PersonhoodThresholdTier { population_size_threshold: 90_000, score_threshold: 19 },
			PersonhoodThresholdTier { population_size_threshold: 100_000, score_threshold: 20 },
			PersonhoodThresholdTier { population_size_threshold: 120_000, score_threshold: 21 },
			PersonhoodThresholdTier { population_size_threshold: u32::MAX, score_threshold: 21 },
		])
		.expect("schedule length <= MAX_PERSONHOOD_THRESHOLD_TIERS");

		assert_eq!(schedule.len() as u32, MAX_PERSONHOOD_THRESHOLD_TIERS);

		#[extrinsic_call]
		_(SystemOrigin::Root, schedule.clone());

		assert_eq!(PersonhoodThresholdSchedule::<T>::get(), schedule);
		Ok(())
	}

	#[benchmark]
	fn set_manager_account() -> Result<(), BenchmarkError> {
		let manager: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		_(SystemOrigin::Root, Some(manager.clone()));

		assert_eq!(ManagerAccount::<T>::get(), Some(manager));
		Ok(())
	}

	#[benchmark]
	fn set_payout_account() -> Result<(), BenchmarkError> {
		let new: T::AccountId = whitelisted_caller();
		let old = PayoutAccount::<T>::get();
		T::Currency::mint_into(&old, T::Currency::minimum_balance() + 10u32.into())?;

		#[extrinsic_call]
		_(SystemOrigin::Root, new.clone());

		assert_eq!(PayoutAccount::<T>::get(), new);
		assert!(T::Currency::total_balance(&old).is_zero());
		Ok(())
	}

	#[benchmark]
	fn as_participant_tx_ext() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		Score::<T>::onboard_for_recognition(&caller)?;

		let tx_ext =
			ScoreAsParticipant::<T>::new(Some(ScoreAsParticipantData { nonce: 0u32.into() }));
		let origin = SystemOrigin::Signed(caller.clone());
		let call: <T as frame_system::Config>::RuntimeCall = Call::<T>::cash_out {}.into();
		let len = call.encode().len();
		frame_system::Pallet::<T>::inc_providers(&caller);

		#[block]
		{
			tx_ext
				.test_run(origin.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
