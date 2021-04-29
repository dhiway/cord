// // Copyright 2019-2021 Dhiway.
// // This file is part of CORD Platform.

// //! Benchmarking of Reserve


// #![cfg(feature = "runtime-benchmarks")]

// use super::*;

// use frame_benchmarking::{account, benchmarks};
// use frame_system::RawOrigin;
// use sp_std::{vec, vec::Vec, boxed::Box};

// const SEED: u32 = 0;

// benchmarks! {
	
// 	// transfer {
// 	// 	let caller :T::AccountId = account("sender", 0, SEED);
//     //     let to :T::AccountId = account("to", 1, SEED);

// 	// 	let amount:BalanceOf<T, I> = 1 ;

// 	// }: _(RawOrigin::Signed(caller.clone()),to,amount)
// 	// verify {
// 	// 	// DIDs::<T>::contains_key(caller)
// 	// }

//     receive {
// 		let caller :T::AccountId = account("sender", 0, SEED);
//         let balance = T::Currency::free_balance(&caller);
// 		let amount:BalanceOf<T,I> ;
//         ensure!(
//             balance >= amount
//         );

// 	}: _(RawOrigin::Signed(caller.clone()),amount)
// 	verify {
// 		// DIDs::<T>::contains_key(caller)
// 	}

// }

// #[cfg(test)]
// mod tests {
// 	use super::*;
// 	use crate::tests::{new_test_ext, Test};
// 	use frame_support::assert_ok;

// 	#[test]
// 	fn test_benchmarks() {
// 		new_test_ext().execute_with(|| {
// 			assert_ok!(test_benchmark_add::<Test>());
// 		});
// 	}
// }