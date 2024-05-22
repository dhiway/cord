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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod types;

pub use crate::{pallet::*, types::*};
use frame_support::{ensure, traits::Get};
use frame_system::WeightInfo;
use identifier::types::Timepoint;
use pallet_chain_space::AuthorizationIdOf;
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{CountOf, RatingOf};
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::BoundedVec;
	use sp_std::{prelude::Clone, str};

	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	// Type of a witness creator identifier.
	pub type WitnessCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;

	// Type of witnesses
	pub type WitnessesOf<T> = BoundedVec<WitnessCreatorOf<T>, <T as Config>::MaxWitnessCount>;

	// Type of comment from witness
	pub type CommentOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	// Type of a document identifier.
	pub type DocumentIdOf<T> = BoundedVec<u8, <T as Config>::MaxEncodedValueLength>;

	// Type of witness signers entry containing list of signers.
	pub type WitnessSignersEntryOf<T> = WitnessSignersEntry<WitnessesOf<T>, BlockNumberFor<T>>;

	// Type of witness entry containing witness & document detail.
	pub type WitnessEntryOf<T> =
		WitnessEntry<WitnessCreatorOf<T>, EntryHashOf<T>, WitnessStatusOf, BlockNumberFor<T>>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, WitnessCreatorOf<Self>>;
		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;

		#[pallet::constant]
		type MaxWitnessCount: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Witness entry identifiers with details stored on chain.
	#[pallet::storage]
	pub type Witness<T> =
		StorageMap<_, Blake2_128Concat, DocumentIdOf<T>, WitnessEntryOf<T>, OptionQuery>;

	/// Witnesses signatures entry for a document stored on chain.
	#[pallet::storage]
	pub type WitnessesSignatures<T> =
		StorageMap<_, Blake2_128Concat, DocumentIdOf<T>, WitnessSignersEntryOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new witness entry has been added.
		/// \[document identifier, creator\]
		Create { identifier: DocumentIdOf<T>, creator: WitnessCreatorOf<T> },

		/// A new signer has signed the document as a witness.
		/// \[document identifier, signer, current_witness_count, required_witness_count,
		/// status\]
		Witness {
			identifier: DocumentIdOf<T>,
			signer: WitnessCreatorOf<T>,
			current_witness_count: u32,
			required_witness_count: u32,
			status: WitnessStatusOf,
			comment: CommentOf<T>,
		},

		/// Witness Approval Complete Event for the Document
		/// \[document identifier\]
		DocumentWitnessComplete { identifier: DocumentIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		/// Unauthorized operation
		UnauthorizedOperation,
		/// Document Id not found in the storage
		DocumentIdNotFound,
		/// Witness count should be less than 5 and greater than 0
		InvalidWitnessCount,
		/// Witness sign count has reached maximum
		MaxWitnessCountReached,
		/// Witness creation already added
		DocumentIdAlreadyExists,
		/// Witness Identifier is already approved,
		DocumentIdAlreadyApproved,
		/// Witness signer did cannot be same as witness creator did
		WitnessSignerCannotBeSameAsWitnessCreator,
		/// Witness signer has already part of witness party.
		SignerIsAlreadyAWitness,
		/// Document digest should remain the same,
		DocumentDigestHasChanged,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a new witness requirement entry in the chain.
		///
		/// This function allows a user to submit a new witness requirement entry for an document.
		/// The witness entry is recorded along with various metadata, including the
		/// author of the witness entry, document identifier, required witness count & current
		/// status.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `identifier` - The identifier is the unique identifier of the document.
		/// * `digest` - A hash representing some unique aspects of the document, used for
		///   identification and integrity purposes.
		/// * `authorization` - An identifier for authorization, used to validate the origin's
		///   permission to make this rating.
		///
		/// # Errors
		/// Returns `Error::<T>::InvalidWitnessCount` if the witness required count is not
		/// within the expected range.
		/// Returns `Error::<T>::DocumentIdAlreadyExists` if the witness entry has been already
		/// made.
		///
		/// # Events
		/// Emits `Create` when a witness entry has been successfully created.
		///
		/// # Example
		/// ```
		/// create(origin, identifier, digest, witness_count, authorization)?;
		/// ```
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn create(
			origin: OriginFor<T>,
			identifier: DocumentIdOf<T>,
			digest: EntryHashOf<T>,
			witness_count: u32,
			authorization: AuthorizationIdOf,
		) -> DispatchResult {
			let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			let _space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
				&authorization,
				&creator.clone(),
			)
			.map_err(<pallet_chain_space::Error<T>>::from)?;

			/* Valid range of witness count is in range [1,5] */
			ensure!(
				(witness_count >= 1) && (witness_count <= T::MaxWitnessCount::get()),
				Error::<T>::InvalidWitnessCount
			);

			ensure!(!<Witness<T>>::contains_key(&identifier), Error::<T>::DocumentIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Witness<T>>::insert(
				&identifier,
				WitnessEntryOf::<T> {
					witness_creator: creator.clone(),
					digest,
					required_witness_count: witness_count,
					current_witness_count: 0,
					witness_status: WitnessStatusOf::WITNESSAPRROVALPENDING,
					created_at: block_number,
				},
			);

			Self::deposit_event(Event::Create { identifier, creator });

			Ok(())
		}

		/// Adds the signer as a witness to the document for which a witness entry already exists.
		///
		/// This function allows a signer to sign as a witness to the document identifier.
		/// The witness is recorded along with various metadata, including the
		/// signer as a witness to the document.
		/// It also updates the status of the Witness entry document based on number of signees as a
		/// witness.
		///
		/// # Arguments
		/// * `origin` - The origin of the call, which should be a signed user in most cases.
		/// * `identifier` - The identifier is the unique identifier of the document.
		/// * `digest` - A hash representing some unique aspects of the document, used for
		///   identification and integrity purposes.
		/// * `comment` - Comment allows the signees to have a comment on the witness document.
		///
		/// # Errors
		/// Returns `Error::<T>::DocumentIdNotFound` if the witness entry is not created.
		/// Returns `Error::<T>::DocumentIdAlreadyApproved` if the witness entry has been already
		/// approved. Returns `Error::<T>::DocumentDigestHasChanged` if the digest for which the
		/// signer acts a witness has been changed from the registered form.
		/// Returns `Error::<T>::WitnessSignerCannotBeSameAsWitnessCreator` if the signer acting as
		/// a witness is the same as person who created the witness requirement entry.
		/// Returns `Error::<T>::SignerIsAlreadyAWitness` if the signer acting as a witness has
		/// already signed the document as a witness.
		/// Returns `Error::<T>::MaxWitnessCountReached` if max witness count upper bound is
		/// breached.
		///
		/// # Events
		/// Emits `Witness` when a witness signature for a document has been made.
		/// Emits `DocumentWitnessComplete` when the required witness count has been reached by the
		/// signers acting as witness to the document.
		///
		/// # Example
		/// ```
		/// witness(origin, identifier, digest, comment)?;
		/// ```
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn witness(
			origin: OriginFor<T>,
			identifier: DocumentIdOf<T>,
			digest: EntryHashOf<T>,
			comment: CommentOf<T>,
		) -> DispatchResult {
			let signer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();

			/* Ensure witness entry identifier exists to sign the document */
			let witness_entry =
				<Witness<T>>::get(&identifier).ok_or(Error::<T>::DocumentIdNotFound)?;

			/* Ensure witness entry is not already approved */
			ensure!(
				witness_entry.witness_status != WitnessStatusOf::WITNESSAPPROVALCOMPLETE,
				Error::<T>::DocumentIdAlreadyApproved
			);

			/* Ensure digest of the document hasn't changed */
			ensure!(witness_entry.digest == digest, Error::<T>::DocumentDigestHasChanged);

			/* Ensure witness signer is not same as witness entry creator */
			ensure!(
				witness_entry.witness_creator != signer,
				Error::<T>::WitnessSignerCannotBeSameAsWitnessCreator
			);

			let block_number = frame_system::Pallet::<T>::block_number();

			/* Set to default value of empty vec if key does not exist */
			let mut witness_signers =
				<WitnessesSignatures<T>>::get(&identifier).unwrap_or_default();

			/* Ensure current signer is not part of existing witness party */
			for existing_signer in &witness_signers.witnesses {
				if existing_signer == &signer {
					return Err(Error::<T>::SignerIsAlreadyAWitness.into());
				}
			}

			/* Append to list of witness signers, if current witness is not a part of party
			 * Handle error possiblity of breaching upper bound of Bounded Vector.
			 */
			if let Err(_) = witness_signers.witnesses.try_push(signer.clone()) {
				return Err(Error::<T>::MaxWitnessCountReached.into());
			}

			/* Update the storage with the modified witness_signers */
			<WitnessesSignatures<T>>::insert(
				&identifier,
				WitnessSignersEntryOf::<T> {
					witnesses: witness_signers.witnesses,
					created_at: block_number,
				},
			);

			let mut witness_status = WitnessStatusOf::WITNESSAPRROVALPENDING;
			let updated_current_witness_count = witness_entry.current_witness_count + 1;

			/* Update the storage by updating the witness count
			 * & witness status when all required witness sign the document
			 */
			if witness_entry.current_witness_count + 1 == witness_entry.required_witness_count {
				witness_status = WitnessStatusOf::WITNESSAPPROVALCOMPLETE;
				<Witness<T>>::insert(
					&identifier,
					WitnessEntryOf::<T> {
						current_witness_count: updated_current_witness_count,
						witness_status: witness_status.clone(),
						..witness_entry.clone()
					},
				)
			} else {
				/* Update the storage by updating the witness count */
				<Witness<T>>::insert(
					&identifier,
					WitnessEntryOf::<T> {
						current_witness_count: updated_current_witness_count,
						..witness_entry.clone()
					},
				);
			}

			//Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Witness {
				identifier: identifier.clone(),
				signer,
				current_witness_count: updated_current_witness_count,
				required_witness_count: witness_entry.required_witness_count,
				status: witness_status,
				comment,
			});

			/* Deposit Witness Approval Complete Event once required witness count is reached */
			if witness_entry.current_witness_count + 1 == witness_entry.required_witness_count {
				Self::deposit_event(Event::DocumentWitnessComplete { identifier });
			}

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
