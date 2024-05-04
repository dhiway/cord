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

//! # Score Pallet
//!
//! The Score Pallet is responsible for maintaining aggregated scores for
//! various entities within the system. It updates these scores based on new
//! journal entries associated with respective account IDs. This pallet
//! plays a crucial role in managing and reflecting the reputation or
//! performance metrics of entities on the blockchain.
//!
//! ## Overview
//!
//! The Score Pallet provides functionality to:
//! - Register new rating entries.
//! - Amend or revise existing ratings.
//! - Aggregate scores based on credit/debit entries.
//!
//! It interacts with other components of the system to ensure accurate and
//! up-to-date scorekeeping.
//!
//! ### Terminology
//!
//! - **Rating Entry:** A record detailing the rating given to an entity, including the score, the
//!   entity being rated, and other metadata.
//! - **Score:** A numerical representation of an entity's aggregated ratings over time.
//! - **Aggregate Score:** The cumulative score of an entity, calculated by summing individual
//!   scores from rating entries.
//! - **Credit Entry:** A positive adjustment or addition to an entity's score.
//! - **Debit Entry:** A negative adjustment or subtraction from an entity's score.
//!
//! ### Assumptions
//!
//! - Scores and ratings are assumed to be represented as integers.
//! - The Score hash is assumed to be generated using the CORD SDK, ensuring integrity and
//!   non-repudiation of score data.
//!
//! ### Storage
//!
//! - `RatingEntries`: Stores all the rating entries, indexed by a unique identifier.
//! - `AggregateScores`: Keeps track of the aggregate scores for each entity and rating type.
//!
//! ### Events
//!
//! The pallet emits events when:
//! - A new rating entry is added.
//! - An existing rating entry is amended.
//! - Aggregate scores are updated.
//!
//! ### Errors
//!
//! Errors indicate conditions where an operation could not be completed:
//! - Invalid rating values.
//! - Unauthorized operations.
//! - Duplicate message identifiers.
//! - Reference identifiers not found.
//!
//! [`Config`]: The configuration trait for the pallet.
//! [`Call`]: The dispatchable calls supported by the pallet.
//! [`Pallet`]: The main struct that implements the pallet's functionality.
//!
//! ## Usage
//!
//! The following example shows how to use the Score Pallet in your runtime:
//!
//! ```rust
//! use score_pallet::{Pallet, Call, Config};
//! ```
//!
//! ## Interface
//!
//! ### Public Functions
//!
//! - `register_rating`: Registers a new rating entry.
//! - `amend_rating`: Amends an existing rating entry.
//! - `revise_rating`: Revises a rating entry, creating a new linked entry.
//!
//! ## Implementation Details
//!
//! The Score Pallet utilizes a combination of storage maps and events to
//! maintain and communicate the state and changes in scoring data. Efficient
//! and secure hash algorithms are used for score calculation and verification.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
pub mod tests;

pub mod weights;

pub mod types;

pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_support::ensure;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{CountOf, RatingOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, Twox64Concat};
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::traits::Hash;
	use sp_std::{prelude::Clone, str};

	/// SS58 Chain Space Identifier
	pub type SpaceIdOf = Ss58Identifier;

	///SS58 Rating Identifier
	pub type RatingEntryIdOf = Ss58Identifier;

	///SS58 Entity Identifier
	pub type EntityIdOf = Ss58Identifier;

	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;

	/// Hash of the registry.
	pub type RatingHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a creator identifier.
	pub type RatingProviderIdOf<T> = pallet_chain_space::SpaceCreatorOf<T>;

	/// Hash of the Rating.
	pub type RatingEntryHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Type for an Identifier
	pub type IdentifierOf = Ss58Identifier;

	///Type for a Collector(Buyer) Identifier
	pub type CollectorIdentifierOf<T> = <T as frame_system::Config>::AccountId;

	pub type JournalIdentifierOf = IdentifierOf;
	pub type MessageIdentifierOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type EntityIdentifierOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type EntityIdentityOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;
	pub type ProviderIdentifierOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	pub type RatingInputEntryOf<T> =
		RatingInputEntry<EntityIdentifierOf<T>, RatingProviderIdOf<T>, RatingTypeOf>;

	pub type RatingEntryOf<T> = RatingEntry<
		EntityIdentifierOf<T>,
		RatingProviderIdOf<T>,
		RatingTypeOf,
		RatingEntryIdOf,
		RatingEntryHashOf<T>,
		MessageIdentifierOf<T>,
		SpaceIdOf,
		AccountIdOf<T>,
		EntryTypeOf,
		<T as timestamp::Config>::Moment,
	>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config + timestamp::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, RatingProviderIdOf<Self>>;

		type RatingProviderIdOf: Parameter + MaxEncodedLen;

		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;
		#[pallet::constant]
		type MaxRatingValue: Get<u32>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// rating entry identifiers with rating details stored on chain.
	#[pallet::storage]
	pub type RatingEntries<T> =
		StorageMap<_, Blake2_128Concat, RatingEntryIdOf, RatingEntryOf<T>, OptionQuery>;

	/// aggregated network score - aggregated and mapped to an entity
	/// identifier.
	#[pallet::storage]
	pub type AggregateScores<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		EntityIdentifierOf<T>,
		Blake2_128Concat,
		RatingTypeOf,
		AggregatedEntryOf,
		OptionQuery,
	>;

	#[pallet::storage]
	pub type MessageIdentifiers<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		MessageIdentifierOf<T>,
		Blake2_128Concat,
		RatingProviderIdOf<T>,
		RatingEntryIdOf,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new rating entry has been added.
		/// \[rating entry identifier, entity, provider, creator\]
		RatingEntryAdded {
			identifier: RatingEntryIdOf,
			entity: EntityIdentifierOf<T>,
			provider: RatingProviderIdOf<T>,
			creator: AccountIdOf<T>,
		},
		/// A rating entry has been amended.
		/// \[rating entry identifier, entity, creator\]
		RatingEntryRevoked {
			identifier: RatingEntryIdOf,
			entity: EntityIdentifierOf<T>,
			provider: RatingProviderIdOf<T>,
			creator: AccountIdOf<T>,
		},
		/// A rating entry has been revised (after amend).
		/// \[rating entry identifier, entity, creator \]
		RatingEntryRevised {
			identifier: RatingEntryIdOf,
			entity: EntityIdentifierOf<T>,
			provider: RatingProviderIdOf<T>,
			creator: AccountIdOf<T>,
		},
		/// Aggregate scores has been updated.
		/// \[entity identifier\]
		AggregateScoreUpdated { entity: EntityIdentifierOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unauthorized operation
		UnauthorizedOperation,
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Invalid digest
		InvalidDigest,
		/// Invalid creator signature
		InvalidSignature,
		/// Invalid Rating Identifier
		InvalidRatingIdentifier,
		/// Transaction already rated
		MessageIdAlreadyExists,
		/// Invalid rating value - should be between 1 and 50
		InvalidRatingValue,
		/// Exceeds the maximum allowed entries in a single transaction
		TooManyJournalEntries,
		/// Invalid entity signature
		InvalidEntitySignature,
		/// Stream digest is not unique
		DigestAlreadyAnchored,
		/// Rating idenfier already exist
		RatingIdentifierAlreadyAdded,
		/// Invalid rating type
		InvalidRatingType,
		/// Rating identifier not found
		RatingIdentifierNotFound,
		/// Referenced rating identifier not found
		ReferenceIdentifierNotFound,
		/// Refrenced identifer is not a debit transaction
		ReferenceNotDebitIdentifier,
		/// Rating Entity mismatch
		EntityMismatch,
		/// Rating Space mismatch
		SpaceMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a new rating in the system.
		///
		/// This function allows a user to submit a new rating for an entity.
		/// The rating is recorded along with various metadata, including the
		/// author of the rating, the space ID, and a unique message identifier.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `entry` - The rating entry, containing details about the entity being rated, the
		///   rating itself, and other metadata.
		/// * `digest` - A hash representing some unique aspects of the rating, used for
		///   identification and integrity purposes.
		/// * `authorization` - An identifier for authorization, used to validate the origin's
		///   permission to make this rating.
		///
		/// # Errors
		/// Returns `Error::<T>::InvalidRatingValue` if the rating value is not
		/// within the expected range.
		/// Returns `Error::<T>::InvalidRatingType` if the entry type or
		/// rating type is not valid.
		/// Returns `Error::<T>::MessageIdAlreadyExists` if the message
		/// identifier is already used.
		/// Returns `Error::<T>::InvalidIdentifierLength` if the generated
		/// identifier for the rating is of invalid length.
		/// Returns `Error::<T>::RatingIdentifierAlreadyAdded` if the rating
		/// identifier is already in use.
		///
		/// # Events
		/// Emits `RatingEntryAdded` when a new rating is successfully
		/// registered.
		///
		/// # Example
		/// ```
		/// register_rating(origin, entry, digest, authorization)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn register_rating(
			origin: OriginFor<T>,
			entry: RatingInputEntryOf<T>,
			digest: RatingEntryHashOf<T>,
			message_id: MessageIdentifierOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let tx_authors = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let provider = tx_authors.subject();
			let creator = tx_authors.sender();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&provider,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			ensure!(
				entry.total_encoded_rating > 0 &&
					entry.count_of_txn > 0 &&
					entry.total_encoded_rating <=
						entry.count_of_txn * T::MaxRatingValue::get() as u64,
				Error::<T>::InvalidRatingValue
			);

			ensure!(entry.rating_type.is_valid_rating_type(), Error::<T>::InvalidRatingType);

			ensure!(
				!<MessageIdentifiers<T>>::contains_key(&message_id, &provider),
				Error::<T>::MessageIdAlreadyExists
			);

			let provider_did = entry.provider_did.clone();
			let entity_id = entry.entity_id.clone();

			// Id Digest = concat (H(<scale_encoded_digest>,(<scale_encoded_entity_id>),
			// (<scale_encoded_message_id> <scale_encoded_space_identifier>,
			// <scale_encoded_provider_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&digest.encode()[..],
					&entity_id.encode()[..],
					&message_id.encode()[..],
					&space_id.encode()[..],
					&provider_did.encode()[..],
				]
				.concat()[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Rating,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<RatingEntries<T>>::contains_key(&identifier),
				Error::<T>::RatingIdentifierAlreadyAdded
			);

			Self::aggregate_score(&entry, EntryTypeOf::Credit)?;

			let entity = entry.entity_id.clone();
			let created_at = Self::get_current_time();

			<RatingEntries<T>>::insert(
				&identifier,
				RatingEntryOf::<T> {
					entry,
					digest,
					message_id: message_id.clone(),
					space: space_id,
					creator_id: creator.clone(),
					entry_type: EntryTypeOf::Credit,
					reference_id: None,
					created_at,
				},
			);

			<MessageIdentifiers<T>>::insert(message_id, &provider_did, &identifier);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(Error::<T>::from)?;

			Self::deposit_event(Event::RatingEntryAdded {
				identifier,
				entity,
				provider: provider_did,
				creator,
			});

			Ok(())
		}

		/// Amends an existing rating entry by creating a debit entry linked to
		/// the original.
		///
		/// This function facilitates the amendment of a previously submitted
		/// rating. It creates a debit entry referencing the original rating
		/// entry. This function is typically used to correct or revoke a
		/// rating.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, usually a signed user.
		/// * `entry_identifier` - The identifier of the rating entry to be amended.
		/// * `message_id` - A new message identifier for the amendment.
		/// * `digest` - A hash representing the amendment, used for identification and integrity
		///   purposes.
		/// * `authorization` - An identifier for authorization, validating the origin's permission
		///   to amend the rating.
		///
		/// # Errors
		/// Returns `Error::<T>::RatingIdentifierNotFound` if the original
		/// rating entry is not found.
		/// Returns `Error::<T>::UnauthorizedOperation` if the origin does not
		/// have the authority to amend the rating.
		/// Returns `Error::<T>::MessageIdAlreadyExists` if the new message
		/// identifier is already in use.
		/// Returns `Error::<T>::InvalidIdentifierLength` if the generated
		/// identifier for the amendment is of invalid length.
		/// Returns `Error::<T>::RatingIdentifierAlreadyAdded` if the amendment
		/// identifier is already in use.
		///
		/// # Events
		/// Emits `RatingEntryRevoked` when a rating entry is successfully
		/// amended.
		///
		/// # Example
		/// ```
		/// amend_rating(origin, entry_identifier, message_id, digest, authorization)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn revoke_rating(
			origin: OriginFor<T>,
			entry_identifier: RatingEntryIdOf,
			message_id: MessageIdentifierOf<T>,
			digest: RatingEntryHashOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let tx_authors = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let provider = tx_authors.subject();
			let creator = tx_authors.sender();

			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&provider,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let rating_details = <RatingEntries<T>>::get(&entry_identifier)
				.ok_or(Error::<T>::RatingIdentifierNotFound)?;

			ensure!(rating_details.space == space_id, Error::<T>::UnauthorizedOperation);

			ensure!(
				!<MessageIdentifiers<T>>::contains_key(&message_id, &provider),
				Error::<T>::MessageIdAlreadyExists
			);

			let provider_did = rating_details.entry.provider_did.clone();
			let entity_id = rating_details.entry.entity_id.clone();

			// Id Digest = concat (H(<scale_encoded_digest>,(<scale_encoded_entity_id>),
			// (<scale_encoded_message_id>) <scale_encoded_space_identifier>,
			// <scale_encoded_provider_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&digest.encode()[..],
					&entity_id.encode()[..],
					&message_id.encode()[..],
					&space_id.encode()[..],
					&provider_did.encode()[..],
				]
				.concat()[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Rating,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<RatingEntries<T>>::contains_key(&identifier),
				Error::<T>::RatingIdentifierAlreadyAdded
			);

			Self::aggregate_score(&rating_details.entry, EntryTypeOf::Debit)?;

			let entity = rating_details.entry.entity_id.clone();
			let created_at = Self::get_current_time();

			<RatingEntries<T>>::insert(
				&identifier,
				RatingEntryOf::<T> {
					creator_id: creator.clone(),
					entry_type: EntryTypeOf::Debit,
					reference_id: Some(entry_identifier.clone()),
					created_at,
					..rating_details
				},
			);

			<MessageIdentifiers<T>>::insert(&message_id, &provider_did, &identifier);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(Error::<T>::from)?;
			Self::update_activity(&entry_identifier, CallTypeOf::Debit)
				.map_err(Error::<T>::from)?;

			Self::deposit_event(Event::RatingEntryRevoked {
				identifier,
				entity,
				provider: provider_did,
				creator,
			});

			Ok(())
		}

		/// Revises an existing rating by creating a new credit entry linked to
		/// the original.
		///
		/// This function allows for the modification of a previously submitted
		/// rating. It creates a new credit entry which is linked to the
		/// original rating (referred to by `amend_ref_id`). This function is
		/// used for correcting or updating an existing rating.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, usually a signed user.
		/// * `entry` - The new rating entry with updated details.
		/// * `digest` - A hash representing the revised rating, used for identification and
		///   integrity.
		/// * `message_id` - A new message identifier for the revised rating.
		/// * `amend_ref_id` - The identifier of the original rating entry that is being revised.
		/// * `authorization` - An identifier for authorization, validating the origin's permission
		///   to revise the rating.
		///
		/// # Errors
		/// Returns `Error::<T>::InvalidRatingValue` if the new rating value is
		/// not within the expected range.
		/// Returns `Error::<T>::InvalidRatingType` if the entry type or
		/// rating type of the new rating is invalid.
		/// Returns `Error::<T>::ReferenceIdentifierNotFound` if the original
		/// rating reference identifier is not found.
		/// Returns `Error::<T>::EntityMismatch` if the entity UID of the new
		/// rating does not match the original.
		/// Returns `Error::<T>::SpaceMismatch` if the space ID does not match
		/// the original. Returns `Error::<T>::ReferenceNotAmendIdentifier` if
		/// the original entry is not a debit entry.
		/// Returns `Error::<T>::MessageIdAlreadyExists` if the new message
		/// identifier is already in use.
		/// Returns `Error::<T>::InvalidIdentifierLength` if the generated
		/// identifier for the revision is of invalid length.
		/// Returns `Error::<T>::RatingIdentifierAlreadyAdded` if the revised
		/// rating identifier is already in use.
		///
		/// # Events
		/// Emits `RatingEntryRevoked` when an existing rating entry is
		/// successfully revised.
		///
		/// # Example
		/// ```
		/// revise_rating(origin, entry, digest, message_id, amend_ref_id, authorization)?;
		/// ```
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn revise_rating(
			origin: OriginFor<T>,
			entry: RatingInputEntryOf<T>,
			digest: RatingEntryHashOf<T>,
			message_id: MessageIdentifierOf<T>,
			debit_ref_id: RatingEntryIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let tx_authors = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let provider = tx_authors.subject();
			let creator = tx_authors.sender();

			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&provider,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			ensure!(
				entry.total_encoded_rating > 0 &&
					entry.count_of_txn > 0 &&
					entry.total_encoded_rating <=
						entry.count_of_txn * T::MaxRatingValue::get() as u64,
				Error::<T>::InvalidRatingValue
			);

			ensure!(entry.rating_type.is_valid_rating_type(), Error::<T>::InvalidRatingType);

			let rating_details = <RatingEntries<T>>::get(&debit_ref_id)
				.ok_or(Error::<T>::ReferenceIdentifierNotFound)?;

			ensure!(entry.entity_id == rating_details.entry.entity_id, Error::<T>::EntityMismatch);
			ensure!(space_id == rating_details.space, Error::<T>::SpaceMismatch);

			let stored_entry_type: EntryTypeOf = rating_details.entry_type;
			ensure!(
				EntryTypeOf::Debit == stored_entry_type,
				Error::<T>::ReferenceNotDebitIdentifier
			);

			ensure!(
				!<MessageIdentifiers<T>>::contains_key(&message_id, &provider),
				Error::<T>::MessageIdAlreadyExists
			);

			let provider_did = entry.provider_did.clone();
			let entity_id = entry.entity_id.clone();
			// Id Digest = concat (H(<scale_encoded_digest>, (<scale_encoded_entity_id>),
			// (<scale_encoded_message_id>), <scale_encoded_space_identifier>,
			// <scale_encoded_provider_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[
					&digest.encode()[..],
					&entity_id.encode()[..],
					&message_id.encode()[..],
					&space_id.encode()[..],
					&provider_did.encode()[..],
				]
				.concat()[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Rating,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<RatingEntries<T>>::contains_key(&identifier),
				Error::<T>::RatingIdentifierAlreadyAdded
			);

			Self::aggregate_score(&entry, EntryTypeOf::Credit)?;
			let entity = rating_details.entry.entity_id.clone();
			let reference_id_option = rating_details.reference_id;
			let created_at = Self::get_current_time();

			<RatingEntries<T>>::insert(
				&identifier,
				RatingEntryOf::<T> {
					entry,
					digest,
					space: space_id,
					message_id: message_id.clone(),
					creator_id: creator.clone(),
					entry_type: EntryTypeOf::Credit,
					reference_id: reference_id_option.clone(),
					created_at,
				},
			);

			<MessageIdentifiers<T>>::insert(message_id, &provider_did, &identifier);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(Error::<T>::from)?;
			if let Some(reference_id) = reference_id_option {
				Self::update_activity(&reference_id, CallTypeOf::Credit)
					.map_err(Error::<T>::from)?;
			}
			Self::deposit_event(Event::RatingEntryRevised {
				identifier,
				entity,
				provider: provider_did,
				creator,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Updated the Score Aggregates of an entity.
	///
	/// This function updates the aggregate score of an entity based on the new
	/// rating entry. It adjusts the total rating and count of transactions
	/// either by adding (in case of a credit entry) or subtracting (in case of
	/// a debit entry) the values from the new rating entry.
	///
	/// # Arguments
	/// * `entry` - A reference to the rating input entry which contains the rating details.
	/// * `rtype` - The type of the rating entry, either `Credit` or `Debit`, indicating how the
	///   aggregate score should be adjusted.
	///
	/// # Returns
	/// Returns `Ok(())` if the operation is successful, or an appropriate error
	/// if it fails.
	///
	/// # Errors
	/// This function returns an error if there are issues with updating the
	/// aggregate scores in storage.
	///
	/// # Events
	/// Emits an `AggregateScoreUpdated` event upon successful update.
	///
	/// # Example
	/// ```
	/// aggregate_score(&entry, EntryTypeOf::Credit)?;
	/// ```
	pub fn aggregate_score(
		entry: &RatingInputEntryOf<T>,
		rtype: EntryTypeOf,
	) -> Result<(), pallet::Error<T>> {
		if let Some(mut aggregate) = <AggregateScores<T>>::get(&entry.entity_id, &entry.rating_type)
		{
			match rtype {
				EntryTypeOf::Credit => {
					aggregate.count_of_txn =
						aggregate.count_of_txn.saturating_add(entry.count_of_txn);
					aggregate.total_encoded_rating =
						aggregate.total_encoded_rating.saturating_add(entry.total_encoded_rating);
				},
				EntryTypeOf::Debit => {
					aggregate.count_of_txn =
						aggregate.count_of_txn.saturating_sub(entry.count_of_txn);
					aggregate.total_encoded_rating =
						aggregate.total_encoded_rating.saturating_sub(entry.total_encoded_rating);
				},
			};
			<AggregateScores<T>>::insert(
				&entry.entity_id,
				&entry.rating_type,
				AggregatedEntryOf {
					count_of_txn: aggregate.count_of_txn,
					total_encoded_rating: aggregate.total_encoded_rating,
				},
			);
		} else {
			let new_score_entry = AggregatedEntryOf {
				count_of_txn: entry.count_of_txn,
				total_encoded_rating: entry.total_encoded_rating,
			};
			<AggregateScores<T>>::insert(&entry.entity_id, &entry.rating_type, new_score_entry);
		}
		Self::deposit_event(Event::AggregateScoreUpdated { entity: entry.entity_id.clone() });
		Ok(())
	}

	/// Updates the global timeline with a new rating event for an entity.
	///
	/// An `EventEntryOf` struct is created, encapsulating the type of action
	/// (`tx_action`) and the `Timepoint` of the event, which is obtained by
	/// calling the `timepoint` function. This entry is then passed to the
	/// `update_timeline` function of the `identifier` pallet, which integrates
	/// it into the global timeline.
	///
	/// # Parameters
	/// - `tx_id`: The identifier of the schema that the activity pertains to.
	/// - `tx_action`: The type of action taken on the schema, encapsulated within `CallTypeOf`.
	///
	/// # Returns
	/// Returns `Ok(())` after successfully updating the timeline. If any errors
	/// occur within the `update_timeline` function, they are not captured here
	/// and the function will still return `Ok(())`.
	pub fn update_activity(tx_id: &RatingEntryIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ = IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Rating, tx_entry);
		Ok(())
	}

	/// Retrieves the current timepoint.
	///
	/// This function returns a `Timepoint` structure containing the current
	/// block number and extrinsic index. It is typically used in conjunction
	/// with `update_activity` to record when an event occurred.
	///
	/// # Returns
	/// - `Timepoint`: A structure containing the current block number and extrinsic index.
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}

	fn get_current_time() -> T::Moment {
		timestamp::Pallet::<T>::get()
	}
}
