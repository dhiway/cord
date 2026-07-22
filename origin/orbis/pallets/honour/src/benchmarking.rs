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

use crate::*;
use codec::Encode;
use core::marker::{Send, Sync};
use frame_benchmarking::{v2::*, BenchmarkError};
use frame_support::traits::UnixTime;
use sp_runtime::{
	traits::{AsTransactionAuthorizedOrigin, DispatchTransaction},
	Saturating,
};

fn advance_to<T: Config>(b: u32) {
	use frame_system::Pallet as System;

	while System::<T>::block_number() < b.into() {
		System::<T>::set_block_number(System::<T>::block_number().saturating_add(1u32.into()));
	}
}

fn assert_has_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

/// Non-zero clock value (seconds since the Unix epoch) the benchmarks set before reading
/// [`Config::Clock`], so `UnixTime::now()` is not queried at genesis (which logs an error and
/// returns `0`).
const BENCH_TIME: Seconds = 1_000;

/// Seeds the membership ring used by [`VoterAuth`](crate::extension::VoterAuth) and produces a
/// proof that verifies for a given vote and message.
///
/// The proof must validate against whatever ring the runtime's [`Config::MemberService`] actually
/// holds, so the setup is environment-specific: the pallet mock proves against its in-process mock
/// ring, while a real runtime seeds its configured member service and proves against that ring.
pub trait BenchmarkHelper<T: Config> {
	/// Set [`Config::Clock`] to `now` seconds since the Unix epoch.
	fn set_time(now: Seconds);

	/// Seed the ring at ring index `0` and return a proof valid for `vote` + `message`.
	fn seed_and_create_proof(vote: &VoteData, message: &[u8]) -> RingProofOf<T>;
}

#[benchmarks(
	where T: Send + Sync,
		<T as frame_system::Config>::RuntimeCall:
			Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<Call<T>> + From<Call<T>> + GetDispatchInfo,
		<T as frame_system::Config>::RuntimeOrigin: AsTransactionAuthorizedOrigin,
)]
mod benches {
	use frame_support::{
		dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
		traits::IsSubType,
	};
	use frame_system::RawOrigin;
	use sp_io::hashing::blake2_256;
	use sp_runtime::{generic::ExtensionVersion, traits::Dispatchable};

	use crate::extension::{VoterAuth, VoterAuthData};

	use super::*;

	#[benchmark]
	fn bestow() -> Result<(), BenchmarkError> {
		const POINT: PointId = 0;
		const POINT_ALIAS: PointAlias = [1; 32];

		const SUBJECT_1: SubjectId = [2; 32];
		const SUBJECT_1_ALIAS: SubjectAlias = [3; 32];

		const SUBJECT_2: SubjectId = [4; 32];
		const SUBJECT_2_ALIAS: SubjectAlias = [5; 32];

		T::BenchmarkHelper::set_time(BENCH_TIME);
		advance_to::<T>(2);
		let account: T::AccountId = whitelisted_caller();

		let origin: <T as frame_system::Config>::RuntimeOrigin = Origin::Voter {
			account: account.clone(),
			aliases: VoteAliases { subject_alias: SUBJECT_1_ALIAS, point_alias: POINT_ALIAS },
		}
		.into();
		let vote_data =
			VoteData { subject: SUBJECT_1, point: POINT, direction: Direction::Honourable };
		Pallet::<T>::bestow(origin, vote_data, Default::default())
			.map_err(|_| BenchmarkError::Stop("bestow failed"))?;

		assert_has_event::<T>(
			Event::<T>::VoteCast { subject: SUBJECT_1, direction: Direction::Honourable }.into(),
		);

		let origin: <T as frame_system::Config>::RuntimeOrigin = Origin::Voter {
			account,
			aliases: VoteAliases { subject_alias: SUBJECT_2_ALIAS, point_alias: POINT_ALIAS },
		}
		.into();
		let vote_data =
			VoteData { subject: SUBJECT_2, point: POINT, direction: Direction::Honourable };
		#[extrinsic_call]
		_(origin, vote_data, Default::default());

		assert_has_event::<T>(
			Event::<T>::VoteReused {
				old_subject: SUBJECT_1,
				old_direction: Direction::Honourable,
				new_subject: SUBJECT_2,
				new_direction: Direction::Honourable,
			}
			.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn extension_validate() -> Result<(), BenchmarkError> {
		T::BenchmarkHelper::set_time(BENCH_TIME);
		let account: T::AccountId = whitelisted_caller();

		let vote = VoteData { subject: [1; 32], point: 0, direction: Direction::Honourable };
		let now = <T as Config>::Clock::now().as_secs();

		let call = Call::bestow { vote: vote.clone(), call_valid_from: now };
		let call: <T as frame_system::Config>::RuntimeCall = call.into();

		let ext_version: ExtensionVersion = 0;
		let message = (ext_version, &call, &account).using_encoded(blake2_256);

		let proof = T::BenchmarkHelper::seed_and_create_proof(&vote, &message);

		let extension: VoterAuth<T> = VoterAuth::new(Some(VoterAuthData {
			account: account.clone(),
			proof,
			ring_index: 0,
			revision: 0,
		}));

		let info = call.get_dispatch_info();
		let len = call.encoded_size();
		let post_info = PostDispatchInfo::default();

		#[block]
		{
			extension
				.test_run(RawOrigin::Signed(account).into(), &call, &info, len, 0, |_| {
					Ok(post_info)
				})
				.unwrap()
				.unwrap();
		}

		Ok(())
	}

	// Implements a test for each benchmark. Execute with:
	// `cargo test -p indiv-pallet-honour --features runtime-benchmarks`.
	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
