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

use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// A global index, formed as the extrinsic index within a block, together with
/// that block's height.
#[derive(
	Copy, Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub struct Timepoint {
	/// The height of the chain at the point in time.
	pub height: u32,
	/// The index of the extrinsic at the point in time.
	pub index: u32,
}

/// Identifier Event Entries
#[derive(Encode, Decode, Clone, MaxEncodedLen, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub struct EventEntry<CallTypeOf> {
	/// Identifier Type.
	pub action: CallTypeOf,
	/// Location of the transaction within the ledger,
	pub location: Timepoint,
}

/// Defining the possible actions that can be performed on a identifier.
#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[repr(u8)]
pub enum CallTypeOf {
	Archive,
	Authorization,
	Capacity,
	CouncilRevoke,
	CouncilRestore,
	Deauthorization,
	Approved,
	Genesis,
	Update,
	Revoke,
	Restore,
	Remove,
	PartialRemove,
	PresentationAdded,
	PresentationRemoved,
	Rotate,
	Usage,
	Transfer,
	Debit,
	Credit,
	Issue,
}
/// Defining the identifier target types.
#[derive(Clone, Copy, RuntimeDebug, Decode, Encode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum IdentifierTypeOf {
	Asset,
	Auth,
	ChainSpace,
	Did,
	Rating,
	Registry,
	Statement,
	Schema,
	Template,
}
