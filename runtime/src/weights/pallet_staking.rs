// Copyright 2019-2021 Dhiway.
// This file is part of the CORD Platform.

//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0 for pallet_staking
//! DATE: 2021-05-09, STEPS: `[3, ]`, REPEAT: 2,
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Interpreted, CHAIN: Some("dev")

// Executed Command:
// ./target/release/cord
// benchmark
// --chain=dev
// --steps=3
// --repeat=2
// --pallet=pallet_staking
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

/// Weight functions needed for pallet_staking.
pub trait WeightInfo {
fn bond() -> Weight;
fn bond_extra() -> Weight;
fn unbond() -> Weight;
fn withdraw_unbonded_update(s: u32, ) -> Weight;
fn withdraw_unbonded_kill(s: u32, ) -> Weight;
fn validate() -> Weight;
fn kick(k: u32, ) -> Weight;
fn nominate(n: u32, ) -> Weight;
fn chill() -> Weight;
fn set_payee() -> Weight;
fn set_controller() -> Weight;
fn set_validator_count() -> Weight;
fn force_no_eras() -> Weight;
fn force_new_era() -> Weight;
fn force_new_era_always() -> Weight;
fn set_invulnerables(v: u32, ) -> Weight;
fn force_unstake(s: u32, ) -> Weight;
fn cancel_deferred_slash(s: u32, ) -> Weight;
fn payout_stakers_dead_controller(n: u32, ) -> Weight;
fn payout_stakers_alive_staked(n: u32, ) -> Weight;
fn rebond(l: u32, ) -> Weight;
fn set_history_depth(e: u32, ) -> Weight;
fn reap_stash(s: u32, ) -> Weight;
fn new_era(v: u32, n: u32, ) -> Weight;
fn submit_solution_better(v: u32, n: u32, a: u32, w: u32, ) -> Weight;
}

