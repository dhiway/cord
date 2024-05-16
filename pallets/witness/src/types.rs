// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2022 Dhiway Networks Pvt. Ltd.
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
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct WitnessEntry<WitnessCreatorOf, EntryHashOf, WitnessStatusOf, BlockNumber> {
	// Creator of the witness Id
	pub witness_creator: WitnessCreatorOf,
	// Digest of the document
	pub digest: EntryHashOf,
	// Number of witness other including creator required for document approval
	pub required_witness_count: u32,
	/// Number of witness currently signed.
	pub current_witness_count: u32,
	// Current witness status of the document
	pub witness_status: WitnessStatusOf,
	// Witness creation block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct WitnessSignersEntry<WitnessesOf, BlockNumber> {
	// Witnesses who signed the document
	pub witnesses: WitnessesOf,
	// Witness inclusion block
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub enum WitnessStatusOf {
	// Status indicating not all witness have signed the document
	WITNESSAPRROVALPENDING,
	// Status indicating all required witness have signed the document
	WITNESSAPPROVALCOMPLETE,
}
