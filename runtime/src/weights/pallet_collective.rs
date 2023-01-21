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

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_collective`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_collective::WeightInfo for WeightInfo<T> {
	// Storage: Collective Members (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:0)
	// Storage: Collective Voting (r:100 w:100)
	// Storage: Collective Prime (r:0 w:1)
	/// The range of component `m` is `[1, 100]`.
	/// The range of component `n` is `[1, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn set_members(m: u32, _n: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(0 as u64)
			// Standard Error: 15_000
			.saturating_add(Weight::from_ref_time(10_832_000 as u64).saturating_mul(m as u64))
			// Standard Error: 15_000
			.saturating_add(Weight::from_ref_time(12_894_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(p as u64)))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(p as u64)))
	}
	// Storage: Collective Members (r:1 w:0)
	/// The range of component `b` is `[1, 1024]`.
	/// The range of component `m` is `[1, 100]`.
	fn execute(b: u32, m: u32, ) -> Weight {
		Weight::from_ref_time(19_069_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(2_000 as u64).saturating_mul(b as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(13_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
	}
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective ProposalOf (r:1 w:0)
	/// The range of component `b` is `[1, 1024]`.
	/// The range of component `m` is `[1, 100]`.
	fn propose_execute(b: u32, m: u32, ) -> Weight {
		Weight::from_ref_time(20_794_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(2_000 as u64).saturating_mul(b as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(22_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
	}
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective ProposalOf (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:1)
	// Storage: Collective ProposalCount (r:1 w:1)
	// Storage: Collective Voting (r:0 w:1)
	/// The range of component `b` is `[1, 1024]`.
	/// The range of component `m` is `[2, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn propose_proposed(b: u32, m: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(27_870_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(3_000 as u64).saturating_mul(b as u64))
			// Standard Error: 1_000
			.saturating_add(Weight::from_ref_time(22_000 as u64).saturating_mul(m as u64))
			// Standard Error: 1_000
			.saturating_add(Weight::from_ref_time(94_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective Voting (r:1 w:1)
	/// The range of component `m` is `[5, 100]`.
	fn vote(m: u32, ) -> Weight {
		Weight::from_ref_time(27_249_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(35_000 as u64).saturating_mul(m as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Collective Voting (r:1 w:1)
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective Proposals (r:1 w:1)
	// Storage: Collective ProposalOf (r:0 w:1)
	/// The range of component `m` is `[4, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn close_early_disapproved(m: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(30_754_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(28_000 as u64).saturating_mul(m as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(81_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Collective Voting (r:1 w:1)
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective ProposalOf (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:1)
	/// The range of component `b` is `[1, 1024]`.
	/// The range of component `m` is `[4, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn close_early_approved(b: u32, m: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(39_508_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(1_000 as u64).saturating_mul(b as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(29_000 as u64).saturating_mul(m as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(90_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Collective Voting (r:1 w:1)
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective Prime (r:1 w:0)
	// Storage: Collective Proposals (r:1 w:1)
	// Storage: Collective ProposalOf (r:0 w:1)
	/// The range of component `m` is `[4, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn close_disapproved(m: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(32_769_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(31_000 as u64).saturating_mul(m as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(83_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Collective Voting (r:1 w:1)
	// Storage: Collective Members (r:1 w:0)
	// Storage: Collective Prime (r:1 w:0)
	// Storage: Collective ProposalOf (r:1 w:1)
	// Storage: Collective Proposals (r:1 w:1)
	/// The range of component `b` is `[1, 1024]`.
	/// The range of component `m` is `[4, 100]`.
	/// The range of component `p` is `[1, 100]`.
	fn close_approved(b: u32, m: u32, p: u32, ) -> Weight {
		Weight::from_ref_time(41_704_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(1_000 as u64).saturating_mul(b as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(28_000 as u64).saturating_mul(m as u64))
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(92_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Collective Proposals (r:1 w:1)
	// Storage: Collective Voting (r:0 w:1)
	// Storage: Collective ProposalOf (r:0 w:1)
	/// The range of component `p` is `[1, 100]`.
	fn disapprove_proposal(p: u32, ) -> Weight {
		Weight::from_ref_time(22_720_000 as u64)
			// Standard Error: 2_000
			.saturating_add(Weight::from_ref_time(74_000 as u64).saturating_mul(p as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
}
