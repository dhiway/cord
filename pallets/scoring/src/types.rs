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
pub struct JournalDetails<
	EntityIdentifierOf,
	RequestIdentifierOf,
	TransactionIdentfierOf,
	CollectorIdentifierOf,
	RequestorIdentifierOf,
	ScoreTypeOf,
	ScoreOf,
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
	pub score_type: ScoreTypeOf,
	///entity rating
	pub score: ScoreOf,
}

#[derive(Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo)]
pub enum ScoreTypeOf {
	Overall,
	Delivery,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct JournalInput<JournalEntry, EntryHashOf, EntitySignatureOf> {
	/// journal entry
	pub entry: JournalEntry,
	/// tx digest
	pub digest: EntryHashOf,
	/// entity signature
	pub signature: EntitySignatureOf,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct JournalEntry<JournalDetails, EntryHashOf, BlockNumberOf> {
	///journal entry
	pub entry: JournalDetails,
	/// tx digest
	pub digest: EntryHashOf,
	/// The block number in which journal entry is included
	pub block: BlockNumberOf,
}

#[derive(
	Encode, Decode, MaxEncodedLen, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, TypeInfo,
)]
pub struct ScoreEntry<CountOf, ScoreOf> {
	///entry count
	pub count: CountOf,
	/// aggregrated Score
	pub score: ScoreOf,
}
