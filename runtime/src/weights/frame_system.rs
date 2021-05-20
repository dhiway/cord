// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for frame_system DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=frame_system
// --extrinsic=*
// --execution=Wasm
// --wasm-execution=Interpreted
// --heap-pages=4096
// --output=./runtime/src/weights/
// --template=./.maintain/weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for frame_system.
pub trait WeightInfo {
	fn remark(b: u32) -> Weight;
	fn set_heap_pages() -> Weight;
	fn set_changes_trie_config() -> Weight;
	fn set_storage(i: u32) -> Weight;
	fn kill_storage(i: u32) -> Weight;
	fn kill_prefix(p: u32) -> Weight;
}

//// Weights for frame_system using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn remark(_b: u32) -> Weight {
		(7_900_000 as Weight)
	}
	fn set_heap_pages() -> Weight {
		(12_000_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn set_changes_trie_config() -> Weight {
		(29_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn set_storage(i: u32) -> Weight {
		(3_601_000 as Weight)
			// Standard Error: 6_000
			.saturating_add((1_699_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
	}
	fn kill_storage(i: u32) -> Weight {
		(25_743_000 as Weight)
			// Standard Error: 33_000
			.saturating_add((1_157_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
	}
	fn kill_prefix(p: u32) -> Weight {
		(59_701_000 as Weight)
			// Standard Error: 54_000
			.saturating_add((1_749_000 as Weight).saturating_mul(p as Weight))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(p as Weight)))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn remark(_b: u32) -> Weight {
		(7_900_000 as Weight)
	}
	fn set_heap_pages() -> Weight {
		(12_000_000 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn set_changes_trie_config() -> Weight {
		(29_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	fn set_storage(i: u32) -> Weight {
		(3_601_000 as Weight)
			// Standard Error: 6_000
			.saturating_add((1_699_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
	}
	fn kill_storage(i: u32) -> Weight {
		(25_743_000 as Weight)
			// Standard Error: 33_000
			.saturating_add((1_157_000 as Weight).saturating_mul(i as Weight))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(i as Weight)))
	}
	fn kill_prefix(p: u32) -> Weight {
		(59_701_000 as Weight)
			// Standard Error: 54_000
			.saturating_add((1_749_000 as Weight).saturating_mul(p as Weight))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(p as Weight)))
	}
}
