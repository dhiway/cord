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
//! - **Schema:**: Schemas are templates used to guarantee the structure, and by extension the
//!   semantics, of the set of claims comprising a Stream/Verifiable Credential. A shared Schema
//!   allows all parties to reference data in a known way. An identifier can optionally link to a
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

use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::traits::UniqueSaturatedInto;

/// Extra Types for Schema
pub mod types;

pub use crate::{types::*, weights::WeightInfo};
use frame_support::ensure;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::{traits::Hash, SaturatedConversion};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Space Identifier
	pub type SpaceIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Hash of the schema.
	pub type SchemaHashOf<T> = <T as frame_system::Config>::Hash;
	/// Type of a CORD account.
	pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of a Schema creator.
	pub type SchemaCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
	/// Type for an input schema
	pub type InputSchemaOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedSchemaLength>;
	/// Type for a schema entry
	pub type SchemaEntryOf<T> =
		SchemaEntry<InputSchemaOf<T>, SchemaHashOf<T>, SchemaCreatorOf<T>, SpaceIdOf>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
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
		pub fn create(
			origin: OriginFor<T>,
			tx_schema: InputSchemaOf<T>,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			ensure!(tx_schema.len() > 0, Error::<T>::EmptyTransaction);
			ensure!(
				tx_schema.len() <= T::MaxEncodedSchemaLength::get() as usize,
				Error::<T>::MaxEncodedSchemaLimitExceeded
			);

			let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator,
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			// Id Digest = concat (H(<scale_encoded_schema_input>,
			// <<scale_encoded_space_identifier>, scale_encoded_creator_identifier>))
			let id_digest = <T as frame_system::Config>::Hashing::hash(
				&[&tx_schema.encode()[..], &space_id.encode()[..], &creator.encode()[..]].concat()
					[..],
			);

			let identifier = Ss58Identifier::create_identifier(
				&(id_digest).encode()[..],
				IdentifierType::Schema,
			)
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
					space: space_id,
				},
			);

			Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;

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

	/// Updates the global timeline with a new activity event for a schema.
	///
	/// An `EventEntryOf` struct is created, encapsulating the type of action
	/// (`tx_action`) and the `Timepoint` of the event, which is obtained by
	/// calling the `timepoint` function. This entry is then passed to the
	/// `update_timeline` function of the `identifier` pallet, which integrates
	/// it into the global timeline.
	///
	/// # Parameters
	/// - `tx_id`: The identifier of the schema that the activity pertains to.
	/// - `tx_action`: The type of action taken on the schema, encapsulated within `CallTypeOf`.
	///
	/// # Returns
	/// Returns `Ok(())` after successfully updating the timeline. If any errors
	/// occur within the `update_timeline` function, they are not captured here
	/// and the function will still return `Ok(())`.
	pub fn update_activity(tx_id: &SchemaIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ = IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Schema, tx_entry);
		Ok(())
	}

	/// Retrieves the current timepoint.
	///
	/// This function returns a `Timepoint` structure containing the current
	/// block number and extrinsic index. It is typically used in conjunction
	/// with `update_activity` to record when an event occurred.
	///
	/// # Returns
	/// - `Timepoint`: A structure containing the current block number and extrinsic index.
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
