// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_im_online
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_im_online
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

/// Weight functions needed for pallet_im_online.
pub trait WeightInfo {
fn validate_unsigned_and_then_heartbeat(k: u32, e: u32, ) -> Weight;
}

//// Weights for pallet_im_online using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn validate_unsigned_and_then_heartbeat(k: u32, e: u32, ) -> Weight {
				(801_600_000 as Weight)
				// Standard Error: 442_000
				.saturating_add((14_617_000 as Weight).saturating_mul(k as Weight))
				// Standard Error: 4_466_000
				.saturating_add((7_571_000 as Weight).saturating_mul(e as Weight))
				.saturating_add(T::DbWeight::get().reads(5 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn validate_unsigned_and_then_heartbeat(k: u32, e: u32, ) -> Weight {
				(801_600_000 as Weight)
				// Standard Error: 442_000
				.saturating_add((14_617_000 as Weight).saturating_mul(k as Weight))
				// Standard Error: 4_466_000
				.saturating_add((7_571_000 as Weight).saturating_mul(e as Weight))
				.saturating_add(RocksDbWeight::get().reads(5 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				}