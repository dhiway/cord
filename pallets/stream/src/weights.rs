#![allow(unused_parens)]
#![allow(unused_imports)]

// Dummy file
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_identity.
pub trait WeightInfo {
	fn create() -> Weight;
	fn update() -> Weight;
	fn revoke() -> Weight;
	fn remove() -> Weight;
	fn council_remove() -> Weight;
	fn digest() -> Weight;
	fn set_metadata() -> Weight;
	fn clear_metadata() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create() -> Weight {
		(722_000_000 as Weight)
			.saturating_add((20_000 as Weight).saturating_mul(650 as Weight))
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn update() -> Weight {
		(522_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn revoke() -> Weight {
		(100_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn remove() -> Weight {
		(100_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn council_remove() -> Weight {
		(100_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn digest() -> Weight {
		(522_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn set_metadata() -> Weight {
		(322_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	fn clear_metadata() -> Weight {
		(322_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
}
