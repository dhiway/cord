#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn on_initialize(s: u32, ) -> Weight;
	fn store(l: u32, ) -> Weight;
	fn remove() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn on_initialize(s: u32, ) -> Weight {
		Weight::from_ref_time(15_912_000 as u64)
			// Standard Error: 5_000
			.saturating_add(Weight::from_ref_time(4_530_000 as u64).saturating_mul(s as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	fn store(l: u32, ) -> Weight {
		Weight::from_ref_time(0 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(6 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	fn remove() -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			.saturating_add(T::DbWeight::get().reads(6 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn on_initialize(s: u32, ) -> Weight {
		Weight::from_ref_time(15_912_000 as u64)
			// Standard Error: 5_000
			.saturating_add(Weight::from_ref_time(4_530_000 as u64).saturating_mul(s as u64))
			.saturating_add(RocksDbWeight::get().reads(1 as u64))
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
	fn store(l: u32, ) -> Weight {
		Weight::from_ref_time(0 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(RocksDbWeight::get().reads(6 as u64))
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
	fn remove() -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			.saturating_add(RocksDbWeight::get().reads(6 as u64))
			.saturating_add(RocksDbWeight::get().writes(1 as u64))
	}
}
