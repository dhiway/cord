// This file is part of CORD – https://cord.network

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

//! Runtime API definition for CORD Identifiers.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::Encode;
pub use cord_primitives::AccountId as CordAccountId;
pub use identifier::{IdentifierOf, IdentifierTypeOf};

sp_api::decl_runtime_apis! {
	pub trait IdentifierApi<IdentifierOf, IdentifierTypeOf>
	where
		IdentifierOf: Encode,
		IdentifierTypeOf: Encode,
	{
		/// Verifies if an identifier exists for any IdentifierType.
		fn identifier_exists(identifier: IdentifierOf) -> bool;

		/// Verifies if an identifier exists for a specific IdentifierType.
		fn identifier_exists_by_type(identifier: IdentifierOf, identifier_type: IdentifierTypeOf) -> bool;
	}
}
