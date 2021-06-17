// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! for pallet_did DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
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

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for did.
pub trait WeightInfo {
	fn submit_did_create_operation_ed25519_keys(n: u32, u: u32) -> Weight;
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32) -> Weight;
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, u: u32) -> Weight;
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, u: u32) -> Weight;
	fn submit_did_delete_operation() -> Weight;
	fn submit_did_call_ed25519_key() -> Weight;
	fn submit_did_call_sr25519_key() -> Weight;
}

/// Weights for did using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn submit_did_create_operation_ed25519_keys(n: u32, _u: u32) -> Weight {
		(84_486_000_u64)
			// Standard Error: 64_000
			.saturating_add((3_433_000_u64).saturating_mul(n as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32) -> Weight {
		(89_550_000_u64)
			// Standard Error: 53_000
			.saturating_add((2_249_000_u64).saturating_mul(n as Weight))
			// Standard Error: 2_000
			.saturating_add((6_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, u: u32) -> Weight {
		(78_788_000_u64)
			// Standard Error: 171_000
			.saturating_add((2_961_000_u64).saturating_mul(n as Weight))
			// Standard Error: 171_000
			.saturating_add((3_032_000_u64).saturating_mul(m as Weight))
			// Standard Error: 8_000
			.saturating_add((24_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, u: u32) -> Weight {
		(62_987_000_u64)
			// Standard Error: 133_000
			.saturating_add((4_353_000_u64).saturating_mul(n as Weight))
			// Standard Error: 133_000
			.saturating_add((3_464_000_u64).saturating_mul(m as Weight))
			// Standard Error: 6_000
			.saturating_add((30_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_delete_operation() -> Weight {
		(74_610_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ed25519_key() -> Weight {
		(77_355_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_call_sr25519_key() -> Weight {
		(79_730_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn submit_did_create_operation_ed25519_keys(n: u32, _u: u32) -> Weight {
		(84_486_000_u64)
			// Standard Error: 64_000
			.saturating_add((3_433_000_u64).saturating_mul(n as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32) -> Weight {
		(89_550_000_u64)
			// Standard Error: 53_000
			.saturating_add((2_249_000_u64).saturating_mul(n as Weight))
			// Standard Error: 2_000
			.saturating_add((6_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, u: u32) -> Weight {
		(78_788_000_u64)
			// Standard Error: 171_000
			.saturating_add((2_961_000_u64).saturating_mul(n as Weight))
			// Standard Error: 171_000
			.saturating_add((3_032_000_u64).saturating_mul(m as Weight))
			// Standard Error: 8_000
			.saturating_add((24_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, u: u32) -> Weight {
		(62_987_000_u64)
			// Standard Error: 133_000
			.saturating_add((4_353_000_u64).saturating_mul(n as Weight))
			// Standard Error: 133_000
			.saturating_add((3_464_000_u64).saturating_mul(m as Weight))
			// Standard Error: 6_000
			.saturating_add((30_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_delete_operation() -> Weight {
		(74_610_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ed25519_key() -> Weight {
		(77_355_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_call_sr25519_key() -> Weight {
		(79_730_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
