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
use crate::{EventBlock, EventTypeOf};
use alloc::vec;
use frame_benchmarking::v2::*;
use origin_primitives::identifier::Ss58Identifier;
use sp_runtime::traits::Hash as _;

fn sample_token() -> Ss58Identifier {
	let digest = [42u8; 32];
	Ss58Identifier::to_encoded(digest, 100, 5, 0).expect("valid token")
}

fn sample_action() -> EventTypeOf {
	let payload = vec![1u8; 32];
	payload.try_into().expect("bounded")
}

#[benchmarks(where AccountId32: From<<T as frame_system::Config>::AccountId>)]
mod benches {
	use super::*;

	#[benchmark]
	fn record_state_event() {
		let token = sample_token();
		let digest = T::Hashing::hash(b"orbis-token-state-event-benchmark");
		let action = sample_action();
		let seal = EventBlock { height: 1, index: 0 };

		#[block]
		{
			<Pallet<T> as Token<T>>::state_event(&token, digest, action.clone(), seal.clone())
				.unwrap();
		}

		let latest = StateHistory::<T>::get(&token, 0).expect("event written");
		assert_eq!(latest.action, action);
		assert_eq!(latest.seal.height, seal.height);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
