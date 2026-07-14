// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Conservative pre-benchmark weights for native Orbis attestations.
//!
//! P7 replaces these values with generated benchmark output. Until then these weights are
//! deliberately non-zero and include the database work of the worst valid branch.

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub trait WeightInfo {
	fn create_schema(definition_bytes: u32, issuers: u32) -> Weight;
	fn set_schema_status() -> Weight;
	fn issue() -> Weight;
	fn issue_delegated() -> Weight;
	fn issue_batch(items: u32) -> Weight;
	fn revoke() -> Weight;
	fn set_emergency_pause() -> Weight;
	fn force_schema_status() -> Weight;
	fn force_revoke() -> Weight;
	fn revoke_delegated() -> Weight;
	fn issue_delegated_batch(items: u32) -> Weight;
	fn revoke_batch(items: u32) -> Weight;
	fn revoke_delegated_batch(items: u32) -> Weight;
	fn revoke_external_status() -> Weight;
	fn revoke_external_status_batch(items: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_schema(definition_bytes: u32, issuers: u32) -> Weight {
		Weight::from_parts(35_000_000, 7_000)
			.saturating_add(Weight::from_parts(2_000, 0).saturating_mul(definition_bytes.into()))
			.saturating_add(Weight::from_parts(100_000, 32).saturating_mul(issuers.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn set_schema_status() -> Weight {
		Weight::from_parts(22_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn issue() -> Weight {
		Weight::from_parts(65_000_000, 12_000)
			.saturating_add(T::DbWeight::get().reads(9))
			.saturating_add(T::DbWeight::get().writes(7))
	}
	fn issue_delegated() -> Weight {
		Self::issue()
			.saturating_add(Weight::from_parts(55_000_000, 2_000))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn issue_batch(items: u32) -> Weight {
		Weight::from_parts(12_000_000, 1_000)
			.saturating_add(Self::issue().saturating_mul(items.into()))
	}
	fn revoke() -> Weight {
		Weight::from_parts(32_000_000, 7_000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_emergency_pause() -> Weight {
		Weight::from_parts(12_000_000, 1_000).saturating_add(T::DbWeight::get().writes(1))
	}
	fn force_schema_status() -> Weight {
		Self::set_schema_status()
	}
	fn force_revoke() -> Weight {
		Self::revoke()
	}
	fn revoke_delegated() -> Weight {
		Self::revoke()
			.saturating_add(Weight::from_parts(55_000_000, 2_000))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn issue_delegated_batch(items: u32) -> Weight {
		Weight::from_parts(15_000_000, 1_000)
			.saturating_add(Self::issue_delegated().saturating_mul(items.into()))
	}
	fn revoke_batch(items: u32) -> Weight {
		Weight::from_parts(10_000_000, 1_000)
			.saturating_add(Self::revoke().saturating_mul(items.into()))
	}
	fn revoke_delegated_batch(items: u32) -> Weight {
		Weight::from_parts(15_000_000, 1_000)
			.saturating_add(Self::revoke_delegated().saturating_mul(items.into()))
	}
	fn revoke_external_status() -> Weight {
		Weight::from_parts(25_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn revoke_external_status_batch(items: u32) -> Weight {
		Weight::from_parts(10_000_000, 1_000)
			.saturating_add(Self::revoke_external_status().saturating_mul(items.into()))
	}
}

impl WeightInfo for () {
	fn create_schema(definition_bytes: u32, issuers: u32) -> Weight {
		Weight::from_parts(
			35_000_000u64
				.saturating_add(2_000u64.saturating_mul(definition_bytes.into()))
				.saturating_add(100_000u64.saturating_mul(issuers.into())),
			7_000,
		)
	}
	fn set_schema_status() -> Weight {
		Weight::from_parts(22_000_000, 4_000)
	}
	fn issue() -> Weight {
		Weight::from_parts(65_000_000, 12_000)
	}
	fn issue_delegated() -> Weight {
		Weight::from_parts(120_000_000, 14_000)
	}
	fn issue_batch(items: u32) -> Weight {
		Weight::from_parts(12_000_000, 1_000)
			.saturating_add(Self::issue().saturating_mul(items.into()))
	}
	fn revoke() -> Weight {
		Weight::from_parts(32_000_000, 7_000)
	}
	fn set_emergency_pause() -> Weight {
		Weight::from_parts(12_000_000, 1_000)
	}
	fn force_schema_status() -> Weight {
		Self::set_schema_status()
	}
	fn force_revoke() -> Weight {
		Self::revoke()
	}
	fn revoke_delegated() -> Weight {
		Weight::from_parts(97_000_000, 9_000)
	}
	fn issue_delegated_batch(items: u32) -> Weight {
		Weight::from_parts(15_000_000, 1_000)
			.saturating_add(Self::issue_delegated().saturating_mul(items.into()))
	}
	fn revoke_batch(items: u32) -> Weight {
		Weight::from_parts(10_000_000, 1_000)
			.saturating_add(Self::revoke().saturating_mul(items.into()))
	}
	fn revoke_delegated_batch(items: u32) -> Weight {
		Weight::from_parts(15_000_000, 1_000)
			.saturating_add(Self::revoke_delegated().saturating_mul(items.into()))
	}
	fn revoke_external_status() -> Weight {
		Weight::from_parts(25_000_000, 4_000)
	}
	fn revoke_external_status_batch(items: u32) -> Weight {
		Weight::from_parts(10_000_000, 1_000)
			.saturating_add(Self::revoke_external_status().saturating_mul(items.into()))
	}
}
