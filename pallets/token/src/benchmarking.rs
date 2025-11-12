// This file is part of CORD – https://cord.network

use super::*;
use crate::{EventBlock, EventTypeOf};
use cord_primitives::identifier::Ss58Identifier;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use sp_core::H256;

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
		let digest = H256::random();
		let action = sample_action();
		let seal = EventBlock { height: 1, index: 0 };

		#[block]
		{
			Pallet::<T>::state_event(&token, digest, action.clone(), seal.clone()).unwrap();
		}

		#[verify]
		{
			let latest = StateHistory::<T>::get(&token, 0).expect("event written");
			assert_eq!(latest.action, action);
			assert_eq!(latest.seal.height, seal.height);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::Test;
	use frame_benchmarking::v2::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), Test);
}
