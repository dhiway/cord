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

//! # Stream Pallet
//!
//! The Stream palllet is used to anchor identifiers representing off-chain data
//! streams. The pallet provides means of creating, updating, revoking and
//! removing identifier data on-chain and holder delegations.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ### Terminology
//!
//! - **Identifier:**: A unique persistent identifier representing a stream,
//!   and it's current status.
//!
// ! - **Holder:**: The holder is the person or organization to whom the
// !   stream is issued to. The crytographic identity of the holder is
// !   attached to the stream. The holder keeps the stream data as an off-chain
// !   stream/verifiable credential in their digital wallet. An example of a
// !   holder would be the citizen that received his or her driver’s license from
// !   the DMV.
// !
// ! - **Issuer:**: The issuer is the person or organization that creates the
// !   stream, anchors the identifier on-chain and assigns it to a holder
// !   (person, organization, or thing). An example of an issuer would be a DMV
// !   which issues driver’s licenses.
// !
// ! - **Verifier:**: The verifier is the person or organization that gets
// !   information shared with them. They can authenticate the information they
// !   receive instantaneously. An example of a verifier would be the security
// !   agent at the airport that asks to see a person’s driver’s license to
// !   verify their identity.
// !
// ! - **Schema:**: Schemas are templates used to guarantee the structure, and by
// !   extension the semantics, of the set of claims comprising a
// !   Stream/Verifiable Credential. A shared Schema allows all parties to
// !   reference data in a known way. An identifier can optionally link to a
// !   valid schema identifier.
// !
// ! - **Registry:**: Registries allows grouping of identifiers with control
// !   delegations. The registry delegates can perform actions on behalf of the
// !   identifier owner.
// !
// ! - **Authorization:**: An stream identifier that is not issued by the issuer
// !   directly but via a (chain of) authority delegations which entitle the
// !   delegated issuer.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! The dispatchable functions of the Stream pallet enable the steps needed for
//! entities to anchor, update, remove link identifiers change their role,
//! alongside some helper functions to get/set the holder delgates and digest
//! information for the identifier.
//!
//! - `create` - Create a new identifier for a given stream which is based on a
//!   Schema. The issuer can optionally provide a reference to an existing
//!   identifier that will be saved along with the identifier.
//! - `update` - Update an existing identifier with stream revision details. The
//!   revision hashes get attached to the identifier to form an immutable audit
//!   log.
//! - `revoke` - Revoke an existing identifier. The revoker must be either the
//!   creator of the identifier being revoked or an entity that in the
//!   delegation tree is an ancestor of the issuer, i.e., it was either the
//!   delegator of the issuer or an ancestor thereof.
//! - `restore` - Restore a revokedidentifier. The revoker must be either the
//!   creator of the identifier being restored or an entity that in the
//!   delegation tree is an ancestor of the issuer, i.e., it was either the
//!   delegator of the issuer or an ancestor thereof.
//! - `remove` - Remove an existing identifier and associated on-chain data.The
//!   remover must be either the creator of the identifier being revoked or an
//!   entity that in the delegation tree is an ancestor of the issuer, i.e., it
//!   was either the delegator of the issuer or an ancestor thereof.
//! - `digest` - Create a presention digest for the identifier. This feature can
//!   be used to support various presenation models used to represent a
//!   stream/credential in the digtal/physical world.
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

use cord_primitives::{ss58identifier, IdentifierOf, StatusOf, STREAM_PREFIX};
use frame_support::{ensure, storage::types::StorageMap};
use sp_std::{prelude::Clone, str};

pub mod types;
pub mod weights;

pub use crate::types::*;

