// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

//! Runtime API definition for assets.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct DecodedIdentifierApi {
	pub network: u32,
	pub pallet: u16,
	pub digest: String,
}

sp_api::decl_runtime_apis! {
	pub trait IdentifierApi {
		/// Decodes an dentifier into its structured form,
		/// or returns `None` if decoding fails.
		fn decode_identifier(identifier: Vec<u8>) -> Option<DecodedIdentifierApi>;

		/// Resolves a pallet name from storage by the given pallet index,
		/// or returns `None` if it doesn't exist.
		fn resolve_pallet(index: u16) -> Option<String>;

	}
}
