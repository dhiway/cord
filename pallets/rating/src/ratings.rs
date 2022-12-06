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

use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// An on-chain rating details mapper to an Identifier.
#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RatingValues {
	/// Rating Count
	pub count: u32,
	/// Rating value
	pub rating: u32,
}

impl sp_std::fmt::Debug for RatingValues {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RatingJournalEntry<T: Config> {
	/// entity identifier
	pub entity: EntityIdentifierOf<T>,
	/// entity rating
	pub overall: RatingValues,
	/// delivery rating
	pub delivery: RatingValues,
	/// transaction digest
	pub digest: RatingHashOf<T>,
}

impl<T: Config> sp_std::fmt::Debug for RatingJournalEntry<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RatingJournal<T: Config> {
	/// rating details
	pub entry: RatingJournalEntry<T>,
	/// rating provider
	pub provider: ProviderIdentifierOf<T>,
	/// The block number in which journal entry is included
	pub block: BlockNumberOf<T>,
}

impl<T: Config> sp_std::fmt::Debug for RatingJournal<T> {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct RatingEntry {
	/// overall rating
	pub overall: RatingValues,
	/// delivery rating
	pub delivery: RatingValues,
}

impl sp_std::fmt::Debug for RatingEntry {
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}
