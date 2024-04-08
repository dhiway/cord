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
pub struct RatingInputEntry<EntityIdentifier, RatingProviderId, RatingTypeOf> {
	/// Identifier for the entity being rated
	pub entity_id: EntityIdentifier,
	/// Unique Identifier (UID) for the rating provider
	pub provider_id: EntityIdentifier,
	/// Count of raing transactions for the entry
	pub count_of_txn: u64,
	/// Cumulative sum of ratings for the entity
	pub total_encoded_rating: u64,
	/// Type of rating (overall/delivery)
	pub rating_type: RatingTypeOf,
	/// DID identifier of the provider
	pub provider_did: RatingProviderId,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum RatingTypeOf {
	Overall,
	Delivery,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum EntryTypeOf {
	Credit,
	Debit,
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
	RatingProviderId,
	RatingTypeOf,
	RatingEntryId,
	RatingEntryHash,
	MessageIdentifier,
	SpaceIdOf,
	AccountId,
	EntryTypeOf,
	Moment,
> {
	pub entry: RatingInputEntry<EntityIdentifier, RatingProviderId, RatingTypeOf>,
	/// rating digest
	pub digest: RatingEntryHash,
	/// messsage identifier of the rating entry
	pub message_id: MessageIdentifier,
	/// Space Identifier
	pub space: SpaceIdOf,
	/// entity anchoring the transaction on-chain
	pub creator_id: AccountId,
	/// Type of the rating entry (credit/debit)
	pub entry_type: EntryTypeOf,
	/// Rating reference entry
	pub reference_id: Option<RatingEntryId>,
	/// The block number in which rating entry is included
	pub created_at: Moment,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct AggregatedEntryOf {
	/// aggregated transaction count
	pub count_of_txn: u64,
	/// aggregated rating
	pub total_encoded_rating: u64,
}
