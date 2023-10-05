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
pub use pallet::*;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

/// Test module for Schemas
#[cfg(test)]
pub mod tests;

/// Extra Types for Schema
pub mod types;

pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_support::ensure;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::curi::Ss58Identifier;
	pub use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{traits::Hash, SaturatedConversion};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Hash of the schema.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a Schema creator.
	pub type SchemaCreatorOf<T> = <T as Config>::SchemaCreatorId;
	/// Type for an input schema
	pub type InputSchemaOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedSchemaLength>;
	/// Type for a schema entry
	pub type SchemaEntryOf<T> = SchemaEntry<
		InputSchemaOf<T>,
		SchemaHashOf<T>,
		<T as Config>::SchemaCreatorId,
		BlockNumberFor<T>,
	>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = Self::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, SchemaCreatorOf<Self>>;
		type SchemaCreatorId: Parameter + MaxEncodedLen;
		#[pallet::constant]
		type MaxEncodedSchemaLength: Get<u32>;
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// schemas stored on chain.
	/// It maps from a schema identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn schemas)]
	pub type Schemas<T> = StorageMap<_, Blake2_128Concat, SchemaIdOf, SchemaEntryOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, digest, author\]
		Created { identifier: SchemaIdOf, creator: SchemaCreatorOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Schema identifier is not unique.
		SchemaAlreadyAnchored,
		/// Schema identifier not found.
		SchemaNotFound,
		// Invalid Schema Identifier Length.
		InvalidIdentifierLength,
		/// The paying account was unable to pay the fees for creating a schema.
		UnableToPayFees,
		/// Creator DID information not found.
		CreatorNotFound,
		/// Schema limit exceeds the permitted size.
		MaxEncodedSchemaLimitExceeded,
		/// Empty transaction.
		EmptyTransaction,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new schema and associates with its identifier.
		/// `create` takes a `InputSchemaOf<T>` and returns a `DispatchResult`
		///
		/// Arguments:
		///
		/// * `origin`: The origin of the transaction.
		/// * `tx_schema`: The schema that is being anchored.
		///
		/// Returns:
		///
		/// DispatchResult
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create(tx_schema.len().saturated_into()))]
		pub fn create(origin: OriginFor<T>, tx_schema: InputSchemaOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_schema.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_schema.len() <= T::MaxEncodedSchemaLength::get() as usize,
				Error::<T>::MaxEncodedSchemaLimitExceeded
			);

			// Id Digest = concat (H(<scale_encoded_schema_input>,
			// <scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&tx_schema.encode()[..], &creator.encode()[..]].concat()[..],
			);

			let identifier = Ss58Identifier::to_schema_id(&(id_digest).encode()[..])
				.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_schema[..]);
			let block_number = frame_system::Pallet::<T>::block_number();

			log::debug!(
				"Schema created with identifier: {:?}, schema: {:?} digest: {:?}, creator:
			{:?}, block_number: {:?}",
				identifier,
				tx_schema,
				digest,
				creator,
				block_number
			);

			<Schemas<T>>::insert(
				&identifier,
				SchemaEntryOf::<T> {
					schema: tx_schema,
					digest,
					creator: creator.clone(),
					created_at: block_number,
				},
			);

			Self::deposit_event(Event::Created { identifier, creator });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// `ensure!` is a macro that takes a boolean expression and an error type.
	/// If the expression is false, it returns the error
	///
	/// Arguments:
	///
	/// * `tx_ident`: The identifier of the transaction to check.
	///
	/// Returns:
	///
	/// A Result<(), Error<T>>
	pub fn is_valid(tx_ident: &SchemaIdOf) -> Result<(), Error<T>> {
		ensure!(<Schemas<T>>::contains_key(tx_ident), Error::<T>::SchemaNotFound);
		Ok(())
	}
}
