// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2024 Dhiway Networks Pvt. Ltd.
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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;

#[derive(
	Encode,
	Decode,
	Clone,
	RuntimeDebug,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
	Default,
)]
pub struct StoreEntry<StoreEntryCreatorOf, EntryHashOf, StoreEntryStateOf, BlockNumber> {
	// Creator of the store entry
	pub entry_creator: StoreEntryCreatorOf,
	// Digest of the entry document
	pub digest: EntryHashOf,
	// State of the Entry
	pub entry_state: StoreEntryStateOf,
	// Store entry creation block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub enum StoreEntryStateOf {
	ACTIVE,
	INACTIVE,
}
