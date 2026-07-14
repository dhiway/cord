// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Conservative pre-benchmark weights for native Orbis Names.
//!
//! These functions keep the pallet integrable before runtime benchmarks are generated. Production
//! activation must replace the constants with benchmark output for the final Orbis runtime.

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub trait WeightInfo {
	fn commit() -> Weight;
	fn cancel_commitment() -> Weight;
	fn prune_commitment() -> Weight;
	fn register() -> Weight;
	fn renew() -> Weight;
	fn transfer() -> Weight;
	fn controller() -> Weight;
	fn resolver_write() -> Weight;
	fn set_text() -> Weight;
	fn set_primary() -> Weight;
	fn remove_name() -> Weight;
	fn reservation() -> Weight;
	fn emergency() -> Weight;
}

impl WeightInfo for () {
	fn commit() -> Weight {
		Weight::from_parts(28_000_000, 3_600)
	}
	fn cancel_commitment() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
	}
	fn prune_commitment() -> Weight {
		Self::cancel_commitment()
	}
	fn register() -> Weight {
		Weight::from_parts(95_000_000, 8_000)
	}
	fn renew() -> Weight {
		Weight::from_parts(32_000_000, 4_000)
	}
	fn transfer() -> Weight {
		Weight::from_parts(58_000_000, 6_000)
	}
	fn controller() -> Weight {
		Weight::from_parts(32_000_000, 4_500)
	}
	fn resolver_write() -> Weight {
		Weight::from_parts(30_000_000, 4_500)
	}
	fn set_text() -> Weight {
		Weight::from_parts(48_000_000, 6_000)
	}
	fn set_primary() -> Weight {
		Weight::from_parts(24_000_000, 4_000)
	}
	fn remove_name() -> Weight {
		Weight::from_parts(85_000_000, 8_000)
	}
	fn reservation() -> Weight {
		Weight::from_parts(35_000_000, 4_500)
	}
	fn emergency() -> Weight {
		Weight::from_parts(100_000_000, 8_000)
	}
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn commit() -> Weight {
		Weight::from_parts(28_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn cancel_commitment() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn prune_commitment() -> Weight {
		Self::cancel_commitment()
	}
	fn register() -> Weight {
		Weight::from_parts(95_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(12))
			.saturating_add(T::DbWeight::get().writes(9))
	}
	fn renew() -> Weight {
		Weight::from_parts(32_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn transfer() -> Weight {
		Weight::from_parts(58_000_000, 6_000)
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	fn controller() -> Weight {
		Weight::from_parts(32_000_000, 4_500)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn resolver_write() -> Weight {
		Weight::from_parts(30_000_000, 4_500)
			// Attestation references add an attestation, schema, and current-block read before
			// the authorized Orbis Names record mutation. Keep one conservative shared resolver weight.
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_text() -> Weight {
		Weight::from_parts(48_000_000, 6_000)
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn set_primary() -> Weight {
		Weight::from_parts(24_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn remove_name() -> Weight {
		Weight::from_parts(85_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(10))
			.saturating_add(T::DbWeight::get().writes(10))
	}
	fn reservation() -> Weight {
		Weight::from_parts(35_000_000, 4_500)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn emergency() -> Weight {
		Weight::from_parts(100_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(12))
			.saturating_add(T::DbWeight::get().writes(12))
	}
}
