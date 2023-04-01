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

//! # Stream Pallet
//!
//! The Stream palllet is used to anchor identifiers representing off-chain documents.
//! The pallet provides means of creating, updating, revoking and removing identifier
//! data on-chain and delegated controls.
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
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	///
	/// Registry Identifier
	pub type RegistryIdOf = Ss58Identifier;
	/// Stream Identifier
	pub type StreamIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Hash of the registry.
	pub type StreamHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a creator identifier.
	pub type StreamCreatorIdOf<T> = pallet_registry::RegistryCreatorIdOf<T>;

	/// Hash of the stream.
	pub type StreamDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	pub type StreamEntryOf<T> =
		StreamEntry<StreamDigestOf<T>, StreamCreatorIdOf<T>, SchemaIdOf, RegistryIdOf, StatusOf>;
	pub type StreamCommitsOf<T> = StreamCommit<
		StreamCommitActionOf,
		StreamDigestOf<T>,
		StreamCreatorIdOf<T>,
		BlockNumberOf<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, StreamCreatorIdOf<Self>>;

		/// The maximum number of commits for a stream.
		#[pallet::constant]
		type MaxStreamCommits: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// stream identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::getter(fn streams)]
	pub type Streams<T> =
		StorageMap<_, Blake2_128Concat, StreamIdOf, StreamEntryOf<T>, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn stream_digests)]
	pub type StreamDigests<T> =
		StorageMap<_, Blake2_128Concat, StreamDigestOf<T>, StreamIdOf, OptionQuery>;

	/// stream commits stored on chain.
	/// It maps from an identifier to a vector of commits.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub(super) type Commits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		StreamIdOf,
		BoundedVec<StreamCommitsOf<T>, T::MaxStreamCommits>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream identifier has been created.
		/// \[stream identifier, stream digest, controller\]
		Create { identifier: StreamIdOf, digest: StreamDigestOf<T>, author: StreamCreatorIdOf<T> },
		/// A stream identifier has been updated.
		/// \[stream identifier, digest, controller\]
		Update { identifier: StreamIdOf, digest: StreamDigestOf<T>, author: StreamCreatorIdOf<T> },
		/// A stream identifier status has been revoked.
		/// \[stream identifier, controller\]
		Revoke { identifier: StreamIdOf, author: StreamCreatorIdOf<T> },
		/// A stream identifier status has been restored.
		/// \[stream identifier, controller\]
		Restore { identifier: StreamIdOf, author: StreamCreatorIdOf<T> },
		/// A stream identifier has been removed.
		/// \[stream identifier,  controller\]
		Remove { identifier: StreamIdOf, author: StreamCreatorIdOf<T> },
		/// A stream digest has been added.
		/// \[stream identifier, digest, controller\]
		Digest { identifier: StreamIdOf, digest: StreamDigestOf<T>, author: StreamCreatorIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stream idenfier is not unique
		StreamAlreadyAnchored,
		/// Stream idenfier not found
		StreamNotFound,
		/// Stream idenfier marked inactive
		RevokedStream,
		/// Stream idenfier not marked inactive
		StreamNotRevoked,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Stream link does not exist
		StreamLinkNotFound,
		/// Stream Link is revoked
		StreamLinkRevoked,
		// Invalid creator signature
		InvalidSignature,
		//Stream hash is not unique
		HashAlreadyAnchored,
		// Expired Tx Signature
		ExpiredSignature,
		// Invalid Stream Identifier
		InvalidStreamIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		// Stream not part of space
		StreamSpaceMismatch,
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
		MaxStreamCommitsExceeded,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the call.
		/// * `stream_digest`: The digest of the stream.
		/// * `schema_id`: The schema id of the stream.
		/// * `authorization`: AuthorizationIdOf
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			stream_digest: StreamDigestOf<T>,
			authorization: AuthorizationIdOf,
			schema_id: Option<SchemaIdOf>,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let registry_id: Ss58Identifier;
			if let Some(schema_id) = schema_id.clone() {
				registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					Some(schema_id.clone()),
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

			// Id Digest = concat (H(<scale_encoded_stream_digest>, <scale_encoded_registry_identifier>, <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&stream_digest.encode()[..], &registry_id.encode()[..], &creator.encode()[..]]
					.concat()[..],
			);

			let identifier = Ss58Identifier::to_stream_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);

			<StreamDigests<T>>::insert(&stream_digest, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamEntryOf::<T> {
					digest: stream_digest,
					creator: creator.clone(),
					schema: schema_id,
					registry: registry_id,
					revoked: false,
				},
			);

			Self::update_commit(
				&identifier,
				stream_digest,
				creator.clone(),
				StreamCommitActionOf::Genesis,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Create {
				identifier,
				digest: stream_digest,
				author: creator,
			});

			Ok(())
		}
		/// Updates the stream identifier with a new digest. The updated digest
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
			stream_id: StreamIdOf,
			stream_digest: StreamDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<StreamDigests<T>>::insert(&stream_digest, &stream_id);

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> {
					digest: stream_digest,
					creator: updater.clone(),
					..stream_details
				},
			);

			Self::update_commit(
				&stream_id,
				stream_digest,
				updater.clone(),
				StreamCommitActionOf::Update,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update {
				identifier: stream_id,
				digest: stream_digest,
				author: updater,
			});

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
			stream_id: StreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> { creator: updater.clone(), revoked: true, ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				StreamCommitActionOf::Revoke,
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
			stream_id: StreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream_details.revoked, Error::<T>::StreamNotRevoked);

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> { creator: updater.clone(), revoked: false, ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				StreamCommitActionOf::Restore,
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
			stream_id: StreamIdOf,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let updater = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;

			if stream_details.creator != updater {
				let registry_id = pallet_registry::Pallet::<T>::is_a_registry_admin(
					&authorization,
					updater.clone(),
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<Streams<T>>::take(&stream_id);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				updater.clone(),
				StreamCommitActionOf::Remove,
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
			stream_id: StreamIdOf,
			stream_digest: StreamDigestOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			if stream_details.creator != creator {
				let registry_id = pallet_registry::Pallet::<T>::is_a_delegate(
					&authorization,
					creator.clone(),
					None,
				)
				.map_err(<pallet_registry::Error<T>>::from)?;

				ensure!(stream_details.registry == registry_id, Error::<T>::UnauthorizedOperation);
			}

			<StreamDigests<T>>::insert(&stream_digest, &stream_id);

			Self::update_commit(
				&stream_id,
				stream_digest,
				creator.clone(),
				StreamCommitActionOf::Genesis,
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
		tx_stream: &StreamIdOf,
		tx_digest: StreamDigestOf<T>,
		proposer: StreamCreatorIdOf<T>,
		commit: StreamCommitActionOf,
	) -> Result<(), Error<T>> {
		Commits::<T>::try_mutate(tx_stream, |commits| {
			commits
				.try_push(StreamCommitsOf::<T> {
					commit,
					digest: tx_digest,
					committed_by: proposer,
					created_at: Self::timepoint(),
				})
				.map_err(|_| Error::<T>::MaxStreamCommitsExceeded)?;

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
