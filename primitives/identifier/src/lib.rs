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

//! # CORD identifiers

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use cord_primitives::curi::Ss58Identifier;
use sp_runtime::BoundedVec;
use sp_std::{prelude::Clone, str};
pub mod types;
pub use crate::types::*;
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;

pub use crate::{pallet::*, types::*};
use sp_std::vec;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::{OptionQuery, *},
		Twox64Concat,
	};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Identifier
	pub type IdentifierOf = Ss58Identifier;
	/// Hash of the entry.
	pub type EntryDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Timeline Events.
	pub type EventEntryOf = EventEntry<CallTypeOf>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The maximum number of activity for a statement.
		#[pallet::constant]
		type MaxEventsHistory: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::storage]
	#[pallet::getter(fn identifiers)]
	pub type Identifiers<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		IdentifierOf,
		Twox64Concat,
		IdentifierTypeOf,
		BoundedVec<EventEntryOf, T::MaxEventsHistory>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Idenfier is not unique
		IdentifierAlreadyAnchored,
		/// Not able to remove an entry from timeline
		UnableToRemoveEntry,
		// Max exvents history exceeded
		MaxEventsHistoryExceeded,
	}
}

impl<T: Config> Pallet<T> {
	pub fn update_timeline(
		id: &IdentifierOf,
		id_type: IdentifierTypeOf,
		entry: EventEntryOf,
	) -> Result<(), Error<T>> {
		Identifiers::<T>::try_mutate(id, id_type, |timeline| -> Result<(), Error<T>> {
			// Initialize the BoundedVec if it doesn't exist or get the existing one.
			let events = timeline.get_or_insert_with(BoundedVec::default);

			// If the BoundedVec is full, we need to make room by removing the oldest entry
			// after genesis.
			if events.len() == T::MaxEventsHistory::get() as usize {
				// Ensure there's more than one event to keep the first one.
				if events.len() > 1 {
					// Remove the second element to make room for the new entry.
					events.remove(1);
				} else {
					// If there's only one event, we can't remove it, so we return an error.
					return Err(Error::<T>::UnableToRemoveEntry)
				}
			}

			// Push the new event onto the BoundedVec, now that we've made room.
			events.try_push(entry).map_err(|_| Error::<T>::MaxEventsHistoryExceeded)?;

			Ok(())
		})
	}
}
