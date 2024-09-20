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

//! # Statement Pallet
//!
//! The Statement Pallet is designed for blockchain systems that need to
//! reference off-chain data without storing the actual data or recipient
//! information on-chain. It provides a robust framework for managing statements
//! that serve as references to data stored externally. This approach ensures
//! data privacy and minimizes on-chain storage requirements while still
//! leveraging the blockchain for data integrity and provenance.
//!
//! ## Overview
//!
//! The pallet allows users to create, update, revoke, restore, and remove
//! statements, each identified by a unique identifier and optionally associated
//! with an off-chain schema. It is built to ensure that only authorized
//! entities can interact with the statements. The pallet maintains a log of all
//! statement activities for audit purposes, but it does not store the actual
//! data or the identities of the data recipients on-chain, thus providing a
//! layer of privacy and reducing the blockchain's storage footprint.
//!
//! ## Features
//!
//! - **Reference Off-Chain Data**: Statements act as on-chain pointers to data stored off-chain,
//!   allowing the blockchain to maintain a lightweight footprint.
//! - **Privacy Preservation**: By not storing recipient information on-chain, the pallet respects
//!   privacy concerns and complies with data protection regulations.
//! - **Activity Logging**: All statement activities are timestamped and recorded on-chain,
//!   providing an immutable history of actions without revealing the actual data.
//!
//! ## Interface
//!
//! The pallet provides dispatchable functions to interact with statements:
//!
//! - `create`: References a new piece of off-chain data.
//! - `create_batch`: References multiple pieces of off-chain data in a batch operation.
//! - `update`: Updates the reference to a piece of off-chain data.
//! - `revoke`: Marks a statement's reference as inactive.
//! - `restore`: Reactivates a revoked statement's reference.
//! - `remove`: Removes a statement's reference from the blockchain.
//!
//!## Related Modules
//!
//! - [`ChainSpace`](../pallet_chain_space/index.html): Manages authorization and capacity for
//!   statement references.
//! - [`Identifier`](../identifier/index.html): Logs the timeline of statement activities.
//!
//! ## Data Privacy
//!
//! This pallet is designed with data privacy in mind. It does not store
//! sensitive or personally identifiable information on-chain, adhering to
//! privacy laws and regulations. Users and developers should ensure that
//! off-chain data storage and retrieval mechanisms also comply with such
//! regulations.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

