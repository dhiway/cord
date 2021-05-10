// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_balances
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_balances
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

/// Weight functions needed for pallet_balances.
pub trait WeightInfo {
fn transfer() -> Weight;
fn transfer_keep_alive() -> Weight;
fn set_balance_creating() -> Weight;
fn set_balance_killing() -> Weight;
fn force_transfer() -> Weight;
}

//// Weights for pallet_balances using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn transfer() -> Weight {
				(623_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn transfer_keep_alive() -> Weight {
				(358_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn set_balance_creating() -> Weight {
				(159_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn set_balance_killing() -> Weight {
				(200_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn force_transfer() -> Weight {
				(613_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(3 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn transfer() -> Weight {
				(623_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn transfer_keep_alive() -> Weight {
				(358_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn set_balance_creating() -> Weight {
				(159_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn set_balance_killing() -> Weight {
				(200_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn force_transfer() -> Weight {
				(613_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				}