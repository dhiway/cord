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
//!   controller, holder, links (space, schema, identifier) and it's current
//!   status.
// ! - **Holder:**: The holder is the person or organization to whom the
// !   stream/credentials is issued to. The crytographic identity of the holder
// !   is attached to the identifier. The holder keeps the stream as a
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
// ! - **Space:**: Spaces allows grouping of identifiers with control
// !   delegations. Spaces can also be used to support various registry use-cases
// !   as the registries usually are a collection of data streams and
// !   identifiers. The space delegates can perform actions on behalf of the
// !   identifier owner. An example is a space linking all identifiers
// !   representing approved ecocystem applications and the associated delegates
// !   can perform addtion, revokation and removal operations.
// !
// ! - **Link:**: Link functionality is  used to attach an identifier with
// !   another valid identifier. This allows credential to be stacked and linked.
// !   This capality allows identifiers to be used to support dynamic additions
// !   according to the ecosystem governance rules. An example here would be of
// !   an identifier representing an immunization card and various identifiers
// !   representing vaccinations details getting linked to it overtime.
// !
// ! - **Delegation:**: An stream identifier that is not issued by the issuer
// !   directly but via a (chain of) delegations which entitle the delegated
// !   issuer. The delegation hireachy is checked from the space and schema
// !   delegates the identifier is attached to. This could be an employe of a
// !   company which is authorized to sign and issue documents.
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
//! - `remove` - Remove an existing identifier and associated on-chain data.The
//!   remover must be either the creator of the identifier being revoked or an
//!   entity that in the delegation tree is an ancestor of the issuer, i.e., it
//!   was either the delegator of the issuer or an ancestor thereof.
//! - `council_remove` - Remove an existing identifier and associated on-chain
//!   data via on-chain governance.
//! - `digest` - Create a presention digest for the identifier. This feature can
//!   be used to support various presenation models used to represent a
//!   stream/credential in the digtal/physical world.
//! - `delegate` - Create a delegation hirearchy for the holder. This allows the
//!   credential to be shared with the delgates. An exampled of holder
//!   delegation is Gaurdianship
//! - `undelegate` - Remove delegates from the delegation hirearchy for the
//!   holder.
//!
//! ## Related Modules
//!
//! - [Space](../pallet_space/index.html): Used to manage Spaces (collections,
//!   registries) and delegates.
//! - [Schema](../pallet_schema/index.html): Used to manage schemas and
//!   delegates.
//! - [Meta](../pallet_meta/index.html): Used to manage data blobs attached to
//!   an identifier. Optional, but usefull for crededntials with public data.
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
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	/// Type of a creator identifier.
	///
	/// Registry Identifier
	pub type RegistryIdOf = IdentifierOf;
	/// Schema Identifier
	pub type SchemaIdOf = IdentifierOf;
	/// Hash of the registry.
	pub type StreamHashOf<T> = <T as frame_system::Config>::Hash;

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
	pub struct Pallet<T>(_);

	/// stream identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::storage_prefix = "streams"]
	pub type Streams<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, StreamEntryOf<T>, OptionQuery>;

	/// stream hashes stored on chain.
	/// It maps from a stream hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::storage_prefix = "stream_digests"]
	pub type StreamDigests<T> =
		StorageMap<_, Blake2_128Concat, StreamDigestOf<T>, IdentifierOf, OptionQuery>;

	/// stream commits stored on chain.
	/// It maps from an identifier to a vector of commits.
	#[pallet::storage]
	#[pallet::storage_prefix = "stream_commits"]
	pub(super) type StreamCommits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<StreamCommitsOf<T>, T::MaxStreamCommits>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new stream identifier has been created.
		/// \[stream identifier, stream hash, controller\]
		Create { identifier: IdentifierOf, digest: StreamDigestOf<T>, author: CreatorIdOf<T> },
		// /// A stream identifier has been updated.
		// /// \[stream identifier, hash, controller\]
		// Update { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		// /// A stream digest has been added to the identifier.
		// /// \[stream identifier, digest, controller\]
		// Digest { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		// /// A stream identifier status has been changed.
		// /// \[stream identifier, digest, controller\]
		// Revoke { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		// /// A stream identifier has been removed.
		// /// \[stream identifier, digest, controller\]
		// Remove { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		// /// A stream identifier has been removed by the council.
		// /// \[stream identifier\]
		// CouncilRemove { identifier: IdentifierOf },
		// /// A metedata entry has been added to the identifier.
		// /// \[stream identifier, controller\]
		// MetadataSet { identifier: IdentifierOf, controller: CordAccountOf<T> },
		// /// An identifier metadata entry has been cleared.
		// /// \[stream identifier, controller\]
		// MetadataCleared { identifier: IdentifierOf, controller: CordAccountOf<T> },
		// /// Delegates has been added to the identifier.
		// /// \[stream identifier, digest, delegator\]
		// AddDelegates { identifier: IdentifierOf, digest: HashOf<T>, delegator: CordAccountOf<T> },
		// /// Stream identifier delegates has been removed.
		// /// \[stream identifier, digest, delegator\]
		// RemoveDelegates { identifier: IdentifierOf, digest: HashOf<T>, delegator: CordAccountOf<T> },
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
		// More than the maximum mumber of delegates.
		TooManyDelegatesToRemove,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new stream identifier and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// * origin: the identity of the Transaction Author. Transaction author
		///   pays the transaction fees and the author identity can be different
		///   from the stream issuer
		/// * tx_stream: the incoming stream. Identifier is generated from the
		///   genesis digest (hash) provided as part of the details
		/// * tx_signature: signature of the issuer aganist the stream genesis
		///   digest (hash).
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn create(origin: OriginFor<T>, stream_digest: StreamDigestOf<T>) -> DispatchResult {
			let source = <T as Config>::EnsureOrigin::ensure_origin(origin)?;
			let creator = source.subject();
			// let author = source.sender();

			ensure!(
				!<StreamDigests<T>>::contains_key(&stream_digest),
				Error::<T>::InvalidTransactionHash
			);

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(&stream_digest).encode()[..], STREAM_PREFIX)
					.into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Streams<T>>::contains_key(&identifier), Error::<T>::StreamAlreadyAnchored);

			// if let Some(ref space) = tx_stream.space {
			// 	pallet_space::SpaceDetails::<T>::from_space_identities(
			// 		space,
			// 		tx_stream.controller.clone(),
			// 	)
			// 	.map_err(<pallet_space::Error<T>>::from)?;
			// } else if let Some(ref schema) = tx_stream.schema {
			// 	pallet_schema::SchemaDetails::<T>::from_schema_identities(
			// 		schema,
			// 		tx_stream.controller.clone(),
			// 	)
			// 	.map_err(<pallet_schema::Error<T>>::from)?;
			// }
			// if let Some(ref link) = tx_stream.link {
			// 	let link_details =
			// 		<Streams<T>>::get(&link).ok_or(Error::<T>::StreamLinkNotFound)?;
			// 	ensure!(!link_details.revoked, Error::<T>::StreamLinkRevoked);
			// }

			<StreamDigests<T>>::insert(&stream_digest, &identifier);

			<Streams<T>>::insert(
				&identifier,
				StreamEntryOf::<T> {
					digest: stream_digest,
					creator: creator.clone(),
					// author: author.clone(),
					schema: None,
					linked: None,
					swarm: None,
					revoked: false,
					counter: 1u64,
				},
			);

			Self::deposit_event(Event::Create {
				identifier,
				digest: stream_digest,
				author: creator,
			});

			Ok(())
		}
		// /// Updates the stream identifier with a new digest. The updated digest
		// /// represents the changes a stream might have undergone. This operation
		// /// can only be performed by the stream issuer or delegates.
		// ///
		// /// * origin: the identity of the Transaction Author. Transaction author
		// ///   pays the transaction fees and the author identity can be different
		// ///   from the stream issuer.
		// /// * tx_stream: the incoming stream. Only updates the stream digest
		// ///   (hash).
		// /// * tx_signature: signature of the issuer/delegate  aganist the stream
		// ///   digest (hash).
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::update())]
		// pub fn update(
		// 	origin: OriginFor<T>,
		// 	tx_stream: StreamParams<T>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;
		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
		// 		Error::<T>::InvalidTransactionHash
		// 	);

		// 	ensure!(
		// 		tx_signature
		// 			.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	let stream_details =
		// 		<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;
		// 	ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		// 	if let Some(ref space) = tx_stream.stream.space {
		// 		ensure!(
		// 			stream_details.stream.space == Some(space.clone()),
		// 			Error::<T>::StreamSpaceMismatch
		// 		);
		// 		pallet_space::SpaceDetails::<T>::from_space_identities(
		// 			space,
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_space::Error<T>>::from)?;
		// 	} else if let Some(ref schema) = tx_stream.stream.schema {
		// 		pallet_schema::SchemaDetails::from_schema_delegates(
		// 			schema,
		// 			stream_details.stream.controller.clone(),
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_schema::Error<T>>::from)?;
		// 	} else {
		// 		ensure!(
		// 			stream_details.stream.controller == tx_stream.stream.controller,
		// 			Error::<T>::UnauthorizedOperation
		// 		);
		// 	}

		// 	<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

		// 	<Streams<T>>::insert(
		// 		&tx_stream.identifier,
		// 		StreamDetails {
		// 			stream: {
		// 				StreamType { digest: tx_stream.stream.digest, ..stream_details.stream }
		// 			},
		// 			..stream_details
		// 		},
		// 	);

		// 	Self::deposit_event(Event::Update {
		// 		identifier: tx_stream.identifier,
		// 		digest: tx_stream.stream.digest,
		// 		author: tx_stream.stream.controller,
		// 	});

		// 	Ok(())
		// }
		// /// Revoke a stream identifier. This operation can only be performed by
		// /// the stream issuer or delegates.
		// ///
		// /// * origin: the identity of the Transaction Author. Transaction author
		// ///   pays the transaction fees and the author identity can be different
		// ///   from the stream issuer.
		// /// * tx_stream: the identifier to revoke along with a unique tx digest
		// ///   (hash).
		// /// * tx_signature: signature of the issuer/delegate aganist the tx
		// ///   digest (hash).
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::revoke())]
		// pub fn revoke(
		// 	origin: OriginFor<T>,
		// 	tx_stream: StreamParams<T>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;

		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
		// 		Error::<T>::InvalidTransactionHash
		// 	);

		// 	ensure!(
		// 		tx_signature
		// 			.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	let stream_details =
		// 		<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;
		// 	ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		// 	if let Some(ref space) = tx_stream.stream.space {
		// 		ensure!(
		// 			stream_details.stream.space == Some(space.clone()),
		// 			Error::<T>::StreamSpaceMismatch
		// 		);
		// 		pallet_space::SpaceDetails::<T>::from_space_identities(
		// 			space,
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_space::Error<T>>::from)?;
		// 	} else if let Some(ref schema) = tx_stream.stream.schema {
		// 		pallet_schema::SchemaDetails::from_schema_delegates(
		// 			schema,
		// 			stream_details.stream.controller.clone(),
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_schema::Error<T>>::from)?;
		// 	} else {
		// 		ensure!(
		// 			stream_details.stream.controller == tx_stream.stream.controller,
		// 			Error::<T>::UnauthorizedOperation
		// 		);
		// 	}

		// 	<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

		// 	<Streams<T>>::insert(
		// 		&tx_stream.identifier,
		// 		StreamDetails { revoked: true, ..stream_details },
		// 	);
		// 	Self::deposit_event(Event::Revoke {
		// 		identifier: tx_stream.identifier,
		// 		digest: tx_stream.stream.digest,
		// 		author: tx_stream.stream.controller,
		// 	});

		// 	Ok(())
		// }

		// ///  Remove a stream from the chain using space identities.
		// ///
		// /// * origin: the identity of the space origin.
		// /// * remove: the stream to remove.
		// /// * tx_signature: signature of the controller.
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		// pub fn remove(
		// 	origin: OriginFor<T>,
		// 	tx_stream: StreamParams<T>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;

		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&tx_stream.stream.digest),
		// 		Error::<T>::InvalidTransactionHash
		// 	);

		// 	ensure!(
		// 		tx_signature
		// 			.verify(&(&tx_stream.stream.digest).encode()[..], &tx_stream.stream.controller),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&tx_stream.identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	let stream_details =
		// 		<Streams<T>>::get(&tx_stream.identifier).ok_or(Error::<T>::StreamNotFound)?;

		// 	if let Some(ref space) = tx_stream.stream.space {
		// 		ensure!(
		// 			stream_details.stream.space == Some(space.clone()),
		// 			Error::<T>::StreamSpaceMismatch
		// 		);
		// 		pallet_space::SpaceDetails::<T>::from_space_identities(
		// 			space,
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_space::Error<T>>::from)?;
		// 	} else if let Some(ref schema) = tx_stream.stream.schema {
		// 		pallet_schema::SchemaDetails::from_schema_delegates(
		// 			schema,
		// 			stream_details.stream.controller,
		// 			tx_stream.stream.controller.clone(),
		// 		)
		// 		.map_err(<pallet_schema::Error<T>>::from)?;
		// 	} else {
		// 		ensure!(
		// 			stream_details.stream.controller == tx_stream.stream.controller,
		// 			Error::<T>::UnauthorizedOperation
		// 		);
		// 	}

		// 	<StreamHashes<T>>::insert(&tx_stream.stream.digest, &tx_stream.identifier);

		// 	<Streams<T>>::remove(&tx_stream.identifier);
		// 	Self::deposit_event(Event::Remove {
		// 		identifier: tx_stream.identifier,
		// 		digest: tx_stream.stream.digest,
		// 		author: tx_stream.stream.controller,
		// 	});

		// 	Ok(())
		// }

		// ///  Remove a stream from the chain using council origin.
		// ///
		// /// * origin: the identity of the council origin.
		// /// * identifier: unique identifier of the incoming stream.
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::council_remove())]
		// pub fn council_remove(origin: OriginFor<T>, identifier: IdentifierOf) -> DispatchResult {
		// 	<T as Config>::ForceOrigin::ensure_origin(origin)?;
		// 	ss58identifier::from_known_format(&identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;
		// 	<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;

		// 	<Streams<T>>::remove(&identifier);
		// 	Self::deposit_event(Event::CouncilRemove { identifier });

		// 	Ok(())
		// }
		// /// Adds stream digest information.
		// ///
		// /// * origin: the identity of the anchor.
		// /// * identifier: unique identifier of the incoming stream.
		// /// * creator: controller or delegate of the stream.
		// /// * digest_hash: hash of the incoming stream.
		// /// * tx_signature: signature of the controller.
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::digest())]
		// pub fn digest(
		// 	origin: OriginFor<T>,
		// 	identifier: IdentifierOf,
		// 	author: CordAccountOf<T>,
		// 	digest_hash: HashOf<T>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;
		// 	ensure!(
		// 		tx_signature.verify(&(&digest_hash).encode()[..], &author),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&digest_hash),
		// 		Error::<T>::DigestHashAlreadyAnchored
		// 	);

		// 	let stream_details =
		// 		<Streams<T>>::get(&identifier).ok_or(Error::<T>::StreamNotFound)?;
		// 	ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		// 	if let Some(ref space) = stream_details.stream.space {
		// 		pallet_space::SpaceDetails::<T>::from_space_identities(space, author.clone())
		// 			.map_err(<pallet_space::Error<T>>::from)?;
		// 	} else if let Some(ref schema) = stream_details.stream.schema {
		// 		pallet_schema::SchemaDetails::from_schema_delegates(
		// 			schema,
		// 			stream_details.stream.controller.clone(),
		// 			author.clone(),
		// 		)
		// 		.map_err(<pallet_schema::Error<T>>::from)?;
		// 	} else {
		// 		ensure!(
		// 			stream_details.stream.controller == author,
		// 			Error::<T>::UnauthorizedOperation
		// 		);
		// 	}

		// 	<StreamHashes<T>>::insert(&digest_hash, &identifier);

		// 	Self::deposit_event(Event::Digest { identifier, digest: digest_hash, author });

		// 	Ok(())
		// }
		// /// Add schema authorisations (delegation).
		// ///
		// /// This transaction can only be performed by the schema controller or
		// /// delegates.
		// ///
		// /// * origin: the identity of the schema anchor.
		// /// * auth: delegation schema details.
		// /// * delegates: authorised identities to add.
		// /// * tx_signature: transaction author signature.
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::delegate())]
		// pub fn delegate(
		// 	origin: OriginFor<T>,
		// 	tx_delegation: StreamDelegationParams<T>,
		// 	delegates: Vec<CordAccountOf<T>>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;

		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&tx_delegation.digest),
		// 		Error::<T>::InvalidTransactionHash
		// 	);
		// 	ensure!(
		// 		tx_signature
		// 			.verify(&(&tx_delegation.digest).encode()[..], &tx_delegation.delegator),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&tx_delegation.identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	let stream_details =
		// 		<Streams<T>>::get(&tx_delegation.identifier).ok_or(Error::<T>::StreamNotFound)?;
		// 	ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		// 	if stream_details.stream.holder != tx_delegation.delegator {
		// 		if let Some(ref space) = stream_details.stream.space {
		// 			pallet_space::SpaceDetails::<T>::from_space_identities(
		// 				space,
		// 				tx_delegation.delegator.clone(),
		// 			)
		// 			.map_err(<pallet_space::Error<T>>::from)?;
		// 		} else if let Some(ref schema) = stream_details.stream.schema {
		// 			pallet_schema::SchemaDetails::from_schema_delegates(
		// 				schema,
		// 				stream_details.stream.controller.clone(),
		// 				tx_delegation.delegator.clone(),
		// 			)
		// 			.map_err(<pallet_schema::Error<T>>::from)?;
		// 		} else {
		// 			ensure!(
		// 				stream_details.stream.controller == tx_delegation.delegator,
		// 				Error::<T>::UnauthorizedOperation
		// 			);
		// 		}
		// 	}

		// 	StreamDelegations::<T>::try_mutate(
		// 		tx_delegation.identifier.clone(),
		// 		|ref mut delegation| {
		// 			ensure!(
		// 				delegation.len() + delegates.len() <= T::MaxSchemaDelegates::get() as usize,
		// 				Error::<T>::TooManyDelegates
		// 			);
		// 			for delegate in delegates {
		// 				delegation
		// 					.try_push(delegate)
		// 					.expect("delegates length is less than T::MaxSchemaDelegates; qed");
		// 			}

		// 			<StreamHashes<T>>::insert(&tx_delegation.digest, &tx_delegation.identifier);
		// 			<Streams<T>>::insert(
		// 				&tx_delegation.identifier,
		// 				StreamDetails { delegates: true, ..stream_details },
		// 			);

		// 			Self::deposit_event(Event::AddDelegates {
		// 				identifier: tx_delegation.identifier,
		// 				digest: tx_delegation.digest,
		// 				delegator: tx_delegation.delegator,
		// 			});
		// 			Ok(())
		// 		},
		// 	)
		// }
		// /// Remove schema authorisations (delegation).
		// ///
		// /// This transaction can only be performed by the schema controller or
		// /// delegates.
		// ///
		// /// * origin: the identity of the schema anchor.
		// /// * schema: delegation schema details.
		// /// * delegates: identities to be removed.
		// /// * tx_signature: transaction author signature.
		// #[pallet::weight(<T as pallet::Config>::WeightInfo::undelegate())]
		// pub fn undelegate(
		// 	origin: OriginFor<T>,
		// 	tx_delegation: StreamDelegationParams<T>,
		// 	delegates: Vec<CordAccountOf<T>>,
		// 	tx_signature: SignatureOf<T>,
		// ) -> DispatchResult {
		// 	<T as Config>::EnsureOrigin::ensure_origin(origin)?;

		// 	ensure!(
		// 		!<StreamHashes<T>>::contains_key(&tx_delegation.digest),
		// 		Error::<T>::InvalidTransactionHash
		// 	);
		// 	ensure!(
		// 		tx_signature
		// 			.verify(&(&tx_delegation.digest).encode()[..], &tx_delegation.delegator),
		// 		Error::<T>::InvalidSignature
		// 	);

		// 	ss58identifier::from_known_format(&tx_delegation.identifier, STREAM_PREFIX)
		// 		.map_err(|_| Error::<T>::InvalidStreamIdentifier)?;

		// 	let stream_details =
		// 		<Streams<T>>::get(&tx_delegation.identifier).ok_or(Error::<T>::StreamNotFound)?;
		// 	ensure!(!stream_details.revoked, Error::<T>::StreamRevoked);

		// 	if stream_details.stream.holder != tx_delegation.delegator {
		// 		if let Some(ref space) = stream_details.stream.space {
		// 			pallet_space::SpaceDetails::<T>::from_space_identities(
		// 				space,
		// 				tx_delegation.delegator.clone(),
		// 			)
		// 			.map_err(<pallet_space::Error<T>>::from)?;
		// 		} else if let Some(ref schema) = stream_details.stream.schema {
		// 			pallet_schema::SchemaDetails::from_schema_delegates(
		// 				schema,
		// 				stream_details.stream.controller.clone(),
		// 				tx_delegation.delegator.clone(),
		// 			)
		// 			.map_err(<pallet_schema::Error<T>>::from)?;
		// 		} else {
		// 			ensure!(
		// 				stream_details.stream.controller == tx_delegation.delegator,
		// 				Error::<T>::UnauthorizedOperation
		// 			);
		// 		}
		// 	}

		// 	StreamDelegations::<T>::try_mutate(
		// 		tx_delegation.identifier.clone(),
		// 		|ref mut delegation| {
		// 			ensure!(
		// 				delegates.len() <= T::MaxSchemaDelegates::get() as usize,
		// 				Error::<T>::TooManyDelegatesToRemove
		// 			);
		// 			for delegate in delegates {
		// 				delegation.retain(|x| x != &delegate);
		// 			}

		// 			if delegation.is_empty() {
		// 				<Streams<T>>::insert(
		// 					&tx_delegation.identifier,
		// 					StreamDetails { delegates: false, ..stream_details },
		// 				);
		// 			}

		// 			<StreamHashes<T>>::insert(&tx_delegation.digest, &tx_delegation.identifier);

		// 			Self::deposit_event(Event::RemoveDelegates {
		// 				identifier: tx_delegation.identifier,
		// 				digest: tx_delegation.digest,
		// 				delegator: tx_delegation.delegator,
		// 			});
		// 			Ok(())
		// 		},
		// 	)
		// }
	}
}
