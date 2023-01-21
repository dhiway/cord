#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn add(l: u32, ) -> Weight;
	fn remove(l: u32,) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {

	fn add(l: u32, ) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(l as u64))
			.saturating_add(T::DbWeight::get().writes(l as u64))
	}
	fn remove(l: u32,) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(l as u64))
			.saturating_add(T::DbWeight::get().writes(l as u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn add(l: u32, ) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(RocksDbWeight::get().reads(l as u64))
			.saturating_add(RocksDbWeight::get().writes(l as u64))
	}
	fn remove(l: u32,) -> Weight {
		Weight::from_ref_time(50_978_000 as u64)
			.saturating_add(Weight::from_ref_time(5_000 as u64).saturating_mul(l as u64))
			.saturating_add(RocksDbWeight::get().reads(l as u64))
			.saturating_add(RocksDbWeight::get().writes(l as u64))
	}
}
