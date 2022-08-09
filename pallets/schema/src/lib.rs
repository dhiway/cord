// This file is part of CORD – https://cord.network

// Copyright (C&) 2019-2022 Dhiway Networks Pvt. Ltd.
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

pub use cord_primitives::{
	ss58identifier, CidOf, IdentifierOf, StatusOf, VersionOf, SCHEMA_IDENTIFIER_PREFIX,
};
use frame_support::{ensure, storage::types::StorageMap, BoundedVec};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str, vec::Vec};

pub mod schemas;
pub mod weights;

pub use crate::schemas::*;
use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the schema.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a cord signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_space::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		/// The maximum number of delegates for a schema.
		#[pallet::constant]
		type MaxSchemaDelegates: Get<u32>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// schemas stored on chain.
	/// It maps from a schema identifier to its details.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Schemas<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SchemaDetails<T>, OptionQuery>;

	/// schema identifiers stored on chain.
	/// It maps from a schema identifier to hash.
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type SchemaHashes<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	/// schema delegations stored on chain.
	/// It maps from an identifier to a vector of delegates.
	#[pallet::storage]
	#[pallet::storage_prefix = "Delegates"]
	pub(super) type Delegations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		BoundedVec<CordAccountOf<T>, T::MaxSchemaDelegates>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, version, controller\]
		Create { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A schema status has been changed.
		/// \[schema identifier, controller\]
		Revoke { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// Schema delegates has been added.
		/// \[schema identifier,  controller\]
		AddDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// Schema delegates has been removed.
		/// \[schema identifier,  controller\]
		RemoveDelegates { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Schema identifier is not unique
		SchemaAlreadyAnchored,
		/// Schema not found
		SchemaNotFound,
		/// Schema is revoked
		SchemaRevoked,
		/// When the author is not the controller or delegate.
		UnauthorizedOperation,
		// Maximum Number of delegates reached.
		TooManyDelegates,
		// Maximum Number of delegates reached.
		TooManyDelegatesToRemove,
		// When the author is not the controller or delegate
		UnauthorizedDelegation,
		// Invalid Schema Identifier
		InvalidSchemaIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		// Invalid Schema Identifier Prefix
		InvalidIdentifierPrefix,
		// Schema is not part of the Space
		SchemaSpaceMismatch,
		// Invalid creator signature
		InvalidSignature,
		// Invalid transaction hash
		InvalidTransactionHash,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add schema authorisations (delegation).
		///
		/// This transaction can only be performed by the schema controller or
		/// delegates.
		///
		/// * origin: the identity of the schema anchor.
		/// * schema: delegation schema details.
		/// * delegates: authorised identities to add.
		/// * tx_signature: transaction author signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::authorise())]
		pub fn authorise(
			origin: OriginFor<T>,
			auth: SchemaParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SchemaHashes<T>>::contains_key(&auth.schema.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&auth.schema.digest).encode()[..], &auth.schema.author),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&auth.identifier, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details =
				<Schemas<T>>::get(&auth.identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(space) = auth.schema.space {
				ensure!(
					schema_details.schema.space == Some(space.clone()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.schema.author != auth.schema.author {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						&space,
						auth.schema.author.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					schema_details.schema.author == auth.schema.author,
					Error::<T>::UnauthorizedDelegation
				);
			}

			Delegations::<T>::try_mutate(auth.identifier.clone(), |ref mut delegation| {
				ensure!(
					delegation.len() + delegates.len() <= T::MaxSchemaDelegates::get() as usize,
					Error::<T>::TooManyDelegates
				);
				for delegate in delegates {
					delegation
						.try_push(delegate)
						.expect("delegates length is less than T::MaxSchemaDelegates; qed");
				}

				<SchemaHashes<T>>::insert(&auth.schema.digest, &auth.identifier);

				Self::deposit_event(Event::AddDelegates {
					identifier: auth.identifier,
					digest: auth.schema.digest,
					author: auth.schema.author,
				});
				Ok(())
			})
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
		#[pallet::weight(<T as pallet::Config>::WeightInfo::deauthorise())]
		pub fn deauthorise(
			origin: OriginFor<T>,
			deauth: SchemaParams<T>,
			delegates: Vec<CordAccountOf<T>>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SchemaHashes<T>>::contains_key(&deauth.schema.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&deauth.schema.digest).encode()[..], &deauth.schema.author),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&deauth.identifier, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details =
				<Schemas<T>>::get(&deauth.identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(space) = deauth.schema.space {
				ensure!(
					schema_details.schema.space == Some(space.clone()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.schema.author != deauth.schema.author {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						&space,
						deauth.schema.author.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					schema_details.schema.author == deauth.schema.author,
					Error::<T>::UnauthorizedOperation
				);
			}

			Delegations::<T>::try_mutate(deauth.identifier.clone(), |ref mut schema_delegates| {
				ensure!(
					delegates.len() <= T::MaxSchemaDelegates::get() as usize,
					Error::<T>::TooManyDelegatesToRemove
				);
				for delegate in delegates {
					schema_delegates.retain(|x| x != &delegate);
				}

				<SchemaHashes<T>>::insert(&deauth.schema.digest, &deauth.identifier);

				Self::deposit_event(Event::RemoveDelegates {
					identifier: deauth.identifier,
					digest: deauth.schema.digest,
					author: deauth.schema.author,
				});
				Ok(())
			})
		}

		/// Create a new schema and associates with its identifier.
		///
		/// * origin: the identity of the schema anchor.
		/// * schema: details of the incoming schema stream.
		/// * tx_signature: transaction author signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			schema: SchemaType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;
			ensure!(
				tx_signature.verify(&(&schema.digest).encode()[..], &schema.author),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&schema.digest).encode()[..], SCHEMA_IDENTIFIER_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			if let Some(ref space) = schema.space {
				pallet_space::SpaceDetails::<T>::from_space_identities(
					space,
					schema.author.clone(),
				)
				.map_err(<pallet_space::Error<T>>::from)?;
			}

			<SchemaHashes<T>>::insert(&schema.digest, &identifier);

			<Schemas<T>>::insert(
				&identifier,
				SchemaDetails { schema: schema.clone(), revoked: false, metadata: false },
			);

			Self::deposit_event(Event::Create {
				identifier,
				digest: schema.digest,
				author: schema.author,
			});

			Ok(())
		}

		/// Revoke a Schema
		///
		///This transaction can only be performed by the schema controller or
		/// delegates
		///
		/// * origin: the identity of the schema controller.
		/// * rev: schema to be revoked.
		/// * tx_signature:  transaction author signature.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::revoke())]
		pub fn revoke(
			origin: OriginFor<T>,
			rev: SchemaParams<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<SchemaHashes<T>>::contains_key(&rev.schema.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&rev.schema.digest).encode()[..], &rev.schema.author),
				Error::<T>::InvalidSignature
			);

			ss58identifier::from_known_format(&rev.identifier, SCHEMA_IDENTIFIER_PREFIX)
				.map_err(|_| Error::<T>::InvalidSchemaIdentifier)?;

			let schema_details =
				<Schemas<T>>::get(&rev.identifier).ok_or(Error::<T>::SchemaNotFound)?;
			ensure!(!schema_details.revoked, Error::<T>::SchemaRevoked);

			if let Some(space) = rev.schema.space {
				ensure!(
					schema_details.schema.space == Some(space.clone()),
					Error::<T>::SchemaSpaceMismatch
				);

				if schema_details.schema.author != rev.schema.author {
					pallet_space::SpaceDetails::<T>::from_space_identities(
						&space,
						rev.schema.author.clone(),
					)
					.map_err(<pallet_space::Error<T>>::from)?;
				}
			} else {
				ensure!(
					schema_details.schema.author == rev.schema.author,
					Error::<T>::UnauthorizedOperation
				);
			}

			<SchemaHashes<T>>::insert(&rev.schema.digest, &rev.identifier);

			<Schemas<T>>::insert(
				&rev.identifier,
				SchemaDetails { revoked: true, ..schema_details },
			);
			Self::deposit_event(Event::Revoke {
				identifier: rev.identifier,
				digest: rev.schema.digest,
				author: rev.schema.author,
			});

			Ok(())
		}
	}
}
