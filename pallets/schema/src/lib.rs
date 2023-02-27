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
pub use crate::{pallet::*, types::*, weights::WeightInfo};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{ss58identifier, IdentifierOf, SCHEMA_PREFIX};
	pub use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Schema Identifier
	pub type SchemaIdOf = IdentifierOf;
	/// Hash of the schema.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;

	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a Schema creator.
	pub type SchemaCreatorOf<T> = <T as Config>::SchemaCreatorId;

	pub type InputSchemaOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedSchemaLength>;
	pub type SchemaEntryOf<T> =
		SchemaEntry<InputSchemaOf<T>, SchemaHashOf<T>, SchemaCreatorOf<T>, BlockNumberFor<T>>;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new schema has been created.
		/// \[schema identifier, digest, author\]
		Created { identifier: IdentifierOf, creator: SchemaCreatorOf<T> },
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
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::create())]
		pub fn create(origin: OriginFor<T>, tx_schema: InputSchemaOf<T>) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_schema.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_schema.len() <= T::MaxEncodedSchemaLength::get() as usize,
				Error::<T>::MaxEncodedSchemaLimitExceeded
			);

			let digest = <T as frame_system::Config>::Hashing::hash(&tx_schema[..]);

			let identifier = IdentifierOf::try_from(
				ss58identifier::generate(&(digest).encode()[..], SCHEMA_PREFIX).into_bytes(),
			)
			.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Schemas<T>>::contains_key(&identifier), Error::<T>::SchemaAlreadyAnchored);

			<Schemas<T>>::insert(
				&identifier,
				SchemaEntryOf::<T> {
					schema: tx_schema,
					digest,
					creator: creator.clone(),
					created_at: Self::timepoint(),
				},
			);

			Self::deposit_event(Event::Created { identifier, creator });

			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		/// The current `Timepoint`.
		pub fn timepoint() -> Timepoint<T::BlockNumber> {
			Timepoint {
				height: frame_system::Pallet::<T>::block_number(),
				index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
			}
		}
	}
}
