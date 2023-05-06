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
	/// Storage: Registry Registries (r:1 w:1)
	/// Proof: Registry Registries (max_values: None, max_size: Some(15544), added: 18019, mode: MaxEncodedLen)
	/// Storage: Registry Commits (r:1 w:1)
	/// Proof: Registry Commits (max_values: None, max_size: Some(69068), added: 71543, mode: MaxEncodedLen)
	/// The range of component `l` is `[1, 15360]`.
	fn create(_l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `42`
		//  Estimated: `91542`
		// Minimum execution time: 74_032_000 picoseconds.
		Weight::from_parts(75_627_621, 91542)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: Registry Registries (r:1 w:1)
	/// Proof: Registry Registries (max_values: None, max_size: Some(15544), added: 18019, mode: MaxEncodedLen)
	/// Storage: Registry Commits (r:1 w:1)
	/// Proof: Registry Commits (max_values: None, max_size: Some(69068), added: 71543, mode: MaxEncodedLen)
	/// The range of component `l` is `[1, 15360]`.
	fn update(l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `15726`
		//  Estimated: `91542`
		// Minimum execution time: 60_378_000 picoseconds.
		Weight::from_parts(62_758_765, 91542)
			// Standard Error: 3
			.saturating_add(Weight::from_parts(4, 0).saturating_mul(l.into()))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
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
