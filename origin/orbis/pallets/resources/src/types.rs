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

//! Resources types

use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;

pub type SignatureOf<T> =
	<<<T as Config>::MemberService as MembershipProver>::Crypto as GenerateVerifiable>::Signature;
pub type ProofOf<T> =
	<<<T as Config>::MemberService as MembershipProver>::Crypto as GenerateVerifiable>::Proof;

/// Friend request period and sequence used to identify a registration slot.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct FriendRequestReference {
	/// Period used in the friend request context.
	pub period: u32,
	/// Sequence used in the friend request context.
	pub seq: u8,
}

/// Friend request statement account registration bound to an anonymous alias.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct FriendRequestRegistration<Account> {
	/// Statement account granted temporary allowance.
	pub account_id: Account,
	/// Friend request slot used in the registration context.
	pub reference: FriendRequestReference,
}

/// Value stored per anonymous statement store allowance entry.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	DebugNoBound,
	TypeInfo,
	MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct StmtStoreAllowanceEntry<T: Config> {
	/// The statement account granted the allowance.
	pub account_id: T::AccountId,
	/// The slot sequence number within the period.
	pub seq: u32,
	/// Timestamp (seconds since Unix epoch) when this entry was last set.
	/// Used to enforce a cooldown before the same alias can replace it within the same period.
	pub since: u64,
}

/// The information related to a particular consumer.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct ConsumerInfo {
	/// An opaque key type which will be used in E2E encrypted communication between consumers.
	pub identifier_key: CommunicationIdentifier,
	/// The credibility of a consumer.
	pub credibility: Credibility,
}

/// The credibility of a consumer.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub enum Credibility {
	/// Recognized as a lite person.
	Lite,
	/// Recognized as a full person with an alias. Since personhood can be suspended, in order to
	/// ensure fair access to the resources, we record a timestamp of the last interaction with
	/// this consumer using the person authentication.
	Person { alias: Alias, last_update: u64, demoted: bool },
}

/// Selects which member collection to verify a ring-VRF proof against.
///
/// [`MembershipCollection::People`] uses `PEOPLE_MEMBER_IDENTIFIER`, while
/// [`MembershipCollection::LitePeople`] uses `LITE_PEOPLE_MEMBER_IDENTIFIER`.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum MembershipCollection {
	/// Proven membership in the people collection.
	People,
	/// Proven membership in the lite-people collection.
	LitePeople,
}

/// Allocation parameters for long-term storage on a remote chain.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct LongTermStorageAllocation {
	/// Maximum number of transactions allowed.
	pub transactions: u32,
	/// Maximum total bytes allowed.
	pub bytes: u64,
}

pub use orbis_transaction_storage_primitives::ReservationId;

/// Stable purpose bound to an isolated Orbis Storage reservation.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen,
)]
pub enum ReservationPurpose {
	Membership { period: u32, alias: Alias, counter: u8, collection: MembershipCollection },
	ProofOfInk { candidate_hash: [u8; 32], allocation_index: u32 },
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEqNoBound,
	EqNoBound,
	DebugNoBound,
	TypeInfo,
	MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct StorageClaim<T: Config> {
	pub purpose: ReservationPurpose,
	pub owner: T::AccountId,
	pub created_at: BlockNumberFor<T>,
}
