// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Entry pallet types.

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct RegistryEntryDetails<RegistryEntryHashOf, StatusOf, ProfileIdOf, RegistryIdentifierOf> {
	/// Type of Registry Entry Digest associated with the document.
	pub tx_hash: RegistryEntryHashOf,
	/// Type of Registry Entry Revoked state.
	pub revoked: StatusOf,
	/// Type of Profile Identity Registry Entry Creator.
	pub creator: ProfileIdOf,
	/// Type of Reistry Entry Identifier.
	pub registry_id: RegistryIdentifierOf,
	// /// Optionally, the document identifier as a bounded vector.
	// pub doc_id: Option<BoundedVec<u8, ConstU32<64>>>,
	// /// Optionally, the identity account (profile) that created (authored) the document.
	// pub doc_author_profile_id: Option<ProfileIdOf>,
	// /// Optionally, the node identifier as a bounded vector.
	// pub doc_node_id: Option<BoundedVec<u8, ConstU32<64>>>,
	// /// Optionally, the document entry identifier as a bounded vector.
	// pub doc_entry_id: Option<BoundedVec<u8, ConstU32<64>>>,
}
