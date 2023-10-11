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

//! # Unique Pallet
//!
//!The digest anchoring functionality is implemented within the
//! unique pallet, which does not support revoke/restore.

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
pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::SaturatedConversion;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;

	/// Unique Identifier
	pub type UniqueIdOf = Ss58Identifier;

	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;

	/// Hash of the registry.
	pub type UniqueHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a creator identifier.
	pub type UniqueCreatorIdOf<T> = pallet_registry::RegistryCreatorIdOf<T>;

	/// Hash of the unique.
	pub type UniqueDigestOf<T> = <T as frame_system::Config>::Hash;

	/// Type for an input schema
	pub type InputUniqueOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedLength>;

	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Type for the unique entity
	pub type UniqueEntryOf<T> =
		UniqueEntry<InputUniqueOf<T>, UniqueCreatorIdOf<T>, Option<RegistryIdOf>, StatusOf>;
	/// Type for the unique activities
	pub type UniqueActivitiesOf<T> = UniqueActivity<
		UniqueCommitActionOf,
		InputUniqueOf<T>,
		UniqueCreatorIdOf<T>,
		BlockNumberFor<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, UniqueCreatorIdOf<Self>>;

		/// The maximum number of activities for a unique.
		#[pallet::constant]
		type MaxUniqueActivities: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// The maximum Hash Encoded length for a given unique.
		type MaxEncodedLength: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// unique Transaction stored on chain.
	/// It maps from a unique transaction to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn unique_identifiers)]
	pub type UniqueDigestEntries<T> =
		StorageMap<_, Blake2_128Concat, InputUniqueOf<T>, UniqueIdOf, OptionQuery>;

	/// unique hashes stored on chain.
	/// It maps from a unique hash to an metadata (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn unique_digest_entries)]
	pub type UniqueIdentifiers<T> =
		StorageMap<_, Blake2_128Concat, UniqueIdOf, UniqueEntryOf<T>, OptionQuery>;

	/// unique activities stored on chain.
	/// It maps from an identifier to a vector of activities.
	#[pallet::storage]
	#[pallet::getter(fn activities)]
	pub(super) type Activities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		UniqueIdOf,
		BoundedVec<UniqueActivitiesOf<T>, T::MaxUniqueActivities>,
		ValueQuery,
	>;

	/// unique digest incoming hash as key and identifier as value

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new unique identifier has been created.
		/// \[unique identifier, unique digest, controller\]
		Create { identifier: UniqueIdOf, digest: InputUniqueOf<T>, author: UniqueCreatorIdOf<T> },
		/// A unique identifier status has been revoked.
		/// \[unique identifier, controller\]
		Revoke { identifier: UniqueIdOf, author: UniqueCreatorIdOf<T> },
		/// A unique identifier status has been updated.
		/// \[unique identifier, unique digest, controller\]
		Update { identifier: UniqueIdOf, digest: InputUniqueOf<T>, author: UniqueCreatorIdOf<T> },
		/// A unique identifier has been removed.
		/// \[unique identifier,  controller\]
		Remove { identifier: UniqueIdOf, author: UniqueCreatorIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		// Unique idenfier is not unique
		UniqueAlreadyAnchored,
		/// Unique idenfier not found
		UniqueNotFound,
		/// Unique idenfier marked inactive
		RevokedUnique,
		/// Unique idenfier not marked inactive
		UniqueNotRevoked,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Unique link does not exist
		UniqueLinkNotFound,
		/// Unique Link is revoked
		UniqueLinkRevoked,
		// Invalid creator signature
		InvalidSignature,
		//Unique hash is not unique
		HashAlreadyAnchored,
		// Expired Tx Signature
		ExpiredSignature,
		// Invalid Unique Identifier
		InvalidUniqueIdentifier,
		// Unique not part of space
		UniqueSpaceMismatch,
		//Unique digest is not unique
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
		MaxUniqueActivitiesExceeded,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		//Registy Id mismatch
		RegistryIdMismatch,
		//Max length of unique exceded
		MaxEncodedLimitExceeded,
		//Empty Transaction
		EmptyTransaction,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		//TODO : Check for optional stream Id from stream pallet
		//TODO : Identifirer check  prefix check
		//TODO : IF Stream Identifier is given we will not create another identifieer
		/// Create a new unique and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `unique_digest`: The digest of the unique.
		/// * `registry_id`: The registry id of the unique.
		/// * `authorization`: AuthorizationIdOf
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create(unique_txn.len().saturated_into()))]
		pub fn create(
			origin: OriginFor<T>,
			unique_txn: InputUniqueOf<T>,
			authorization: Option<AuthorizationIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(!unique_txn.is_empty(), Error::<T>::EmptyTransaction);

			ensure!(
				unique_txn.len() <= T::MaxEncodedLength::get() as usize,
				Error::<T>::MaxEncodedLimitExceeded
			);

			let mut u_reqistryid: Option<RegistryIdOf> = None;

			if let Some(authorization) = authorization.clone() {
				let registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					None,
				)
				.map_err(<pallet_registry::Error<T>>::from)?;
				u_reqistryid = Some(registry_id);
			}

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&unique_txn.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::to_unique_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			// Check if the unique_txn or identifier already exist
			// and return an error if they do
			ensure!(
				!<UniqueDigestEntries<T>>::contains_key(&unique_txn) ||
					!<UniqueIdentifiers<T>>::contains_key(&identifier),
				Error::<T>::UniqueAlreadyAnchored
			);

			<UniqueDigestEntries<T>>::insert(&unique_txn, &identifier);

			// Update the UniqueDigestEntries storage
			<UniqueIdentifiers<T>>::insert(
				&identifier,
				UniqueEntryOf::<T> {
					digest: unique_txn.clone(),
					creator: creator.clone(),
					registry: Some(u_reqistryid),
					revoked: false,
				},
			);

			// Perform additional operations and handle committing the changes
			Self::update_commit(
				&identifier,
				unique_txn.clone(),
				creator.clone(),
				UniqueCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			// Emit a Create event
			Self::deposit_event(Event::Create { identifier, digest: unique_txn, author: creator });

			Ok(())
		}

		/// Updates the unique identifier with a new digest. The updated digest
		/// represents the changes a unique reference document might have
		/// undergone. Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `unique_id`: The identifier of the unique to be updated.
		/// * `unique_digest`: The hash of the unique reference document.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update(unique_txn.len().saturated_into()))]
		pub fn update(
			origin: OriginFor<T>,
			unique_id: UniqueIdOf,
			unique_txn: InputUniqueOf<T>,
			authorization: Option<AuthorizationIdOf>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let unique_details =
				<UniqueIdentifiers<T>>::get(&unique_id).ok_or(Error::<T>::UniqueNotFound)?;

			// If it is same digest then it should throw unique anchored error
			ensure!(unique_details.digest != unique_txn, Error::<T>::UniqueAlreadyAnchored);
			ensure!(!unique_details.revoked, Error::<T>::RevokedUnique);

			if unique_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization.unwrap(),
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					unique_details.registry.clone().unwrap_or(None) == Some(registry_id),
					Error::<T>::UnauthorizedOperation
				);
			}

			//Update entries and Identifiers mapping storage mapping

			<UniqueDigestEntries<T>>::insert(&unique_txn, &unique_id);

			<UniqueIdentifiers<T>>::insert(
				&unique_id,
				UniqueEntryOf::<T> {
					digest: unique_txn.clone(),
					creator: updater.clone(),
					..unique_details
				},
			);
			Self::update_commit(
				&unique_id,
				unique_txn.clone(),
				updater.clone(),
				UniqueCommitActionOf::Update,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update {
				identifier: unique_id,
				digest: unique_txn,
				author: updater,
			});

			Ok(())
		}

		/// Revokes a unique.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `unique_digest`: The unique identifier.
		/// * `authorization`: The authorization ID of the delegate who is
		///   allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke(unique_txn.len().saturated_into()))]
		pub fn revoke(
			origin: OriginFor<T>,
			unique_txn: InputUniqueOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			// Check if unique_txn exists and return an error if it doesn't
			ensure!(
				<UniqueDigestEntries<T>>::contains_key(&unique_txn),
				Error::<T>::UniqueNotFound
			);

			// Retriving identifier from storage or return an error if it doesn't exist
			let identifier =
				<UniqueDigestEntries<T>>::get(&unique_txn).ok_or(Error::<T>::UniqueNotFound)?;

			// Retrieve unique_details from storage or return an error if it doesn't exist
			let unique_details =
				<UniqueIdentifiers<T>>::get(&identifier).ok_or(Error::<T>::UniqueNotFound)?;

			// Return an error if the unique is already revoked
			ensure!(!unique_details.revoked, Error::<T>::RevokedUnique);

			// Check if the updater is authorized as a delegate
			if unique_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					updater.clone(),
					None,
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				// Return an error if the updater is not authorized as a delegate
				ensure!(
					unique_details.registry == Some(Some(registry_id)),
					Error::<T>::UnauthorizedOperation
				);
			}

			<UniqueIdentifiers<T>>::insert(
				&identifier,
				UniqueEntryOf::<T> { creator: updater.clone(), revoked: true, ..unique_details },
			);

			// Perform additional operations and handle committing the changes
			Self::update_commit(
				&identifier,
				unique_txn.clone(),
				updater.clone(),
				UniqueCommitActionOf::Revoke,
			)
			.map_err(<Error<T>>::from)?;

			// Emit a Revoke event
			Self::deposit_event(Event::Revoke { identifier, author: updater });

			Ok(())
		}

		/// Removes a unique from the registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `unique_id`: The unique id of the unique to be removed.
		/// * `authorization`: The authorization ID of the delegate
		/// who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove(0))]
		pub fn remove(
			origin: OriginFor<T>,
			unique_id: UniqueIdOf,
			authorization: Option<AuthorizationIdOf>,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let unique_details =
				<UniqueIdentifiers<T>>::get(&unique_id).ok_or(Error::<T>::UniqueNotFound)?;

			if unique_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization.unwrap(),
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(
					unique_details.registry.clone().unwrap_or(None) == Some(registry_id),
					Error::<T>::UnauthorizedOperation
				);
			}

			<UniqueIdentifiers<T>>::take(&unique_id);

			Self::update_commit(
				&unique_id,
				unique_details.digest,
				updater.clone(),
				UniqueCommitActionOf::Remove,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Remove { identifier: unique_id, author: updater });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// `update_commit` takes a unique id, a digest, a proposer, and a commit
	/// action, and pushes the commit action to the unique's commits
	///
	/// Arguments:
	///
	/// * `tx_unique`: The unique id of the unique that the commit is being made
	///   to.
	/// * `tx_digest`: The digest of the transaction that was committed.
	/// * `proposer`: The account that is proposing the transaction.
	/// * `commit`: Action taken on a unique.
	///
	/// Returns:
	///
	/// The `Result` type is being returned.
	pub fn update_commit(
		tx_stream: &UniqueIdOf,
		tx_digest: InputUniqueOf<T>,
		proposer: UniqueCreatorIdOf<T>,
		commit: UniqueCommitActionOf,
	) -> Result<(), Error<T>> {
		Activities::<T>::try_mutate(tx_stream, |activities| {
			activities
				.try_push(UniqueActivitiesOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxUniqueActivitiesExceeded)?;

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
