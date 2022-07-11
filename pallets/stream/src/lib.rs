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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

use cord_primitives::{mark, IdentifierOf, StatusOf};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};

pub mod streams;
pub mod weights;

pub use crate::streams::*;

use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the Stream.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the controller.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	// stream identifier prefix.
	pub const STREAM_IDENTIFIER_PREFIX: u16 = 51;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_schema::Config + pallet_space::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer> + Parameter;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	// #[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// streams stored on chain.
	/// It maps from stream Id to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Streams<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, StreamDetails<T>, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type StreamHashes<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// stream digest stored on chain.
	/// It maps from a stream digest hash to Id (resolve from hash).
	#[pallet::storage]
	#[pallet::storage_prefix = "Digests"]
	pub type StreamDigests<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream hash, identifier, controller\]
		Anchor { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A stream has been updated.
		/// \[stream identifier, hash, controller\]
		Update { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A stream digest has been added.
		/// \[stream identifier, hash, controller\]
		Digest { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A stream status has been changed.
		/// \[stream identifier, controller\]
		Revoke { identifier: IdentifierOf, hash: HashOf<T>, author: CordAccountOf<T> },
		/// A stream has been removed.
		/// \[stream identifier\]
		Remove { identifier: IdentifierOf, author: CordAccountOf<T> },
		/// A stream has been removed by the council.
		/// \[stream identifier\]
		CouncilRemove { identifier: IdentifierOf },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stream idenfier is not unique
		StreamAlreadyAnchored,
		/// Stream idenfier not found
		StreamNotFound,
		/// Stream idenfier marked inactive
		StreamRevoked,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its controller.
		///
		/// * origin: the identity of the Tx Author.
		/// * stream: the incoming stream.
		/// * tx_signature: creator signature.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(3, 2))]
		pub fn create(
			origin: OriginFor<T>,
			stream: StreamType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&stream.hash),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&stream.hash).encode()[..], &stream.author),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				mark::generate(&(&stream.hash).encode()[..], STREAM_IDENTIFIER_PREFIX).into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);

			if let Some(ref schema) = stream.schema {
				pallet_schema::SchemaDetails::<T>::from_schema_identities(
					schema,
					stream.author.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			}

			if let Some(ref link) = stream.link {
				let link_details =
					<Streams<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!link_details.revoked, Error::<T>::StreamLinkRevoked);
			}
			if let Some(ref space) = stream.space {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					stream.author.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			}
			<StreamHashes<T>>::insert(&stream.hash, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamDetails { stream: stream.clone(), revoked: false },
			);

			Self::deposit_event(Event::Anchor {
				identifier,
				hash: stream.hash,
				author: stream.author,
			});

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identity of the Tx Author.
		/// * update: the incoming stream.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(50_000 + T::DbWeight::get().reads_writes(2, 2))]
		pub fn update(
			origin: OriginFor<T>,
			update: StreamParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&update.stream.hash),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&update.stream.hash).encode()[..], &update.stream.author),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&update.identifier, STREAM_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let tx_prev_details =
				<Streams<T>>::get(&update.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!tx_prev_details.revoked, Error::<T>::StreamRevoked);

			if let Some(ref space) = update.stream.space {
				ensure!(
					tx_prev_details.stream.space == Some(space.clone()),
					Error::<T>::StreamSpaceMismatch
				);

				if tx_prev_details.stream.author != update.stream.author {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						space,
						update.stream.author.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					tx_prev_details.stream.author == update.stream.author,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&update.stream.hash, &update.identifier);

			<Streams<T>>::insert(
				&update.identifier,
				StreamDetails { stream: update.stream.clone(), revoked: false },
			);
			Self::deposit_event(Event::Update {
				identifier: update.identifier,
				hash: update.stream.hash,
				author: update.stream.author,
			});

			Ok(())
		}
		/// Revoke a stream
		///
		/// * origin: the identity of the Tx Author.
		/// * revoke: the stream to revoke.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(30_000 + T::DbWeight::get().reads_writes(2, 3))]
		pub fn revoke(
			origin: OriginFor<T>,
			revoke: StreamParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&revoke.stream.hash),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&revoke.stream.hash).encode()[..], &revoke.stream.author),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&revoke.identifier, STREAM_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let tx_prev_details =
				<Streams<T>>::get(&revoke.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!tx_prev_details.revoked, Error::<T>::StreamRevoked);

			if let Some(space) = revoke.stream.space {
				ensure!(
					tx_prev_details.stream.space == Some(space.clone()),
					Error::<T>::StreamSpaceMismatch
				);

				if tx_prev_details.stream.author != revoke.stream.author {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						&space,
						revoke.stream.author.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					tx_prev_details.stream.author == revoke.stream.author,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&revoke.stream.hash, &revoke.identifier);

			<Streams<T>>::insert(
				&revoke.identifier,
				StreamDetails {
					revoked: true,
					stream: {
						StreamType {
							author: revoke.stream.author.clone(),
							..tx_prev_details.stream
						}
					},
				},
			);
			Self::deposit_event(Event::Revoke {
				identifier: revoke.identifier,
				hash: revoke.stream.hash,
				author: revoke.stream.author,
			});

			Ok(())
		}

		///  Remove a stream from the chain using space identities.
		///
		/// * origin: the identity of the space origin.
		/// * identifier: unique identifier of the incoming stream.
		/// * space: stream space link identifier.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(3, 3))]
		pub fn remove_space_stream(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			space: IdentifierOf,
		) -> DispatchResult {
			let controller = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			mark::from_known_format(&identifier, STREAM_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;

			ensure!(
				stream_details.stream.space == Some(space.clone()),
				Error::<T>::StreamSpaceMismatch
			);

			if stream_details.stream.author != controller {
				pallet_space::SpaceDetails::<T>::from_space_identities(&space, controller.clone())
					.map_err(<pallet_space::Error<T>>::from)?;
			}

			<Streams<T>>::remove(&identifier);
			Self::deposit_event(Event::Remove { identifier, author: controller });

			Ok(())
		}

		///  Remove a stream from the chain using council origin.
		///
		/// * origin: the identity of the council origin.
		/// * identifier: unique identifier of the incoming stream.
		#[pallet::weight(52_000 + T::DbWeight::get().reads_writes(3, 3))]
		pub fn council_remove(origin: OriginFor<T>, identifier: IdentifierOf) -> DispatchResult {
			<T as Config>::ForceOrigin::ensure_origin(origin)?;
			mark::from_known_format(&identifier, STREAM_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;
			<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;

			<Streams<T>>::remove(&identifier);
			Self::deposit_event(Event::CouncilRemove { identifier });

			Ok(())
		}
		/// Adds stream digest information.
		///
		/// * origin: the identity of the anchor.
		/// * identifier: unique identifier of the incoming stream.
		/// * creator: controller or delegate of the stream.
		/// * digest_hash: hash of the incoming stream.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(30_000 + T::DbWeight::get().reads_writes(2, 1))]
		pub fn digest(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			creator: CordAccountOf<T>,
			digest_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&digest_hash).encode()[..], &creator),
				Error::<T>::InvalidSignature
			);

			mark::from_known_format(&identifier, STREAM_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			ensure!(
				!<StreamDigests<T>>::contains_key(&digest_hash),
				Error::<T>::DigestHashAlreadyAnchored
			);

			let tx_prev_details =
				<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!tx_prev_details.revoked, Error::<T>::StreamRevoked);

			if let Some(ref schema) = tx_prev_details.stream.schema {
				pallet_schema::SchemaDetails::<T>::from_schema_identities(schema, creator.clone())
					.map_err(<pallet_schema::Error<T>>::from)?;
			}

			<StreamDigests<T>>::insert(&digest_hash, &identifier);

			Self::deposit_event(Event::Digest { identifier, hash: digest_hash, author: creator });

			Ok(())
		}
	}
}
