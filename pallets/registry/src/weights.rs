#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

// Dummy file
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for space.
pub trait WeightInfo {
	fn add_admin_delegate() -> Weight;
	fn add_delegate() -> Weight;
	fn remove_delegate() -> Weight;
	fn create(l: u32,) -> Weight;
	fn update() -> Weight;
	fn archive() -> Weight;
	fn restore() -> Weight;
	fn transfer() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
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
	fn create(l: u32,) -> Weight {
		Weight::from_parts(522_000_000,0)
			.saturating_add(Weight::from_parts(2000,0).saturating_mul(l.into()))
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
