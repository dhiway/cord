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
	fn add_registrar(r: u32) -> Weight;
}

impl WeightInfo for () {
	fn add_registrar(r: u32) -> Weight {
		(21_825_000 as Weight)
			// Standard Error: 3_000
			.saturating_add((288_000 as Weight).saturating_mul(r as Weight))
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
