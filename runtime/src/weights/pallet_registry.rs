#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

// Dummy file
use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;


pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_registry::WeightInfo for WeightInfo<T> {
	fn add_admin_delegate() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn add_delegate() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn remove_delegate() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn create() -> Weight {
		Weight::from_parts(522_000_000,0)
			.saturating_add(Weight::from_parts(2000,0))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn update() -> Weight {
		Weight::from_parts(522_000_000,0)
			.saturating_add(Weight::from_parts(2000,0))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn archive() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	fn restore() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	fn transfer() -> Weight {
		Weight::from_parts(322_000_000,0)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
