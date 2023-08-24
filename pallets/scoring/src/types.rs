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
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct RatingEntryDetails<
	EntityIdentifierOf,
	RequestIdentifierOf,
	TransactionIdentfierOf,
	CollectorIdentifierOf,
	RequestorIdentifierOf,
	RatingTypeOf,
	RatingOf,
> {
	///entity Identifier
	pub entity: EntityIdentifierOf,
	/// unique request identifier for which the score is provided
	pub uid: RequestIdentifierOf,
	/// transaction identifier for which the score is requested
	pub tid: TransactionIdentfierOf,
	///score collector identifier
	pub collector: CollectorIdentifierOf,
	///score requestor identifier
	pub requestor: RequestorIdentifierOf,
	/// score type
	pub rating_type: RatingTypeOf,
	///entity rating
	pub rating: RatingOf,
	
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RatingTypeOf {
	Overall,
	Delivery,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct RatingInput<RatingEntryDetails, RatingEntryHashOf, RatingCreatorIdOf> {
	/// journal entry
	pub entry: RatingEntryDetails,
	/// tx digest
	pub digest: RatingEntryHashOf,
	/// entity signature
	pub creator: RatingCreatorIdOf,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct RatingEntry<RatingEntryDetails, RatingEntryHashOf, BlockNumber,RegistryIdOf,RatingCreatorIdOf> {
	///journal entry
	pub entry: RatingEntryDetails,
	/// tx digest
	pub digest: RatingEntryHashOf,
	/// The block number in which journal entry is included
	pub created_at: BlockNumber,
	/// Registry Identifier
	pub registry: RegistryIdOf,
	/// Rating creator.
	pub creator: RatingCreatorIdOf,

}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct ScoreEntry<CountOf, RatingOf> {
	///entry count
	pub count: CountOf,
	/// aggregrated Score
	pub rating: RatingOf,
}
