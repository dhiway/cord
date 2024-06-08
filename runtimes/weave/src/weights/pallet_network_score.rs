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

//! Autogenerated weights for `pallet_network_score`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2024-03-18, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `smohan-dev-host`, CPU: `AMD EPYC 7B12`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: 1024

// Executed Command:
// ./target/production/cord
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_network_score
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --header=./HEADER-GPL3
// --output=./runtime/src/weights/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::Weight};
use core::marker::PhantomData;

/// Weight functions for `pallet_network_score`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_network_score::WeightInfo for WeightInfo<T> {
	/// Storage: `ChainSpace::Authorizations` (r:1 w:0)
	/// Proof: `ChainSpace::Authorizations` (`max_values`: None, `max_size`: Some(184), added: 2659, mode: `MaxEncodedLen`)
	/// Storage: `ChainSpace::Spaces` (r:1 w:1)
	/// Proof: `ChainSpace::Spaces` (`max_values`: None, `max_size`: Some(206), added: 2681, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::MessageIdentifiers` (r:1 w:1)
	/// Proof: `NetworkScore::MessageIdentifiers` (`max_values`: None, `max_size`: Some(236), added: 2711, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::RatingEntries` (r:1 w:1)
	/// Proof: `NetworkScore::RatingEntries` (`max_values`: None, `max_size`: Some(680), added: 3155, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::AggregateScores` (r:1 w:1)
	/// Proof: `NetworkScore::AggregateScores` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Identifier::Identifiers` (r:1 w:1)
	/// Proof: `Identifier::Identifiers` (`max_values`: None, `max_size`: Some(4294967295), added: 2474, mode: `MaxEncodedLen`)
	/// The range of component `l` is `[1, 15360]`.
	fn register_rating(_l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `801`
		//  Estimated: `4145`
		// Minimum execution time: 48_370_000 picoseconds.
		Weight::from_parts(50_137_334, 0)
			.saturating_add(Weight::from_parts(0, 4145))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(5))
	}
	/// Storage: `ChainSpace::Authorizations` (r:1 w:0)
	/// Proof: `ChainSpace::Authorizations` (`max_values`: None, `max_size`: Some(184), added: 2659, mode: `MaxEncodedLen`)
	/// Storage: `ChainSpace::Spaces` (r:1 w:1)
	/// Proof: `ChainSpace::Spaces` (`max_values`: None, `max_size`: Some(206), added: 2681, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::RatingEntries` (r:2 w:1)
	/// Proof: `NetworkScore::RatingEntries` (`max_values`: None, `max_size`: Some(680), added: 3155, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::MessageIdentifiers` (r:1 w:1)
	/// Proof: `NetworkScore::MessageIdentifiers` (`max_values`: None, `max_size`: Some(236), added: 2711, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::AggregateScores` (r:1 w:1)
	/// Proof: `NetworkScore::AggregateScores` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Identifier::Identifiers` (r:2 w:2)
	/// Proof: `Identifier::Identifiers` (`max_values`: None, `max_size`: Some(4294967295), added: 2474, mode: `MaxEncodedLen`)
	/// The range of component `l` is `[1, 15360]`.
	fn revoke_rating(l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1329`
		//  Estimated: `7300`
		// Minimum execution time: 64_009_000 picoseconds.
		Weight::from_parts(66_046_914, 0)
			.saturating_add(Weight::from_parts(0, 7300))
			// Standard Error: 9
			.saturating_add(Weight::from_parts(15, 0).saturating_mul(l.into()))
			.saturating_add(T::DbWeight::get().reads(9))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: `ChainSpace::Authorizations` (r:1 w:0)
	/// Proof: `ChainSpace::Authorizations` (`max_values`: None, `max_size`: Some(184), added: 2659, mode: `MaxEncodedLen`)
	/// Storage: `ChainSpace::Spaces` (r:1 w:1)
	/// Proof: `ChainSpace::Spaces` (`max_values`: None, `max_size`: Some(206), added: 2681, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::RatingEntries` (r:2 w:1)
	/// Proof: `NetworkScore::RatingEntries` (`max_values`: None, `max_size`: Some(680), added: 3155, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::MessageIdentifiers` (r:1 w:1)
	/// Proof: `NetworkScore::MessageIdentifiers` (`max_values`: None, `max_size`: Some(236), added: 2711, mode: `MaxEncodedLen`)
	/// Storage: `NetworkScore::AggregateScores` (r:1 w:1)
	/// Proof: `NetworkScore::AggregateScores` (`max_values`: None, `max_size`: Some(171), added: 2646, mode: `MaxEncodedLen`)
	/// Storage: `Timestamp::Now` (r:1 w:0)
	/// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	/// Storage: `Identifier::Identifiers` (r:2 w:2)
	/// Proof: `Identifier::Identifiers` (`max_values`: None, `max_size`: Some(4294967295), added: 2474, mode: `MaxEncodedLen`)
	/// The range of component `l` is `[1, 15360]`.
	fn revise_rating(_l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1428`
		//  Estimated: `7300`
		// Minimum execution time: 65_800_000 picoseconds.
		Weight::from_parts(68_253_387, 0)
			.saturating_add(Weight::from_parts(0, 7300))
			.saturating_add(T::DbWeight::get().reads(9))
			.saturating_add(T::DbWeight::get().writes(6))
	}
}
