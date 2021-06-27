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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for did.
pub trait WeightInfo {
	fn submit_did_create_operation_ed25519_keys(n: u32, u: u32, ) -> Weight;
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32, ) -> Weight;
	fn submit_did_create_operation_ecdsa_keys(n: u32, u: u32, ) -> Weight;
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, u: u32, ) -> Weight;
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, u: u32, ) -> Weight;
	fn submit_did_update_operation_ecdsa_keys(n: u32, m: u32, u: u32, ) -> Weight;
	fn submit_did_delete_operation() -> Weight;
	fn submit_did_call_ed25519_key() -> Weight;
	fn submit_did_call_sr25519_key() -> Weight;
	fn submit_did_call_ecdsa_key() -> Weight;
}

/// Weights for did using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn submit_did_create_operation_ed25519_keys(n: u32, u: u32, ) -> Weight {
		(77_831_000_u64)
			// Standard Error: 42_000
			.saturating_add((1_626_000_u64).saturating_mul(n as Weight))
			// Standard Error: 1_000
			.saturating_add((6_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32, ) -> Weight {
		(75_106_000_u64)
			// Standard Error: 41_000
			.saturating_add((1_959_000_u64).saturating_mul(n as Weight))
			// Standard Error: 1_000
			.saturating_add((23_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_ecdsa_keys(n: u32, u: u32, ) -> Weight {
		(181_586_000_u64)
			// Standard Error: 51_000
			.saturating_add((1_768_000_u64).saturating_mul(n as Weight))
			// Standard Error: 2_000
			.saturating_add((3_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, _u: u32, ) -> Weight {
		(71_793_000_u64)
			// Standard Error: 39_000
			.saturating_add((3_444_000_u64).saturating_mul(n as Weight))
			// Standard Error: 39_000
			.saturating_add((2_574_000_u64).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, _u: u32, ) -> Weight {
		(73_079_000_u64)
			// Standard Error: 59_000
			.saturating_add((3_770_000_u64).saturating_mul(n as Weight))
			// Standard Error: 59_000
			.saturating_add((2_991_000_u64).saturating_mul(m as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ecdsa_keys(n: u32, m: u32, u: u32, ) -> Weight {
		(170_458_000_u64)
			// Standard Error: 67_000
			.saturating_add((3_849_000_u64).saturating_mul(n as Weight))
			// Standard Error: 67_000
			.saturating_add((3_102_000_u64).saturating_mul(m as Weight))
			// Standard Error: 3_000
			.saturating_add((1_000_u64).saturating_mul(u as Weight))
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_delete_operation() -> Weight {
		(69_009_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ed25519_key() -> Weight {
		(71_504_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_call_sr25519_key() -> Weight {
		(75_271_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ecdsa_key() -> Weight {
		(177_543_000_u64)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn submit_did_create_operation_ed25519_keys(n: u32, u: u32, ) -> Weight {
		(77_831_000_u64)
			// Standard Error: 42_000
			.saturating_add((1_626_000_u64).saturating_mul(n as Weight))
			// Standard Error: 1_000
			.saturating_add((6_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_sr25519_keys(n: u32, u: u32, ) -> Weight {
		(75_106_000_u64)
			// Standard Error: 41_000
			.saturating_add((1_959_000_u64).saturating_mul(n as Weight))
			// Standard Error: 1_000
			.saturating_add((23_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_create_operation_ecdsa_keys(n: u32, u: u32, ) -> Weight {
		(181_586_000_u64)
			// Standard Error: 51_000
			.saturating_add((1_768_000_u64).saturating_mul(n as Weight))
			// Standard Error: 2_000
			.saturating_add((3_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ed25519_keys(n: u32, m: u32, _u: u32, ) -> Weight {
		(71_793_000_u64)
			// Standard Error: 39_000
			.saturating_add((3_444_000_u64).saturating_mul(n as Weight))
			// Standard Error: 39_000
			.saturating_add((2_574_000_u64).saturating_mul(m as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_sr25519_keys(n: u32, m: u32, _u: u32, ) -> Weight {
		(73_079_000_u64)
			// Standard Error: 59_000
			.saturating_add((3_770_000_u64).saturating_mul(n as Weight))
			// Standard Error: 59_000
			.saturating_add((2_991_000_u64).saturating_mul(m as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_update_operation_ecdsa_keys(n: u32, m: u32, u: u32, ) -> Weight {
		(170_458_000_u64)
			// Standard Error: 67_000
			.saturating_add((3_849_000_u64).saturating_mul(n as Weight))
			// Standard Error: 67_000
			.saturating_add((3_102_000_u64).saturating_mul(m as Weight))
			// Standard Error: 3_000
			.saturating_add((1_000_u64).saturating_mul(u as Weight))
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_delete_operation() -> Weight {
		(69_009_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ed25519_key() -> Weight {
		(71_504_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_call_sr25519_key() -> Weight {
		(75_271_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn submit_did_call_ecdsa_key() -> Weight {
		(177_543_000_u64)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
