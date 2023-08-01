#![allow(unused_parens)]
#![allow(unused_imports)]

// Dummy file
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use core::marker::PhantomData;

/// Weight functions needed for pallet_rating.
pub trait WeightInfo {
	fn entries() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn entries() -> Weight {
		Weight::from_parts(26_208_134, 18961)
		// Standard Error: 94
		.saturating_add(T::DbWeight::get().reads(3_u64))
		.saturating_add(T::DbWeight::get().writes(4_u64))
	}
}