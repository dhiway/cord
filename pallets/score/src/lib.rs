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

use cord_primitives::{
	ss58identifier, CountOf, IdentifierOf, ScoreIdentifierOf, ScoreOf, SCORE_PREFIX,
};
use frame_support::ensure;
use sp_runtime::traits::{IdentifyAccount, Saturating, Verify};
use sp_std::{prelude::Clone, str};

pub mod scores;
pub mod weights;

pub use crate::scores::*;

pub use crate::weights::WeightInfo;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub(crate) type CordAccountOf<T> = <T as frame_system::Config>::AccountId;
	pub type EntitySignatureOf<T> = <T as Config>::Signature;
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

	/// business entity
	pub type EntityIdentifierOf<T> = <T as frame_system::Config>::AccountId;
	/// seller application
	pub type RequestorIdentifierOf<T> = <T as frame_system::Config>::AccountId;
	/// buyer application
	pub type CollectorIdentifierOf<T> = <T as frame_system::Config>::AccountId;

	pub type EntryHashOf<T> = <T as frame_system::Config>::Hash;
	pub type JournalIdentifierOf = IdentifierOf;
	pub type RequestIdentifierOf = ScoreIdentifierOf;
	pub type TransactionIdentifierOf = ScoreIdentifierOf;

	pub type JournalDetailsOf<T> = JournalDetails<
		EntityIdentifierOf<T>,
		RequestIdentifierOf,
		TransactionIdentifierOf,
		CollectorIdentifierOf<T>,
		RequestorIdentifierOf<T>,
		ScoreTypeOf,
		ScoreOf,
	>;
	pub type JournalInputOf<T> =
		JournalInput<JournalDetailsOf<T>, EntryHashOf<T>, EntitySignatureOf<T>>;
	pub type JournalEntryOf<T> =
		JournalEntry<JournalDetailsOf<T>, EntryHashOf<T>, BlockNumberOf<T>>;
	pub type ScoreEntryOf = ScoreEntry<CountOf, ScoreOf>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::Origin,
			Success = CordAccountOf<Self>,
		>;
		type Signature: Verify<Signer = <Self as pallet::Config>::Signer>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;
		type Signer: IdentifyAccount<AccountId = CordAccountOf<Self>> + Parameter;
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		#[pallet::constant]
		type MinScoreValue: Get<u32>;
		#[pallet::constant]
		type MaxScoreValue: Get<u32>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// journal entry identifiers stored on chain.
	#[pallet::storage]
	#[pallet::getter(fn journal)]
	pub type Journal<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		JournalIdentifierOf,
		Blake2_128Concat,
		ScoreTypeOf,
		JournalEntryOf<T>,
		OptionQuery,
	>;

	/// network score - aggregated and mapped to an entity identifier.
	#[pallet::storage]
	#[pallet::getter(fn score)]
	pub type Scores<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		EntityIdentifierOf<T>,
		Blake2_128Concat,
		ScoreTypeOf,
		ScoreEntryOf,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn journal_hashes)]
	pub type JournalHashes<T> = StorageMap<_, Blake2_128Concat, EntryHashOf<T>, (), ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn tid_entries)]
	pub type TidEntries<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		TransactionIdentifierOf,
		Blake2_128Concat,
		ScoreTypeOf,
		EntityIdentifierOf<T>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new journal entry has been added.
		/// \[entry identifier, entity, author\]
		JournalEntry {
			identifier: JournalIdentifierOf,
			entity: EntityIdentifierOf<T>,
			author: RequestorIdentifierOf<T>,
		},
		/// Aggregate scores has been updated.
		/// \[entity identifier\]
		AggregateUpdated { entity: EntityIdentifierOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid Identifer Length
		InvalidIdentifierLength,
		// Invalid digest
		InvalidDigest,
		// Invalid creator signature
		InvalidSignature,
		// Invalid Rating Identifier
		InvalidRatingIdentifier,
		// Transaction already rated
		TransactionAlreadyRated,
		// Invalid rating value - should be between 1 and 50
		InvalidRatingValue,
		// Exceeds the maximum allowed entries in a single transaction
		TooManyJournalEntries,
		// Invalid entity signature
		InvalidEntitySignature,
		//Stream digest is not unique
		DigestAlreadyAnchored,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		///
		/// Create a new rating identifier and associates it with its
		/// controller. The controller (issuer) is the owner of the identifier.
		///
		/// * origin: the identity of the Transaction Author. Transaction author
		///   pays the transaction fees
		/// * tx_journal: the incoming rating entry.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::entries())]
		pub fn entries(origin: OriginFor<T>, journal: JournalInputOf<T>) -> DispatchResult {
			let author = <T as Config>::EnsureOrigin::ensure_origin(origin)?;

			ensure!(
				(journal.entry.score >= T::MinScoreValue::get()
					&& journal.entry.score <= T::MaxScoreValue::get()),
				Error::<T>::InvalidRatingValue
			);
			ensure!(
				!<JournalHashes<T>>::contains_key(&journal.digest),
				Error::<T>::DigestAlreadyAnchored
			);

			ensure!(
				journal.signature.verify(&(&journal.digest).encode()[..], &journal.entry.entity),
				Error::<T>::InvalidEntitySignature
			);

			let identifier = JournalIdentifierOf::try_from(
				ss58identifier::generate(&(&journal.digest).encode()[..], SCORE_PREFIX)
					.into_bytes(),
			)
			.map_err(|()| Error::<T>::InvalidIdentifierLength)?;

			ensure!(
				!<TidEntries<T>>::contains_key(&journal.entry.tid, &journal.entry.score_type),
				Error::<T>::TransactionAlreadyRated
			);

			let block_number = frame_system::Pallet::<T>::block_number();

			<Journal<T>>::insert(
				&identifier,
				&journal.entry.score_type,
				JournalEntryOf::<T> {
					entry: journal.entry.clone(),
					digest: journal.digest,
					block: block_number,
				},
			);
			<JournalHashes<T>>::insert(&journal.digest, ());
			<TidEntries<T>>::insert(
				&journal.entry.tid,
				&journal.entry.score_type,
				&journal.entry.entity,
			);

			Self::deposit_event(Event::JournalEntry {
				identifier,
				entity: journal.entry.entity.clone(),
				author: author.clone(),
			});

			Self::aggregate_score(&journal.entry);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn aggregate_score(entry: &JournalDetailsOf<T>) {
		if let Some(mut aggregate) = <Scores<T>>::get(&entry.entity, &entry.score_type) {
			aggregate.count.saturating_inc();
			aggregate.score = aggregate.score.saturating_add(entry.score);

			<Scores<T>>::insert(
				&entry.entity,
				&entry.score_type,
				ScoreEntryOf { count: aggregate.count, score: aggregate.score },
			);
		} else {
			<Scores<T>>::insert(
				&entry.entity,
				&entry.score_type,
				ScoreEntryOf { count: 1, score: entry.score },
			);
		}
		Self::deposit_event(Event::AggregateUpdated { entity: entry.entity.clone() });
	}
}