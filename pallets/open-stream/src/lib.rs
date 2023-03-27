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

//! # Open Stream Pallet
//!
//! The Stream palllet is used to anchor identifiers representing public on-chain information
//! /information. The on-chain is an opaque blob representing the public information. The blob
//!  Could be JSON, a Hash, or raw text. Up to the community to decide how exactly to use this.
//! The pallet provides means of discovering, creating, updating, revoking and removing the
//! information.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

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
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	///
	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Stream Identifier
	pub type OpenStreamIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Type of a creator identifier.
	pub type OpenStreamCreatorIdOf<T> = pallet_registry::RegistryCreatorIdOf<T>;
	/// Type of an open stream information.
	pub type OpenStreamOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedOpenStreamLength>;

	/// Hash of the stream.
	pub type OpenStreamDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	pub type OpenStreamEntryOf<T> = OpenStreamEntry<
		OpenStreamOf<T>,
		OpenStreamDigestOf<T>,
		OpenStreamCreatorIdOf<T>,
		SchemaIdOf,
		RegistryIdOf,
		StatusOf,
	>;
	pub type OpenStreamCommitsOf<T> = OpenStreamCommit<
		OpenStreamCommitActionOf,
		OpenStreamDigestOf<T>,
		OpenStreamCreatorIdOf<T>,
		BlockNumberOf<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, OpenStreamCreatorIdOf<Self>>;

		/// The maximum number of commits for a stream.
		#[pallet::constant]
		type MaxOpenStreamCommits: Get<u32>;

		/// The maximum number of commits for a stream.
		#[pallet::constant]
		type MaxEncodedOpenStreamLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// stream identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::getter(fn open_streams)]
	pub type OpenStreams<T> =
		StorageMap<_, Blake2_128Concat, OpenStreamIdOf, OpenStreamEntryOf<T>, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn open_stream_digests)]
	pub type OpenStreamDigests<T> =
		StorageMap<_, Blake2_128Concat, OpenStreamDigestOf<T>, OpenStreamIdOf, OptionQuery>;

	/// stream commits stored on chain.
	/// It maps from an identifier to a vector of commits.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub(super) type Commits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		OpenStreamIdOf,
		BoundedVec<OpenStreamCommitsOf<T>, T::MaxOpenStreamCommits>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new open stream identifier has been created.
		/// \[stream identifier, stream digest, controller\]
		Create {
			identifier: OpenStreamIdOf,
			digest: OpenStreamDigestOf<T>,
			author: OpenStreamCreatorIdOf<T>,
		},
		/// An open stream identifier has been updated.
		/// \[open stream identifier, digest, controller\]
		Update {
			identifier: OpenStreamIdOf,
			digest: OpenStreamDigestOf<T>,
			author: OpenStreamCreatorIdOf<T>,
		},
		/// An open stream identifier status has been revoked.
		/// \[open stream identifier, controller\]
		Revoke { identifier: OpenStreamIdOf, author: OpenStreamCreatorIdOf<T> },
		/// An open stream identifier status has been restored.
		/// \[open stream identifier, controller\]
		Restore { identifier: OpenStreamIdOf, author: OpenStreamCreatorIdOf<T> },
		/// An open stream identifier has been removed.
		/// \[open stream identifier,  controller\]
		Remove { identifier: OpenStreamIdOf, author: OpenStreamCreatorIdOf<T> },
		/// An open stream digest has been added.
		/// \[open stream identifier, digest, controller\]
		Digest {
			identifier: OpenStreamIdOf,
			digest: OpenStreamDigestOf<T>,
			author: OpenStreamCreatorIdOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stream idenfier is not unique
		OpenStreamAlreadyAnchored,
		/// Stream idenfier not found
		OpenStreamNotFound,
		/// Stream idenfier marked inactive
		RevokedOpenStream,
		/// Stream idenfier not marked inactive
		OpenStreamNotRevoked,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		//Stream hash is not unique
		HashAlreadyAnchored,
		// Invalid Stream Identifier
		InvalidOpenStreamIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		// Stream not part of space
		OpenStreamSpaceMismatch,
		//Stream digest is not unique
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
		MaxOpenStreamCommitsExceeded,
		// Maximum stream length exceeded
		MaxEncodedOpenStreamLimitExceeded,
		/// Empty transaction.
		EmptyTransaction,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `stream_digest`: The hash of the stream reference document.
		/// * `authorization`: The authorization ID of the delegate that is allowed to create the stream.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			tx_stream: OpenStreamOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_stream.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_stream.len() <= T::MaxEncodedOpenStreamLength::get() as usize,
				Error::<T>::MaxEncodedOpenStreamLimitExceeded
			);

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_stream[..]);

			let identifier = Ss58Identifier::to_open_stream_id(&(digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<OpenStreams<T>>::contains_key(&identifier),
				Error::<T>::OpenStreamAlreadyAnchored
			);

			let (registry_id, schema_id) =
				pallet_registry::Pallet::<T>::is_a_delegate(&authorization, creator.clone())?;

			<OpenStreamDigests<T>>::insert(&digest, &identifier);

			<OpenStreams<T>>::insert(
				&identifier,
				OpenStreamEntryOf::<T> {
					stream: tx_stream,
					digest,
					creator: creator.clone(),
					schema: schema_id.clone(),
					registry: registry_id,
					revoked: false,
				},
			);

			Self::update_commit(
				&identifier,
				digest,
				creator.clone(),
				OpenStreamCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Create { identifier, digest, author: creator });

			Ok(())
		}
		/// Updates the stream identifier with a new information and digest. The updated digest
		/// represents the changes a stream reference document might have undergone.
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `stream_id`: The identifier of the stream to be updated.
		/// * `stream_digest`: The hash of the stream reference document.
		/// * `authorization`: The authorization ID of the delegate who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			stream_id: OpenStreamIdOf,
			tx_stream: OpenStreamOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_stream.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_stream.len() <= T::MaxEncodedOpenStreamLength::get() as usize,
				Error::<T>::MaxEncodedOpenStreamLimitExceeded
			);

			let stream_details =
				<OpenStreams<T>>::get(&stream_id).ok_or(Error::<T>::OpenStreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedOpenStream);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_stream[..]);

			<OpenStreamDigests<T>>::insert(&digest, &stream_id);

			<OpenStreams<T>>::insert(
				&stream_id,
				OpenStreamEntryOf::<T> { digest, creator: updater.clone(), ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				digest,
				updater.clone(),
				OpenStreamCommitActionOf::Update,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update { identifier: stream_id, digest, author: updater });

			Ok(())
		}
		/// Revokes a stream.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `stream_id`: The stream identifier.
		/// * `authorization`: The authorization ID of the delegate who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn revoke(
			origin: OriginFor<T>,
			stream_id: OpenStreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details =
				<OpenStreams<T>>::get(&stream_id).ok_or(Error::<T>::OpenStreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedOpenStream);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<OpenStreams<T>>::insert(
				&stream_id,
				OpenStreamEntryOf::<T> {
					creator: updater.clone(),
					revoked: true,
					..stream_details
				},
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				OpenStreamCommitActionOf::Revoke,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Revoke { identifier: stream_id, author: updater });

			Ok(())
		}

		/// Restore a a previously revoked stream.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `stream_id`: The stream identifier.
		/// * `authorization`: The authorization ID of the delegate who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn restore(
			origin: OriginFor<T>,
			stream_id: OpenStreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details =
				<OpenStreams<T>>::get(&stream_id).ok_or(Error::<T>::OpenStreamNotFound)?;
			ensure!(stream_details.revoked, Error::<T>::OpenStreamNotRevoked);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<OpenStreams<T>>::insert(
				&stream_id,
				OpenStreamEntryOf::<T> {
					creator: updater.clone(),
					revoked: false,
					..stream_details
				},
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				OpenStreamCommitActionOf::Restore,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restore { identifier: stream_id, author: updater });

			Ok(())
		}

		/// Removes a stream from the registry.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `stream_id`: The stream id of the stream to be removed.
		/// * `authorization`: The authorization ID of the delegate who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn remove(
			origin: OriginFor<T>,
			stream_id: OpenStreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details =
				<OpenStreams<T>>::get(&stream_id).ok_or(Error::<T>::OpenStreamNotFound)?;

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<OpenStreams<T>>::take(&stream_id);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				OpenStreamCommitActionOf::Remove,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restore { identifier: stream_id, author: updater });

			Ok(())
		}

		/// Adds stream digest information.
		/// `digest` is a function that takes a stream identifier, a stream digest, and an authorization
		/// identifier, and inserts the stream digest into the `StreamDigests` storage map, and then deposits an
		/// event. This operation can only be performed bythe stream issuer or delegated authorities.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `stream_id`: The stream identifier.
		/// * `stream_digest`: StreamDigestOf<T>
		/// * `authorization`: The authorization ID of the delegate who is allowed to perform this action.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn digest(
			origin: OriginFor<T>,
			stream_id: OpenStreamIdOf,
			stream_digest: OpenStreamDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details =
				<OpenStreams<T>>::get(&stream_id).ok_or(Error::<T>::OpenStreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedOpenStream);

			if stream_details.creator != creator {
				let (registry_id, _) =
					pallet_registry::Pallet::<T>::is_a_delegate(&authorization, creator.clone())
						.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<OpenStreamDigests<T>>::insert(&stream_digest, &stream_id);

			Self::update_commit(
				&stream_id,
				stream_digest,
				creator.clone(),
				OpenStreamCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Digest {
				identifier: stream_id,
				digest: stream_digest,
				author: creator,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// `update_commit` takes a stream id, a digest, a proposer, and a commit action, and pushes the commit
	/// action to the stream's commits
	///
	/// Arguments:
	///
	/// * `tx_stream`: The stream id of the stream that the commit is being made to.
	/// * `tx_digest`: The digest of the transaction that was committed.
	/// * `proposer`: The account that is proposing the transaction.
	/// * `commit`: Action taken on a stream.
	///
	/// Returns:
	///
	/// The `Result` type is being returned.
	pub fn update_commit(
		tx_stream: &OpenStreamIdOf,
		tx_digest: OpenStreamDigestOf<T>,
		proposer: OpenStreamCreatorIdOf<T>,
		commit: OpenStreamCommitActionOf,
	) -> Result<(), Error<T>> {
		Commits::<T>::try_mutate(tx_stream, |commits| {
			commits
				.try_push(OpenStreamCommitsOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxOpenStreamCommitsExceeded)?;

			Ok(())
		})
	}
	pub fn timepoint() -> Timepoint<T::BlockNumber> {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
