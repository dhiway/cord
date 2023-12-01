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

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct EntityDetails<EntityIdentifier> {
	/// Unique Identifier (UID) for the entity being rated
	pub entity_uid: EntityIdentifier,
	/// messsage identifier of the rating entry
	pub entity_id: EntityIdentifier,
}

#[derive(
	Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
pub struct RatingInputEntry<EntityIdentifier, EntityTypeOf, RatingTypeOf> {
	/// Unique Identifier (UID) for the entity being rated
	pub entity_uid: EntityIdentifier,
	/// Unique Identifier (UID) for the rating provider
	pub provider_uid: EntityIdentifier,
	/// Count of raing transactions for the entry
	pub count_of_txn: u64,
	/// Cumulative sum of ratings for the entity
	pub total_rating: u64,
	/// Type of the entity (seller/logistic)
	pub entity_type: EntityTypeOf,
	/// Type of rating (overall/delivery)
	pub rating_type: RatingTypeOf,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RatingTypeOf {
	Overall,
	Delivery,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum EntityTypeOf {
	Retail,
	Logistic,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum EntryTypeOf {
	Credit,
	Debit,
}

impl EntityTypeOf {
	pub fn is_valid_entity_type(&self) -> bool {
		matches!(self, Self::Retail | Self::Logistic)
	}
}

impl RatingTypeOf {
	pub fn is_valid_rating_type(&self) -> bool {
		matches!(self, Self::Overall | Self::Delivery)
	}
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct RatingEntry<
	EntityIdentifier,
	EntityTypeOf,
	RatingTypeOf,
	RatingEntryId,
	RatingEntryHash,
	MessageIdentifier,
	SpaceIdOf,
	RatingCreatorId,
	EntryTypeOf,
	BlockNumber,
> {
	pub entry: RatingInputEntry<EntityIdentifier, EntityTypeOf, RatingTypeOf>,
	/// rating digest
	pub digest: RatingEntryHash,
	/// messsage identifier of the rating entry
	pub message_id: MessageIdentifier,
	/// Space Identifier
	pub space: SpaceIdOf,
	/// Rating creator.
	pub creator: RatingCreatorId,
	/// Type of the rating entry (credit/debit)
	pub entry_type: EntryTypeOf,
	/// Rating reference entry
	pub reference_id: Option<RatingEntryId>,
	/// The block number in which journal entry is included
	pub created_at: BlockNumber,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct AggregatedEntryOf {
	/// aggregated transaction count
	pub count_of_txn: u64,
	/// aggregated rating
	pub total_rating: u64,
}
