// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_scheduler
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_scheduler
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

/// Weight functions needed for pallet_scheduler.
pub trait WeightInfo {
fn schedule(s: u32, ) -> Weight;
fn cancel(s: u32, ) -> Weight;
fn schedule_named(s: u32, ) -> Weight;
fn cancel_named(s: u32, ) -> Weight;
}

//// Weights for pallet_scheduler using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn schedule(s: u32, ) -> Weight {
				(129_600_000 as Weight)
				// Standard Error: 836_000
				.saturating_add((1_787_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn cancel(s: u32, ) -> Weight {
				(37_188_000 as Weight)
				// Standard Error: 7_019_000
				.saturating_add((189_063_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn schedule_named(_s: u32, ) -> Weight {
				(265_700_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn cancel_named(s: u32, ) -> Weight {
				(233_172_000 as Weight)
				// Standard Error: 3_867_000
				.saturating_add((165_328_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn schedule(s: u32, ) -> Weight {
				(129_600_000 as Weight)
				// Standard Error: 836_000
				.saturating_add((1_787_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn cancel(s: u32, ) -> Weight {
				(37_188_000 as Weight)
				// Standard Error: 7_019_000
				.saturating_add((189_063_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn schedule_named(_s: u32, ) -> Weight {
				(265_700_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn cancel_named(s: u32, ) -> Weight {
				(233_172_000 as Weight)
				// Standard Error: 3_867_000
				.saturating_add((165_328_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				}