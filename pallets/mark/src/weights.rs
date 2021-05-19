// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

<<<<<<< HEAD
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_mark DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
=======
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_mark
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
>>>>>>> origin/develop
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_mark
// --extrinsic=*
// --execution=Wasm
// --wasm-execution=Interpreted
// --heap-pages=4096
// --output=./pallets/mark/src/weights.rs
// --template=./.maintain/weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_mark.
pub trait WeightInfo {
<<<<<<< HEAD
	fn anchor() -> Weight;
	fn revoke(d: u32) -> Weight;
	fn restore(d: u32) -> Weight;
=======
fn anchor() -> Weight;
fn revoke(d: u32, ) -> Weight;
fn restore(d: u32, ) -> Weight;
>>>>>>> origin/develop
}

//// Weights for pallet_mark using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
<<<<<<< HEAD
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn anchor() -> Weight {
		(422_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn revoke(d: u32) -> Weight {
		(183_367_000 as Weight)
			// Standard Error: 861_000
			.saturating_add((56_433_000 as Weight).saturating_mul(d as Weight))
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(d as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn restore(d: u32) -> Weight {
		(155_567_000 as Weight)
			// Standard Error: 2_307_000
			.saturating_add((63_533_000 as Weight).saturating_mul(d as Weight))
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(d as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn anchor() -> Weight {
		(422_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	fn revoke(d: u32) -> Weight {
		(183_367_000 as Weight)
			// Standard Error: 861_000
			.saturating_add((56_433_000 as Weight).saturating_mul(d as Weight))
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(d as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn restore(d: u32) -> Weight {
		(155_567_000 as Weight)
			// Standard Error: 2_307_000
			.saturating_add((63_533_000 as Weight).saturating_mul(d as Weight))
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(d as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
=======
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn anchor() -> Weight {
				(422_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(5 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn revoke(d: u32, ) -> Weight {
				(183_367_000 as Weight)
				// Standard Error: 861_000
				.saturating_add((56_433_000 as Weight).saturating_mul(d as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(d as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn restore(d: u32, ) -> Weight {
				(155_567_000 as Weight)
				// Standard Error: 2_307_000
				.saturating_add((63_533_000 as Weight).saturating_mul(d as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(d as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn anchor() -> Weight {
				(422_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(5 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn revoke(d: u32, ) -> Weight {
				(183_367_000 as Weight)
				// Standard Error: 861_000
				.saturating_add((56_433_000 as Weight).saturating_mul(d as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(d as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn restore(d: u32, ) -> Weight {
				(155_567_000 as Weight)
				// Standard Error: 2_307_000
				.saturating_add((63_533_000 as Weight).saturating_mul(d as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(d as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				}
>>>>>>> origin/develop
