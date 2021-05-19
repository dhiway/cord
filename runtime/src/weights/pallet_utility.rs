// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_utility
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_utility
// --extrinsic=*
// --execution=Wasm
// --wasm-execution=Interpreted
// --heap-pages=4096
// --output=./runtime/src/weights/
// --template=./.maintain/weight-template.hbs


#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_utility.
pub trait WeightInfo {
fn batch(c: u32, ) -> Weight;
fn as_derivative() -> Weight;
fn batch_all(c: u32, ) -> Weight;
}

//// Weights for pallet_utility using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn batch(c: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 666_000
				.saturating_add((73_766_000 as Weight).saturating_mul(c as Weight))
				}
				fn as_derivative() -> Weight {
				(71_000_000 as Weight)
				}
				fn batch_all(c: u32, ) -> Weight {
				(233_100_000 as Weight)
				// Standard Error: 428_000
				.saturating_add((71_220_000 as Weight).saturating_mul(c as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn batch(c: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 666_000
				.saturating_add((73_766_000 as Weight).saturating_mul(c as Weight))
				}
				fn as_derivative() -> Weight {
				(71_000_000 as Weight)
				}
				fn batch_all(c: u32, ) -> Weight {
				(233_100_000 as Weight)
				// Standard Error: 428_000
				.saturating_add((71_220_000 as Weight).saturating_mul(c as Weight))
				}
				}