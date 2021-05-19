// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

<<<<<<< HEAD
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_delegation DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
=======
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_delegation
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
>>>>>>> origin/develop
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


#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_delegation.
pub trait WeightInfo {
fn create_root() -> Weight;
fn revoke_root(r: u32, ) -> Weight;
fn add_delegation() -> Weight;
fn revoke_delegation_root_child(r: u32, ) -> Weight;
fn revoke_delegation_leaf(r: u32, ) -> Weight;
}

<<<<<<< HEAD
//// Weights for pallet_delegation using the Substrate node and recommended
//// hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_root() -> Weight {
		(114_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn revoke_root(r: u32) -> Weight {
		(133_750_000 as Weight)
			// Standard Error: 1_696_000
			.saturating_add((173_550_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
	}
	fn add_delegation() -> Weight {
		(352_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn revoke_delegation_root_child(r: u32) -> Weight {
		(51_450_000 as Weight)
			// Standard Error: 2_505_000
			.saturating_add((172_050_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
	}
	fn revoke_delegation_leaf(r: u32) -> Weight {
		(188_650_000 as Weight)
			// Standard Error: 14_091_000
			.saturating_add((70_350_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn create_root() -> Weight {
		(114_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn revoke_root(r: u32) -> Weight {
		(133_750_000 as Weight)
			// Standard Error: 1_696_000
			.saturating_add((173_550_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().reads((2 as Weight).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
	}
	fn add_delegation() -> Weight {
		(352_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(4 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	fn revoke_delegation_root_child(r: u32) -> Weight {
		(51_450_000 as Weight)
			// Standard Error: 2_505_000
			.saturating_add((172_050_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads((2 as Weight).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
	}
	fn revoke_delegation_leaf(r: u32) -> Weight {
		(188_650_000 as Weight)
			// Standard Error: 14_091_000
			.saturating_add((70_350_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
=======
//// Weights for pallet_delegation using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn create_root() -> Weight {
				(114_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn revoke_root(r: u32, ) -> Weight {
				(133_750_000 as Weight)
				// Standard Error: 1_696_000
				.saturating_add((173_550_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(r as
				Weight)))
				}
				fn add_delegation() -> Weight {
				(352_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn revoke_delegation_root_child(r: u32, ) -> Weight {
				(51_450_000 as Weight)
				// Standard Error: 2_505_000
				.saturating_add((172_050_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(r as
				Weight)))
				}
				fn revoke_delegation_leaf(r: u32, ) -> Weight {
				(188_650_000 as Weight)
				// Standard Error: 14_091_000
				.saturating_add((70_350_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn create_root() -> Weight {
				(114_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn revoke_root(r: u32, ) -> Weight {
				(133_750_000 as Weight)
				// Standard Error: 1_696_000
				.saturating_add((173_550_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().reads((2 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(r as
				Weight)))
				}
				fn add_delegation() -> Weight {
				(352_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn revoke_delegation_root_child(r: u32, ) -> Weight {
				(51_450_000 as Weight)
				// Standard Error: 2_505_000
				.saturating_add((172_050_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(RocksDbWeight::get().reads((2 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(r as
				Weight)))
				}
				fn revoke_delegation_leaf(r: u32, ) -> Weight {
				(188_650_000 as Weight)
				// Standard Error: 14_091_000
				.saturating_add((70_350_000 as Weight).saturating_mul(r as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(r as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				}
>>>>>>> origin/develop