use cord_primitives::StatusOf;
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::{prelude::Clone, str};
pub mod types;
pub mod weights;
pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_system::pallet_prelude::BlockNumberFor;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::SaturatedConversion;
use sp_std::{vec, vec::Vec};

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::{OptionQuery, *};
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Space Identifier
	pub type SpaceIdOf = Ss58Identifier;
	/// Statement Identifier
	pub type StatementIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Type of a creator identifier.
	pub type StatementCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
	/// Hash of the statement.
	pub type StatementDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for the statement details
	pub type StatementDetailsOf<T> = StatementDetails<StatementDigestOf<T>, SchemaIdOf, SpaceIdOf>;
	/// Type for the statement entry details
	pub type StatementEntryStatusOf<T> = StatementEntryStatus<StatementCreatorOf<T>, StatusOf>;
	/// Type for the statement entry details
	pub type StatementPresentationDetailsOf<T> = StatementPresentationDetails<
		StatementCreatorOf<T>,
		PresentationTypeOf,
		StatementDigestOf<T>,
		SpaceIdOf,
	>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, StatementCreatorOf<Self>>;
		/// Maximum entires supported per batch call
		#[pallet::constant]
		type MaxDigestsPerBatch: Get<u16>;
		/// Maximum removals per call
		#[pallet::constant]
		type MaxRemoveEntries: Get<u16>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// statement identifiers stored on chain.
	/// It maps from an identifier to its details.
	/// Only stores the latest state.
	#[pallet::storage]
	pub type Statements<T> =
		StorageMap<_, Blake2_128Concat, StatementIdOf, StatementDetailsOf<T>, OptionQuery>;

	/// statement uniques stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type Entries<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementCreatorOf<T>,
		OptionQuery,
	>;

	/// statement uniques stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type Presentations<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementPresentationDetailsOf<T>,
		OptionQuery,
	>;

	/// Revocation registry of statement entries stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type RevocationList<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementEntryStatusOf<T>,
		OptionQuery,
	>;

	/// Storage for Identifier lookup.
	/// It maps from a statement entry digest and registry id to an identifier.
	#[pallet::storage]
	pub type IdentifierLookup<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		StatementDigestOf<T>,
		Twox64Concat,
		SpaceIdOf,
		StatementIdOf,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new statement identifier has been registered.
		/// \[statement identifier, statement digest, controller\]
		Register {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A statement identifier has been updated.
		/// \[statement identifier, digest, controller\]
		Update {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A statement identifier status has been revoked.
		/// \[statement identifier, controller\]
		Revoke { identifier: StatementIdOf, author: StatementCreatorOf<T> },
		/// A statement identifier status has been restored.
		/// \[statement identifier, controller\]
		Restore { identifier: StatementIdOf, author: StatementCreatorOf<T> },
		/// A statement identifier has been removed.
		/// \[statement identifier,  controller\]
		Remove { identifier: StatementIdOf, author: StatementCreatorOf<T> },
		/// A statement identifier has been removed.
		/// \[statement identifier,  controller\]
		PartialRemoval { identifier: StatementIdOf, removed: u32, author: StatementCreatorOf<T> },
		/// A statement digest has been added.
		/// \[statement identifier, digest, controller\]
		PresentationAdded {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A statement digest has been added.
		/// \[statement identifier, digest, controller\]
		PresentationRemoved {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A statement batch has been processed.
		/// \[successful count, failed count, failed indices,
		/// controller]
		RegisterBatch {
			successful: u32,
			failed: u32,
			indices: Vec<u16>,
			author: StatementCreatorOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Statement idenfier is not unique
		StatementAlreadyAnchored,
		/// Statement idenfier not found
		StatementNotFound,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Statement entry not found
		StatementEntryNotFound,
		/// Statement entry marked inactive
		StatementRevoked,
		/// Statement idenfier not marked inactive
		StatementNotRevoked,
		/// Statement link does not exist
		StatementLinkNotFound,
		/// Statement Link is revoked
		StatementLinkRevoked,
		/// Invalid creator signature
		InvalidSignature,
		/// Statement hash is not unique
		HashAlreadyAnchored,
		/// Expired Tx Signature
		ExpiredSignature,
		/// Invalid Statement Identifier
		InvalidStatementIdentifier,
		/// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		/// Statement not part of space
		StatementSpaceMismatch,
		/// Statement digest is not unique
		DigestHashAlreadyAnchored,
		/// Invalid transaction hash
		InvalidTransactionHash,
		/// Metadata limit exceeded
		MetadataLimitExceeded,
		/// Metadata already set for the entry
		MetadataAlreadySet,
		/// Metadata not found for the entry
		MetadataNotFound,
		/// Maximum Number of delegates reached.
		TooManyDelegates,
		/// More than the maximum mumber of delegates.
		TooManyDelegatesToRemove,
		/// Authorization not found
		AuthorizationDetailsNotFound,
		/// Maximum number of activities exceeded
		MaxStatementActivitiesExceeded,
		/// Attestation is not found
		AttestationNotFound,
		//Max Digests in a call
		MaxDigestLimitExceeded,
		/// Bulk Transaction Failed
		BulkTransactionFailed,
		/// Associate digest already present
		AssociateDigestAlreadyAnchored,
		/// Presentation is already anchored.
		PresentationDigestAlreadyAnchored,
		/// Presentation not found
		PresentationNotFound,
		/// Statement digest already present on the chain.
		StatementDigestAlreadyAnchored,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a new statement within a specified space subject to
		/// authorization and capacity constraints.
		///
		/// The function first ensures that the call's origin is authorized and
		/// retrieves the subject, referred to as the creator. It then verifies
		/// that the creator is a delegate for the space associated with the
		/// given authorization. Following this, it checks that the space has
		/// not exceeded its allowed number of statements.
		///
		/// A unique identifier for the statement is generated by hashing the
		/// encoded statement digest, space identifier, and creator identifier.
		/// The function ensures that this identifier has not been used to
		/// anchor another statement.
		///
		/// Once the identifier is confirmed to be unique, the statement details
		/// are inserted into the `Statements` storage. Additionally, the
		/// statement entry and identifier lookup are recorded in their
		/// respective storages. The space's usage count is incremented to
		/// reflect the addition of the new statement.
		///
		/// The function also logs the creation event by updating the activity
		/// log and emits an event to signal the successful creation of the
		/// statement.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   creator.
		/// - `digest`: The digest of the statement, serving as a unique identifier.
		/// - `authorization`: The authorization ID, verifying the creator's delegation status.
		/// - `schema_id`: An optional schema identifier to be associated with the statement.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the
		/// statement creation. On success, it returns `Ok(())`. On failure, it
		/// provides an error detailing the cause.
		///
		/// # Errors
		/// The function can fail for several reasons including unauthorized
		/// origin, the creator not being a delegate, space capacity being
		/// exceeded, invalid statement identifier, or the statement already
		/// being anchored. Errors related to incrementing space usage or
		/// updating the activity log may also occur.
		///
		/// # Events
		/// - `Create`: Emitted when a statement is successfully created, containing the
		///   `identifier`, `digest`, and `author` (creator).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			// Id Digest = concat (H(<scale_encoded_statement_digest>,
			// <scale_encoded_space_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&digest.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Statement,
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Statements<T>>::contains_key(&identifier),
				Error::<T>::StatementAlreadyAnchored
			);

			<Statements<T>>::insert(
				&identifier,
				StatementDetailsOf::<T> {
					digest,
					schema: schema_id.clone(),
					space: space_id.clone(),
				},
			);

			<Entries<T>>::insert(&identifier, digest, creator.clone());
			<IdentifierLookup<T>>::insert(digest, &space_id, &identifier);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Register { identifier, digest, author: creator });

			Ok(())
		}

		/// Updates the digest of an existing statement after performing a
		/// series of validations. Initially, the function confirms that the
		/// call's origin is authorized and identifies the updater. It then
		/// retrieves the statement details associated with the provided
		/// `statement_id`. Before proceeding, the function checks whether the
		/// statement has already been revoked; if so, it halts further
		/// execution. Additionally, it ensures that the new digest provided for
		/// the update is different from the current one to avoid unnecessary
		/// operations.
		///
		/// Upon passing these checks, the updater's delegation status for the
		/// space linked to the statement is verified. The existing statement is
		/// then marked as revoked, and the new digest is recorded. This
		/// involves updating the `Entries` storage with the new digest and the
		/// updater's information, as well as adjusting the `IdentifierLookup`
		/// to reflect the change. The `Statements` storage is also updated with
		/// the new details of the statement.
		///
		/// Subsequently, the space usage count is incremented to account for
		/// the updated statement. An activity log entry is created to record
		/// the update event. To conclude the process, an `Update` event is
		/// emitted, which includes the statement identifier, the new digest,
		/// and the authoring updater's details.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   updater.
		/// - `statement_id`: The identifier of the statement to be updated.
		/// - `new_statement_digest`: The new digest to replace the existing one for the statement.
		/// - `authorization`: The authorization ID, verifying the updater's delegation status.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the update
		/// operation. On success, it returns `Ok(())`. On failure, it provides
		/// an error detailing the cause.
		///
		/// # Errors
		/// The function can fail due to several reasons including an
		/// unauthorized origin, the statement not found, the statement being
		/// revoked, the new digest being the same as the existing one, or the
		/// updater not being authorized for the operation.
		///
		/// # Events
		/// - `Update`: Emitted when a statement is successfully updated, containing the
		///   `identifier`, `digest`, and `author`
		/// (updater).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			new_statement_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			ensure!(statement_details.space == space_id, Error::<T>::UnauthorizedOperation);

			ensure!(
				!<RevocationList<T>>::contains_key(&statement_id, statement_details.digest),
				Error::<T>::StatementRevoked
			);

			ensure!(
				!<Entries<T>>::contains_key(&statement_id, new_statement_digest),
				Error::<T>::StatementDigestAlreadyAnchored
			);

			<RevocationList<T>>::insert(
				&statement_id,
				statement_details.digest,
				StatementEntryStatusOf::<T> { creator: updater.clone(), revoked: true },
			);

			<Entries<T>>::insert(&statement_id, new_statement_digest, updater.clone());

			<IdentifierLookup<T>>::insert(
				new_statement_digest,
				statement_details.space.clone(),
				&statement_id,
			);

			<Statements<T>>::insert(
				&statement_id,
				StatementDetailsOf::<T> { digest: new_statement_digest, ..statement_details },
			);

			Self::update_activity(&statement_id, CallTypeOf::Update).map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update {
				identifier: statement_id,
				digest: new_statement_digest,
				author: updater,
			});

			Ok(())
		}

		/// Revokes an existing statement, rendering it invalid for future
		/// operations. The revocation process involves several authorization
		/// and state checks to ensure the integrity of the operation.
		///
		/// Initially, the function authenticates the origin of the call to
		/// identify the updater, who is attempting the revocation. It then
		/// retrieves the details of the statement using the provided
		/// `statement_id`. If the statement is not found, the function fails
		/// with an error.
		///
		/// Before proceeding, the function checks whether the statement has
		/// already been revoked. If it has, the function terminates early to
		/// prevent redundant revocation attempts. If the statement is active,
		/// the function then determines whether the updater is the original
		/// creator of the statement or a delegate with proper authorization. If
		/// the updater is not the creator, they must be a delegate with
		/// authorization for the space associated with the statement, and the
		/// function checks for this condition.
		///
		/// Once the updater's authority to revoke the statement is confirmed,
		/// the function marks the statement as revoked in the `RevocationList`.
		/// It updates the activity log to record the revocation event. Finally,
		/// it emits a `Revoked` event, indicating the successful revocation of
		/// the statement with the statement identifier and the
		/// updater's information.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   updater.
		/// - `statement_id`: The identifier of the statement to be revoked.
		/// - `authorization`: The authorization ID, verifying the updater's delegation status if
		///   they are not the creator.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the
		/// revocation. On success, it returns `Ok(())`. On failure, it provides
		/// an error detailing the cause, such as the statement not being found
		/// or already being revoked, or the updater not having the authority to
		/// revoke the statement.
		///
		/// # Errors
		/// The function can fail due to several reasons including the statement
		/// not being found, already being revoked, or the updater lacking the
		/// authority to perform the revocation.
		///
		/// # Events
		/// - `Revoked`: Emitted when a statement is successfully revoked, containing the
		///   `identifier` of the statement and
		/// the `author` who is the updater.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke())]
		pub fn revoke(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			ensure!(
				!<RevocationList<T>>::contains_key(&statement_id, statement_details.digest),
				Error::<T>::StatementRevoked
			);

			ensure!(statement_details.space == space_id, Error::<T>::UnauthorizedOperation);

			<RevocationList<T>>::insert(
				&statement_id,
				statement_details.digest,
				StatementEntryStatusOf::<T> { creator: updater.clone(), revoked: true },
			);

			Self::update_activity(&statement_id, CallTypeOf::Revoke).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Revoke { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Restores a previously revoked statement, re-enabling its validity
		/// within the system. The restoration is contingent upon a set of
		/// checks to ensure that the action is permitted and appropriate.
		///
		/// The function commences by authenticating the origin of the call to
		/// ascertain the identity of the updater attempting the restoration. It
		/// then fetches the details of the statement using the `statement_id`
		/// provided. If the statement does not exist, the function aborts and
		/// signals an error.
		///
		/// A crucial step in the process is to verify that the statement has
		/// indeed been revoked; if not, the function ceases further execution.
		/// Assuming the statement is revoked, the function then ascertains
		/// whether the updater is either the original creator of the statement
		/// or a delegate with the requisite authorization. If the updater
		/// is not the creator, their delegation status for the space linked to
		/// the statement is verified.
		///
		/// Upon confirming the updater's authority to restore the statement,
		/// the function removes the statement from the `RevocationList`,
		/// effectively reactivating it. It then logs the restoration event in
		/// the activity log. To finalize the process, a `Restored` event is
		/// broadcast, indicating the successful restoration of the statement
		/// with its identifier and the updater's details.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   updater.
		/// - `statement_id`: The identifier of the statement to be restored.
		/// - `authorization`: The authorization ID, verifying the updater's delegation status if
		///   they are not the creator.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the
		/// restoration. On success, it returns `Ok(())`. On failure, it
		/// provides an error detailing the cause, such as the statement not
		/// being found, not being revoked, or the updater not having the
		/// authority to restore the statement.
		///
		/// # Errors
		/// The function can fail for several reasons including the statement
		/// not being found, not being revoked, or the updater lacking the
		/// authority to perform the restoration.
		///
		/// # Events
		/// - `Restored`: Emitted when a statement is successfully restored, containing the
		///   `identifier` of the statement
		/// and the `author` who is the updater.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::restore())]
		pub fn restore(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			ensure!(
				<RevocationList<T>>::contains_key(&statement_id, statement_details.digest),
				Error::<T>::StatementNotRevoked
			);

			ensure!(statement_details.space == space_id, Error::<T>::UnauthorizedOperation);

			<RevocationList<T>>::remove(&statement_id, statement_details.digest);

			Self::update_activity(&statement_id, CallTypeOf::Restore).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restore { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Removes a statement and its associated entries from the system. The
		/// removal can be either complete or partial, depending on the number
		/// of entries associated with the statement and a predefined maximum
		/// removal limit.
		///
		/// The function begins by authenticating the origin of the call to
		/// identify the updater. It then retrieves the statement details using
		/// the provided `statement_id`. If the statement cannot be found, the
		/// function fails with an error. An early authorization check is
		/// performed to ensure that the updater has the proper delegation
		/// status for the space associated with the statement.
		///
		/// The function counts the number of entries linked to the statement
		/// and compares this to the maximum number of entries that can be
		/// removed in a single operation, as specified by `MaxRemoveEntries`.
		/// If the count is less than or equal to the maximum, a complete
		/// removal is initiated; otherwise, a partial removal is performed.
		///
		/// In a complete removal, all entries and their lookups are removed,
		/// the statement is deleted, and the space usage is decremented
		/// accordingly. In a partial removal, only up to the maximum number of
		/// entries are removed, and the space usage is decremented by the
		/// number of entries actually removed.
		///
		/// After the removal process, the function updates the activity log to
		/// record the event. It then emits either a `Removed` event for a
		/// complete removal or a `PartialRemoval` event for a partial removal,
		/// providing details of the operation including the statement
		/// identifier and the updater's information.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   updater.
		/// - `statement_id`: The identifier of the statement to be removed.
		/// - `authorization`: The authorization ID, verifying the updater's delegation status.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the removal.
		/// On success, it returns `Ok(())`. On failure, it provides an error
		/// detailing the cause, such as the statement not being found or the
		/// updater not having the authority to perform the removal.
		///
		/// # Errors
		/// The function can fail for several reasons including the statement
		/// not being found or the updater lacking the authority to perform the
		/// removal.
		///
		/// # Events
		/// - `Removed`: Emitted when a statement and all its entries are completely removed.
		/// - `PartialRemoval`: Emitted when only a portion of the entries are removed, detailing
		///   the number of entries
		/// removed.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove(T::MaxRemoveEntries::get() as u32))]
		pub fn remove(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResultWithPostInfo {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&updater,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			ensure!(statement_details.space == space_id, Error::<T>::UnauthorizedOperation);

			// Count the entries in `Entries`.
			let entries_count = <Entries<T>>::iter_prefix(&statement_id).count();
			let max_removals = T::MaxRemoveEntries::get() as usize;

			// Determine if a complete or partial removal is needed.
			let is_complete_removal = entries_count <= max_removals;

			// Start the removal process.
			let mut removed_count = 0;
			if is_complete_removal {
				// Perform a complete removal.
				for (digest, _) in <Entries<T>>::iter_prefix(&statement_id) {
					<IdentifierLookup<T>>::remove(digest, &space_id);
				}
				let _ =
					<RevocationList<T>>::clear_prefix(&statement_id, entries_count as u32, None);
				let _ = <Entries<T>>::clear_prefix(&statement_id, entries_count as u32, None);
				<Statements<T>>::remove(&statement_id);
				pallet_chain_space::Pallet::<T>::decrement_usage_entries(
					&space_id,
					entries_count as u16,
				)
				.map_err(<pallet_chain_space::Error<T>>::from)?;
			} else {
				// Perform a partial removal.
				for (digest, _) in <Entries<T>>::iter_prefix(&statement_id).take(max_removals) {
					<IdentifierLookup<T>>::remove(digest, &space_id);
					<RevocationList<T>>::remove(&statement_id, digest);
					<Entries<T>>::remove(&statement_id, digest);
					removed_count += 1;
				}
				pallet_chain_space::Pallet::<T>::decrement_usage_entries(
					&space_id,
					removed_count as u16,
				)
				.map_err(<pallet_chain_space::Error<T>>::from)?;
			}

			// Update activity and emit the appropriate event.
			Self::update_activity(
				&statement_id,
				if is_complete_removal { CallTypeOf::Remove } else { CallTypeOf::PartialRemove },
			)
			.map_err(<Error<T>>::from)?;

			let event = if is_complete_removal {
				Event::Remove { identifier: statement_id, author: updater }
			} else {
				Event::PartialRemoval {
					identifier: statement_id,
					removed: removed_count as u32,
					author: updater,
				}
			};

			Self::deposit_event(event);

			Ok(Some(<T as Config>::WeightInfo::remove(entries_count as u32)).into())
		}

		/// Creates multiple statements in a batch operation. This function
		/// takes a vector of statement digests and attempts to create a new
		/// statement for each digest. It performs checks on the batch size,
		/// ensures the creator has the proper authorization, and verifies that
		/// the space has enough capacity to accommodate the batch of new
		/// statements.
		///
		/// The function iterates over the provided digests, generating a unique
		/// identifier for each and attempting to create a new statement. If a
		/// statement with the generated identifier already exists, or if there
		/// is an error in generating the identifier, the digest is marked as
		/// failed. Otherwise, the new statement is recorded along
		/// with its details. The function also updates the activity log for
		/// each successful creation.
		///
		/// After processing all digests, the function ensures that at least one
		/// statement was successfully created. It then increments the usage
		/// count of the space by the number of successful creations. Finally, a
		/// `BatchCreate` event is emitted, summarizing the results of the batch
		/// operation, including the number of successful and failed creations,
		/// the indices of the failed digests, and the author of the batch
		/// creation.
		///
		/// # Parameters
		/// - `origin`: The origin of the dispatch call, which should be a signed message from the
		///   creator.
		/// - `digests`: A vector of statement digests to be processed in the batch operation.
		/// - `authorization`: The authorization ID, verifying the creator's delegation status.
		/// - `schema_id`: An optional schema identifier that may be associated with the statements.
		///
		/// # Returns
		/// A `DispatchResult` indicating the success or failure of the batch
		/// creation. On success, it returns `Ok(())`. On failure, it provides
		/// an error detailing the cause, such as exceeding the maximum number
		/// of digests, the space capacity being exceeded, or all digests
		/// failing to create statements.
		///
		/// # Errors
		/// The function can fail for several reasons, including exceeding the
		/// maximum number of digests allowed in a batch, the space capacity
		/// being exceeded, or if no statements could be successfully created.
		///
		/// # Events
		/// - `BatchCreate`: Emitted upon the completion of the batch operation, providing details
		///   of the outcome.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::register_batch(digests.len().saturated_into()))]
		#[rustfmt::skip]
		pub fn register_batch(
			origin: OriginFor<T>,
			digests: Vec<StatementDigestOf<T>>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			ensure!(
				digests.len() <= T::MaxDigestsPerBatch::get() as usize,
				Error::<T>::MaxDigestLimitExceeded
			);

			let mut success = 0u32;
			let mut fail = 0u32;
			let mut indices: Vec<u16> = Vec::new();

			for (index, digest) in digests.iter().enumerate() {
				let id_digest = <T as frame_system::Config>::Hashing::hash(
					&[
						&digest.encode()[..],
						&space_id.clone().encode()[..],
						&creator.clone().encode()[..],
					]
					.concat()[..],
				);

				let identifier_result = Ss58Identifier::create_identifier(
					&(id_digest).encode()[..],
					IdentifierType::Statement,
				);

				match identifier_result {
					Ok(identifier) => {
						if <Statements<T>>::contains_key(&identifier) {
							fail += 1;
							indices.push(index as u16);
						} else {
							<Statements<T>>::insert(
								&identifier,
								StatementDetailsOf::<T> {
									digest: *digest,
									schema: schema_id.clone(),
									space: space_id.clone(),
								},
							);

							<Entries<T>>::insert(&identifier, digest, creator.clone());
							<IdentifierLookup<T>>::insert(digest, &space_id, &identifier);

							if Self::update_activity(&identifier, CallTypeOf::Genesis).is_err() {
								fail += 1;
								indices.push(index as u16);
							} else {
								success += 1;
							}
						}
					},
					Err(_) => {
						fail += 1;
						indices.push(index as u16);
					},
				}
			}

			ensure!(success > 0, Error::<T>::BulkTransactionFailed);
			if digests.len() > 1 {
				let increment = (digests.len() - 1) as u16;

				pallet_chain_space::Pallet::<T>::increment_usage_entries(&space_id, increment)
					.map_err(<pallet_chain_space::Error<T>>::from)?;
			}

			Self::deposit_event(Event::RegisterBatch {
				successful: success,
				failed: fail,
				indices,
				author: creator,
			});

			Ok(())
		}

		/// Adds a presentation to a specified statement.
		///
		/// This privileged function is reserved for execution by the council or
		/// root origin only. It allows the removal of a presentation associated
		/// with a given  `statement_id`. The function performs authorization
		/// checks based on the provided `authorization` parameter, ensuring
		/// that the operation is performed within the correct chain space.
		///
		/// # Parameters
		/// - `origin`: The transaction's origin, restricted to council or root.
		/// - `statement_id`: The identifier of the statement to which the presentation will be
		///   added.
		/// - `presentation_digest`: The digest that uniquely identifies the new presentation.
		/// - `presentation_type`: The type categorization of the presentation.
		/// - `authorization`: The authorization identifier for the creator, required to perform the
		///   addition.
		///
		/// # Errors
		/// - Returns `StatementNotFound` if the `statement_id` does not correspond to any existing
		///   statement.
		/// - Returns `StatementRevoked` if the statement associated with the `statement_id` has
		///   been revoked.
		/// - Returns `UnauthorizedOperation` if the operation is not authorized within the
		///   associated space.
		/// - Returns `PresentationDigestAlreadyAnchored` if the `presentation_digest` is not
		///   unique.
		///
		/// # Events
		/// - Emits `PresentationAdded` upon the successful addition of the presentation.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add_presentation( ))]
		pub fn add_presentation(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			presentation_digest: StatementDigestOf<T>,
			presentation_type: PresentationTypeOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;

			ensure!(
				!<RevocationList<T>>::contains_key(&statement_id, statement_details.digest),
				Error::<T>::StatementRevoked
			);

			ensure!(statement_details.space == space_id, Error::<T>::UnauthorizedOperation);

			// Check for presentation digest uniqueness to fail early if the digest is
			// already present.
			ensure!(
				!<Presentations<T>>::contains_key(&statement_id, presentation_digest),
				Error::<T>::PresentationDigestAlreadyAnchored
			);

			<IdentifierLookup<T>>::insert(presentation_digest, &space_id, &statement_id);

			<Presentations<T>>::insert(
				&statement_id,
				presentation_digest,
				StatementPresentationDetailsOf::<T> {
					creator: creator.clone(),
					presentation_type,
					digest: statement_details.digest,
					space: space_id,
				},
			);

			Self::update_activity(&statement_id, CallTypeOf::PresentationAdded)
				.map_err(<Error<T>>::from)?;

			// Emit an event to signal the successful addition of a new view.
			Self::deposit_event(Event::PresentationAdded {
				identifier: statement_id,
				digest: presentation_digest,
				author: creator,
			});

			Ok(())
		}

		/// Removes a presentation from a specified statement state.
		///
		/// This privileged function is reserved for execution by the council or
		/// root origin only. It allows the removal of a presentation associated
		/// with the `statement_id` and identified by `presentation_digest`. The
		/// function validates the `authorization` of the caller within the
		/// specified chain space before proceeding with the removal.
		///
		/// # Parameters
		/// - `origin`: The transaction's origin, restricted to council or root.
		/// - `statement_id`: The identifier of the statement associated with the presentation.
		/// - `presentation_digest`: The digest that uniquely identifies the presentation to be
		///   removed.
		/// - `authorization`: The authorization identifier that the remover must have to perform
		///   the removal.
		///
		/// # Errors
		/// - Returns `PresentationNotFound` if the specified presentation does not exist.
		/// - Returns `UnauthorizedOperation` if the origin is not authorized to perform this
		///   action.
		///
		/// # Events
		/// - Emits `PresentationRemoved` upon the successful removal of the presentation.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove_presentation( ))]
		pub fn remove_presentation(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			presentation_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let remover = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&remover,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			let presentation_details = Presentations::<T>::get(&statement_id, presentation_digest)
				.ok_or(Error::<T>::PresentationNotFound)?;

			ensure!(presentation_details.space == space_id, Error::<T>::UnauthorizedOperation);

			Presentations::<T>::remove(&statement_id, presentation_digest);
			IdentifierLookup::<T>::remove(presentation_digest, &space_id);

			pallet_chain_space::Pallet::<T>::decrement_usage(&space_id)
				.map_err(<pallet_chain_space::Error<T>>::from)?;

			Self::update_activity(&statement_id, CallTypeOf::PresentationRemoved)?;

			Self::deposit_event(Event::PresentationRemoved {
				identifier: statement_id,
				digest: presentation_digest,
				author: remover,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Updates the global timeline with a new activity event for a statement.
	/// This function is called whenever a significant action is performed on a
	/// statement, ensuring that all such activities are logged with a timestamp
	/// for future reference and accountability.
	///
	/// An `EventEntryOf` struct is created, encapsulating the type of action
	/// (`tx_action`) and the `Timepoint` of the event, which is obtained by
	/// calling the `timepoint` function. This entry is then passed to the
	/// `update_timeline` function of the `identifier` pallet, which integrates
	/// it into the global timeline.
	///
	/// # Parameters
	/// - `tx_id`: The identifier of the statement that the activity pertains to.
	/// - `tx_action`: The type of action taken on the statement, encapsulated within `CallTypeOf`.
	///
	/// # Returns
	/// Returns `Ok(())` after successfully updating the timeline. If any errors
	/// occur within the `update_timeline` function, they are not captured here
	/// and the function will still return `Ok(())`.
	///
	/// # Usage
	/// This function is not intended to be called directly by external entities
	/// but is invoked internally within the pallet's logic whenever a
	/// statement's status is altered.
	pub fn update_activity(tx_id: &StatementIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ =
			IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Statement, tx_entry);
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
}
