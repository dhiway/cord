// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use codec::{Decode, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::{BoundedVec, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound};
use indiv_support::{
	members_notifier_subscriber::MembersTypeConfig,
	traits::{Identifier, RingExponent, RingIndex},
};
use scale_info::TypeInfo;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
use verifiable::GenerateVerifiable;

pub use indiv_support::members_notifier_subscriber::SequenceNumber;

/// Paging state for the updates.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug, TypeInfo, MaxEncodedLen, Default)]
pub struct PagingState<BlockNumber: Default> {
	/// Page index that `on_ring_root_change` writes to.
	pub write_page: u16,
	/// Page index that the offchain worker reads from and sends to subscribers.
	pub send_page: u16,
	/// Block number when the last update batch was sent.
	pub last_update_block: BlockNumber,
}

/// Ring root members type for this pallet's Config.
pub type MembersOf<T> = <<T as Config>::Crypto as GenerateVerifiable>::Members;

/// Wrapper type that implements MembersTypeConfig using the pallet's Config.
/// This bridges the pallet-specific Config to the shared type trait.
pub struct NotifierConfig<T>(PhantomData<T>);

impl<T: Config> MembersTypeConfig for NotifierConfig<T> {
	type Crypto = T::Crypto;
	type MaxUpdatesPerBatch = T::MaxUpdatesPerBatch;
	type MaxCollections = T::MaxCollections;
}

/// Represents a single ring root update to be sent to subscribers.
pub type RingRootUpdate<T> =
	indiv_support::members_notifier_subscriber::RingRootUpdate<NotifierConfig<T>>;

/// Batch of ring root updates sent to a subscriber.
pub type RingRootUpdatesBatch<T> =
	indiv_support::members_notifier_subscriber::RingRootUpdatesBatch<NotifierConfig<T>>;

/// The state of the batch being distributed via offchain worker.
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug, TypeInfo, MaxEncodedLen, Default)]
pub struct BatchDistributionState<BlockNumber: Default> {
	/// Monotonically increasing batch number. Subscribers compare this against their
	/// `last_init_sequence` to determine whether they already have the data.
	pub sequence: SequenceNumber,
	/// Unix timestamp (seconds) when the batch was created, included in updates sent to
	/// subscribers.
	pub source_time: u64,
	/// Block number when the batch was sealed and made available for offchain distribution.
	pub sealed_at: BlockNumber,
	/// Number of subscribers still waiting to receive this batch. Decremented on each
	/// successful send; the batch is cleared when this reaches zero.
	pub remaining_subscribers: u32,
}

/// Invalidity reasons for authorize closure validation.
#[derive(Clone)]
pub enum CustomInvalidity {
	TransactionNotLocal = 200,
	BatchInProgress = 201,
	NoPendingWork = 202,
	NoBatchActive = 203,
	AlreadySent = 204,
	SubscriberNotFound = 205,
	NoPendingInit = 206,
	BatchNotStuck = 207,
}

impl From<CustomInvalidity> for TransactionValidityError {
	fn from(e: CustomInvalidity) -> Self {
		InvalidTransaction::Custom(e as u8).into()
	}
}

/// State for paginated subscriber initialization when ring roots exceed XCM message size limit.
/// Stored when `subscribe` call requires multiple XCM messages to send all initial data.
#[derive(
	CloneNoBound, PartialEqNoBound, EqNoBound, Encode, Decode, DebugNoBound, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct PendingInitState<T: Config> {
	/// Collections this subscriber subscribed to (sorted, no duplicates).
	pub collections: BoundedVec<(Identifier, RingExponent), T::MaxCollectionsPerSubscriber>,
	/// Index of the current collection being processed.
	pub current_collection_index: u32,
	/// Ring index to start after in the next page.
	/// `None` means start from the beginning of the collection.
	pub after_ring_index: Option<RingIndex>,
	/// Sequence number assigned to this initialization.
	pub sequence: SequenceNumber,
	/// Source time for all batches in this initialization.
	pub source_time: u64,
	/// Pallet index of members-subscriber on the subscriber chain.
	pub pallet_index: u8,

	#[codec(skip)]
	pub _phantom: PhantomData<T>,
}

/// Information about a subscriber parachain.
#[derive(
	CloneNoBound, PartialEqNoBound, EqNoBound, Encode, Decode, DebugNoBound, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct SubscriberInfo<T: Config> {
	/// Ring collections this subscriber subscribed to (sorted, no duplicates).
	pub collections: BoundedVec<(Identifier, RingExponent), T::MaxCollectionsPerSubscriber>,
	/// BatchSequenceNumber at the time this subscriber was initialized.
	/// Used to skip updates: if last_init_sequence >= batch's sequence number,
	/// the subscriber already has the data via initialization.
	pub last_init_sequence: SequenceNumber,
	/// Pallet index of members-subscriber on the subscriber chain.
	/// Used for XCM Transact call encoding.
	pub pallet_index: u8,
}
