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

//! Runtime API definition for transaction weight information.
#![warn(unused_extern_crates)]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet_network_membership::RuntimeDispatchWeightInfo;

sp_api::decl_runtime_apis! {
	#[api_version(1)]
	pub trait TransactionWeightApi {
		fn query_weight_info(uxt: Block::Extrinsic ) -> RuntimeDispatchWeightInfo;
	}
}
