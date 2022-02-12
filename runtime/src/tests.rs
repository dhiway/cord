// CORD Chain Node – https://cord.network
// Copyright (C) 2019-2022 Dhiway
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::*;
use codec::Encode;
use frame_support::weights::{GetDispatchInfo, WeightToFeePolynomial};
use frame_system::offchain::CreateSignedTransaction;
use pallet_transaction_payment::Multiplier;
use separator::Separatable;
use sp_runtime::{assert_eq_error_rate, FixedPointNumber};

#[test]
fn payout_weight_portion() {
	use pallet_staking::WeightInfo;
	let payout_weight = <Runtime as pallet_staking::Config>::WeightInfo::payout_stakers_alive_staked(
		MaxNominatorRewardedPerValidator::get(),
	) as f64;
	let block_weight = BlockWeights::get().max_block as f64;

	println!(
		"a full payout takes {:.2} of the block weight [{} / {}]",
		payout_weight / block_weight,
		payout_weight,
		block_weight
	);
	assert!(payout_weight * 2f64 < block_weight);
}

#[test]
#[ignore]
fn block_cost() {
	let max_block_weight = BlockWeights::get().max_block;
	let raw_fee = WeightToFee::calc(&max_block_weight);

	println!(
		"Full Block weight == {} // WeightToFee(full_block) == {} plank",
		max_block_weight,
		raw_fee.separated_string(),
	);
}

#[test]
#[ignore]
fn transfer_cost_min_multiplier() {
	let min_multiplier = runtime_common::MinimumMultiplier::get();
	let call = pallet_balances::Call::<Runtime>::transfer_keep_alive {
		dest: Default::default(),
		value: Default::default(),
	};
	let info = call.get_dispatch_info();
	// convert to outer call.
	let call = Call::Balances(call);
	let len = call.using_encoded(|e| e.len()) as u32;

	let mut ext = sp_io::TestExternalities::new_empty();
	let mut test_with_multiplier = |m| {
		ext.execute_with(|| {
			pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(m);
			let fee = TransactionPayment::compute_fee(len, &info, 0);
			println!(
				"weight = {:?} // multiplier = {:?} // full transfer fee = {:?}",
				info.weight.separated_string(),
				pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get(),
				fee.separated_string(),
			);
		});
	};

	test_with_multiplier(min_multiplier);
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000_000u128));
	test_with_multiplier(Multiplier::saturating_from_rational(1, 1_000_000_000u128));
}

#[test]
fn full_block_council_election_cost() {
	// the number of voters needed to consume almost a full block in council election, and how
	// much it is going to cost.
	use pallet_elections_phragmen::WeightInfo;

	// Loser candidate lose a lot of money; sybil attack by candidates is even more expensive,
	// and we don't care about it here. For now, we assume no extra candidates, and only
	// superfluous voters.
	let candidates = DesiredMembers::get() + DesiredRunnersUp::get();
	let mut voters = 1u32;
	let weight_with = |v| {
		<Runtime as pallet_elections_phragmen::Config>::WeightInfo::election_phragmen(
			candidates,
			v,
			v * 16,
		)
	};

	while weight_with(voters) <= BlockWeights::get().max_block {
		voters += 1;
	}

	let cost = voters as Balance * (VotingBondBase::get() + 16 * VotingBondFactor::get());
	let cost_ways = cost / WAY;
	println!(
		"can support {} voters in a single block for council elections; total bond {}",
		voters, cost_ways,
	);
	assert!(cost_ways > 150_000); // DOLLAR ~ new DOT ~ 10e10
}

#[test]
fn nominator_limit() {
	use pallet_election_provider_multi_phase::WeightInfo;
	// starting point of the nominators.
	let target_voters: u32 = 10_000;

	// assuming we want around 5k candidates and 1k active validators. (March 31, 2021)
	let all_targets: u32 = 5_000;
	let desired: u32 = 1_000;
	let weight_with = |active| {
		<Runtime as pallet_election_provider_multi_phase::Config>::WeightInfo::submit_unsigned(
			active,
			all_targets,
			active,
			desired,
		)
	};

	let mut active = target_voters;
	while weight_with(active) <= OffchainSolutionWeightLimit::get() || active == target_voters {
		active += 1;
	}

	println!("can support {} nominators to yield a weight of {}", active, weight_with(active));
	assert!(active > target_voters, "we need to reevaluate the weight of the election system");
}

#[test]
fn signed_deposit_is_sensible() {
	// ensure this number does not change, or that it is checked after each change.
	// a 1 MB solution should take (40 + 10) DOTs of deposit.
	let deposit = SignedDepositBase::get() + (SignedDepositByte::get() * 1024 * 1024);
	assert_eq_error_rate!(deposit, 50 * WAY, WAY);
}

#[test]
fn validate_transaction_submitter_bounds() {
	fn is_submit_signed_transaction<T>()
	where
		T: CreateSignedTransaction<Call>,
	{
	}

	is_submit_signed_transaction::<Runtime>();
}
#[test]
fn perbill_as_onchain_accuracy() {
	type OnChainAccuracy = <Runtime as onchain::Config>::Accuracy;
	let maximum_chain_accuracy: Vec<UpperOf<OnChainAccuracy>> = (0..MAX_NOMINATIONS)
		.map(|_| <UpperOf<OnChainAccuracy>>::from(OnChainAccuracy::one().deconstruct()))
		.collect();
	let _: UpperOf<OnChainAccuracy> =
		maximum_chain_accuracy.iter().fold(0, |acc, x| acc.checked_add(*x).unwrap());
}

#[test]
fn call_size() {
	assert!(
			core::mem::size_of::<Call>() <= 200,
			"size of Call is more than 200 bytes: some calls have too big arguments, use Box to reduce the
			size of Call.
			If the limit is too strong, maybe consider increase the limit to 300.",
		);
}
