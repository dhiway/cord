#![allow(unused_parens)]
#![allow(unused_imports)]

// Dummy file
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_rating.
pub trait WeightInfo {
	fn add(l: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn add(l: u32) -> Weight {
		(522_000_000 as Weight)
			.saturating_add((2000 as Weight).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
}
