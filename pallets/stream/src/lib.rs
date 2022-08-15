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

use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, STREAM_PREFIX};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};

pub mod streams;
pub mod weights;

pub use crate::streams::*;

pub use crate::weights::WeightInfo;
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
		/// The maximum number of delegates for a stream.
		#[pallet::constant]
		type MaxStreamDelegates: Get<u32>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// #[pallet::hooks]
	// impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

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

	/// stream delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::storage_prefix = "Delegates"]
	pub(super) type StreamDelegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxStreamDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream has been created.
		/// \[stream identifier, stream hash, controller\]
		Anchor { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A stream has been updated.
		/// \[stream identifier, hash, controller\]
		Update { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A stream digest has been added.
		/// \[stream identifier, digest, controller\]
		Digest { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A stream status has been changed.
		/// \[stream identifier, digest, controller\]
		Revoke { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A stream has been removed.
		/// \[stream identifier, digest, controller\]
		Remove { identifier: IdentifierOf, author: CordAccountOf<T> },
		/// A stream has been removed by the council.
		/// \[stream identifier\]
		CouncilRemove { identifier: IdentifierOf },
		/// A metedata entry has been added.
		/// \[identifier, controller\]
		MetadataSet { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// A metadata entry has been cleared.
		/// \[identifier, controller\]
		MetadataCleared { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// Stream delegates has been added.
		/// \[stream identifier,  delegator\]
		AddDelegates { identifier: IdentifierOf, digest: HashOf<T>, delegator: CordAccountOf<T> },
		/// Stream delegates has been removed.
		/// \[stream identifier,  delegator\]
		RemoveDelegates { identifier: IdentifierOf, digest: HashOf<T>, delegator: CordAccountOf<T> },
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
		// Metadata limit exceeded
		MetadataLimitExceeded,
		// Metadata already set for the entry
		MetadataAlreadySet,
		// Metadata not found for the entry
		MetadataNotFound,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Maximum Number of delegates reached.
		TooManyDelegatesToRemove,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream and associates it with its controller.
		///
		/// * origin: the identity of the Tx Author.
		/// * stream: the incoming stream.
		/// * tx_signature: creator signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_stream: StreamType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&tx_stream.digest).encode()[..], &tx_stream.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&tx_stream.digest).encode()[..], STREAM_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);

			if let Some(ref space) = tx_stream.space {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					tx_stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else if let Some(ref schema) = tx_stream.schema {
				pallet_schema::SchemaDetails::<T>::from_schema_identities(
					schema,
					tx_stream.controller.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			}
			if let Some(ref link) = tx_stream.link {
				let link_details =
					<Streams<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
				ensure!(!link_details.revoked, Error::<T>::StreamLinkRevoked);
			}

			<StreamHashes<T>>::insert(&tx_stream.digest, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamDetails {
					stream: tx_stream.clone(),
					revoked: false,
					meta: false,
					delegates: false,
				},
			);

			Self::deposit_event(Event::Anchor {
				identifier,
				digest: tx_stream.digest,
				author: tx_stream.controller,
			});

			Ok(())
		}
		/// Updates the stream information.
		///
		/// * origin: the identity of the Tx Author.
		/// * update: the incoming stream.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			tx_stream: StreamParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

			if let Some(ref space) = tx_stream.stream.space {
				ensure!(
					stream_details.stream.space == Some(space.clone()),
					Error::<T>::StreamSpaceMismatch
				);
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else if let Some(ref schema) = tx_stream.stream.schema {
				pallet_schema::SchemaDetails::from_schema_delegates(
					schema,
					stream_details.stream.controller.clone(),
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			} else {
				ensure!(
					stream_details.stream.controller == tx_stream.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

			<Streams<T>>::insert(
				&tx_stream.identifier,
				StreamDetails {
					stream: {
						StreamType { digest: tx_stream.stream.digest, ..stream_details.stream }
					},
					..stream_details
				},
			);

			Self::deposit_event(Event::Update {
				identifier: tx_stream.identifier,
				digest: tx_stream.stream.digest,
				author: tx_stream.stream.controller,
			});

			Ok(())
		}
		/// Revoke a stream
		///
		/// * origin: the identity of the Tx Author.
		/// * revoke: the stream to revoke.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke())]
		pub fn revoke(
			origin: OriginFor<T>,
			tx_stream: StreamParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

			if let Some(ref space) = tx_stream.stream.space {
				ensure!(
					stream_details.stream.space == Some(space.clone()),
					Error::<T>::StreamSpaceMismatch
				);
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else if let Some(ref schema) = tx_stream.stream.schema {
				pallet_schema::SchemaDetails::from_schema_delegates(
					schema,
					stream_details.stream.controller.clone(),
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			} else {
				ensure!(
					stream_details.stream.controller == tx_stream.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

			<Streams<T>>::insert(
				&tx_stream.identifier,
				StreamDetails { revoked: true, ..stream_details },
			);
			Self::deposit_event(Event::Revoke {
				identifier: tx_stream.identifier,
				digest: tx_stream.stream.digest,
				author: tx_stream.stream.controller,
			});

			Ok(())
		}

		///  Remove a stream from the chain using space identities.
		///
		/// * origin: the identity of the space origin.
		/// * remove: the stream to remove.
		/// * tx_signature: signature of the controller.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		pub fn remove(
			origin: OriginFor<T>,
			tx_stream: StreamParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature
					.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;

			if let Some(ref space) = tx_stream.stream.space {
				ensure!(
					stream_details.stream.space == Some(space.clone()),
					Error::<T>::StreamSpaceMismatch
				);
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			} else if let Some(ref schema) = tx_stream.stream.schema {
				pallet_schema::SchemaDetails::from_schema_delegates(
					schema,
					stream_details.stream.controller,
					tx_stream.stream.controller.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			} else {
				ensure!(
					stream_details.stream.controller == tx_stream.stream.controller,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

			<Streams<T>>::remove(&tx_stream.identifier);
			Self::deposit_event(Event::Remove {
				identifier: tx_stream.identifier,
				author: tx_stream.stream.controller,
			});

			Ok(())
		}

		///  Remove a stream from the chain using council origin.
		///
		/// * origin: the identity of the council origin.
		/// * identifier: unique identifier of the incoming stream.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::council_remove())]
		pub fn council_remove(origin: OriginFor<T>, identifier: IdentifierOf) -> DispatchResult {
			<T as Config>::ForceOrigin::ensure_origin(origin)?;
			ss58identifier::from_known_format(&identifier, STREAM_PREFIX)
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
		#[pallet::weight(<T as pallet::Config>::WeightInfo::digest())]
		pub fn digest(
			origin: OriginFor<T>,
			identifier: IdentifierOf,
			author: CordAccountOf<T>,
			digest_hash: HashOf<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&digest_hash).encode()[..], &author),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&digest_hash),
				Error::<T>::DigestHashAlreadyAnchored
			);

			let stream_details =
				<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

			if let Some(ref space) = stream_details.stream.space {
				pallet_space::SpaceDetails::<T>::from_space_identities(space, author.clone())
					.map_err(<pallet_space::Error<T>>::from)?;
			} else if let Some(ref schema) = stream_details.stream.schema {
				pallet_schema::SchemaDetails::from_schema_delegates(
					schema,
					stream_details.stream.controller.clone(),
					author.clone(),
				)
				.map_err(<pallet_schema::Error<T>>::from)?;
			} else {
				ensure!(
					stream_details.stream.controller == author,
					Error::<T>::UnauthorizedOperation
				);
			}

			<StreamHashes<T>>::insert(&digest_hash, &identifier);

			Self::deposit_event(Event::Digest { identifier, digest: digest_hash, author });

			Ok(())
		}
		/// Add schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller or
		/// delegates.
		///
		/// * origin: the identity of the schema anchor.
		/// * auth: delegation schema details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: transaction author signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::delegate())]
		pub fn delegate(
			origin: OriginFor<T>,
			tx_delegation: StreamDelegationParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_delegation.digest),
				Error::<T>::InvalidTransactionHash
			);
			ensure!(
				tx_signature
					.verify(&(&tx_delegation.digest).encode()[..], &tx_delegation.delegator),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_delegation.identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&tx_delegation.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

			if stream_details.stream.holder != tx_delegation.delegator {
				if let Some(ref space) = stream_details.stream.space {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						space,
						tx_delegation.delegator.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				} else if let Some(ref schema) = stream_details.stream.schema {
					pallet_schema::SchemaDetails::from_schema_delegates(
						schema,
						stream_details.stream.controller.clone(),
						tx_delegation.delegator.clone(),
					)
					.map_err(<pallet_schema::Error<T>>::from)?;
				} else {
					ensure!(
						stream_details.stream.controller == tx_delegation.delegator,
						Error::<T>::UnauthorizedOperation
					);
				}
			}

			StreamDelegations::<T>::try_mutate(
				tx_delegation.identifier.clone(),
				|ref mut delegation| {
					ensure!(
						delegation.len() + delegates.len() <= T::MaxSchemaDelegates::get() as usize,
						Error::<T>::TooManyDelegates
					);
					for delegate in delegates {
						delegation
							.try_push(delegate)
							.expect("delegates length is less than T::MaxSchemaDelegates; qed");
					}

					<StreamHashes<T>>::insert(&tx_delegation.digest, &tx_delegation.identifier);
					<Streams<T>>::insert(
						&tx_delegation.identifier,
						StreamDetails { delegates: true, ..stream_details },
					);

					Self::deposit_event(Event::AddDelegates {
						identifier: tx_delegation.identifier,
						digest: tx_delegation.digest,
						delegator: tx_delegation.delegator,
					});
					Ok(())
				},
			)
		}
		/// Remove schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller or
		/// delegates.
		///
		/// * origin: the identity of the schema anchor.
		/// * schema: delegation schema details.
		/// * delegates: identities to be removed.
		/// * tx_signature: transaction author signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::undelegate())]
		pub fn undelegate(
			origin: OriginFor<T>,
			tx_delegation: StreamDelegationParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<StreamHashes<T>>::contains_key(&tx_delegation.digest),
				Error::<T>::InvalidTransactionHash
			);
			ensure!(
				tx_signature
					.verify(&(&tx_delegation.digest).encode()[..], &tx_delegation.delegator),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&tx_delegation.identifier, STREAM_PREFIX)
				.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

			let stream_details =
				<Streams<T>>::get(&tx_delegation.identifier).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

			if stream_details.stream.holder != tx_delegation.delegator {
				if let Some(ref space) = stream_details.stream.space {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						space,
						tx_delegation.delegator.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				} else if let Some(ref schema) = stream_details.stream.schema {
					pallet_schema::SchemaDetails::from_schema_delegates(
						schema,
						stream_details.stream.controller.clone(),
						tx_delegation.delegator.clone(),
					)
					.map_err(<pallet_schema::Error<T>>::from)?;
				} else {
					ensure!(
						stream_details.stream.controller == tx_delegation.delegator,
						Error::<T>::UnauthorizedOperation
					);
				}
			}

			StreamDelegations::<T>::try_mutate(
				tx_delegation.identifier.clone(),
				|ref mut delegation| {
					ensure!(
						delegates.len() <= T::MaxSchemaDelegates::get() as usize,
						Error::<T>::TooManyDelegatesToRemove
					);
					for delegate in delegates {
						delegation.retain(|x| x != &delegate);
					}

					if delegation.is_empty() {
						<Streams<T>>::insert(
							&tx_delegation.identifier,
							StreamDetails { delegates: false, ..stream_details },
						);
					}

					<StreamHashes<T>>::insert(&tx_delegation.digest, &tx_delegation.identifier);

					Self::deposit_event(Event::RemoveDelegates {
						identifier: tx_delegation.identifier,
						digest: tx_delegation.digest,
						delegator: tx_delegation.delegator,
					});
					Ok(())
				},
			)
		}
	}
}
