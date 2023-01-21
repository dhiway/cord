// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 Dhiway Networks Pvt. Ltd.
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

//! # Schema Pallet
//!
//! A pallet which enables users to generate Schema Identifier,
//! store the Schema hash (blake2b as hex string) on chain and
//!  associate it with their account id.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ### Terminology
//!
//! - **Schema:**: Schemas are templates used to guarantee the structure, and by
//!   extension the semantics, of the set of claims comprising a
//!   Stream/Verifiable Credential. A shared Schema allows all parties to
//!   reference data in a known way. An identifier can optionally link to a
//!   valid schema identifier.
//!
//! ## Assumptions
//!
//! - The Schema hash was created using CORD SDK.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// Test module for Schemas
#[cfg(test)]
pub mod tests;

pub mod types;
pub use crate::{types::*, weights::WeightInfo};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{ss58identifier, IdentifierOf, SCHEMA_PREFIX};
	pub use cord_utilities::signature::{SignatureVerificationError, VerifySignature};
	use frame_support::{
		pallet_prelude::*,
		traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{traits::Saturating, SaturatedConversion};
	use sp_std::vec::Vec;

	use sp_std::boxed::Box;
	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Hash of the schema.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub(crate) type CordAccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a Schema creator.
	pub type SchemaCreatorOf<T> = <T as Config>::SchemaCreatorId;
	/// Type of a signature verification operation over the creator details.
	pub type CreatorSignatureVerificationOf<T> = <T as Config>::CreatorSignatureVerification;
	/// Type of the DID signature that the creator generates.
	pub type CreatorSignatureTypeOf<T> =
		<CreatorSignatureVerificationOf<T> as VerifySignature>::Signature;

	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type InputSchemaMetatOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedMetaLength>;

	pub type InputSchemaOf<T> = SchemaInput<
		SchemaHashOf<T>,
		SchemaCreatorOf<T>,
		CreatorSignatureTypeOf<T>,
		InputSchemaMetatOf<T>,
	>;

	pub type SchemaEntryOf<T> =
		SchemaEntry<SchemaHashOf<T>, SchemaCreatorOf<T>, InputSchemaMetatOf<T>, BlockNumberFor<T>>;

	pub(crate) type BalanceOf<T> =
		<<T as Config>::Currency as Currency<CordAccountIdOf<T>>>::Balance;

	type NegativeImbalanceOf<T> =
		<<T as Config>::Currency as Currency<CordAccountIdOf<T>>>::NegativeImbalance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = CordAccountIdOf<Self>,
		>;
		type Signature: Parameter;
		type CreatorSignatureVerification: VerifySignature<
			SignerId = Self::SchemaCreatorId,
			Payload = Vec<u8>,
			Signature = Self::Signature,
		>;
		type SchemaCreatorId: Parameter + MaxEncodedLen;
		type Currency: Currency<CordAccountIdOf<Self>>;
		type SchemaFee: Get<BalanceOf<Self>>;
		type FeeCollector: OnUnbalanced<NegativeImbalanceOf<Self>>;
		#[pallet::constant]
		type MaxSignatureByteLength: Get<u16>;
		#[pallet::constant]
		type MaxEncodedMetaLength: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// schemas stored on chain.
	/// It maps from a schema identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, SchemaEntryOf<T>, OptionQuery>;

	/// schema identifiers stored on chain.
	/// It maps from a schema hash to identifier.
	#[pallet::storage]
	#[pallet::getter(fn schema_hashes)]
	pub type SchemaHashes<T> =
		StorageMap<_, Blake2_128Concat, SchemaHashOf<T>, IdentifierOf, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, digest, author\]
		Created { identifier: IdentifierOf, digest: SchemaHashOf<T>, author: CordAccountIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Schema identifier is not unique
		SchemaAlreadyAnchored,
		/// Schema identifier not found
		SchemaNotFound,
		// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		/// The paying account was unable to pay the fees for creating a schema.
		UnableToPayFees,
		/// Invalid creator signature
		InvalidSignature,
		/// Creator DID information not found
		CreatorNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema and associates with its identifier.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create(tx_schema
				.meta
				.as_ref()
				.map(|ac| ac.len().saturated_into::<u32>())
				.unwrap_or(0)))]
		pub fn create(origin: OriginFor<T>, tx_schema: Box<InputSchemaOf<T>>) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			// Check the free balance before we do any heavy computation
			let balance = <T::Currency as Currency<CordAccountIdOf<T>>>::free_balance(&author);
			<T::Currency as Currency<CordAccountIdOf<T>>>::ensure_can_withdraw(
				&author,
				T::SchemaFee::get(),
				WithdrawReasons::FEE,
				balance.saturating_sub(T::SchemaFee::get()),
			)?;

			let SchemaInput { digest, controller, signature, meta } = *tx_schema.clone();

			// Verify that the hash root signature is correct.
			CreatorSignatureVerificationOf::<T>::verify_authentication_signature(
				&controller,
				&digest.encode(),
				&signature,
			)
			.map_err(|err| match err {
				SignatureVerificationError::SignerInformationNotPresent => {
					Error::<T>::CreatorNotFound
				},
				SignatureVerificationError::SignatureInvalid => Error::<T>::InvalidSignature,
			})?;

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(digest).encode()[..], SCHEMA_PREFIX).into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			// Collect the fees.
			let imbalance = <T::Currency as Currency<CordAccountIdOf<T>>>::withdraw(
				&author,
				T::SchemaFee::get(),
				WithdrawReasons::FEE,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::UnableToPayFees)?;

			T::FeeCollector::on_unbalanced(imbalance);

			let block_number = frame_system::Pallet::<T>::block_number();

			<SchemaHashes<T>>::insert(&digest, &identifier);
			<Schemas<T>>::insert(
				&identifier,
				SchemaEntryOf::<T> { digest, controller, meta, block_number },
			);

			Self::deposit_event(Event::Created { identifier, digest, author });

			Ok(())
		}
	}
}
