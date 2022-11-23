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

//! # Rating Pallet
//!
//! The Rating palllet is used to anchor identifiers representing rating seeked
//! from the sellers. The pallet provides means of creating and removing of
//! identifier data on-chain.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ### Terminology
//!
//! - **Identifier:**: A unique persistent identifier representing a rating,
//!   controller, ratings and number of people who rated in last interval.
// !
// ! - **Issuer:**: The issuer is the person or organization that creates the
// !   rating, anchors the identifier on-chain and assigns it to a holder
// !   (person, organization, or thing). An example of an issuer would be a DMV
// !   which issues driver’s licenses.
// !
// ! - **Rating:**: Avg rating.
// !
// ! - **Quantity:**: Number of reviews.
// !
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! The dispatchable functions of the Rating pallet enable the steps needed for
//! entities to anchor, update, remove link identifiers change their role,
//! alongside some helper functions to get/set the holder delgates and digest
//! information for the identifier.
//!
//! - `create` - Create a new identifier for a given rating which is based on a
//!   Schema. The issuer can optionally provide a reference to an existing
//!   identifier that will be saved along with the identifier.
//! - `remove` - Remove an existing identifier and associated on-chain data.The
//!   remover must be either the creator of the identifier being revoked or an
//!   entity that in the delegation tree is an ancestor of the issuer, i.e., it
//!   was either the delegator of the issuer or an ancestor thereof.
//!
//! ## Related Modules
//!
//! - [Meta](../pallet_meta/index.html): Used to manage data blobs attached to
//!   an identifier. Optional, but usefull for crededntials with public data.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![warn(unused_crate_dependencies)]

use cord_primitives::{ss58identifier, IdentifierOf, RatingEntityOf, StatusOf, RATING_PREFIX};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_std::{prelude::Clone, str};

pub mod ratings;
pub mod weights;

pub use crate::ratings::*;

pub use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Hash of the rating.
	pub type HashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	/// Type for a signature.
	pub type SignatureOf<T> = <T as Config>::Signature;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type EnsureOrigin: EnsureOrigin<
			Success = CordAccountOf<Self>,
			<Self as frame_system::Config>::Origin,
		>;
		type ForceOrigin: EnsureOrigin<Self::Origin>;
		/// The signature of the rating issuer.
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		/// The identity of the rating issuer.
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// rating identifiers stored on chain.
	/// It maps from an identifier to its details. Chain will only store the
	/// last updated state of the data.
	#[pallet::storage]
	#[pallet::storage_prefix = "Identifiers"]
	pub type Ratings<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, RatingDetails<T>, OptionQuery>;

	/// rating hashes stored on chain.
	/// It maps from a rating hash to an identifier (resolve from hash).
	#[pallet::storage]
	#[pallet::storage_prefix = "Hashes"]
	pub type RatingHashes<T> =
		StorageMap<_, Blake2_128Concat, HashOf<T>, IdentifierOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new rating identifier has been created.
		/// \[rating identifier, rating hash, controller\]
		Anchor { identifier: IdentifierOf, digest: HashOf<T>, author: CordAccountOf<T> },
		/// A metedata entry has been added to the identifier.
		/// \[rating identifier, controller\]
		MetadataSet { identifier: IdentifierOf, controller: CordAccountOf<T> },
		/// An identifier metadata entry has been cleared.
		/// \[rating identifier, controller\]
		MetadataCleared { identifier: IdentifierOf, controller: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Rating idenfier is not unique
		RatingAlreadyAnchored,
		/// Rating idenfier not found
		RatingNotFound,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		// Invalid creator signature
		InvalidSignature,
		//Rating hash is not unique
		HashAlreadyAnchored,
		// Expired Tx Signature
		ExpiredSignature,
		// Invalid Rating Identifier
		InvalidRatingIdentifier,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		//Rating digest is not unique
		DigestHashAlreadyAnchored,
		// Invalid transaction hash
		InvalidTransactionHash,
		// Metadata limit exceeded
		MetadataLimitExceeded,
		// Metadata already set for the entry
		MetadataAlreadySet,
		// Metadata not found for the entry
		MetadataNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new rating identifier and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// * origin: the identity of the Transaction Author. Transaction author
		///   pays the transaction fees and the author identity can be different
		///   from the rating issuer
		/// * tx_rating: the incoming rating. Identifier is generated from the
		///   genesis digest (hash) provided as part of the details
		/// * tx_signature: signature of the issuer aganist the rating genesis
		///   digest (hash).
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			tx_rating: RatingType<T>,
			tx_signature: SignatureOf<T>,
		) -> DispatchResult {
			<T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				!<RatingHashes<T>>::contains_key(&tx_rating.digest),
				Error::<T>::InvalidTransactionHash
			);

			ensure!(
				tx_signature.verify(&(&tx_rating.digest).encode()[..], &tx_rating.controller),
				Error::<T>::InvalidSignature
			);

			let identifier: IdentifierOf = BoundedVec::<u8, ConstU32<48>>::try_from(
				ss58identifier::generate(&(&tx_rating.digest).encode()[..], RATING_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Ratings<T>>::contains_key(&identifier), Error::<T>::RatingAlreadyAnchored);

			<RatingHashes<T>>::insert(&tx_rating.digest, &identifier);

			<Ratings<T>>::insert(
				&identifier,
				RatingDetails { rating: tx_rating.clone(), meta: false },
			);

			Self::deposit_event(Event::Anchor {
				identifier,
				digest: tx_rating.digest,
				author: tx_rating.controller,
			});

			Ok(())
		}
	}
}
