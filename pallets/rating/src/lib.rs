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

use cord_primitives::{ss58identifier, IdentifierOf, RATING_PREFIX};
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::{traits::Zero, SaturatedConversion};
use sp_std::{prelude::Clone, str, vec::Vec};

pub mod ratings;
pub mod weights;

pub use crate::ratings::*;

pub use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, sp_runtime::traits::Hash};
	use frame_system::pallet_prelude::*;

	/// Hash of the rating.
	pub type RatingHashOf<T> = <T as frame_system::Config>::Hash;
	pub type EntityIdentifierOf<T> = <T as frame_system::Config>::AccountId;
	pub type ProviderIdentifierOf<T> = <T as frame_system::Config>::AccountId;
	/// Type of the identitiy.
	pub type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for a block number.
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::Origin,
			Success = CordAccountOf<Self>,
		>;
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The maximum number of delegates for a schema.
		#[pallet::constant]
		type MaxJournalInputEntries: Get<u32>;
		#[pallet::constant]
		type MinRatingValue: Get<u32>;
		#[pallet::constant]
		type MaxRatingValue: Get<u32>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// rating identifiers stored on chain.
	/// rating journal - maps from an identifier to its details.
	#[pallet::storage]
	#[pallet::getter(fn journal)]
	pub type Journal<T> =
		StorageMap<_, Blake2_128Concat, IdentifierOf, RatingJournal<T>, OptionQuery>;

	/// network score - aggregated and mapped to an entity identifier.
	#[pallet::storage]
	#[pallet::getter(fn score)]
	pub type Score<T> =
		StorageMap<_, Blake2_128Concat, EntityIdentifierOf<T>, RatingEntry, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new rating identifier has been created.
		/// \[rating identifier, entity, author\]
		Entry { identifier: IdentifierOf, entity: EntityIdentifierOf<T>, author: CordAccountOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Rating idenfier is not unique
		RatingAlreadyAnchored,
		/// Invalid Identifer Length
		InvalidIdentifierLength,
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
		//Rating digest is not unique
		DigestHashAlreadyAnchored,
		// Invalid transaction hash
		InvalidTransactionHash,
		JournalAlreadyAnchored,
		InvalidRatingValue,
		TooManyEntries,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new rating identifier and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// * origin: the identity of the Transaction Author. Transaction author
		///   pays the transaction fees
		/// * tx_journal: the incoming rating entry. Identifier is generated from the
		///   combination of the digest (hash), entity identifier and author identity provided as part of the details
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add(tx_journal.len().saturated_into()))]
		pub fn add(origin: OriginFor<T>, tx_journal: Vec<RatingJournalEntry<T>>) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				tx_journal.len() <= T::MaxJournalInputEntries::get() as usize,
				Error::<T>::TooManyEntries
			);

			for journal_entry in tx_journal {
				ensure!(
					journal_entry.overall.rating
						<= journal_entry.overall.count * T::MaxRatingValue::get(),
					Error::<T>::InvalidRatingValue
				);

				ensure!(
					journal_entry.delivery.rating
						<= journal_entry.delivery.count * T::MaxRatingValue::get(),
					Error::<T>::InvalidRatingValue
				);

				let id_digest = <T as frame_system::Config>::Hashing::hash(
					&[
						&journal_entry.digest.encode()[..],
						&journal_entry.entity.encode()[..],
						&author.encode()[..],
					]
					.concat()[..],
				);

				let identifier = IdentifierOf::try_from(
					ss58identifier::generate(&(&id_digest).encode()[..], RATING_PREFIX)
						.into_bytes(),
				)
				.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

				ensure!(
					!<Journal<T>>::contains_key(&identifier),
					Error::<T>::JournalAlreadyAnchored
				);

				let block_number = frame_system::Pallet::<T>::block_number();

				<Journal<T>>::insert(
					&identifier,
					RatingJournal {
						entry: journal_entry.clone(),
						provider: author.clone(),
						block: block_number,
					},
				);

				if let Some(mut score_entry) = <Score<T>>::get(&journal_entry.entity) {
					if !journal_entry.overall.count.is_zero() {
						ensure!(
							journal_entry.overall.rating >= T::MinRatingValue::get(),
							Error::<T>::InvalidRatingValue
						);

						score_entry.overall.count =
							score_entry.overall.count.saturating_add(journal_entry.overall.count);
						score_entry.overall.rating =
							score_entry.overall.rating.saturating_add(journal_entry.overall.rating);
					}

					if !journal_entry.delivery.count.is_zero() {
						ensure!(
							journal_entry.delivery.rating >= T::MinRatingValue::get(),
							Error::<T>::InvalidRatingValue
						);

						score_entry.delivery.count =
							score_entry.delivery.count.saturating_add(journal_entry.delivery.count);
						score_entry.delivery.rating = score_entry
							.delivery
							.rating
							.saturating_add(journal_entry.delivery.rating);
					}

					<Score<T>>::insert(
						&journal_entry.entity,
						RatingEntry {
							overall: score_entry.overall,
							delivery: score_entry.delivery,
						},
					);
				} else {
					<Score<T>>::insert(
						&journal_entry.entity,
						RatingEntry {
							overall: journal_entry.overall,
							delivery: journal_entry.delivery,
						},
					);
				}

				Self::deposit_event(Event::Entry {
					identifier,
					entity: journal_entry.entity,
					author: author.clone(),
				});
			}
			Ok(())
		}
	}
}
