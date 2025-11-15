// Dummy Weights

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;
use frame_system::Config;

/// Weights: TBD.
/// The `()` impl provides zero weights for tests/dev.
pub trait WeightInfo {
	fn nominate() -> Weight;
	fn remove() -> Weight;
	fn set_invulnerables(_n: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: Config> WeightInfo for SubstrateWeight<T> {
	fn nominate() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn remove() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn set_invulnerables(_n: u32) -> Weight {
		Weight::from_parts(0, 0)
	}
}

impl WeightInfo for () {
	fn nominate() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn remove() -> Weight {
		Weight::from_parts(0, 0)
	}
	fn set_invulnerables(_n: u32) -> Weight {
		Weight::from_parts(0, 0)
	}
}
