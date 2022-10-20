#![allow(unused_parens)]
#![allow(unused_imports)]

// Dummy file
use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_schema::WeightInfo for WeightInfo<T> {
	fn create(l: u32) -> Weight {
		Weight::from_ref_time(522_000_000 as u64)
			.saturating_add(Weight::from_ref_time(20_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
}
