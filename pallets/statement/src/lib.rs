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
//! The Statement pallet is used to anchor identifiers representing off-chain
//! documents. The pallet provides means of creating, updating, revoking and
//! removing identifier data on-chain and delegated controls.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

use cord_primitives::{curi::Ss58Identifier, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{prelude::Clone, str};
pub mod types;
pub mod weights;
pub use crate::types::*;
use frame_system::pallet_prelude::BlockNumberFor;

pub use crate::{pallet::*, types::*, weights::WeightInfo};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::{OptionQuery, *};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Statement Identifier
	pub type StatementIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Hash of the registry.
	pub type StatementHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a creator identifier.
	pub type StatementCreatorIdOf<T> = pallet_registry::RegistryCreatorIdOf<T>;

	/// Hash of the statement.
	pub type StatementDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for the statement entity
	pub type StatementEntryOf<T> = StatementEntry<StatementDigestOf<T>, SchemaIdOf, RegistryIdOf>;
	/// Type for the statement digest entity
	pub type AttestationDetailsOf<T> = AttestationDetails<StatementCreatorIdOf<T>, StatusOf>;
	/// Type for the statement commits
	pub type StatementActivityOf<T> = StatementCommit<
		StatementCommitActionOf,
		StatementDigestOf<T>,
		StatementCreatorIdOf<T>,
		BlockNumberFor<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, StatementCreatorIdOf<Self>>;

		/// The maximum number of activity for a statement.
		#[pallet::constant]
		type MaxStatementActivities: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// statement identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::getter(fn statements)]
	pub type Statements<T> =
		StorageMap<_, Blake2_128Concat, StatementIdOf, StatementEntryOf<T>, OptionQuery>;

	/// statement hashes stored on chain.
	/// It maps from a statement hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn statement_digests)]
	pub type StatementDigests<T> =
		StorageMap<_, Blake2_128Concat, StatementDigestOf<T>, StatementIdOf, OptionQuery>;

	/// statement activities stored on chain.
	/// It maps from an identifier to a vector of activities.
	#[pallet::storage]
	#[pallet::getter(fn activities)]
	pub(super) type Activities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		StatementIdOf,
		BoundedVec<StatementActivityOf<T>, T::MaxStatementActivities>,
		ValueQuery,
	>;

	/// statement uniques stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	#[pallet::getter(fn attestations)]
	pub type Attestations<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		StatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		AttestationDetailsOf<T>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new statement identifier has been created.
		/// \[statement identifier, statement digest, controller\]
		Create {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
		/// A statement identifier has been updated.
		/// \[statement identifier, digest, controller\]
		Update {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
		/// A statement identifier status has been revoked.
		/// \[statement identifier, controller\]
		Revoke { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement identifier status has been restored.
		/// \[statement identifier, controller\]
		Restore { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement identifier has been removed.
		/// \[statement identifier,  controller\]
		Remove { identifier: StatementIdOf, author: StatementCreatorIdOf<T> },
		/// A statement digest has been added.
		/// \[statement identifier, digest, controller\]
		Digest {
			identifier: StatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorIdOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Statement idenfier is not unique
		StatementAlreadyAnchored,
		/// Statement idenfier not found
		StatementNotFound,
		/// Statement idenfier marked inactive
		RevokedStatement,
		/// Statement idenfier not marked inactive
		StatementNotRevoked,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Statement link does not exist
		StatementLinkNotFound,
		/// Statement Link is revoked
		StatementLinkRevoked,
		// Invalid creator signature
		InvalidSignature,
		//Statement hash is not unique
		HashAlreadyAnchored,
		// Expired Tx Signature
		ExpiredSignature,
		// Invalid Statement Identifier
		InvalidStatementIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		// Statement not part of space
		StatementSpaceMismatch,
		//Statement digest is not unique
		DigestHashAlreadyAnchored,
		// Invalid transaction hash
		InvalidTransactionHash,
		// Metadata limit exceeded
		MetadataLimitExceeded,
		// Metadata already set for the entry
		MetadataAlreadySet,
		// Metadata not found for the entry
		MetadataNotFound,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// More than the maximum mumber of delegates.
		TooManyDelegatesToRemove,
		// Authorization not found
		AuthorizationDetailsNotFound,
		// Maximum number of activities exceeded
		MaxStatementActivitiesExceeded,
		// Attestation is not found
		AttestationNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new statement and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `statement_digest`: The digest of the statement.
		/// * `authorization`: AuthorizationIdOf.
		/// * `schema_id`: The schema id of the statement.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			statement_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_id: Ss58Identifier;
			if let Some(schema_id) = schema_id.clone() {
				registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					Some(schema_id),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;
			} else {
				registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					None,
				)
				.map_err(<pallet_registry::Error<T>>::from)?;
			}

			// Id Digest = concat (H(<scale_encoded_statement_digest>,
			// <scale_encoded_registry_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&statement_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]]
					.concat()[..],
			);

			let identifier = Ss58Identifier::to_statement_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<Statements<T>>::contains_key(&identifier),
				Error::<T>::StatementAlreadyAnchored
			);

			<StatementDigests<T>>::insert(statement_digest, &identifier);

			<Statements<T>>::insert(
				&identifier,
				StatementEntryOf::<T> {
					digest: statement_digest,
					schema: schema_id.clone(),
					registry: registry_id,
				},
			);

			<Attestations<T>>::insert(
				&identifier,
				statement_digest,
				AttestationDetailsOf::<T> { creator: creator.clone(), revoked: false },
			);

			Self::update_activity(
				&identifier,
				statement_digest,
				creator.clone(),
				StatementCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Create {
				identifier,
				digest: statement_digest,
				author: creator,
			});

			Ok(())
		}
		/// Updates the statement identifier with a new digest. The updated
		/// digest represents the changes a statement reference document might
		/// have undergone. Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `statement_id`: The identifier of the statement to be updated.
		/// * `statement_digest`: The hash of the statement reference document.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn update(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			statement_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;
			let statement_attestations =
				<Attestations<T>>::get(&statement_id, statement_details.digest)
					.ok_or(Error::<T>::AttestationNotFound)?;
			// If it is same digest then it should throw statement anchored error
			ensure!(
				statement_details.digest != statement_digest,
				Error::<T>::StatementAlreadyAnchored
			);
			ensure!(!statement_attestations.revoked, Error::<T>::RevokedStatement);

			if statement_attestations.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StatementDigests<T>>::insert(statement_digest, &statement_id);

			<Statements<T>>::insert(
				&statement_id,
				StatementEntryOf::<T> { digest: statement_digest, ..statement_details },
			);

			// Update the old info saying that got revoked
			<Attestations<T>>::insert(
				&statement_id,
				statement_details.digest,
				AttestationDetailsOf::<T> { creator: updater.clone(), revoked: true },
			);

			// Update the new entry with hash to say this is active
			<Attestations<T>>::insert(
				&statement_id,
				statement_digest,
				AttestationDetailsOf::<T> { creator: updater.clone(), revoked: false },
			);

			Self::update_activity(
				&statement_id,
				statement_digest,
				updater.clone(),
				StatementCommitActionOf::Update,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update {
				identifier: statement_id,
				digest: statement_digest,
				author: updater,
			});

			Ok(())
		}
		/// Revokes a statement.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement identifier.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn revoke(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;
			let statement_attestations =
				<Attestations<T>>::get(&statement_id, statement_details.digest)
					.ok_or(Error::<T>::AttestationNotFound)?;
			// If it is same digest then it should throw statement anchored error
			ensure!(!statement_attestations.revoked, Error::<T>::RevokedStatement);

			if statement_attestations.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			// Update the info saying it got revoked
			<Attestations<T>>::insert(
				&statement_id,
				statement_details.digest,
				AttestationDetailsOf::<T> { creator: updater.clone(), revoked: true },
			);

			Self::update_activity(
				&statement_id,
				statement_details.digest,
				updater.clone(),
				StatementCommitActionOf::Revoke,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Revoke { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Restore a previously revoked statement.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement identifier.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn restore(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;
			let statement_attestations =
				<Attestations<T>>::get(&statement_id, statement_details.digest)
					.ok_or(Error::<T>::AttestationNotFound)?;
			ensure!(statement_attestations.revoked, Error::<T>::StatementNotRevoked);

			if statement_attestations.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			// Update the existing info saying it got activated again (ie, revoked: false)
			<Attestations<T>>::insert(
				&statement_id,
				statement_details.digest,
				AttestationDetailsOf::<T> { creator: updater.clone(), revoked: false },
			);

			Self::update_activity(
				&statement_id,
				statement_details.digest,
				updater.clone(),
				StatementCommitActionOf::Restore,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restore { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Removes a statement from the registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement id of the statement to be removed.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn remove(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;
			let statement_attestations =
				<Attestations<T>>::get(&statement_id, statement_details.digest)
					.ok_or(Error::<T>::AttestationNotFound)?;

			if statement_attestations.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<Statements<T>>::take(&statement_id);

			// Remove all entries with this identifier in the doublemap. Not planning to use
			// the result
			let _ = <Attestations<T>>::clear_prefix(&statement_id, 0, None);

			Self::update_activity(
				&statement_id,
				statement_details.digest,
				updater.clone(),
				StatementCommitActionOf::Remove,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Remove { identifier: statement_id, author: updater });

			Ok(())
		}

		/// Adds statement digest information.
		/// `digest` is a function that takes a statement identifier, a
		/// statement digest, and an authorization identifier, and inserts the
		/// statement digest into the `StatementDigests` storage map, and then
		/// deposits an event. This operation can only be performed bythe
		/// statement issuer or delegated authorities.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `statement_id`: The statement identifier.
		/// * `statement_digest`: StatementDigestOf<T>
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn digest(
			origin: OriginFor<T>,
			statement_id: StatementIdOf,
			statement_digest: StatementDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let statement_details =
				<Statements<T>>::get(&statement_id).ok_or(Error::<T>::StatementNotFound)?;
			let statement_attestations =
				<Attestations<T>>::get(&statement_id, statement_details.digest)
					.ok_or(Error::<T>::AttestationNotFound)?;
			ensure!(!statement_attestations.revoked, Error::<T>::RevokedStatement);

			if statement_attestations.creator != creator {
				let registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					None,
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					statement_details.registry == registry_id,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StatementDigests<T>>::insert(statement_digest, &statement_id);

			// Treat this as a new entry
			<Attestations<T>>::insert(
				&statement_id,
				statement_details.digest,
				AttestationDetailsOf::<T> { creator: creator.clone(), revoked: false },
			);

			Self::update_activity(
				&statement_id,
				statement_digest,
				creator.clone(),
				StatementCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Digest {
				identifier: statement_id,
				digest: statement_digest,
				author: creator,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// `update_activity` takes a statement id, a digest, a proposer, and a
	/// commit action, and pushes the commit action to the statement's
	/// activities
	///
	/// Arguments:
	///
	/// * `tx_statement`: The statement id of the statement that the commit is
	///   being made to.
	/// * `tx_digest`: The digest of the transaction that was committed.
	/// * `proposer`: The account that is proposing the transaction.
	/// * `commit`: Action taken on a statement.
	///
	/// Returns:
	///
	/// The `Result` type is being returned.
	pub fn update_activity(
		tx_statement: &StatementIdOf,
		tx_digest: StatementDigestOf<T>,
		proposer: StatementCreatorIdOf<T>,
		commit: StatementCommitActionOf,
	) -> Result<(), Error<T>> {
		Activities::<T>::try_mutate(tx_statement, |activities| {
			activities
				.try_push(StatementActivityOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxStatementActivitiesExceeded)?;

			Ok(())
		})
	}

	/// Returns a `Timepoint` struct representing the current point in time.
	///
	/// The `Timepoint` consists of the height (block number) and index
	/// (extrinsic index), providing a snapshot of the current state in the
	/// blockchain.
	///
	/// # Returns
	///
	/// A `Timepoint` struct representing the current point in time, with the
	/// following fields:
	/// - `height`: The height of the blockchain at the current point in time.
	/// - `index`: The index of the extrinsic within the current block.
	pub fn timepoint() -> Timepoint<BlockNumberFor<T>> {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
