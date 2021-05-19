// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_multisig
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_multisig
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

/// Weight functions needed for pallet_multisig.
pub trait WeightInfo {
fn as_multi_threshold_1(z: u32, ) -> Weight;
fn as_multi_create(s: u32, z: u32, ) -> Weight;
fn as_multi_create_store(s: u32, z: u32, ) -> Weight;
fn as_multi_approve(s: u32, z: u32, ) -> Weight;
fn as_multi_approve_store(s: u32, z: u32, ) -> Weight;
fn as_multi_complete(s: u32, z: u32, ) -> Weight;
fn approve_as_multi_create(s: u32, ) -> Weight;
fn approve_as_multi_approve(s: u32, ) -> Weight;
fn approve_as_multi_complete(s: u32, ) -> Weight;
fn cancel_as_multi(s: u32, ) -> Weight;
}

//// Weights for pallet_multisig using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn as_multi_threshold_1(z: u32, ) -> Weight {
				(157_800_000 as Weight)
				// Standard Error: 1_000
				.saturating_add((57_000 as Weight).saturating_mul(z as Weight))
				}
				fn as_multi_create(s: u32, z: u32, ) -> Weight {
				(320_404_000 as Weight)
				// Standard Error: 52_000
				.saturating_add((864_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 0
				.saturating_add((1_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn as_multi_create_store(s: u32, z: u32, ) -> Weight {
				(279_242_000 as Weight)
				// Standard Error: 313_000
				.saturating_add((1_414_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 3_000
				.saturating_add((119_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(T::DbWeight::get().reads(3 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn as_multi_approve(s: u32, z: u32, ) -> Weight {
				(240_319_000 as Weight)
				// Standard Error: 130_000
				.saturating_add((1_943_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 1_000
				.saturating_add((1_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn as_multi_approve_store(s: u32, z: u32, ) -> Weight {
				(400_048_000 as Weight)
				// Standard Error: 324_000
				.saturating_add((2_025_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 3_000
				.saturating_add((114_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn as_multi_complete(s: u32, z: u32, ) -> Weight {
				(660_564_000 as Weight)
				// Standard Error: 518_000
				.saturating_add((10_714_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 5_000
				.saturating_add((276_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(T::DbWeight::get().reads(3 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn approve_as_multi_create(s: u32, ) -> Weight {
				(326_063_000 as Weight)
				// Standard Error: 100_000
				.saturating_add((919_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn approve_as_multi_approve(s: u32, ) -> Weight {
				(213_594_000 as Weight)
				// Standard Error: 76_000
				.saturating_add((2_278_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn approve_as_multi_complete(s: u32, ) -> Weight {
				(5_266_016_000 as Weight)
				// Standard Error: 1_523_000
				.saturating_add((12_592_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(3 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn cancel_as_multi(s: u32, ) -> Weight {
				(2_380_391_000 as Weight)
				// Standard Error: 725_000
				.saturating_add((680_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn as_multi_threshold_1(z: u32, ) -> Weight {
				(157_800_000 as Weight)
				// Standard Error: 1_000
				.saturating_add((57_000 as Weight).saturating_mul(z as Weight))
				}
				fn as_multi_create(s: u32, z: u32, ) -> Weight {
				(320_404_000 as Weight)
				// Standard Error: 52_000
				.saturating_add((864_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 0
				.saturating_add((1_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn as_multi_create_store(s: u32, z: u32, ) -> Weight {
				(279_242_000 as Weight)
				// Standard Error: 313_000
				.saturating_add((1_414_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 3_000
				.saturating_add((119_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(RocksDbWeight::get().reads(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn as_multi_approve(s: u32, z: u32, ) -> Weight {
				(240_319_000 as Weight)
				// Standard Error: 130_000
				.saturating_add((1_943_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 1_000
				.saturating_add((1_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn as_multi_approve_store(s: u32, z: u32, ) -> Weight {
				(400_048_000 as Weight)
				// Standard Error: 324_000
				.saturating_add((2_025_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 3_000
				.saturating_add((114_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn as_multi_complete(s: u32, z: u32, ) -> Weight {
				(660_564_000 as Weight)
				// Standard Error: 518_000
				.saturating_add((10_714_000 as Weight).saturating_mul(s as Weight))
				// Standard Error: 5_000
				.saturating_add((276_000 as Weight).saturating_mul(z as Weight))
				.saturating_add(RocksDbWeight::get().reads(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn approve_as_multi_create(s: u32, ) -> Weight {
				(326_063_000 as Weight)
				// Standard Error: 100_000
				.saturating_add((919_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn approve_as_multi_approve(s: u32, ) -> Weight {
				(213_594_000 as Weight)
				// Standard Error: 76_000
				.saturating_add((2_278_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn approve_as_multi_complete(s: u32, ) -> Weight {
				(5_266_016_000 as Weight)
				// Standard Error: 1_523_000
				.saturating_add((12_592_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn cancel_as_multi(s: u32, ) -> Weight {
				(2_380_391_000 as Weight)
				// Standard Error: 725_000
				.saturating_add((680_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				}