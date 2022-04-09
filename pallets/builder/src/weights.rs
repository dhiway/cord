// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_reserve DATE: 2021-05-17, STEPS: `[20, ]`, REPEAT: 10,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --execution=wasm
// --pallet=pallet_reserve
// --extrinsic=*
// --steps=20
// --repeat=10
// --output=./pallets/reserve/src/weights.rs
// --template=./.maintain/weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_builder.
pub trait WeightInfo {
	fn transfer() -> Weight;
	fn receive() -> Weight;
}

//// Weights for pallet_reserve using the Substrate node and recommended
//// hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn transfer() -> Weight {
		(254_453_000 as Weight).saturating_add(T::DbWeight::get().reads(2 as Weight))
	}
	fn receive() -> Weight {
		(112_761_000 as Weight)
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn transfer() -> Weight {
		(254_453_000 as Weight).saturating_add(RocksDbWeight::get().reads(2 as Weight))
	}
	fn receive() -> Weight {
		(112_761_000 as Weight)
	}
}