pub use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	///
	/// Registry Identifier
	pub type RegistryIdOf = IdentifierOf;
	/// Stream Identifier
	pub type StreamIdOf = IdentifierOf;
	/// Schema Identifier
	pub type SchemaIdOf = IdentifierOf;
	/// Authorization Identifier
	pub type AuthorizationIdOf = IdentifierOf;
	/// Hash of the registry.
	pub type StreamHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a creator identifier.
	pub type CreatorIdOf<T> = <T as Config>::CreatorId;
	/// Hash of the stream.
	pub type StreamDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	pub type StreamEntryOf<T> =
		StreamEntry<StreamDigestOf<T>, CreatorIdOf<T>, SchemaIdOf, RegistryIdOf, StatusOf>;
	pub type StreamCommitsOf<T> =
		StreamCommit<StreamCommitActionOf, StreamDigestOf<T>, CreatorIdOf<T>, BlockNumberOf<T>>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_registry::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, CreatorIdOf<Self>>;
		type CreatorId: Parameter + MaxEncodedLen;

		/// The maximum number of commits for a stream.
		#[pallet::constant]
		type MaxStreamCommits: Get<u32>;

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
	#[pallet::getter(fn streams)]
	pub type Streams<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, StreamEntryOf<T>, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::getter(fn stream_digests)]
	pub type StreamDigests<T> =
		StorageMap<_, Blake2_128Concat, StreamDigestOf<T>, IdentifierOf, OptionQuery>;

	/// stream commits stored on chain.
	/// It maps from an identifier to a vector of commits.
	#[pallet::storage]
	#[pallet::getter(fn commits)]
	pub(super) type Commits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<StreamCommitsOf<T>, T::MaxStreamCommits>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream identifier has been created.
		/// \[stream identifier, stream digest, controller\]
		Create { identifier: IdentifierOf, digest: StreamDigestOf<T>, author: CreatorIdOf<T> },
		/// A stream identifier has been updated.
		/// \[stream identifier, digest, controller\]
		Update { identifier: IdentifierOf, digest: StreamDigestOf<T>, author: CreatorIdOf<T> },
		/// A stream identifier status has been revoked.
		/// \[stream identifier, controller\]
		Revoke { identifier: IdentifierOf, author: CreatorIdOf<T> },
		/// A stream identifier status has been restored.
		/// \[stream identifier, controller\]
		Restore { identifier: IdentifierOf, author: CreatorIdOf<T> },
		/// A stream identifier has been removed.
		/// \[stream identifier,  controller\]
		Remove { identifier: IdentifierOf, author: CreatorIdOf<T> },
		/// A stream digest has been added.
		/// \[stream identifier, digest, controller\]
		Digest { identifier: IdentifierOf, digest: StreamDigestOf<T>, author: CreatorIdOf<T> },
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
		MaxStreamCommitsExceeded,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream identifier and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn create(
			origin: OriginFor<T>,
			stream_digest: StreamDigestOf<T>,
			schema_id: SchemaIdOf,
			authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(&stream_digest).encode()[..], STREAM_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);

			let authorization_details = <pallet_registry::Authorizations<T>>::get(authorization_id)
				.ok_or(Error::<T>::AuthorizationDetailsNotFound)?;

			// TODO: Check if the authorization is valid

			<StreamDigests<T>>::insert(&stream_digest, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamEntryOf::<T> {
					digest: stream_digest,
					creator: creator.clone(),
					schema: schema_id,
					registry: authorization_details.registry,
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
		/// represents the changes a stream might have undergone. This operation can
		/// only be performed by the stream issuer or delegated authorities.
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn update(
			origin: OriginFor<T>,
			stream_id: StreamIdOf,
			stream_digest: StreamDigestOf<T>,
			_authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			// TODO: Check if the authorization is valid
			ensure!(stream_details.creator == creator, Error::<T>::UnauthorizedOperation);

			<StreamDigests<T>>::insert(&stream_digest, &stream_id);

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> { digest: stream_digest, ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				stream_digest,
				creator.clone(),
				StreamCommitActionOf::Update,
			)
			.map_err(<Error<T>>::from)?;

			Self::deposit_event(Event::Update {
				identifier: stream_id,
				digest: stream_digest,
				author: creator,
			});

			Ok(())
		}
		/// Revoke a stream identifier. This operation can only be performed by
		/// the stream issuer or delegated authorities.
		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn revoke(
			origin: OriginFor<T>,
			stream_id: StreamIdOf,
			_authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			// TODO: Check if the authorization is valid
			ensure!(stream_details.creator == creator, Error::<T>::UnauthorizedOperation);

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> { revoked: true, ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				creator.clone(),
				StreamCommitActionOf::Revoke,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Revoke { identifier: stream_id, author: creator });

			Ok(())
		}

		/// Restore a revoked stream identifier. This operation can only be performed by
		/// the stream issuer or delegated authorities.
		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn restore(
			origin: OriginFor<T>,
			stream_id: StreamIdOf,
			_authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream_details.revoked, Error::<T>::StreamNotRevoked);

			// TODO: Check if the authorization is valid
			ensure!(stream_details.creator == creator, Error::<T>::UnauthorizedOperation);

			<Streams<T>>::insert(
				&stream_id,
				StreamEntryOf::<T> { revoked: false, ..stream_details },
			);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				creator.clone(),
				StreamCommitActionOf::Restore,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Restore { identifier: stream_id, author: creator });

			Ok(())
		}
		/// Remove a stream from the chain using space identities.This operation can only be performed by
		/// the stream issuer or delegated authorities.
		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn remove(
			origin: OriginFor<T>,
			stream_id: StreamIdOf,
			_authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(stream_details.revoked, Error::<T>::StreamNotRevoked);

			// TODO: Check if the authorization is valid
			ensure!(stream_details.creator == creator, Error::<T>::UnauthorizedOperation);

			<Streams<T>>::remove(&stream_id);

			Self::update_commit(
				&stream_id,
				stream_details.digest,
				creator.clone(),
				StreamCommitActionOf::Remove,
			)
			.map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Remove { identifier: stream_id, author: creator });

			Ok(())
		}

		/// Adds stream digest information. This operation can only be performed by
		/// the stream issuer or delegated authorities.
		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn digest(
			origin: OriginFor<T>,
			stream_id: StreamIdOf,
			stream_digest: StreamDigestOf<T>,
			_authorization_id: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let stream_details = <Streams<T>>::get(&stream_id).ok_or(Error::<T>::StreamNotFound)?;
			ensure!(!stream_details.revoked, Error::<T>::RevokedStream);

			// TODO: Check if the authorization is valid
			ensure!(stream_details.creator == creator, Error::<T>::UnauthorizedOperation);

			<StreamDigests<T>>::insert(&stream_digest, &stream_id);

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
	pub fn update_commit(
		tx_stream: &StreamIdOf,
		tx_digest: StreamDigestOf<T>,
		proposer: CreatorIdOf<T>,
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
