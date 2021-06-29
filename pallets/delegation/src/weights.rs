// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_delegation DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_delegation
// --extrinsic=*
// --execution=Wasm
// --wasm-execution=Interpreted
// --heap-pages=4096
// --output=./pallets/delegation/src/weights.rs
// --template=./.maintain/weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for delegation.
pub trait WeightInfo {
	fn create_root() -> Weight;
	fn revoke_root(r: u32, ) -> Weight;
	fn add_delegation() -> Weight;
	fn revoke_delegation_root_child(r: u32, c: u32, ) -> Weight;
	fn revoke_delegation_leaf(r: u32, c: u32, ) -> Weight;
}

/// Weights for delegation using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_root() -> Weight {
		(22_442_000_u64)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn revoke_root(r: u32, ) -> Weight {
		(24_418_000_u64)
			// Standard Error: 29_000
			.saturating_add((17_591_000_u64).saturating_mul(r as Weight))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes(1_u64))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(r as Weight)))
	}
	fn add_delegation() -> Weight {
		(83_497_000_u64)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	fn revoke_delegation_root_child(r: u32, _c: u32, ) -> Weight {
		(11_210_000_u64)
			// Standard Error: 55_000
			.saturating_add((17_485_000_u64).saturating_mul(r as Weight))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(r as Weight)))
	}
	fn revoke_delegation_leaf(r: u32, c: u32, ) -> Weight {
		(27_413_000_u64)
			// Standard Error: 30_000
			.saturating_add((111_000_u64).saturating_mul(r as Weight))
			// Standard Error: 30_000
			.saturating_add((4_692_000_u64).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(c as Weight)))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn create_root() -> Weight {
		(22_442_000_u64)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn revoke_root(r: u32, ) -> Weight {
		(24_418_000_u64)
			// Standard Error: 29_000
			.saturating_add((17_591_000_u64).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().reads((2_u64).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(r as Weight)))
	}
	fn add_delegation() -> Weight {
		(83_497_000_u64)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	fn revoke_delegation_root_child(r: u32, _c: u32, ) -> Weight {
		(11_210_000_u64)
			// Standard Error: 55_000
			.saturating_add((17_485_000_u64).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads((2_u64).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(r as Weight)))
	}
	fn revoke_delegation_leaf(r: u32, c: u32, ) -> Weight {
		(27_413_000_u64)
			// Standard Error: 30_000
			.saturating_add((111_000_u64).saturating_mul(r as Weight))
			// Standard Error: 30_000
			.saturating_add((4_692_000_u64).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().reads((1_u64).saturating_mul(c as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
