// This file is part of CORD – https://cord.network

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
