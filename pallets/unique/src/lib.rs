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
//! stream pallet, which does not support revoke/restore.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

// #[cfg(any(feature = "mock", test))]
// pub mod mock;

// #[cfg(test)]
// pub mod tests;

use cord_primitives::{curi::Ss58Identifier, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{prelude::Clone, str};
pub mod types;
pub mod weights;
pub use crate::types::*;

pub use crate::{pallet::*, types::*, weights::WeightInfo};

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
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Type for the unique entity
	pub type UniqueEntryOf<T> =
		UniqueEntry<UniqueDigestOf<T>, UniqueCreatorIdOf<T>, Option<RegistryIdOf>, StatusOf>;
	/// Type for the unique commits
	pub type UniqueCommitsOf<T> = UniqueCommit<
		UniqueCommitActionOf,
		UniqueDigestOf<T>,
		UniqueCreatorIdOf<T>,
		BlockNumberOf<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, UniqueCreatorIdOf<Self>>;

		/// The maximum number of commits for a unique.
		#[pallet::constant]
		type MaxUniqueCommits: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// unique identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::getter(fn uniques)]
	pub type Uniques<T> =
		StorageMap<_, Blake2_128Concat, UniqueIdOf, UniqueEntryOf<T>, OptionQuery>;

	/// unique hashes stored on chain.
	/// It maps from a unique hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn unique_digests)]
	pub type UniqueDigests<T> =
		StorageMap<_, Blake2_128Concat, UniqueDigestOf<T>, UniqueIdOf, OptionQuery>;

	/// unique commits stored on chain.
	/// It maps from an identifier to a vector of commits.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub(super) type Commits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		UniqueIdOf,
		BoundedVec<UniqueCommitsOf<T>, T::MaxUniqueCommits>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new unique identifier has been created.
		/// \[unique identifier, unique digest, controller\]
		Create { identifier: UniqueIdOf, digest: UniqueDigestOf<T>, author: UniqueCreatorIdOf<T> },
		/// A unique identifier has been updated.
		/// \[unique identifier, digest, controller\]
		Update { identifier: UniqueIdOf, digest: UniqueDigestOf<T>, author: UniqueCreatorIdOf<T> },
		/// A unique identifier status has been revoked.
		/// \[unique identifier, controller\]
		Revoke { identifier: UniqueIdOf, author: UniqueDigestOf<T> },
		/// A unique identifier status has been restored.
		/// \[unique identifier, controller\]
		Restore { identifier: UniqueIdOf, author: UniqueDigestOf<T> },
		/// A unique identifier has been removed.
		/// \[unique identifier,  controller\]
		Remove { identifier: UniqueIdOf, author: UniqueDigestOf<T> },
		/// A unique digest has been added.
		/// \[unique identifier, digest, controller\]
		Digest { identifier: UniqueIdOf, digest: UniqueDigestOf<T>, author: UniqueCreatorIdOf<T> },
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
		//Stream hash is not unique
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
		// Maximum number of commits exceeded
		MaxUniqueCommitsExceeded,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		//Registy Id mismatch
		RegistryIdMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
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
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			unique_digest: UniqueDigestOf<T>,
			authorization: AuthorizationIdOf,
			registry_id: Option<RegistryIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			if let Some(registry_id) = registry_id.clone() {
				// check for authorization
				pallet_registry::Pallet::<T>::is_an_authority(&registry_id, creator.clone())?;

				let redistry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					creator.clone(),
				)?;

				ensure!(registry_id == redistry_id, Error::<T>::RegistryIdMismatch)
				// Todo: Check for is_a_delegate ?
			}

			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&unique_digest.encode()[..], &creator.encode()[..]].concat()[..],
			);

			//TODO : Have created `to_unique_id` in primitives::curi. Is this Required ?
			let identifier = Ss58Identifier::to_unique_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Uniques<T>>::contains_key(&identifier), Error::<T>::UniqueAlreadyAnchored);

			<UniqueDigests<T>>::insert(unique_digest, &identifier);

			<Uniques<T>>::insert(
				&identifier,
				UniqueEntryOf::<T> {
					digest: unique_digest,
					creator: creator.clone(),
					registry: Some(registry_id),
					revoked: false,
				},
			);

			Self::update_commit(
				&identifier,
				unique_digest,
				creator.clone(),
				UniqueCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Create {
				identifier,
				digest: unique_digest,
				author: creator,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// `update_commit` takes a stream id, a digest, a proposer, and a commit
	/// action, and pushes the commit action to the stream's commits
	///
	/// Arguments:
	///
	/// * `tx_stream`: The stream id of the stream that the commit is being made
	///   to.
	/// * `tx_digest`: The digest of the transaction that was committed.
	/// * `proposer`: The account that is proposing the transaction.
	/// * `commit`: Action taken on a stream.
	///
	/// Returns:
	///
	/// The `Result` type is being returned.
	pub fn update_commit(
		tx_stream: &UniqueIdOf,
		tx_digest: UniqueDigestOf<T>,
		proposer: UniqueCreatorIdOf<T>,
		commit: UniqueCommitActionOf,
	) -> Result<(), Error<T>> {
		Commits::<T>::try_mutate(tx_stream, |commits| {
			commits
				.try_push(UniqueCommitsOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxUniqueCommitsExceeded)?;

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
	pub fn timepoint() -> Timepoint<T::BlockNumber> {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