//// Weights for pallet_staking using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
		impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
				fn bond() -> Weight {
				(427_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(5 as Weight))
				.saturating_add(T::DbWeight::get().writes(4 as Weight))
				}
				fn bond_extra() -> Weight {
				(344_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn unbond() -> Weight {
				(308_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(5 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn withdraw_unbonded_update(s: u32, ) -> Weight {
				(297_050_000 as Weight)
				// Standard Error: 91_000
				.saturating_add((380_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(5 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn withdraw_unbonded_kill(s: u32, ) -> Weight {
				(501_800_000 as Weight)
				// Standard Error: 239_000
				.saturating_add((20_108_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(7 as Weight))
				.saturating_add(T::DbWeight::get().writes(7 as Weight))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn validate() -> Weight {
				(110_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn kick(k: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 7_103_000
				.saturating_add((284_437_000 as Weight).saturating_mul(k as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(k as
				Weight)))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(k as
				Weight)))
				}
				fn nominate(n: u32, ) -> Weight {
				(126_200_000 as Weight)
				// Standard Error: 1_487_000
				.saturating_add((27_300_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn chill() -> Weight {
				(205_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				fn set_payee() -> Weight {
				(199_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn set_controller() -> Weight {
				(150_000_000 as Weight)
				.saturating_add(T::DbWeight::get().reads(3 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn set_validator_count() -> Weight {
				(9_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn force_no_eras() -> Weight {
				(11_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn force_new_era() -> Weight {
				(11_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn force_new_era_always() -> Weight {
				(10_000_000 as Weight)
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn set_invulnerables(v: u32, ) -> Weight {
				(14_500_000 as Weight)
				// Standard Error: 9_000
				.saturating_add((299_000 as Weight).saturating_mul(v as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn force_unstake(s: u32, ) -> Weight {
				(393_500_000 as Weight)
				// Standard Error: 1_851_000
				.saturating_add((24_500_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().writes(7 as Weight))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn cancel_deferred_slash(s: u32, ) -> Weight {
				(441_453_687_000 as Weight)
				// Standard Error: 438_489_000
				.saturating_add((2_555_013_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(1 as Weight))
				.saturating_add(T::DbWeight::get().writes(1 as Weight))
				}
				fn payout_stakers_dead_controller(n: u32, ) -> Weight {
				(3_992_327_000 as Weight)
				// Standard Error: 38_345_000
				.saturating_add((316_673_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(T::DbWeight::get().reads(11 as Weight))
				.saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(n as
				Weight)))
				}
				fn payout_stakers_alive_staked(n: u32, ) -> Weight {
				(645_650_000 as Weight)
				// Standard Error: 5_688_000
				.saturating_add((449_400_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(T::DbWeight::get().reads(12 as Weight))
				.saturating_add(T::DbWeight::get().reads((5 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				.saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(n as
				Weight)))
				}
				fn rebond(l: u32, ) -> Weight {
				(213_515_000 as Weight)
				// Standard Error: 2_105_000
				.saturating_add((6_335_000 as Weight).saturating_mul(l as Weight))
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().writes(3 as Weight))
				}
				fn set_history_depth(e: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 1_717_000
				.saturating_add((96_817_000 as Weight).saturating_mul(e as Weight))
				.saturating_add(T::DbWeight::get().reads(2 as Weight))
				.saturating_add(T::DbWeight::get().writes(4 as Weight))
				.saturating_add(T::DbWeight::get().writes((7 as Weight).saturating_mul(e as
				Weight)))
				}
				fn reap_stash(s: u32, ) -> Weight {
				(419_335_000 as Weight)
				// Standard Error: 1_726_000
				.saturating_add((19_315_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(T::DbWeight::get().reads(4 as Weight))
				.saturating_add(T::DbWeight::get().writes(8 as Weight))
				.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn new_era(v: u32, n: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 418_361_000
				.saturating_add((12_226_581_000 as Weight).saturating_mul(v as Weight))
				// Standard Error: 38_032_000
				.saturating_add((1_331_198_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(T::DbWeight::get().reads(7 as Weight))
				.saturating_add(T::DbWeight::get().reads((4 as Weight).saturating_mul(v as
				Weight)))
				.saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(8 as Weight))
				.saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(v as
				Weight)))
				}
				fn submit_solution_better(v: u32, n: u32, a: u32, w: u32, ) -> Weight {
				(162_463_060_000 as Weight)
				// Standard Error: 405_008_000
				.saturating_add((102_967_000 as Weight).saturating_mul(v as Weight))
				// Standard Error: 162_071_000
				.saturating_add((170_948_000 as Weight).saturating_mul(n as Weight))
				// Standard Error: 405_008_000
				.saturating_add((274_029_000 as Weight).saturating_mul(a as Weight))
				// Standard Error: 964_957_000
				.saturating_add((105_597_000 as Weight).saturating_mul(w as Weight))
				.saturating_add(T::DbWeight::get().reads(6 as Weight))
				.saturating_add(T::DbWeight::get().reads((4 as Weight).saturating_mul(a as
				Weight)))
				.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(w as
				Weight)))
				.saturating_add(T::DbWeight::get().writes(2 as Weight))
				}
				}

				// For backwards compatibility and tests
				impl WeightInfo for () {
				fn bond() -> Weight {
				(427_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(5 as Weight))
				.saturating_add(RocksDbWeight::get().writes(4 as Weight))
				}
				fn bond_extra() -> Weight {
				(344_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn unbond() -> Weight {
				(308_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(5 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn withdraw_unbonded_update(s: u32, ) -> Weight {
				(297_050_000 as Weight)
				// Standard Error: 91_000
				.saturating_add((380_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(5 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn withdraw_unbonded_kill(s: u32, ) -> Weight {
				(501_800_000 as Weight)
				// Standard Error: 239_000
				.saturating_add((20_108_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(7 as Weight))
				.saturating_add(RocksDbWeight::get().writes(7 as Weight))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn validate() -> Weight {
				(110_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn kick(k: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 7_103_000
				.saturating_add((284_437_000 as Weight).saturating_mul(k as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(k as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(k as
				Weight)))
				}
				fn nominate(n: u32, ) -> Weight {
				(126_200_000 as Weight)
				// Standard Error: 1_487_000
				.saturating_add((27_300_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn chill() -> Weight {
				(205_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				fn set_payee() -> Weight {
				(199_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn set_controller() -> Weight {
				(150_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().reads(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn set_validator_count() -> Weight {
				(9_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn force_no_eras() -> Weight {
				(11_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn force_new_era() -> Weight {
				(11_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn force_new_era_always() -> Weight {
				(10_000_000 as Weight)
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn set_invulnerables(v: u32, ) -> Weight {
				(14_500_000 as Weight)
				// Standard Error: 9_000
				.saturating_add((299_000 as Weight).saturating_mul(v as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn force_unstake(s: u32, ) -> Weight {
				(393_500_000 as Weight)
				// Standard Error: 1_851_000
				.saturating_add((24_500_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes(7 as Weight))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn cancel_deferred_slash(s: u32, ) -> Weight {
				(441_453_687_000 as Weight)
				// Standard Error: 438_489_000
				.saturating_add((2_555_013_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(1 as Weight))
				.saturating_add(RocksDbWeight::get().writes(1 as Weight))
				}
				fn payout_stakers_dead_controller(n: u32, ) -> Weight {
				(3_992_327_000 as Weight)
				// Standard Error: 38_345_000
				.saturating_add((316_673_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(RocksDbWeight::get().reads(11 as Weight))
				.saturating_add(RocksDbWeight::get().reads((3 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(n as
				Weight)))
				}
				fn payout_stakers_alive_staked(n: u32, ) -> Weight {
				(645_650_000 as Weight)
				// Standard Error: 5_688_000
				.saturating_add((449_400_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(RocksDbWeight::get().reads(12 as Weight))
				.saturating_add(RocksDbWeight::get().reads((5 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				.saturating_add(RocksDbWeight::get().writes((3 as Weight).saturating_mul(n as
				Weight)))
				}
				fn rebond(l: u32, ) -> Weight {
				(213_515_000 as Weight)
				// Standard Error: 2_105_000
				.saturating_add((6_335_000 as Weight).saturating_mul(l as Weight))
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes(3 as Weight))
				}
				fn set_history_depth(e: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 1_717_000
				.saturating_add((96_817_000 as Weight).saturating_mul(e as Weight))
				.saturating_add(RocksDbWeight::get().reads(2 as Weight))
				.saturating_add(RocksDbWeight::get().writes(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes((7 as Weight).saturating_mul(e as
				Weight)))
				}
				fn reap_stash(s: u32, ) -> Weight {
				(419_335_000 as Weight)
				// Standard Error: 1_726_000
				.saturating_add((19_315_000 as Weight).saturating_mul(s as Weight))
				.saturating_add(RocksDbWeight::get().reads(4 as Weight))
				.saturating_add(RocksDbWeight::get().writes(8 as Weight))
				.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(s as
				Weight)))
				}
				fn new_era(v: u32, n: u32, ) -> Weight {
				(0 as Weight)
				// Standard Error: 418_361_000
				.saturating_add((12_226_581_000 as Weight).saturating_mul(v as Weight))
				// Standard Error: 38_032_000
				.saturating_add((1_331_198_000 as Weight).saturating_mul(n as Weight))
				.saturating_add(RocksDbWeight::get().reads(7 as Weight))
				.saturating_add(RocksDbWeight::get().reads((4 as Weight).saturating_mul(v as
				Weight)))
				.saturating_add(RocksDbWeight::get().reads((3 as Weight).saturating_mul(n as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(8 as Weight))
				.saturating_add(RocksDbWeight::get().writes((3 as Weight).saturating_mul(v as
				Weight)))
				}
				fn submit_solution_better(v: u32, n: u32, a: u32, w: u32, ) -> Weight {
				(162_463_060_000 as Weight)
				// Standard Error: 405_008_000
				.saturating_add((102_967_000 as Weight).saturating_mul(v as Weight))
				// Standard Error: 162_071_000
				.saturating_add((170_948_000 as Weight).saturating_mul(n as Weight))
				// Standard Error: 405_008_000
				.saturating_add((274_029_000 as Weight).saturating_mul(a as Weight))
				// Standard Error: 964_957_000
				.saturating_add((105_597_000 as Weight).saturating_mul(w as Weight))
				.saturating_add(RocksDbWeight::get().reads(6 as Weight))
				.saturating_add(RocksDbWeight::get().reads((4 as Weight).saturating_mul(a as
				Weight)))
				.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(w as
				Weight)))
				.saturating_add(RocksDbWeight::get().writes(2 as Weight))
				}
				}