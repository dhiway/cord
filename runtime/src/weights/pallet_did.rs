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

/// Weight functions for `pallet_democracy`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_did::WeightInfo for WeightInfo<T> {
    // Storage: System Account (r:2 w:2)
	// Storage: Did DidBlacklist (r:1 w:0)
	// Storage: Did Did (r:1 w:1)
	fn create_ed25519_key(n: u32) -> Weight {
		Weight::from_ref_time(3_207_750_000 as u64)
			.saturating_add(Weight::from_ref_time(1_263_549 as u64).saturating_mul(n as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: System Account (r:2 w:2)
	// Storage: Did DidBlacklist (r:1 w:0)
	// Storage: Did Did (r:1 w:1)
	fn create_sr25519_key(n: u32) -> Weight {
		Weight::from_ref_time(3_220_307_000 as u64)
			.saturating_add(Weight::from_ref_time(1_239_702 as u64).saturating_mul(n as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: System Account (r:2 w:2)
	// Storage: Did DidBlacklist (r:1 w:0)
	// Storage: Did Did (r:1 w:1)
	fn create_ecdsa_key(n: u32) -> Weight {
		Weight::from_ref_time(5_057_009_000 as u64)
			.saturating_add(Weight::from_ref_time(1_422_221 as u64).saturating_mul(n as u64))

			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
	// Storage: Did DidEndpointsCount (r:1 w:1)
	// Storage: Did Did (r:1 w:1)
	// Storage: Did DidBlacklist (r:0 w:1)
	// Storage: Did ServiceEndpoints (r:0 w:1)
	fn delete(c: u32, ) -> Weight {
		Weight::from_ref_time(1_097_385_000 as u64)
			// Standard Error: 312_000
			.saturating_add(Weight::from_ref_time(30_895_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(c as u64)))
	}
	// Storage: Did Did (r:1 w:1)
	// Storage: Did DidEndpointsCount (r:1 w:1)
	// Storage: Did DidBlacklist (r:0 w:1)
	// Storage: Did ServiceEndpoints (r:0 w:1)
	fn reclaim_deposit(c: u32, ) -> Weight {
		Weight::from_ref_time(1_265_977_000 as u64)
			// Standard Error: 304_000
			.saturating_add(Weight::from_ref_time(29_080_000 as u64).saturating_mul(c as u64))
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(c as u64)))
	}
	// Storage: Did Did (r:1 w:1)
	fn submit_did_call_ed25519_key() -> Weight {
		Weight::from_ref_time(1_466_431_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn submit_did_call_sr25519_key() -> Weight {
		Weight::from_ref_time(1_482_638_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn submit_did_call_ecdsa_key() -> Weight {
		Weight::from_ref_time(3_293_485_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn set_authentication_key() -> Weight {
		Weight::from_ref_time(1_043_910_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn set_delegation_key() -> Weight {
		Weight::from_ref_time(1_046_560_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn remove_delegation_key() -> Weight {
		Weight::from_ref_time(945_099_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn set_attestation_key() -> Weight {
		Weight::from_ref_time(1_041_744_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn remove_attestation_key() -> Weight {
		Weight::from_ref_time(954_541_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn add_key_agreement_key() -> Weight {
		Weight::from_ref_time(1_028_188_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:1)
	fn remove_key_agreement_key() -> Weight {
		Weight::from_ref_time(944_145_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Did Did (r:1 w:0)
	// Storage: Did DidEndpointsCount (r:1 w:1)
	// Storage: Did ServiceEndpoints (r:1 w:1)
	fn add_service_endpoint() -> Weight {
		Weight::from_ref_time(1_106_396_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Did ServiceEndpoints (r:1 w:1)
	// Storage: Did DidEndpointsCount (r:1 w:1)
	fn remove_service_endpoint() -> Weight {
		Weight::from_ref_time(818_709_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Did Did (r:1 w:0)
	fn signature_verification_sr25519(l: u32, ) -> Weight {
		Weight::from_ref_time(697_214_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(65_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
	}
	// Storage: Did Did (r:1 w:0)
	fn signature_verification_ed25519(l: u32, ) -> Weight {
		Weight::from_ref_time(662_104_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(74_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
	}
	// Storage: Did Did (r:1 w:0)
	fn signature_verification_ecdsa(l: u32, ) -> Weight {
		Weight::from_ref_time(2_499_548_000 as u64)
			// Standard Error: 0
			.saturating_add(Weight::from_ref_time(29_000 as u64).saturating_mul(l as u64))
			.saturating_add(T::DbWeight::get().reads(1 as u64))
	}
}