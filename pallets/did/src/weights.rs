// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

<<<<<<< HEAD
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_did DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
=======
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_did
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
>>>>>>> origin/develop
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_did
// --extrinsic=*
// --execution=Wasm
// --wasm-execution=Interpreted
// --heap-pages=4096
// --output=./pallets/did/src/weights.rs
// --template=./.maintain/weight-template.hbs


#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_did.
pub trait WeightInfo {
fn anchor() -> Weight;
fn remove() -> Weight;
}

//// Weights for pallet_did using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
<<<<<<< HEAD
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn anchor() -> Weight {
		(89_000_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn remove() -> Weight {
		(71_000_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn anchor() -> Weight {
		(89_000_000 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn remove() -> Weight {
		(71_000_000 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
=======
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn anchor() -> Weight {
				(89_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn remove() -> Weight {
				(71_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn anchor() -> Weight {
				(89_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn remove() -> Weight {
				(71_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				}
>>>>>>> origin/develop
