#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;
/// Weight functions needed for schema.
pub trait WeightInfo {
	fn create(l: u32) -> Weight;
}
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create(l: u32) -> Weight {
		Weight::from_ref_time(522_000_000 as u64)
			.saturating_add(Weight::from_ref_time(20_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
}
