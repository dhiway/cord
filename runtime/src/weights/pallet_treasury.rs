// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_treasury`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_treasury::WeightInfo for WeightInfo<T> {
	/// Storage: Treasury ProposalCount (r:1 w:1)
	/// Proof: Treasury ProposalCount (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Treasury Approvals (r:1 w:1)
	/// Proof: Treasury Approvals (max_values: Some(1), max_size: Some(402), added: 897, mode: MaxEncodedLen)
	/// Storage: Treasury Proposals (r:0 w:1)
	/// Proof: Treasury Proposals (max_values: None, max_size: Some(108), added: 2583, mode: MaxEncodedLen)
	fn spend() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6`
		//  Estimated: `3376`
		// Minimum execution time: 15_818_000 picoseconds.
		Weight::from_parts(16_068_000, 0)
			.saturating_add(Weight::from_parts(0, 3376))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: Treasury ProposalCount (r:1 w:1)
	/// Proof: Treasury ProposalCount (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: Treasury Proposals (r:0 w:1)
	/// Proof: Treasury Proposals (max_values: None, max_size: Some(108), added: 2583, mode: MaxEncodedLen)
	fn propose_spend() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `107`
		//  Estimated: `1489`
		// Minimum execution time: 24_148_000 picoseconds.
		Weight::from_parts(24_591_000, 0)
			.saturating_add(Weight::from_parts(0, 1489))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: Treasury Proposals (r:1 w:1)
	/// Proof: Treasury Proposals (max_values: None, max_size: Some(108), added: 2583, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn reject_proposal() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `265`
		//  Estimated: `7166`
		// Minimum execution time: 35_683_000 picoseconds.
		Weight::from_parts(36_012_000, 0)
			.saturating_add(Weight::from_parts(0, 7166))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: Treasury Proposals (r:1 w:0)
	/// Proof: Treasury Proposals (max_values: None, max_size: Some(108), added: 2583, mode: MaxEncodedLen)
	/// Storage: Treasury Approvals (r:1 w:1)
	/// Proof: Treasury Approvals (max_values: Some(1), max_size: Some(402), added: 897, mode: MaxEncodedLen)
	/// The range of component `p` is `[0, 99]`.
	fn approve_proposal(p: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `433 + p * (8 ±0)`
		//  Estimated: `5460`
		// Minimum execution time: 10_345_000 picoseconds.
		Weight::from_parts(12_391_108, 0)
			.saturating_add(Weight::from_parts(0, 5460))
			// Standard Error: 811
			.saturating_add(Weight::from_parts(27_830, 0).saturating_mul(p.into()))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Treasury Approvals (r:1 w:1)
	/// Proof: Treasury Approvals (max_values: Some(1), max_size: Some(402), added: 897, mode: MaxEncodedLen)
	fn remove_approval() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `90`
		//  Estimated: `1887`
		// Minimum execution time: 7_743_000 picoseconds.
		Weight::from_parts(8_156_000, 0)
			.saturating_add(Weight::from_parts(0, 1887))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: Treasury Deactivated (r:1 w:1)
	/// Proof: Treasury Deactivated (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Balances InactiveIssuance (r:1 w:1)
	/// Proof: Balances InactiveIssuance (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Treasury Approvals (r:1 w:1)
	/// Proof: Treasury Approvals (max_values: Some(1), max_size: Some(402), added: 897, mode: MaxEncodedLen)
	/// Storage: Treasury Proposals (r:100 w:100)
	/// Proof: Treasury Proposals (max_values: None, max_size: Some(108), added: 2583, mode: MaxEncodedLen)
	/// Storage: System Account (r:200 w:200)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Bounties BountyApprovals (r:1 w:1)
	/// Proof: Bounties BountyApprovals (max_values: Some(1), max_size: Some(402), added: 897, mode: MaxEncodedLen)
	/// The range of component `p` is `[0, 100]`.
	fn on_initialize_proposals(p: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `217 + p * (251 ±0)`
		//  Estimated: `8756 + p * (7789 ±0)`
		// Minimum execution time: 39_348_000 picoseconds.
		Weight::from_parts(46_076_895, 0)
			.saturating_add(Weight::from_parts(0, 8756))
			// Standard Error: 26_068
			.saturating_add(Weight::from_parts(31_896_041, 0).saturating_mul(p.into()))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().reads((3_u64).saturating_mul(p.into())))
			.saturating_add(T::DbWeight::get().writes(4))
			.saturating_add(T::DbWeight::get().writes((3_u64).saturating_mul(p.into())))
			.saturating_add(Weight::from_parts(0, 7789).saturating_mul(p.into()))
	}
}